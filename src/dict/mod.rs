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
    pub async fn lookup(client: &reqwest::Client, word: &str) -> Result<LookupResult> {
        let url = format!("https://jisho.org/api/v1/search/words?keyword={}", urlencoding::encode(word));
        let resp = client.get(&url).send().await?;

        if resp.status().is_success() {
            let json: serde_json::Value = resp.json().await?;
            if let Some(items) = json["data"].as_array() {
                let best_entry = items
                    .iter()
                    .find(|entry| {
                        let is_common = entry["is_common"].as_bool().unwrap_or(false);
                        let matches_word = entry["japanese"].as_array().map_or(false, |jap_arr| {
                            jap_arr.iter().any(|j| {
                                j["word"].as_str() == Some(word) || j["reading"].as_str() == Some(word)
                            })
                        });
                        is_common && matches_word
                    })
                    .or_else(|| {
                        items.iter().find(|entry| {
                            entry["japanese"].as_array().map_or(false, |jap_arr| {
                                jap_arr.iter().any(|j| {
                                    j["word"].as_str() == Some(word) || j["reading"].as_str() == Some(word)
                                })
                            })
                        })
                    })
                    .or_else(|| {
                        items.iter().find(|entry| entry["is_common"].as_bool().unwrap_or(false))
                    })
                    .or_else(|| items.first());

                if let Some(data) = best_entry {
                    let reading = data["japanese"][0]["reading"]
                        .as_str()
                        .unwrap_or(word)
                        .to_string();

                    let mut defs = Vec::new();
                    if let Some(senses) = data["senses"].as_array() {
                        let filtered_senses: Vec<_> = senses
                            .iter()
                            .filter(|s| {
                                let pos_first = s["parts_of_speech"]
                                    .as_array()
                                    .and_then(|a| a.first())
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                pos_first != "Wikipedia definition"
                            })
                            .collect();

                        let active_senses = if filtered_senses.is_empty() {
                            senses.iter().collect::<Vec<_>>()
                        } else {
                            filtered_senses
                        };

                        for (i, sense) in active_senses.iter().take(5).enumerate() {
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
        }

        Ok(LookupResult {
            expression: word.to_string(),
            reading: word.to_string(),
            definition: "1. [def] vocabulary word".to_string(),
            pitch_accent: "LH".to_string(),
        })
    }
}
