use serde::{Serialize, Deserialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum Language { EN, RU }

#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub language: Option<Language>,
}

impl AppConfig {
    pub fn load() -> Self {
        let path = Self::path();
        if let Ok(content) = fs::read_to_string(path) {
            serde_json::from_str(&content).unwrap_or(Self { language: None })
        } else {
            Self { language: None }
        }
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string(self) {
            let _ = fs::write(path, content);
        }
    }

    fn path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_default();
        PathBuf::from(home).join(".config/vapordose/config.json")
    }
}
