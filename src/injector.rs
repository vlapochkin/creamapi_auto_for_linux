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
                        // Fallback to copy and remove if rename fails
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
                // If backup already exists and target exists, target is likely a previous proxy. Remove it.
                fs::remove_file(&target.path).ok();
            }

            // Step 2 & 3: Copy resource to target path (assumes the original name)
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

            // Step 4: Copy config and write AppID
            let config_src = crate::utils::get_resources_path().join("SmokeAPI.config.json");
            if let Some(parent_dir) = target.path.parent() {
                let config_dst = parent_dir.join("SmokeAPI.config.json");
                
                if config_src.exists() {
                    fs::copy(&config_src, &config_dst).ok();
                }
                
                self.generate_config(parent_dir, &game.appid)?;
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
                "libsteam_api.so" => "libsteam_api_o.so",
                _ => continue,
            };

            let backup_path = target.path.with_file_name(backup_name);

            if backup_path.exists() {
                // Delete current proxy
                if target.path.exists() {
                    fs::remove_file(&target.path).ok();
                }
                // Rename backup to original
                fs::rename(&backup_path, &target.path).with_context(|| format!("Failed to restore {:?}", backup_path))?;
            }

            // Remove configs
            if let Some(parent_dir) = target.path.parent() {
                let config_path = parent_dir.join("SmokeAPI.config.json");
                if config_path.exists() {
                    fs::remove_file(&config_path).ok();
                }
            }
        }
        Ok(())
    }

    fn generate_config(&self, dir: &Path, appid: &str) -> Result<()> {
        let config_path = dir.join("SmokeAPI.config.json");
        let content = format!(
            "{{\n  \"appid\": {},\n  \"unlock_all\": true,\n  \"logging\": false\n}}",
            appid
        );
        fs::write(&config_path, content).with_context(|| format!("Failed to write config"))?;
        Ok(())
    }

    pub fn get_proton_instructions(&self, game: &SteamGame) -> Option<String> {
        let mut needs_override = false;
        for t in &game.targets {
            if !t.is_linux {
                needs_override = true;
            }
        }
        
        if needs_override {
            Some(format!("WINEDLLOVERRIDES=\"steam_api64=n,b;steam_api=n,b\" %command%"))
        } else {
            None
        }
    }
}
