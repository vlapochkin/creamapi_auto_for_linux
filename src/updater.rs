use anyhow::{Context, Result};
use reqwest::Client;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use zip::ZipArchive;
use bytes::Bytes;

pub async fn check_and_download_core() -> Result<String> {
    // We use SmokeAPI as the core engine for DLC unlocking.
    // Credits to: https://github.com/acidicoala/SmokeAPI
    let client = Client::builder()
        .user_agent("CreamAPI-Auto-Linux/0.1.0 (SmokeAPI-Integration)")
        .build()?;

    let url = "https://api.github.com/repos/acidicoala/SmokeAPI/releases/latest";
    let resp: serde_json::Value = client.get(url).send().await?.json().await?;

    let tag_name = resp["tag_name"].as_str().context("No tag_name in release")?;
    let version_file = crate::utils::get_resources_path().join("version.txt");

    if version_file.exists() {
        let current_version = fs::read_to_string(&version_file).unwrap_or_default();
        if current_version.trim() == tag_name {
            return Ok(format!("Обновление не требуется. Текущая версия: {}", tag_name));
        }
    }

    let assets = resp["assets"].as_array().context("No assets found in release")?;
    
    let mut download_url = None;
    for asset in assets {
        if let Some(name) = asset["name"].as_str() {
            if name.ends_with(".zip") || name.contains("linux") || name.contains("win") {
                download_url = asset["browser_download_url"].as_str().map(|s| s.to_string());
                break;
            }
        }
    }

    let download_url = download_url.context("Could not find suitable archive asset")?;
    let archive_bytes = client.get(&download_url).send().await?.bytes().await?;
    
    extract_from_memory(archive_bytes).await?;

    // Update version
    fs::write(&version_file, tag_name)?;

    Ok(format!("Успешно обновлено до {}", tag_name))
}

async fn extract_from_memory(bytes: Bytes) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let reader = Cursor::new(bytes);
        let mut archive = ZipArchive::new(reader)?;

        let resources_dir = crate::utils::get_resources_path();
        if !resources_dir.exists() {
            fs::create_dir_all(&resources_dir)?;
        }

        let target_files = vec![
            "libsmoke_api32.so",
            "libsmoke_api64.so",
            "smoke_api32.dll",
            "smoke_api64.dll",
            "SmokeAPI.config.json",
        ];

        let mut extracted = 0;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let filename = file.name().to_string();
            
            if target_files.contains(&filename.as_str()) {
                let outpath = resources_dir.join(&filename);
                let mut outfile = fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
                extracted += 1;
            }
        }

        if extracted == 0 {
            anyhow::bail!("Could not find target files in the downloaded archive.");
        }

        Ok(())
    }).await?
}
