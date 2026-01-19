//! Settings persistence

use serde::{Deserialize, Serialize};

/// A custom find/replace rule for post-processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplacementRule {
    pub find: String,
    pub replace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub hotkey: String,
    pub api_url: String,
    pub api_key: String,

    // Post-processing settings
    #[serde(default)]
    pub auto_capitalize: bool,
    #[serde(default)]
    pub remove_filler_words: bool,
    #[serde(default = "default_filler_words")]
    pub filler_words: Vec<String>,
    #[serde(default)]
    pub custom_replacements: Vec<ReplacementRule>,
}

fn default_filler_words() -> Vec<String> {
    vec![
        "um".to_string(),
        "uh".to_string(),
        "like".to_string(),
        "you know".to_string(),
        "basically".to_string(),
        "actually".to_string(),
        "sort of".to_string(),
        "kind of".to_string(),
        "i mean".to_string(),
        "right".to_string(),
    ]
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: "ScrollLock".to_string(),
            api_url: String::new(),
            api_key: String::new(),
            auto_capitalize: true,
            remove_filler_words: true,
            filler_words: default_filler_words(),
            custom_replacements: Vec::new(),
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let mut path = dirs::data_local_dir().unwrap_or_default();
        path.push("windows-whisper");
        std::fs::create_dir_all(&path).ok();
        path.push("settings.json");

        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str(&content) {
                return settings;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let mut path = dirs::data_local_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        path.push("windows-whisper");
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        path.push("settings.json");

        let content = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, content).map_err(|e| e.to_string())?;
        Ok(())
    }
}
