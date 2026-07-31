use anyhow::{Context, Result};
use std::fs;
use std::io;
use std::path::Path;
use crate::steam_scanner::SteamGame;

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
                    ("libsteam_api_o.so", res)
                }
                _ => anyhow::bail!("Unknown target file: {}", filename),
            };

            let backup_path = target.path.with_file_name(backup_name);
            
            // Step 1: Backup (Rename original to _o)
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

            // Step 4: Deploy SmokeAPI v4 config into target dir AND game root install_dir
            if let Some(parent_dir) = target.path.parent() {
                self.generate_config(parent_dir)?;
            }
        }

        // Deploy SmokeAPI config to game root directory as well (crucial for Unity / Paradox games like Cities Skylines)
        self.generate_config(&game.install_dir)?;

        // Auto-inject Proton WINEDLLOVERRIDES into Steam localconfig.vdf if game has Windows targets
        if game.targets.iter().any(|t| !t.is_linux) {
            crate::steam_vdf::apply_proton_launch_options(&game.appid).ok();
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
                "libsteam_api.so" => "libsteam_api_o.so",
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
                if config_path.exists() {
                    fs::remove_file(&config_path).ok();
                }
            }
        }

        // Clean root config
        let root_config = game.install_dir.join("SmokeAPI.config.json");
        if root_config.exists() {
            fs::remove_file(&root_config).ok();
        }

        // Auto-remove Proton WINEDLLOVERRIDES from Steam localconfig.vdf
        crate::steam_vdf::remove_proton_launch_options(&game.appid).ok();

        Ok(())
    }

    /// Generates standard SmokeAPI v4 config JSON
    fn generate_config(&self, dir: &Path) -> Result<()> {
        let config_path = dir.join("SmokeAPI.config.json");
        let content = r#"{
  "$version": 4,
  "logging": false,
  "default_app_status": "unlocked",
  "override_dlc_status": {}
}"#;
        fs::write(&config_path, content).with_context(|| format!("Failed to write SmokeAPI.config.json at {:?}", config_path))?;
        Ok(())
    }

    pub fn save_custom_config(&self, game: &SteamGame, dlcs_csv: &str) -> Result<()> {
        let dlc_ids: Vec<&str> = dlcs_csv
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        let mut override_map = serde_json::Map::new();
        for id in dlc_ids {
            override_map.insert(id.to_string(), serde_json::Value::String("unlocked".to_string()));
        }

        let mut root_json = serde_json::Map::new();
        root_json.insert("$version".to_string(), serde_json::Value::Number(4.into()));
        root_json.insert("logging".to_string(), serde_json::Value::Bool(false));
        root_json.insert("default_app_status".to_string(), serde_json::Value::String("unlocked".to_string()));
        root_json.insert("override_dlc_status".to_string(), serde_json::Value::Object(override_map));

        let formatted = serde_json::to_string_pretty(&root_json).unwrap_or_default();

        for target in &game.targets {
            if let Some(parent_dir) = target.path.parent() {
                let config_path = parent_dir.join("SmokeAPI.config.json");
                fs::write(&config_path, &formatted).with_context(|| format!("Failed to write custom config"))?;
            }
        }
        let root_config = game.install_dir.join("SmokeAPI.config.json");
        fs::write(&root_config, &formatted).ok();

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
