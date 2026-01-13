//! Audio recording module using cpal with thread isolation
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat};
use hound::{WavSpec, WavWriter};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct AudioRecorder {
    command_tx: Option<Sender<AudioCommand>>,
    is_recording: Arc<AtomicBool>,
}

enum AudioCommand {
    Stop(Sender<Result<(), String>>),
    DrainChunk(Sender<Result<Vec<u8>, String>>),
}

// AudioRecorder is Send because it only holds connection to the thread
// The thread holds the Stream (which is !Send)
unsafe impl Send for AudioRecorder {}
unsafe impl Sync for AudioRecorder {}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            command_tx: None,
            is_recording: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start_recording(
        &mut self,
        level_tx: Option<Sender<f32>>,
        chunk_overlap_seconds: u32,
    ) -> Result<(), String> {
        if self.is_recording.load(Ordering::SeqCst) {
            return Ok(()); // Already recording
        }

        let (cmd_tx, cmd_rx) = channel();
        self.command_tx = Some(cmd_tx);
        self.is_recording.store(true, Ordering::SeqCst);

        let is_recording_clone = self.is_recording.clone();

        // Spawn thread to handle audio stream
        thread::spawn(move || {
            let samples = Arc::new(Mutex::new(Vec::new()));
            let samples_producer = samples.clone();
            let is_recording_flag = is_recording_clone.clone();

            let err_fn = |err| eprintln!("Audio stream error: {}", err);

            let host = cpal::default_host();
            let device = match host.default_input_device() {
                Some(d) => d,
                None => {
                    eprintln!("No input device");
                    return;
                }
            };

            let config = match device.default_input_config() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Error getting config: {}", e);
                    return;
                }
            };

            let sample_rate = config.sample_rate().0;
            let channels = config.channels() as usize;

            // Prepare level sender
            let level_tx_16 = level_tx.clone();
            let level_tx_32 = level_tx.clone();

            // Stream creation
            let stream_res = match config.sample_format() {
                SampleFormat::I16 => device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        if is_recording_flag.load(Ordering::SeqCst) {
                            let mut s = samples_producer.lock().unwrap();
                            let mut sum_sq = 0.0;
                            let count = data.len();

                            for chunk in data.chunks(channels) {
                                // Mono mix
                                let mono: i32 = chunk.iter().map(|&x| x as i32).sum();
                                let val = (mono / channels as i32) as i16;
                                s.push(val);

                                // RMS calculation
                                let norm = val as f32 / 32768.0;
                                sum_sq += norm * norm;
                            }

                            if let Some(tx) = &level_tx_16 {
                                if count > 0 {
                                    let rms = (sum_sq * channels as f32 / count as f32).sqrt();
                                    let _ = tx.send(rms);
                                }
                            }
                        }
                    },
                    err_fn,
                    None,
                ),
                SampleFormat::F32 => device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if is_recording_flag.load(Ordering::SeqCst) {
                            let mut s = samples_producer.lock().unwrap();
                            let mut sum_sq = 0.0;
                            let count = data.len();

                            for chunk in data.chunks(channels) {
                                let mono: f32 = chunk.iter().sum();
                                let val = mono / channels as f32;
                                s.push(Sample::from_sample(val));

                                sum_sq += val * val;
                            }

                            if let Some(tx) = &level_tx_32 {
                                if count > 0 {
                                    let rms = (sum_sq * channels as f32 / count as f32).sqrt();
                                    let _ = tx.send(rms);
                                }
                            }
                        }
                    },
                    err_fn,
                    None,
                ),
                _ => {
                    eprintln!("Unsupported format"); // Should communicate back but simple log for now
                    return;
                }
            };

            if let Ok(stream) = stream_res {
                if let Err(e) = stream.play() {
                    eprintln!("Failed to play stream: {}", e);
                    return;
                }

                let overlap_samples = sample_rate as usize * chunk_overlap_seconds as usize;
                let mut last_chunk_index: usize = 0;

                // Wait for commands
                while let Ok(command) = cmd_rx.recv() {
                    match command {
                        AudioCommand::DrainChunk(reply_tx) => {
                            let mut buffer = samples.lock().unwrap();
                            let chunk_end = buffer.len();
                            if chunk_end <= last_chunk_index {
                                let _ = reply_tx.send(Err("No new audio".to_string()));
                                continue;
                            }

                            let chunk_start = last_chunk_index.saturating_sub(overlap_samples);
                            let chunk_samples = buffer[chunk_start..chunk_end].to_vec();

                            if overlap_samples > 0 {
                                let retain_start = chunk_end.saturating_sub(overlap_samples);
                                let retained = buffer.split_off(retain_start);
                                *buffer = retained;
                                last_chunk_index = buffer.len();
                            } else {
                                buffer.clear();
                                last_chunk_index = 0;
                            }

                            drop(buffer);

                            let resampled = if sample_rate != 16000 {
                                resample(&chunk_samples, sample_rate, 16000)
                            } else {
                                chunk_samples
                            };

                            let wav_data = encode_wav(&resampled, 16000);
                            let _ = reply_tx.send(wav_data);
                        }
                        AudioCommand::Stop(reply_tx) => {
                            drop(stream); // Stops recording
                            is_recording_clone.store(false, Ordering::SeqCst);
                            let _ = reply_tx.send(Ok(()));
                            break;
                        }
                    }
                }
            } else {
                eprintln!("Failed to build stream: {:?}", stream_res.err());
            }
        });

        Ok(())
    }

    pub fn drain_chunk(&mut self) -> Result<Vec<u8>, String> {
        if let Some(tx) = &self.command_tx {
            let (reply_tx, reply_rx) = channel();

            tx.send(AudioCommand::DrainChunk(reply_tx))
                .map_err(|_| "Failed to send chunk command".to_string())?;

            match reply_rx.recv() {
                Ok(res) => res,
                Err(_) => Err("Failed to receive audio chunk".to_string()),
            }
        } else {
            Err("Not recording".to_string())
        }
    }

    pub fn stop_recording(&mut self) -> Result<(), String> {
        if let Some(tx) = self.command_tx.take() {
            let (reply_tx, reply_rx) = channel();

            tx.send(AudioCommand::Stop(reply_tx))
                .map_err(|_| "Failed to send stop command".to_string())?;

            match reply_rx.recv() {
                Ok(res) => res,
                Err(_) => Err("Failed to stop recording".to_string()),
            }
        } else {
            Err("Not recording".to_string())
        }
    }
}

fn resample(samples: &[i16], from_rate: u32, to_rate: u32) -> Vec<i16> {
    let ratio = from_rate as f64 / to_rate as f64;
    let new_len = (samples.len() as f64 / ratio) as usize;
    let mut resampled = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let src_idx = (i as f64 * ratio) as usize;
        if src_idx < samples.len() {
            resampled.push(samples[src_idx]);
        }
    }
    resampled
}

fn encode_wav(samples: &[i16], sample_rate: u32) -> Result<Vec<u8>, String> {
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = WavWriter::new(&mut cursor, spec)
            .map_err(|e| format!("Failed to create WAV writer: {}", e))?;

        for &sample in samples {
            writer
                .write_sample(sample)
                .map_err(|e| format!("Failed to write sample: {}", e))?;
        }
        writer
            .finalize()
            .map_err(|e| format!("Failed to finalize: {}", e))?;
    }

    Ok(cursor.into_inner())
}
