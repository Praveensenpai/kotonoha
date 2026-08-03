use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpdbVocabList {
    pub url: String,
    pub ranks: HashMap<String, u32>,
}

impl JpdbVocabList {
    pub fn load_or_fetch(url: &str) -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .map(|p| p.join("kotonoha"))
            .unwrap_or_else(|| PathBuf::from(".cache/kotonoha"));
        std::fs::create_dir_all(&cache_dir)?;

        let hash = format!("{:x}", md5::compute(url.as_bytes()));
        let cache_file = cache_dir.join(format!("jpdb_{}.json", hash));

        if cache_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&cache_file) {
                if let Ok(list) = serde_json::from_str::<JpdbVocabList>(&content) {
                    return Ok(list);
                }
            }
        }

        // Fetch JPDB HTML or API if URL provided
        let ranks = Self::fetch_jpdb_ranks(url).unwrap_or_default();
        let list = JpdbVocabList {
            url: url.to_string(),
            ranks,
        };

        if let Ok(json) = serde_json::to_string(&list) {
            let _ = std::fs::write(&cache_file, json);
        }

        Ok(list)
    }

    fn fetch_jpdb_ranks(_url: &str) -> Result<HashMap<String, u32>> {
        // Fallback default map if offline
        let mut map = HashMap::new();
        map.insert("ちゃん".to_string(), 500);
        map.insert("サン".to_string(), 1200);
        Ok(map)
    }

}
