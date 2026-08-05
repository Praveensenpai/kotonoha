use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupResult {
    pub expression: String,
    pub reading: String,
    pub definition: String,
    pub pitch_accent: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextHint {
    AsStated,
}

const AS_STATED_PATTERNS: &[&str] = &[
    "言うとおり", "言う通り", "言ったとおり", "言った通り", "いうとおり", "いう通り",
    "いったとおり", "いった通り", "思うとおり", "思う通り", "思ったとおり", "思った通り",
    "おもうとおり", "おもう通り", "おもったとおり", "おもった通り", "見るとおり",
    "見る通り", "見たとおり", "見た通り", "そのとおり", "その通り", "予定どおり",
    "予定通り", "説明どおり", "説明通り",
];

pub fn context_hint(sentence: &str, target_word: &str) -> Option<ContextHint> {
    if !matches!(target_word, "とおり" | "通り" | "どおり") {
        return None;
    }
    AS_STATED_PATTERNS
        .iter()
        .any(|pattern| sentence.contains(pattern))
        .then_some(ContextHint::AsStated)
}

fn sense_line_score(line: &str, hint: ContextHint) -> i32 {
    let line = line.to_ascii_lowercase();
    match hint {
        ContextHint::AsStated => {
            let positive = [
                "according to",
                "in accordance",
                "just as",
                "exactly as",
                "as ",
                "following",
                "manner",
            ];
            let negative = ["street", "road", "avenue", "thoroughfare", "traffic", "flow of"];
            positive.iter().map(|term| if line.contains(term) { 100 } else { 0 }).sum::<i32>()
                - negative.iter().map(|term| if line.contains(term) { 25 } else { 0 }).sum::<i32>()
        }
    }
}

pub fn format_contextual_definition(
    definition: &str,
    hint: Option<ContextHint>,
    max_senses: usize,
    max_glosses: usize,
) -> String {
    let Some(hint) = hint else {
        return truncate_definition(definition, max_senses, max_glosses);
    };

    let mut lines: Vec<&str> = definition.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    if lines.is_empty() {
        return definition.to_string();
    }

    if lines.iter().any(|line| sense_line_score(line, hint) > 0) {
        lines.sort_by_key(|line| -sense_line_score(line, hint));
    }
    truncate_definition(&lines.join("\n"), max_senses, max_glosses)
}

pub fn has_contextual_sense(definition: &str, hint: ContextHint) -> bool {
    definition.lines().any(|line| sense_line_score(line, hint) >= 100)
}

/// Returns true for the legacy value used when a dictionary lookup failed.
/// This value must never be persisted as if it were a real definition.
/// Splits a raw multi-sense definition into individual sense lines.
pub fn parse_senses(def: &str) -> Vec<String> {
    def.lines()
        .map(|line| {
            let t = line.trim();
            t.strip_prefix('│').unwrap_or(t).trim().to_string()
        })
        .filter(|line| !line.is_empty())
        .collect()
}

pub fn is_placeholder_definition(definition: &str) -> bool {
    definition.trim() == "1. [def] vocabulary word"
}

pub fn truncate_definition(def: &str, max_senses: usize, max_glosses: usize) -> String {
    if def.is_empty() || is_placeholder_definition(def) || def == "No dictionary definition found" {
        return def.to_string();
    }

    let mut new_senses = Vec::new();
    let mut num = 1;

    for line in def.lines() {
        if num > max_senses {
            break;
        }

        let clean = line.trim();
        let clean = clean.strip_prefix('│').unwrap_or(clean).trim();

        if let Some(dot_idx) = clean.find(". [") {
            let rest = &clean[dot_idx + 2..];
            if let Some(close_bracket) = rest.find(']') {
                let pos_part = &rest[..close_bracket + 1];
                let glosses_part = rest[close_bracket + 1..].trim();
                let glosses: Vec<&str> = glosses_part.split(", ").collect();
                let truncated_glosses: Vec<&str> = glosses.into_iter().take(max_glosses).collect();
                new_senses.push(format!("{}. {} {}", num, pos_part, truncated_glosses.join(", ")));
                num += 1;
                continue;
            }
        }

        new_senses.push(clean.to_string());
    }

    if new_senses.is_empty() {
        def.to_string()
    } else {
        new_senses.join("\n│                 ")
    }
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
        let resp = client.get(&url).header("User-Agent", "kotonoha/0.0.1").send().await?;

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
                    .rev()
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
                            let is_only_wiki = senses.iter().all(|s| {
                                s["parts_of_speech"]
                                    .as_array()
                                    .and_then(|a| a.first())
                                    .and_then(|v| v.as_str())
                                    == Some("Wikipedia definition")
                            });
                            if is_only_wiki {
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

    pub fn parse_entry(
        data: &serde_json::Value,
        word: &str,
        max_senses: usize,
        max_glosses: usize,
    ) -> LookupResult {
        let kanji_expr = data["japanese"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v["word"].as_str())
            .unwrap_or(word)
            .to_string();

        let reading = data["japanese"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v["reading"].as_str())
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

        let definition = if defs.is_empty() {
            "No dictionary definition found".to_string()
        } else {
            defs.join("\n│                 ")
        };

        LookupResult {
            expression: kanji_expr,
            reading,
            definition,
            pitch_accent: "LH".to_string(),
        }
    }

    pub async fn lookup_all_candidates(
        client: &reqwest::Client,
        word: &str,
        max_senses: usize,
        max_glosses: usize,
    ) -> Result<Vec<LookupResult>> {
        let url = format!(
            "https://jisho.org/api/v1/search/words?keyword={}",
            urlencoding::encode(word)
        );
        let resp = client.get(&url).header("User-Agent", "kotonoha/0.0.1").send().await?;

        let mut results = Vec::new();
        if resp.status().is_success() {
            let json: serde_json::Value = resp.json().await?;
            if let Some(items) = json["data"].as_array() {
                for item in items {
                    let res = Self::parse_entry(item, word, max_senses, max_glosses);
                    if !is_placeholder_definition(&res.definition)
                        && res.definition != "No dictionary definition found"
                    {
                        results.push(res);
                    }
                }
            }
        }
        Ok(results)
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

    #[test]
    fn test_truncate_definition() {
        let raw = "1. [Adverb] word1, word2, word3, word4, word5\n│                 2. [Adverb] wordA, wordB, wordC, wordD, wordE\n│                 3. [Noun] wordX, wordY, wordZ\n│                 4. [Noun] extra sense";
        let truncated = truncate_definition(raw, 2, 3);
        assert_eq!(
            truncated,
            "1. [Adverb] word1, word2, word3\n│                 2. [Adverb] wordA, wordB, wordC"
        );
    }

    #[test]
    fn detects_as_stated_context() {
        assert_eq!(context_hint("ひまわりの言うとおり、僕は用事があった", "とおり"), Some(ContextHint::AsStated));
        assert_eq!(context_hint("この通りは広い", "通り"), None);
    }

    #[test]
    fn prioritizes_contextual_toori_sense() {
        let raw = "1. [Noun] street, road, avenue\n│                 2. [Noun] traffic, coming and going\n│                 3. [Noun] in accordance with, according to, just as";
        let result = format_contextual_definition(
            raw,
            Some(ContextHint::AsStated),
            3,
            4,
        );
        assert!(result.starts_with("1. [Noun] in accordance with"));
    }

    #[tokio::test]
    async fn test_hen_lookup_first_result() {
        let client = reqwest::Client::new();
        let res = DictionaryService::lookup(&client, "辺").await.unwrap();
        println!("HEN RESULT: {:?}", res);
        assert_eq!(res.reading, "へん");
        assert!(res.definition.contains("area") || res.definition.contains("vicinity") || res.definition.contains("region"));
    }

    #[tokio::test]
    async fn test_ato_lookup_all_candidates() {
        let client = reqwest::Client::new();
        let candidates = DictionaryService::lookup_all_candidates(&client, "あと", 3, 4).await.unwrap();
        println!("ATO CANDIDATES COUNT: {}", candidates.len());
        assert!(candidates.len() >= 2);
        let has_ato_after = candidates.iter().any(|c| c.expression == "後" || c.definition.contains("behind") || c.definition.contains("after"));
        let has_ato_trace = candidates.iter().any(|c| c.expression == "跡" || c.definition.contains("trace"));
        assert!(has_ato_after && has_ato_trace);
    }
}
