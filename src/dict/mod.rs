use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupResult {
    pub expression: String,
    pub reading: String,
    pub definition: String,
    pub pitch_accent: String,
}

pub struct DictionaryService;

impl DictionaryService {
    pub async fn lookup(word: &str) -> Result<LookupResult> {
        let url = format!("https://jisho.org/api/v1/search/words?keyword={}", urlencoding::encode(word));
        let resp = reqwest::get(&url).await?;

        if resp.status().is_success() {
            let json: serde_json::Value = resp.json().await?;
            if let Some(data) = json["data"].as_array().and_then(|a| a.first()) {
                let reading = data["japanese"][0]["reading"]
                    .as_str()
                    .unwrap_or(word)
                    .to_string();

                let mut defs = Vec::new();
                if let Some(senses) = data["senses"].as_array() {
                    for (i, sense) in senses.iter().take(5).enumerate() {
                        if let Some(english) = sense["english_definitions"].as_array() {
                            let items: Vec<&str> = english.iter().filter_map(|v| v.as_str()).collect();
                            let pos_str = sense["parts_of_speech"]
                                .as_array()
                                .and_then(|a| a.first())
                                .and_then(|v| v.as_str())
                                .unwrap_or("def");
                            if !items.is_empty() {
                                defs.push(format!("{}. [{}] {}", i + 1, pos_str, items.join(", ")));
                            }
                        }
                    }
                }

                let definition = if defs.is_empty() {
                    "1. [def] vocabulary word".to_string()
                } else {
                    defs.join("\n│                 ")
                };

                return Ok(LookupResult {
                    expression: word.to_string(),
                    reading,
                    definition,
                    pitch_accent: "LH".to_string(),
                });
            }
        }

        Ok(LookupResult {
            expression: word.to_string(),
            reading: word.to_string(),
            definition: "1. [def] vocabulary word".to_string(),
            pitch_accent: "LH".to_string(),
        })
    }
}
