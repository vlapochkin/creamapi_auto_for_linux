use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

const CACHE_TTL_SECS: u64 = 7 * 24 * 3600; // 7 days

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamCacheEntry {
    pub timestamp: u64,
    pub is_free: bool,
    pub dlc_list: Vec<u32>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SteamCache {
    pub entries: HashMap<String, SteamCacheEntry>,
}

impl SteamCache {
    fn cache_file_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_default();
        let cache_dir = PathBuf::from(home).join(".cache").join("vapordose");
        if !cache_dir.exists() {
            let _ = fs::create_dir_all(&cache_dir);
        }
        cache_dir.join("steam_cache.json")
    }

    pub fn load() -> Self {
        let path = Self::cache_file_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cache) = serde_json::from_str::<SteamCache>(&content) {
                    return cache;
                }
            }
        }
        SteamCache::default()
    }

    pub fn save(&self) {
        let path = Self::cache_file_path();
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, json);
        }
    }

    pub fn get(&self, appid: &str) -> Option<SteamCacheEntry> {
        if let Some(entry) = self.entries.get(appid) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if now.saturating_sub(entry.timestamp) < CACHE_TTL_SECS {
                return Some(entry.clone());
            }
        }
        None
    }

    pub fn insert(&mut self, appid: String, is_free: bool, dlc_list: Vec<u32>) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.entries.insert(appid, SteamCacheEntry { timestamp, is_free, dlc_list });
    }
}
