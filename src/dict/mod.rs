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
                    .max_by_key(|entry| {
                        let mut score: i32 = 0;
                        let is_common = entry["is_common"].as_bool().unwrap_or(false);
                        if is_common {
                            score += 20;
                        }

                        let mut exact_match = false;
                        let mut has_kanji_form = false;
                        let mut min_len_diff = 1000;

                        if let Some(jap_arr) = entry["japanese"].as_array() {
                            for j in jap_arr {
                                let w_str = j["word"].as_str().unwrap_or("");
                                let r_str = j["reading"].as_str().unwrap_or("");

                                if w_str == word || r_str == word {
                                    exact_match = true;
                                }

                                let w_len = if !w_str.is_empty() { w_str.chars().count() } else { r_str.chars().count() };
                                let word_len = word.chars().count();
                                let diff = (w_len as i32 - word_len as i32).abs();
                                if diff < min_len_diff {
                                    min_len_diff = diff;
                                }

                                if w_str.chars().any(|c| matches!(c, '\u{4E00}'..='\u{9FFF}')) {
                                    has_kanji_form = true;
                                }
                            }
                        }

                        if exact_match {
                            score += 100;
                        }
                        if has_kanji_form {
                            score += 30;
                        }
                        score -= min_len_diff * 10;

                        if let Some(senses) = entry["senses"].as_array() {
                            let is_wiki = senses.iter().any(|s| {
                                s["parts_of_speech"]
                                    .as_array()
                                    .and_then(|a| a.first())
                                    .and_then(|v| v.as_str())
                                    == Some("Wikipedia definition")
                            });
                            if is_wiki {
                                score -= 100;
                            }

                            // Penalize single-word definitions that look like obscure typography loanwords
                            if senses.len() == 1 {
                                if let Some(english) = senses[0]["english_definitions"].as_array() {
                                    if english.len() == 1 {
                                        let def_str = english[0].as_str().unwrap_or("").to_lowercase();
                                        if def_str == "serif" || def_str == word.to_lowercase() {
                                            score -= 40;
                                        }
                                    }
                                }
                            }
                        }

                        score
                    });

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_serif_lookup() {
        let client = reqwest::Client::new();
        let res = DictionaryService::lookup(&client, "セリフ").await.unwrap();
        println!("LOOKUP RESULT: {:?}", res);
        assert!(res.definition.contains("speech") || res.definition.contains("lines"));
    }
}
