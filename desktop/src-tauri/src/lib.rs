//! Windows Whisper - Library exports

pub mod audio;
pub mod settings;
pub mod commands;

use audio::AudioRecorder;
use settings::Settings;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use image::EncodableLayout;
use tokio::sync::{mpsc, watch};

#[derive(Debug, Serialize, Deserialize)]
struct TranscribeResponse {
    success: bool,
    text: Option<String>,
    error: Option<String>,
}

struct ChunkControl {
    stop_tx: watch::Sender<bool>,
    chunk_tx: mpsc::Sender<Vec<u8>>,
    timer_handle: tauri::async_runtime::JoinHandle<()>,
    worker_handle: tauri::async_runtime::JoinHandle<()>,
}

pub struct AppState {
    pub recorder: Mutex<Option<AudioRecorder>>,
    pub settings: Mutex<Settings>,
    pub is_recording: Mutex<bool>,
    pub chunk_texts: Mutex<Vec<String>>,
    pub chunk_control: Mutex<Option<ChunkControl>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            recorder: Mutex::new(None),
            settings: Mutex::new(Settings::load()),
            is_recording: Mutex::new(false),
            chunk_texts: Mutex::new(Vec::new()),
            chunk_control: Mutex::new(None),
        }
    }
}


const CHUNK_SECONDS: u64 = 10;
const CHUNK_OVERLAP_SECONDS: u32 = 1;
const CHUNK_TRIM_WORDS: usize = 3;
const CHUNK_MAX_OVERLAP_WORDS: usize = 12;
const CHUNK_MAX_REPEAT_PHRASE_WORDS: usize = 4;

async fn transcribe_audio_chunk(
    api_url: &str,
    api_key: &str,
    audio_data: &[u8],
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let audio_base64 = STANDARD.encode(audio_data);

    let response = client
        .post(format!("{}/transcribe", api_url))
        .header("X-API-Key", api_key)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "audio": audio_base64 }))
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let result: TranscribeResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    if result.success {
        result.text.ok_or_else(|| "No text in response".to_string())
    } else {
        Err(result.error.unwrap_or_else(|| "Unknown error".to_string()))
    }
}

fn trim_trailing_words(text: &str, words: usize) -> String {
    let mut parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() > words {
        parts.truncate(parts.len() - words);
    }
    parts.join(" ")
}

fn normalize_word(word: &str) -> String {
    let trimmed = word.trim_matches(|c: char| !c.is_alphanumeric());
    if trimmed.is_empty() {
        word.to_ascii_lowercase()
    } else {
        trimmed.to_ascii_lowercase()
    }
}

fn merge_with_overlap(existing: &str, next: &str) -> String {
    let existing_words: Vec<&str> = existing.split_whitespace().collect();
    let next_words: Vec<&str> = next.split_whitespace().collect();
    if existing_words.is_empty() {
        return next.to_string();
    }
    if next_words.is_empty() {
        return existing.to_string();
    }

    let max_overlap = existing_words
        .len()
        .min(next_words.len())
        .min(CHUNK_MAX_OVERLAP_WORDS);
    let mut overlap = 0;

    'outer: for k in (1..=max_overlap).rev() {
        for i in 0..k {
            if normalize_word(existing_words[existing_words.len() - k + i])
                != normalize_word(next_words[i])
            {
                continue 'outer;
            }
        }
        overlap = k;
        break;
    }

    let mut merged = String::new();
    merged.push_str(existing.trim());
    if overlap < next_words.len() {
        let rest = next_words[overlap..].join(" ");
        if !rest.is_empty() {
            if !merged.is_empty() {
                merged.push(' ');
            }
            merged.push_str(rest.trim());
        }
    }
    merged
}

fn collapse_duplicate_words(text: &str) -> String {
    let mut output: Vec<&str> = Vec::new();
    let mut prev_norm: Option<String> = None;

    for word in text.split_whitespace() {
        let norm = normalize_word(word);
        if let Some(prev) = &prev_norm {
            if *prev == norm {
                continue;
            }
        }
        output.push(word);
        prev_norm = Some(norm);
    }

    output.join(" ")
}

fn collapse_repeated_phrases(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() < 4 {
        return text.to_string();
    }

    let mut output: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let remaining = words.len() - i;
        let max_n = (remaining / 2).min(CHUNK_MAX_REPEAT_PHRASE_WORDS);
        let mut collapsed = false;

        for n in (2..=max_n).rev() {
            let mut matches = true;
            for j in 0..n {
                if normalize_word(words[i + j]) != normalize_word(words[i + n + j]) {
                    matches = false;
                    break;
                }
            }
            if matches {
                output.extend_from_slice(&words[i..i + n]);
                i += 2 * n;
                collapsed = true;
                break;
            }
        }

        if !collapsed {
            output.push(words[i]);
            i += 1;
        }
    }

    output.join(" ")
}

fn consolidate_chunk_texts(chunks: &[String]) -> String {
    let last_idx = chunks.iter().rposition(|text| !text.trim().is_empty());     
    let mut combined = String::new();

    for (idx, chunk) in chunks.iter().enumerate() {
        let trimmed = chunk.trim();
        if trimmed.is_empty() {
            continue;
        }
        let part = if Some(idx) == last_idx {
            trimmed.to_string()
        } else {
            trim_trailing_words(trimmed, CHUNK_TRIM_WORDS)
        };
        if !part.is_empty() {
            combined = merge_with_overlap(&combined, &part);
        }
    }

    let merged = collapse_repeated_phrases(&combined);
    collapse_duplicate_words(&merged)
}

async fn drain_chunk_from_recorder(app: AppHandle) -> Result<Vec<u8>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut recorder = state.recorder.lock().unwrap();
        if let Some(ref mut rec) = *recorder {
            rec.drain_chunk()
        } else {
            Err("No recorder available".to_string())
        }
    })
    .await
    .map_err(|_| "Failed to drain audio chunk".to_string())?
}

async fn stop_recorder(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut recorder = state.recorder.lock().unwrap();
        if let Some(ref mut rec) = *recorder {
            rec.stop_recording()
        } else {
            Err("No recorder available".to_string())
        }
    })
    .await
    .map_err(|_| "Failed to stop recorder".to_string())?
}

async fn shutdown_chunking(app: AppHandle, final_chunk: Option<Vec<u8>>) -> Vec<String> {
    let control = {
        let state = app.state::<AppState>();
        let mut guard = state.chunk_control.lock().unwrap();
        guard.take()
    };

    if let Some(control) = control {
        let _ = control.stop_tx.send(true);
        let _ = control.timer_handle.await;
        if let Some(chunk) = final_chunk {
            let _ = control.chunk_tx.send(chunk).await;
        }
        drop(control.chunk_tx);
        let _ = control.worker_handle.await;
    }

    let state = app.state::<AppState>();
    let mut texts = state.chunk_texts.lock().unwrap();
    let collected = texts.clone();
    texts.clear();
    collected
}

#[cfg(target_os = "windows")]
fn paste_text() -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Failed to create enigo: {}", e))?;

    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|e| format!("Failed to press Control: {}", e))?;
    enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| format!("Failed to press V: {}", e))?;
    enigo
        .key(Key::Control, Direction::Release)
        .map_err(|e| format!("Failed to release Control: {}", e))?;
    
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn paste_text() -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
fn caret_position() -> Option<(i32, i32)> {
    use std::{mem, ptr};
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, GUITHREADINFO,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == 0 {
            return None;
        }

        let thread_id = GetWindowThreadProcessId(hwnd, ptr::null_mut());
        if thread_id == 0 {
            return None;
        }

        let mut info = GUITHREADINFO {
            cbSize: mem::size_of::<GUITHREADINFO>() as u32,
            ..mem::zeroed()
        };
        if GetGUIThreadInfo(thread_id, &mut info) == 0 {
            return None;
        }

        if info.hwndCaret == 0 {
            return None;
        }

        let mut point = POINT {
            x: info.rcCaret.left,
            y: info.rcCaret.top,
        };
        if ClientToScreen(info.hwndCaret, &mut point) == 0 {
            return None;
        }

        Some((point.x, point.y))
    }
}

#[cfg(not(target_os = "windows"))]
fn caret_position() -> Option<(i32, i32)> {
    None
}

fn position_popup(window: &tauri::WebviewWindow, anchor: (i32, i32)) {
    let (width, height) = window
        .outer_size()
        .map(|size| (size.width as i32, size.height as i32))
        .unwrap_or((120, 50));
    let mut x = anchor.0 - (width / 2);
    let mut y = anchor.1 - height - 10;

    if y < 0 {
        y = anchor.1 + 20;
    }

    if let Ok(Some(monitor)) = window.monitor_from_point(anchor.0 as f64, anchor.1 as f64) {
        let pos = monitor.position();
        let size = monitor.size();
        let min_x = pos.x as i32;
        let min_y = pos.y as i32;
        let max_x = min_x + size.width as i32 - width;
        let max_y = min_y + size.height as i32 - height;
        x = x.clamp(min_x, max_x.max(min_x));
        y = y.clamp(min_y, max_y.max(min_y));
    }

    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
        x,
        y,
    }));
}

async fn cancel_recording(app: AppHandle) {
    let state = app.state::<AppState>();
    
    // Stop recording state
    {
        *state.is_recording.lock().unwrap() = false;
    }

    let _ = shutdown_chunking(app.clone(), None).await;
    let _ = stop_recorder(app.clone()).await;
    
    // Unregister Escape logic
    let escape_shortcut = Shortcut::new(Some(Modifiers::empty()), Code::Escape);
    let _ = app.global_shortcut().unregister(escape_shortcut);
    
    // Hide window
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    
    let _ = app.emit("show-idle", ());
}

async fn handle_hotkey_press(app: AppHandle) {
    let state = app.state::<AppState>();
    
    // Check if recording with a scoped lock
    let is_recording_val = {
        let is_rec = state.is_recording.lock().unwrap();
        *is_rec
    };
    
    let escape_shortcut = Shortcut::new(Some(Modifiers::empty()), Code::Escape);

    if is_recording_val {
        // STOP RECORDING
        {
            *state.is_recording.lock().unwrap() = false;
        }

        // Unregister Escape
        let _ = app.global_shortcut().unregister(escape_shortcut);

        // Stop chunking and finalize transcription
        let settings = state.settings.lock().unwrap().clone();
        let has_api = !settings.api_url.is_empty() && !settings.api_key.is_empty();

        // Show processing state
        let _ = app.emit("show-processing", ());

        let control = {
            let mut guard = state.chunk_control.lock().unwrap();
            guard.take()
        };

        if let Some(control) = control {
            let _ = control.stop_tx.send(true);
            let _ = control.timer_handle.await;

            let final_chunk = if has_api {
                drain_chunk_from_recorder(app.clone()).await.ok()
            } else {
                None
            };

            let _ = stop_recorder(app.clone()).await;

            if let Some(chunk) = final_chunk {
                let _ = control.chunk_tx.send(chunk).await;
            }
            drop(control.chunk_tx);
            let _ = control.worker_handle.await;
        } else {
            let _ = stop_recorder(app.clone()).await;
        }

        if !has_api {
            let _ = app.emit("show-error", "API not configured. Right-click tray to configure.");
            return;
        }

        let chunk_texts = {
            let mut texts = state.chunk_texts.lock().unwrap();
            let collected = texts.clone();
            texts.clear();
            collected
        };

        let text = consolidate_chunk_texts(&chunk_texts);
        if text.is_empty() {
            let _ = app.emit("show-error", "No text returned from transcription".to_string());
            return;
        }

        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }

        use tauri_plugin_clipboard_manager::ClipboardExt;
        if let Err(e) = app.clipboard().write_text(&text) {
            eprintln!("Failed to write clipboard: {}", e);
        }

        // Small delay before pasting
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        // Paste via clipboard to avoid simulated typing glitches
        if let Err(e) = paste_text() {
            eprintln!("Failed to paste text: {}", e);
        }

        let _ = app.emit("show-success", text);
    } else {
        // START RECORDING
        {
            *state.is_recording.lock().unwrap() = true;
        }
        
        // Initialize recorder if needed
        {
            let mut recorder = state.recorder.lock().unwrap();
            if recorder.is_none() {
                 *recorder = Some(AudioRecorder::new());
            }
             
            if let Some(ref mut rec) = *recorder {
                 // Create volume channel
                 let (vol_tx, vol_rx) = std::sync::mpsc::channel();
                 
                 // Spawn listener
                 let app_handle = app.clone();
                 std::thread::spawn(move || {
                     while let Ok(level) = vol_rx.recv() {
                         let _ = app_handle.emit("audio-level", level);
                     }
                 });

                 if let Err(e) = rec.start_recording(Some(vol_tx), CHUNK_OVERLAP_SECONDS) {
                    let _ = app.emit("show-error", format!("Failed to start recording: {}", e));
                    *state.is_recording.lock().unwrap() = false;
                    return;
                }
            }
        }

        {
            let mut texts = state.chunk_texts.lock().unwrap();
            texts.clear();
        }
        {
            let mut control = state.chunk_control.lock().unwrap();
            *control = None;
        }

        let (chunk_tx, mut chunk_rx) = mpsc::channel::<Vec<u8>>(4);
        let (stop_tx, mut stop_rx) = watch::channel(false);

        let worker_app = app.clone();
        let worker_handle = tauri::async_runtime::spawn(async move {
            while let Some(chunk) = chunk_rx.recv().await {
                let settings = {
                    let state = worker_app.state::<AppState>();
                    let settings = state.settings.lock().unwrap().clone();
                    settings
                };
                if settings.api_url.is_empty() || settings.api_key.is_empty() {
                    continue;
                }
                match transcribe_audio_chunk(&settings.api_url, &settings.api_key, &chunk).await {
                    Ok(text) => {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            let state = worker_app.state::<AppState>();
                            state.chunk_texts.lock().unwrap().push(trimmed.to_string());
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to transcribe chunk: {}", e);
                    }
                }
            }
        });

        let timer_app = app.clone();
        let timer_tx = chunk_tx.clone();
        let timer_handle = tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(CHUNK_SECONDS));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if *stop_rx.borrow() {
                            break;
                        }
                        match drain_chunk_from_recorder(timer_app.clone()).await {
                            Ok(chunk) => {
                                if !chunk.is_empty() {
                                    if timer_tx.send(chunk).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Err(_) => {}
                        }
                    }
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });

        {
            let mut control = state.chunk_control.lock().unwrap();
            *control = Some(ChunkControl {
                stop_tx,
                chunk_tx,
                timer_handle,
                worker_handle,
            });
        }

        // Register Escape to cancel
        let _ = app.global_shortcut().register(escape_shortcut);
        
        // Show and position the popup window near the caret
        if let Some(window) = app.get_webview_window("main") {
            // Keep the popup from stealing focus when it appears.
            let _ = window.set_focusable(false);
            let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
                width: 80,
                height: 30,
            }));
            let anchor = caret_position().or_else(|| {
                use device_query::{DeviceQuery, DeviceState};
                let device_state = DeviceState::new();
                let mouse = device_state.get_mouse();
                Some((mouse.coords.0, mouse.coords.1))
            });
            if let Some(anchor) = anchor {
                position_popup(&window, anchor);
            }
            let _ = window.show();
            
            // Follow the caret while recording; fall back to mouse clicks if needed.
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                use device_query::{DeviceQuery, DeviceState};
                let mut last_anchor: Option<(i32, i32)> = None;
                let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(16));

                loop {
                    interval.tick().await;
                    
                    // Check if still recording
                    let is_recording = {
                        let state = app_handle.state::<AppState>();
                        let guard = state.is_recording.lock().unwrap();
                        *guard
                    };
                    
                    if !is_recording {
                        break;
                    }
                    
                    let anchor = caret_position().or_else(|| {
                        // Create per-iteration to avoid holding a !Send type across await.
                        let device_state = DeviceState::new();
                        let mouse = device_state.get_mouse();
                        if mouse.button_pressed.iter().any(|&b| b) {
                            Some((mouse.coords.0, mouse.coords.1))
                        } else {
                            None
                        }
                    });

                    if let Some(anchor) = anchor {
                        let should_move = match last_anchor {
                            Some((last_x, last_y)) => {
                                (last_x - anchor.0).abs() > 1 || (last_y - anchor.1).abs() > 1
                            }
                            None => true,
                        };

                        if should_move {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                position_popup(&window, anchor);
                            }
                            last_anchor = Some(anchor);
                        }
                    }
                }
            });
        }
        
        let _ = app.emit("show-recording", ());
    }
}

fn parse_hotkey(hotkey_str: &str) -> Option<Shortcut> {
    let parts: Vec<&str> = hotkey_str.split('+').map(|s| s.trim()).collect();
    let mut modifiers = Modifiers::empty();
    let mut key_code = None;

    for part in parts {
        match part.to_uppercase().as_str() {
            "CTRL" | "CONTROL" => modifiers |= Modifiers::CONTROL,
            "SHIFT" => modifiers |= Modifiers::SHIFT,
            "ALT" => modifiers |= Modifiers::ALT,
            "WIN" | "SUPER" | "META" => modifiers |= Modifiers::SUPER,
            "SPACE" => key_code = Some(Code::Space),
            "ENTER" => key_code = Some(Code::Enter),
            "TAB" => key_code = Some(Code::Tab),
            "ESCAPE" | "ESC" => key_code = Some(Code::Escape),
            "SCROLLLOCK" | "SCROLL" => key_code = Some(Code::ScrollLock),
            "PRINTSCREEN" | "PRINT" | "PRTSC" => key_code = Some(Code::PrintScreen),
            "PAUSE" | "PAUSEBREAK" => key_code = Some(Code::Pause),
            "INSERT" | "INS" => key_code = Some(Code::Insert),
            "DELETE" | "DEL" => key_code = Some(Code::Delete),
            "HOME" => key_code = Some(Code::Home),
            "END" => key_code = Some(Code::End),
            "PAGEUP" | "PGUP" => key_code = Some(Code::PageUp),
            "PAGEDOWN" | "PGDN" => key_code = Some(Code::PageDown),
            "UP" | "ARROWUP" => key_code = Some(Code::ArrowUp),
            "DOWN" | "ARROWDOWN" => key_code = Some(Code::ArrowDown),
            "LEFT" | "ARROWLEFT" => key_code = Some(Code::ArrowLeft),
            "RIGHT" | "ARROWRIGHT" => key_code = Some(Code::ArrowRight),
            k if k.len() == 1 => {
                // Single character keys
                let c = k.chars().next().unwrap();
                key_code = match c {
                    'A' => Some(Code::KeyA),
                    'B' => Some(Code::KeyB),
                    'C' => Some(Code::KeyC),
                    'D' => Some(Code::KeyD),
                    'E' => Some(Code::KeyE),
                    'F' => Some(Code::KeyF),
                    'G' => Some(Code::KeyG),
                    'H' => Some(Code::KeyH),
                    'I' => Some(Code::KeyI),
                    'J' => Some(Code::KeyJ),
                    'K' => Some(Code::KeyK),
                    'L' => Some(Code::KeyL),
                    'M' => Some(Code::KeyM),
                    'N' => Some(Code::KeyN),
                    'O' => Some(Code::KeyO),
                    'P' => Some(Code::KeyP),
                    'Q' => Some(Code::KeyQ),
                    'R' => Some(Code::KeyR),
                    'S' => Some(Code::KeyS),
                    'T' => Some(Code::KeyT),
                    'U' => Some(Code::KeyU),
                    'V' => Some(Code::KeyV),
                    'W' => Some(Code::KeyW),
                    'X' => Some(Code::KeyX),
                    'Y' => Some(Code::KeyY),
                    'Z' => Some(Code::KeyZ),
                    '0' => Some(Code::Digit0),
                    '1' => Some(Code::Digit1),
                    '2' => Some(Code::Digit2),
                    '3' => Some(Code::Digit3),
                    '4' => Some(Code::Digit4),
                    '5' => Some(Code::Digit5),
                    '6' => Some(Code::Digit6),
                    '7' => Some(Code::Digit7),
                    '8' => Some(Code::Digit8),
                    '9' => Some(Code::Digit9),
                    _ => None,
                };
            }
            k if k.starts_with('F') && k.len() <= 3 => {
                // Function keys F1-F12
                if let Ok(num) = k[1..].parse::<u8>() {
                    key_code = match num {
                        1 => Some(Code::F1),
                        2 => Some(Code::F2),
                        3 => Some(Code::F3),
                        4 => Some(Code::F4),
                        5 => Some(Code::F5),
                        6 => Some(Code::F6),
                        7 => Some(Code::F7),
                        8 => Some(Code::F8),
                        9 => Some(Code::F9),
                        10 => Some(Code::F10),
                        11 => Some(Code::F11),
                        12 => Some(Code::F12),
                        _ => None,
                    };
                }
            }
            _ => {}
        }
    }

    key_code.map(|code| Shortcut::new(Some(modifiers), code))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(move |app, shortcut, event| {
             if event.state == ShortcutState::Pressed {
                 // Check if it's the configured hotkey
                 let state = app.state::<AppState>();
                 let hotkey_str = state.settings.lock().unwrap().hotkey.clone();
                 if let Some(cfg_shortcut) = parse_hotkey(&hotkey_str) {
                     if shortcut == &cfg_shortcut {
                         let app_handle = app.clone();
                         tauri::async_runtime::spawn(async move {
                             handle_hotkey_press(app_handle).await;
                         });
                         return;
                     }
                 }
                 
                 // Check if it is Escape
                 if shortcut.matches(Modifiers::empty(), Code::Escape) {
                      let app_handle = app.clone();
                         tauri::async_runtime::spawn(async move {
                             cancel_recording(app_handle).await;
                         });
                 }
             }
        }).build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::hide_popup, 
            commands::get_settings, 
            commands::save_settings
        ])
        .setup(|app| {
            // Create tray menu
            let settings_item = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&settings_item, &quit_item])?;

            let icon = include_bytes!("../icons/icon.png");
            let image_buffer = image::load_from_memory(icon)
                .map_err(|e| e.to_string())?
                .to_rgba8();
            let (width, height) = image_buffer.dimensions();
            let rgba = image_buffer.as_bytes().to_vec();
            let icon_image = Image::new(&rgba, width, height);

            // Create tray icon
            let _tray = TrayIconBuilder::new()
                .icon(icon_image)
                .menu(&menu)
                .tooltip("Windows Whisper - Push to Talk")
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "settings" => {
                            // Open settings window
                            if app.get_webview_window("settings").is_none() {
                                let _ = WebviewWindowBuilder::new(
                                    app,
                                    "settings",
                                    WebviewUrl::App("settings.html".into()),
                                )
                                .title("Settings")
                                .inner_size(400.0, 300.0)
                                .resizable(false)
                                .center()
                                .build();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // Register global shortcut
            let state = app.state::<AppState>();
            let hotkey_str = state.settings.lock().unwrap().hotkey.clone();
            
            if let Some(shortcut) = parse_hotkey(&hotkey_str) {
                app.global_shortcut().register(shortcut)?;
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
