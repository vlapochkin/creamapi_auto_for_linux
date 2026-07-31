use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use regex::Regex;
use reqwest::Client;
use std::fs;
use std::io::Read;

#[derive(Debug, Clone, PartialEq)]
pub enum GameType { Proton, Native, Mixed, Unknown }

#[derive(Debug, Clone, PartialEq)]
pub enum AppCategory { SystemTool, FreeToPlay, PaidWithDLC, NoDLC, DrmFree, Unknown }

#[derive(Debug, Clone)]
pub struct TargetFile {
    pub path: PathBuf,
    pub is_64bit: bool,
    pub is_linux: bool,
}

#[derive(Debug, Clone)]
pub struct SteamGame {
    pub appid: String,
    pub name: String,
    pub install_dir: PathBuf,
    pub game_type: GameType,
    pub targets: Vec<TargetFile>,
    pub category: AppCategory,
    pub dlc_list: Vec<u32>,
    pub has_anticheat: bool,
    pub is_online_multiplayer: bool,
    pub anticheat_name: Option<String>,
    pub is_patched: bool,
    pub icon_path: Option<PathBuf>,
}

pub fn discover_steam_libraries() -> Vec<PathBuf> {
    let mut libraries = Vec::new();
    let home = std::env::var("HOME").unwrap_or_default();
    let paths = vec![
        Path::new(&home).join(".local/share/Steam"),
        Path::new(&home).join(".steam/root"),
        Path::new(&home).join(".steam/steam"),
        Path::new(&home).join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    ];
    for p in paths {
        if p.exists() {
            let real_path = fs::canonicalize(&p).unwrap_or(p);
            if !libraries.contains(&real_path) {
                libraries.push(real_path);
            }
        }
    }
    let mut all_libraries = libraries.clone();
    for steam_path in &libraries {
        let vdf_path = steam_path.join("steamapps/libraryfolders.vdf");
        if let Ok(extra_paths) = parse_library_folders(&vdf_path) {
            for path in extra_paths {
                if path.exists() {
                    let real_path = fs::canonicalize(&path).unwrap_or(path);
                    if !all_libraries.contains(&real_path) {
                        all_libraries.push(real_path);
                    }
                }
            }
        }
    }

    // Auto-detect MicroSD cards and external mounts (/run/media/ & /run/media/$USER/*) for Steam Deck
    let run_media = Path::new("/run/media");
    if run_media.exists() {
        let mut search_dirs = vec![run_media.to_path_buf()];
        let user = std::env::var("USER").unwrap_or_default();
        if !user.is_empty() {
            let user_media = run_media.join(&user);
            if user_media.exists() {
                search_dirs.push(user_media);
            }
        }
        for s_dir in search_dirs {
            if let Ok(entries) = fs::read_dir(&s_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        let steamapps = p.join("steamapps");
                        if steamapps.exists() {
                            let real_path = fs::canonicalize(&p).unwrap_or_else(|_| p.clone());
                            if !all_libraries.contains(&real_path) {
                                all_libraries.push(real_path);
                            }
                        }
                        let steam_lib = p.join("SteamLibrary");
                        if steam_lib.exists() {
                            let real_path = fs::canonicalize(&steam_lib).unwrap_or_else(|_| p.clone());
                            if !all_libraries.contains(&real_path) {
                                all_libraries.push(real_path);
                            }
                        }
                    }
                }
            }
        }
    }

    all_libraries
}

fn parse_library_folders(vdf_path: &Path) -> Result<Vec<PathBuf>> {
    if !vdf_path.exists() { return Ok(Vec::new()); }
    let content = fs::read_to_string(vdf_path)?;
    let mut paths = Vec::new();
    let re = Regex::new(r#""path"\s+"([^"]+)""#).unwrap();
    for cap in re.captures_iter(&content) {
        if let Some(path_match) = cap.get(1) { paths.push(PathBuf::from(path_match.as_str())); }
    }
    Ok(paths)
}

use crate::steam_cache::SteamCache;
use std::collections::HashSet;

pub async fn scan_games(libraries: &[PathBuf]) -> Vec<SteamGame> {
    let mut games = Vec::new();
    let mut seen_appids = HashSet::new();

    for lib_path in libraries {
        let apps_path = lib_path.join("steamapps");
        if !apps_path.exists() { continue; }
        if let Ok(entries) = fs::read_dir(&apps_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("acf") {
                    if let Ok(game) = parse_acf(&path) {
                        if !seen_appids.contains(&game.appid) {
                            seen_appids.insert(game.appid.clone());
                            games.push(game);
                        }
                    }
                }
            }
        }
    }

    let mut cache = SteamCache::load();
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| Client::new());

    for game in &mut games {
        check_local_anticheat(game);
        let lower_name = game.name.to_lowercase();
        if lower_name.contains("proton") || lower_name.contains("steam linux runtime") || 
           lower_name.contains("redistributable") || lower_name.contains("sdk") || 
           game.appid == "228980" || game.appid == "1391110" {
            game.category = AppCategory::SystemTool;
            continue;
        }

        // Check local cache first
        if let Some(cached_entry) = cache.get(&game.appid) {
            game.dlc_list = cached_entry.dlc_list;
            game.category = if cached_entry.is_free {
                AppCategory::FreeToPlay
            } else if game.dlc_list.is_empty() {
                AppCategory::NoDLC
            } else {
                AppCategory::PaidWithDLC
            };
            continue;
        }

        // Fetch from Steam API if not cached
        let url = format!("https://store.steampowered.com/api/appdetails?appids={}", game.appid);
        if let Ok(resp) = client.get(&url).send().await {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(app_data) = json.get(&game.appid).and_then(|a| a.get("data")) {
                    let is_free = app_data.get("is_free").and_then(|v| v.as_bool()).unwrap_or(false);
                    let mut dlc_list = Vec::new();
                    if let Some(dlcs) = app_data.get("dlc").and_then(|v| v.as_array()) {
                        for dlc in dlcs { if let Some(id) = dlc.as_u64() { dlc_list.push(id as u32); } }
                    }
                    game.dlc_list = dlc_list.clone();
                    game.category = if is_free { AppCategory::FreeToPlay } else if game.dlc_list.is_empty() { AppCategory::NoDLC } else { AppCategory::PaidWithDLC };
                    
                    // Save to cache
                    cache.insert(game.appid.clone(), is_free, dlc_list);
                }
            }
        }
    }
    cache.save();
    games
}

fn check_local_anticheat(game: &mut SteamGame) {
    if !game.install_dir.exists() { return; }
    for entry in WalkDir::new(&game.install_dir).max_depth(6).into_iter().flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let path_str = entry.path().to_string_lossy().to_lowercase();
        
        let ac_detected = if name.contains("easyanticheat") || name.contains("eac") || path_str.contains("easyanticheat") {
            Some("EasyAntiCheat".to_string())
        } else if name.contains("battleye") || path_str.contains("battleye") {
            Some("BattlEye".to_string())
        } else if name.contains("vanguard") || path_str.contains("vanguard") {
            Some("Vanguard".to_string())
        } else if name.contains("ricochet") || path_str.contains("ricochet") {
            Some("Ricochet".to_string())
        } else if name.contains("xigncode") || path_str.contains("xigncode") {
            Some("XIGNCODE3".to_string())
        } else if name.contains("equ8") || name.contains("nprotect") || name.contains("ace_kernel") || name.contains("mhyprot") {
            Some("Anti-Cheat".to_string())
        } else if name.contains("denuvo") && (name.contains("anticheat") || name.contains("ac")) {
            Some("Denuvo Anti-Cheat".to_string())
        } else {
            None
        };

        if let Some(ac_name) = ac_detected {
            game.has_anticheat = true;
            game.is_online_multiplayer = true;
            game.anticheat_name = Some(ac_name);
            break;
        }
    }
}

fn is_elf64(path: &Path) -> bool {
    if let Ok(mut f) = fs::File::open(path) {
        let mut header = [0u8; 5];
        if f.read_exact(&mut header).is_ok() {
            if header[0..4] == [0x7f, b'E', b'L', b'F'] { return header[4] == 2; }
        }
    }
    true
}

fn find_game_icon(appid: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    
    // 1. PRIORITY: Steam Desktop Icons (steam_icon_<appid>.png/svg/ico)
    let icon_search_paths = vec![
        Path::new(&home).join(".local/share/icons"),
        Path::new(&home).join(".icons"),
        Path::new("/usr/share/icons").to_path_buf(),
        Path::new("/usr/share/pixmaps").to_path_buf(),
    ];

    let target_prefix = format!("steam_icon_{}", appid);
    for search_path in icon_search_paths {
        if search_path.exists() {
            let mut candidates = Vec::new();
            for entry in WalkDir::new(&search_path).max_depth(5).into_iter().flatten() {
                let name = entry.file_name().to_string_lossy();
                if name.starts_with(&target_prefix) {
                    candidates.push(entry.path().to_path_buf());
                }
            }
            if let Some(best) = candidates.into_iter().max_by_key(|p| {
                let s = p.to_string_lossy();
                if s.contains("256x256") || s.contains("scalable") { 4 }
                else if s.contains("128x128") { 3 }
                else if s.contains("64x64") { 2 }
                else if s.contains("48x48") || s.contains("32x32") { 1 }
                else { 0 }
            }) {
                return Some(best);
            }
        }
    }

    // 2. SECONDARY: appcache/librarycache/<appid>/ (logo.png, header.jpg, hash jpg)
    let cache_dirs = vec![
        Path::new(&home).join(".steam/root/appcache/librarycache"),
        Path::new(&home).join(".steam/steam/appcache/librarycache"),
        Path::new(&home).join(".local/share/Steam/appcache/librarycache"),
        Path::new(&home).join(".var/app/com.valvesoftware.Steam/.local/share/Steam/appcache/librarycache"),
    ];

    for cache_dir in cache_dirs {
        let app_dir = cache_dir.join(appid);
        if app_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&app_dir) {
                let mut logo_png = None;
                let mut header_jpg = None;
                let mut hash_jpg = None;

                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        let name = path.file_name().unwrap_or_default().to_string_lossy();
                        if name == "logo.png" {
                            logo_png = Some(path);
                        } else if name.contains("header") && name.ends_with(".jpg") {
                            header_jpg = Some(path);
                        } else if name.len() >= 40 && name.ends_with(".jpg") {
                            hash_jpg = Some(path);
                        }
                    }
                }

                if let Some(p) = logo_png { return Some(p); }
                if let Some(p) = header_jpg { return Some(p); }
                if let Some(p) = hash_jpg { return Some(p); }
            }
        }

        let direct_icon = cache_dir.join(format!("{}_icon.jpg", appid));
        if direct_icon.exists() { return Some(direct_icon); }
    }

    None
}

#[test]
fn test_find_game_icon() {
    let test_appids = vec!["594650", "383980", "669330", "286160", "252950"];
    for appid in test_appids {
        let icon = find_game_icon(appid);
        println!("AppID: {}, Icon: {:?}", appid, icon);
    }
}

fn parse_acf(acf_path: &Path) -> Result<SteamGame> {
    let content = fs::read_to_string(acf_path)?;
    let re_appid = Regex::new(r#""appid"\s+"([^"]+)""#).unwrap();
    let re_name = Regex::new(r#""name"\s+"([^"]+)""#).unwrap();
    let re_installdir = Regex::new(r#""installdir"\s+"([^"]+)""#).unwrap();
    let appid = re_appid.captures(&content).and_then(|c| c.get(1)).map(|m| m.as_str().to_string()).context("Missing appid")?;
    let name = re_name.captures(&content).and_then(|c| c.get(1)).map(|m| m.as_str().to_string()).context("Missing name")?;
    let installdir_name = re_installdir.captures(&content).and_then(|c| c.get(1)).map(|m| m.as_str().to_string()).context("Missing installdir")?;
    let install_dir = acf_path.parent().unwrap().join("common").join(installdir_name);
    
    let mut targets = Vec::new();
    let mut has_proton = false;
    let mut has_native = false;
    let mut is_patched = false;

    let icon_path = find_game_icon(&appid);

    if install_dir.exists() {
        for entry in WalkDir::new(&install_dir).max_depth(5).into_iter().flatten() {
            if !entry.file_type().is_file() { continue; }
            let filename = entry.file_name().to_string_lossy();
            let path = entry.path().to_path_buf();
            if filename == "steam_api.dll" {
                targets.push(TargetFile { path: path.clone(), is_64bit: false, is_linux: false });
                has_proton = true;
                if path.with_file_name("steam_api_o.dll").exists() { is_patched = true; }
            } else if filename == "steam_api64.dll" {
                targets.push(TargetFile { path: path.clone(), is_64bit: true, is_linux: false });
                has_proton = true;
                if path.with_file_name("steam_api64_o.dll").exists() { is_patched = true; }
            } else if filename == "libsteam_api.so" {
                let is_64 = is_elf64(&path);
                targets.push(TargetFile { path: path.clone(), is_64bit: is_64, is_linux: true });
                has_native = true;
                if path.with_file_name("libsteam_api_o.so").exists() { is_patched = true; }
            }
        }
    }
    let game_type = if has_proton && has_native { GameType::Mixed }
    else if has_proton { GameType::Proton } else if has_native { GameType::Native } else { GameType::Unknown };

    Ok(SteamGame {
        appid, name, install_dir, game_type, targets, category: AppCategory::Unknown,
        dlc_list: Vec::new(), has_anticheat: false, is_online_multiplayer: false,
        anticheat_name: None, is_patched, icon_path,
    })
}
