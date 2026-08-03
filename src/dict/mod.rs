use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupResult {
    pub expression: String,
    pub reading: String,
    pub definition: String,
    pub pitch_accent: String,
}

/// Returns true for the legacy value used when a dictionary lookup failed.
/// This value must never be persisted as if it were a real definition.
pub fn is_placeholder_definition(definition: &str) -> bool {
    definition.trim() == "1. [def] vocabulary word"
}

pub fn split_morae(reading: &str) -> Vec<String> {
    let small_kana = ['ゃ', 'ゅ', 'ょ', 'ぁ', 'ぃ', 'ぅ', 'ぇ', 'ぉ', 'ャ', 'ュ', 'ョ', 'ァ', 'ィ', 'ゥ', 'ェ', 'ォ'];
    let mut morae = Vec::new();
    let chars: Vec<char> = reading.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && small_kana.contains(&chars[i + 1]) {
            morae.push(format!("{}{}", chars[i], chars[i + 1]));
            i += 2;
        } else {
            morae.push(chars[i].to_string());
            i += 1;
        }
    }
    morae
}

pub fn format_pitch_accent(reading: &str, pitch_num: usize) -> (String, String, usize) {
    let morae = split_morae(reading);
    let total_morae = morae.len();
    if total_morae == 0 {
        return (reading.to_string(), format!("[{}] H (0 morae)", pitch_num), 0);
    }

    let mut pattern = vec![0; total_morae];
    let k = pitch_num;

    if k == 1 {
        pattern[0] = 1;
    } else if k == 0 || k >= total_morae {
        for i in 1..total_morae {
            pattern[i] = 1;
        }
    } else {
        for i in 1..k.min(total_morae) {
            pattern[i] = 1;
        }
    }

    let mut overbar_str = String::new();
    let mut hl_str = String::new();

    for (i, mora) in morae.iter().enumerate() {
        let is_high = pattern[i] == 1;
        if is_high {
            hl_str.push('H');
            for c in mora.chars() {
                overbar_str.push(c);
                overbar_str.push('\u{0305}');
            }
        } else {
            hl_str.push('L');
            for c in mora.chars() {
                overbar_str.push(c);
                overbar_str.push('\u{0332}');
            }
        }
    }

    (overbar_str, format!("[{}] {} ({} morae)", k, hl_str, total_morae), total_morae)
}

pub struct DictionaryService;

impl DictionaryService {
    #[allow(dead_code)]
    pub async fn lookup(client: &reqwest::Client, word: &str) -> Result<LookupResult> {
        Self::lookup_with_limits(client, word, 3, 4).await
    }

    pub async fn lookup_with_limits(
        client: &reqwest::Client,
        word: &str,
        max_senses: usize,
        max_glosses: usize,
    ) -> Result<LookupResult> {
        let res = Self::lookup_internal(client, word, true, max_senses, max_glosses).await?;
        if !is_placeholder_definition(&res.definition)
            && res.definition != "No dictionary definition found"
            && !res.definition.contains("[Noun] serif")
        {
            return Ok(res);
        }

        let stem_fallbacks = [
            ("り", "る"),
            ("い", "う"),
            ("ち", "つ"),
            ("き", "く"),
            ("ぎ", "ぐ"),
            ("み", "む"),
            ("び", "ぶ"),
            ("し", "す"),
        ];

        for (stem_end, verb_end) in stem_fallbacks {
            if word.ends_with(stem_end) {
                let verb_form = format!("{}{}", &word[..word.len() - stem_end.len()], verb_end);
                if let Ok(fallback_res) = Self::lookup_internal(client, &verb_form, true, max_senses, max_glosses).await {
                    if !is_placeholder_definition(&fallback_res.definition)
                        && fallback_res.definition != "No dictionary definition found"
                    {
                        return Ok(LookupResult {
                            expression: word.to_string(),
                            reading: fallback_res.reading,
                            definition: fallback_res.definition,
                            pitch_accent: fallback_res.pitch_accent,
                        });
                    }
                }
            }
        }

        // If no exact match and no verb stem match, try inexact candidate lookup (e.g. 月曜 -> 月曜日)
        if res.definition == "No dictionary definition found" || is_placeholder_definition(&res.definition) {
            if let Ok(inexact_res) = Self::lookup_internal(client, word, false, max_senses, max_glosses).await {
                if !is_placeholder_definition(&inexact_res.definition)
                    && inexact_res.definition != "No dictionary definition found"
                {
                    return Ok(inexact_res);
                }
            }
        }

        Ok(res)
    }

    async fn lookup_internal(
        client: &reqwest::Client,
        word: &str,
        exact_only: bool,
        max_senses: usize,
        max_glosses: usize,
    ) -> Result<LookupResult> {
        let url = format!("https://jisho.org/api/v1/search/words?keyword={}", urlencoding::encode(word));
        let resp = client.get(&url).send().await?;

        if resp.status().is_success() {
            let json: serde_json::Value = resp.json().await?;
            if let Some(items) = json["data"].as_array() {
                let exact_entries: Vec<&serde_json::Value> = items.iter().filter(|entry| {
                    entry["japanese"].as_array().map_or(false, |forms| {
                        forms.iter().any(|j| {
                            j["word"].as_str() == Some(word) || j["reading"].as_str() == Some(word)
                        })
                    })
                }).collect();

                if exact_only && exact_entries.is_empty() {
                    return Ok(LookupResult {
                        expression: word.to_string(),
                        reading: word.to_string(),
                        definition: "No dictionary definition found".to_string(),
                        pitch_accent: "LH".to_string(),
                    });
                }

                let candidates = if exact_entries.is_empty() { items.iter().collect() } else { exact_entries };
                let best_entry = candidates
                    .into_iter()
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
                    let reading = data["japanese"].as_array()
                        .and_then(|forms| forms.iter().find(|j| {
                            j["word"].as_str() == Some(word) || j["reading"].as_str() == Some(word)
                        }).or_else(|| forms.first()))
                        .and_then(|j| j["reading"].as_str())
                        .unwrap_or(word)
                        .to_string();

                    let mut defs = Vec::new();
                    if let Some(senses) = data["senses"].as_array() {
                        let mut num = 1;
                        for sense in senses {
                            if num > max_senses {
                                break;
                            }

                            let pos_str = sense["parts_of_speech"]
                                .as_array()
                                .and_then(|a| a.first())
                                .and_then(|v| v.as_str())
                                .unwrap_or("Vocab");

                            if pos_str == "Wikipedia definition" {
                                continue;
                            }

                            if let Some(defs_arr) = sense["english_definitions"].as_array() {
                                let def_list: Vec<&str> = defs_arr
                                    .iter()
                                    .filter_map(|d| d.as_str())
                                    .take(max_glosses)
                                    .collect();
                                if !def_list.is_empty() {
                                    defs.push(format!("{}. [{}] {}", num, pos_str, def_list.join(", ")));
                                    num += 1;
                                }
                            }
                        }
                    }

                    let definition = defs.join("\n│                 ");

                    if definition.is_empty() {
                        return Ok(LookupResult {
                            expression: word.to_string(),
                            reading,
                            definition: "No dictionary definition found".to_string(),
                            pitch_accent: "LH".to_string(),
                        });
                    }

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
            definition: "No dictionary definition found".to_string(),
            pitch_accent: "LH".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_definition_is_detected() {
        assert!(is_placeholder_definition("1. [def] vocabulary word"));
        assert!(!is_placeholder_definition("1. [Noun] Monday"));
    }

    #[tokio::test]
    async fn test_serif_lookup() {
        let client = reqwest::Client::new();
        let res = DictionaryService::lookup(&client, "セリフ").await.unwrap();
        println!("LOOKUP RESULT: {:?}", res);
        assert!(res.definition.contains("speech") || res.definition.contains("lines"));
    }

    #[tokio::test]
    async fn test_sakibashiri_lookup() {
        let client = reqwest::Client::new();
        let res = DictionaryService::lookup(&client, "先走り").await.unwrap();
        println!("SAKIBASHIRI RESULT: {:?}", res);
        assert!(res.definition.contains("rash") || res.definition.contains("act") || res.definition.contains("ahead"));
    }

    #[tokio::test]
    async fn test_definition_limits() {
        let client = reqwest::Client::new();
        let res = DictionaryService::lookup_with_limits(&client, "つまり", 2, 3).await.unwrap();
        println!("LIMITED RESULT:\n{}", res.definition);
        let lines: Vec<&str> = res.definition.lines().collect();
        assert!(lines.len() <= 2);
    }
}
