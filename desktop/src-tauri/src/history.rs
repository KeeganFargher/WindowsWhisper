//! Transcription history management

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const MAX_HISTORY_ENTRIES: usize = 50;
const HISTORY_FILE: &str = "history.json";

/// A single transcription log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionLog {
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Raw transcription before post-processing
    pub raw_text: String,
    /// Text after post-processing was applied
    pub processed_text: String,
}

/// Collection of transcription history entries
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TranscriptionHistory {
    pub entries: Vec<TranscriptionLog>,
}

impl TranscriptionHistory {
    /// Get the path to the history file
    fn get_path() -> PathBuf {
        let mut path = dirs::data_local_dir().unwrap_or_default();
        path.push("windows-whisper");
        std::fs::create_dir_all(&path).ok();
        path.push(HISTORY_FILE);
        path
    }

    /// Load history from disk
    pub fn load() -> Self {
        let path = Self::get_path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(history) = serde_json::from_str(&content) {
                return history;
            }
        }
        Self::default()
    }

    /// Save history to disk
    pub fn save(&self) -> Result<(), String> {
        let path = Self::get_path();
        let content = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, content).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Add a new transcription entry
    pub fn add_entry(&mut self, raw_text: String, processed_text: String) {
        let timestamp = chrono::Local::now().to_rfc3339();

        self.entries.insert(
            0,
            TranscriptionLog {
                timestamp,
                raw_text,
                processed_text,
            },
        );

        // Keep only the most recent entries
        if self.entries.len() > MAX_HISTORY_ENTRIES {
            self.entries.truncate(MAX_HISTORY_ENTRIES);
        }

        // Auto-save after adding
        let _ = self.save();
    }

    /// Clear all history entries
    pub fn clear(&mut self) {
        self.entries.clear();
        let _ = self.save();
    }
}
