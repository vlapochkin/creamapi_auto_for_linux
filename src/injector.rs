use anyhow::{Context, Result};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use crate::steam_scanner::SteamGame;

fn get_user_config_dir() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config"))
}

pub struct Injector {}

impl Injector {
    pub fn new() -> Self { Self {} }

    pub fn backup_and_deploy(&self, game: &SteamGame) -> Result<()> {
        if game.targets.is_empty() {
            anyhow::bail!("No targets found for this game.");
        }

        for target in &game.targets {
            if !target.path.starts_with(&game.install_dir) {
                anyhow::bail!("Security error: Path {:?} is outside of game directory", target.path);
            }

            let filename = target.path.file_name().unwrap_or_default().to_string_lossy();
            
            // Determine names
            let (backup_name, resource_name) = match filename.as_ref() {
                "steam_api64.dll" => ("steam_api64_o.dll", "smoke_api64.dll"),
                "steam_api.dll" => ("steam_api_o.dll", "smoke_api32.dll"),
                "libsteam_api.so" => {
                    let res = if target.is_64bit { "libsmoke_api64.so" } else { "libsmoke_api32.so" };
                    ("libsteam_api.so.orig", res)
                }
                _ => anyhow::bail!("Unknown target file: {}", filename),
            };

            let backup_path = target.path.with_file_name(backup_name);
            let root_backup_path = game.install_dir.join("libsteam_api_o.so");

            // Step 1: Backup (Rename original to .orig / root _o.so)
            if !backup_path.exists() {
                if target.path.exists() {
                    if let Err(_) = fs::rename(&target.path, &backup_path) {
                        if let Err(e) = fs::copy(&target.path, &backup_path).and_then(|_| fs::remove_file(&target.path)) {
                            if e.kind() == io::ErrorKind::PermissionDenied {
                                anyhow::bail!("Permission Denied: Cannot backup file. The filesystem might be read-only.");
                            }
                            return Err(e).with_context(|| format!("Failed to backup {:?}", target.path));
                        }
                    }
                    if target.is_linux {
                        fs::copy(&backup_path, &root_backup_path).ok();
                    }
                } else {
                    anyhow::bail!("Original file missing: {:?}", target.path);
                }
            } else if target.path.exists() {
                fs::remove_file(&target.path).ok();
            }

            // Step 2 & 3: Copy proxy binary to target path
            let source_file = crate::utils::get_resources_path().join(resource_name);
            if !source_file.exists() {
                fs::rename(&backup_path, &target.path).ok(); // Restore on fail
                anyhow::bail!("Resource file not found: {:?}", source_file);
            }

            if let Err(e) = fs::copy(&source_file, &target.path) {
                fs::rename(&backup_path, &target.path).ok(); // Restore on fail
                if e.kind() == io::ErrorKind::PermissionDenied {
                    anyhow::bail!("Permission Denied: Cannot write proxy file.");
                }
                return Err(e).with_context(|| format!("Failed to deploy proxy to {:?}", target.path));
            }

            // Step 4: Deploy SmokeAPI v4 config into target dir
            if let Some(parent_dir) = target.path.parent() {
                self.generate_configs(parent_dir, &game.appid)?;
            }
        }

        // Deploy SmokeAPI config to game root directory (crucial for Unity / Paradox games)
        self.generate_configs(&game.install_dir, &game.appid)?;

        // Deploy SmokeAPI config to user config directory (~/.config/SmokeAPI/)
        if let Some(user_config_dir) = get_user_config_dir() {
            let smoke_config_dir = user_config_dir.join("SmokeAPI");
            fs::create_dir_all(&smoke_config_dir).ok();
            self.generate_configs(&smoke_config_dir, &game.appid)?;
            let app_specific = smoke_config_dir.join(format!("{}.json", game.appid));
            self.write_smoke_config(&app_specific, &game.appid, &[])?;
        }

        // Auto-inject launch options into Steam localconfig.vdf
        for target in &game.targets {
            if target.is_linux {
                let ld_preload_str = format!(r#"LD_PRELOAD=\"{}\""#, target.path.display());
                crate::steam_vdf::apply_launch_options(&game.appid, &ld_preload_str).ok();
            } else {
                crate::steam_vdf::apply_proton_launch_options(&game.appid).ok();
            }
        }

        Ok(())
    }

    pub fn restore_original(&self, game: &SteamGame) -> Result<()> {
        for target in &game.targets {
            if !target.path.starts_with(&game.install_dir) {
                continue;
            }

            let filename = target.path.file_name().unwrap_or_default().to_string_lossy();
            let backup_name = match filename.as_ref() {
                "steam_api64.dll" => "steam_api64_o.dll",
                "steam_api.dll" => "steam_api_o.dll",
                "libsteam_api.so" => "libsteam_api.so.orig",
                _ => continue,
            };

            let backup_path = target.path.with_file_name(backup_name);

            if backup_path.exists() {
                if target.path.exists() {
                    fs::remove_file(&target.path).ok();
                }
                fs::rename(&backup_path, &target.path).with_context(|| format!("Failed to restore {:?}", backup_path))?;
            }

            if let Some(parent_dir) = target.path.parent() {
                let config_path = parent_dir.join("SmokeAPI.config.json");
                let ini_path = parent_dir.join("cream_api.ini");
                fs::remove_file(&config_path).ok();
                fs::remove_file(&ini_path).ok();
            }
        }

        // Clean root config & backups
        let root_config = game.install_dir.join("SmokeAPI.config.json");
        let root_ini = game.install_dir.join("cream_api.ini");
        let root_backup = game.install_dir.join("libsteam_api_o.so");
        fs::remove_file(&root_config).ok();
        fs::remove_file(&root_ini).ok();
        fs::remove_file(&root_backup).ok();

        // Clean user config dir
        if let Some(user_config_dir) = get_user_config_dir() {
            let app_specific = user_config_dir.join("SmokeAPI").join(format!("{}.json", game.appid));
            fs::remove_file(&app_specific).ok();
        }

        // Auto-remove launch options from Steam localconfig.vdf
        crate::steam_vdf::remove_launch_options(&game.appid).ok();

        Ok(())
    }

    /// Generates standard SmokeAPI v4 config JSON and cream_api.ini fallback
    fn generate_configs(&self, dir: &Path, appid: &str) -> Result<()> {
        let json_path = dir.join("SmokeAPI.config.json");
        self.write_smoke_config(&json_path, appid, &[])?;

        let ini_path = dir.join("cream_api.ini");
        let ini_content = format!(
            "[steam]\nappid = {}\nunlockall = true\nextraprotection = false\n\n[dlc]\n",
            appid
        );
        fs::write(&ini_path, ini_content).ok();

        Ok(())
    }

    fn write_smoke_config(&self, path: &Path, appid: &str, dlcs: &[&str]) -> Result<()> {
        let mut override_map = serde_json::Map::new();
        for id in dlcs {
            override_map.insert(id.to_string(), serde_json::Value::String("unlocked".to_string()));
        }

        let mut root_json = serde_json::Map::new();
        root_json.insert("$version".to_string(), serde_json::Value::Number(4.into()));
        root_json.insert("logging".to_string(), serde_json::Value::Bool(false));
        root_json.insert("default_app_status".to_string(), serde_json::Value::String("unlocked".to_string()));
        root_json.insert("unlock_all".to_string(), serde_json::Value::Bool(true));
        if let Ok(appid_num) = appid.parse::<u64>() {
            root_json.insert("appid".to_string(), serde_json::Value::Number(appid_num.into()));
        }
        root_json.insert("override_dlc_status".to_string(), serde_json::Value::Object(override_map));

        let formatted = serde_json::to_string_pretty(&root_json).unwrap_or_default();
        fs::write(path, formatted).with_context(|| format!("Failed to write SmokeAPI config at {:?}", path))?;
        Ok(())
    }

    pub fn save_custom_config(&self, game: &SteamGame, dlcs_csv: &str) -> Result<()> {
        let dlc_ids: Vec<&str> = dlcs_csv
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        for target in &game.targets {
            if let Some(parent_dir) = target.path.parent() {
                let config_path = parent_dir.join("SmokeAPI.config.json");
                self.write_smoke_config(&config_path, &game.appid, &dlc_ids)?;
            }
        }
        let root_config = game.install_dir.join("SmokeAPI.config.json");
        self.write_smoke_config(&root_config, &game.appid, &dlc_ids).ok();

        if let Some(user_config_dir) = get_user_config_dir() {
            let smoke_config_dir = user_config_dir.join("SmokeAPI");
            let app_specific = smoke_config_dir.join(format!("{}.json", game.appid));
            self.write_smoke_config(&app_specific, &game.appid, &dlc_ids).ok();
        }

        Ok(())
    }

    pub fn get_proton_instructions(&self, game: &SteamGame) -> Option<String> {
        if game.targets.iter().any(|t| !t.is_linux) {
            Some(format!("WINEDLLOVERRIDES=\"steam_api64=n,b;steam_api=n,b\" %command%"))
        } else {
            None
        }
    }
}
