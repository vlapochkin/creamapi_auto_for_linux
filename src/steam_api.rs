use anyhow::Result;
use reqwest::Client;
use serde_json::Value;

pub struct SteamApiFetcher {
    client: Client,
}

impl SteamApiFetcher {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(6))
            .user_agent("Mozilla/5.0 (X11; Linux x86_64)")
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }

    /// Fetch DLC appids and their names for a given game appid
    pub async fn fetch_dlcs_for_app(&self, appid: &str) -> Result<Vec<(u32, String)>> {
        let url = format!("https://store.steampowered.com/api/appdetails?appids={}", appid);
        let resp = self.client.get(&url).send().await?.json::<Value>().await?;

        let app_data = &resp[appid]["data"];
        if !resp[appid]["success"].as_bool().unwrap_or(false) || app_data.is_null() {
            anyhow::bail!("Failed to get app details from Steam API");
        }

        let dlc_ids: Vec<u32> = app_data["dlc"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|id| id as u32))
                    .collect()
            })
            .unwrap_or_default();

        if dlc_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut dlc_results = Vec::new();
        
        // Fetch DLC names in chunks of 10 to avoid request URL length limits
        for chunk in dlc_ids.chunks(10) {
            let chunk_ids_str = chunk.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
            let chunk_url = format!("https://store.steampowered.com/api/appdetails?appids={}", chunk_ids_str);
            
            if let Ok(chunk_resp) = self.client.get(&chunk_url).send().await {
                if let Ok(json) = chunk_resp.json::<Value>().await {
                    for &dlc_id in chunk {
                        let id_str = dlc_id.to_string();
                        let dlc_name = json[&id_str]["data"]["name"]
                            .as_str()
                            .unwrap_or("DLC")
                            .to_string();
                        dlc_results.push((dlc_id, dlc_name));
                    }
                }
            }
        }

        Ok(dlc_results)
    }
}
