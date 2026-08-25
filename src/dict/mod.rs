pub mod offline;

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
    "言うとおり",
    "言う通り",
    "言ったとおり",
    "言った通り",
    "いうとおり",
    "いう通り",
    "いったとおり",
    "いった通り",
    "思うとおり",
    "思う通り",
    "思ったとおり",
    "思った通り",
    "おもうとおり",
    "おもう通り",
    "おもったとおり",
    "おもった通り",
    "見るとおり",
    "見る通り",
    "見たとおり",
    "見た通り",
    "そのとおり",
    "その通り",
    "予定どおり",
    "予定通り",
    "説明どおり",
    "説明通り",
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
            let negative = [
                "street",
                "road",
                "avenue",
                "thoroughfare",
                "traffic",
                "flow of",
            ];
            positive
                .iter()
                .map(|term| if line.contains(term) { 100 } else { 0 })
                .sum::<i32>()
                - negative
                    .iter()
                    .map(|term| if line.contains(term) { 25 } else { 0 })
                    .sum::<i32>()
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

    let mut lines: Vec<&str> = definition
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.is_empty() {
        return definition.to_string();
    }

    if lines.iter().any(|line| sense_line_score(line, hint) > 0) {
        lines.sort_by_key(|line| -sense_line_score(line, hint));
    }
    truncate_definition(&lines.join("\n"), max_senses, max_glosses)
}

pub fn has_contextual_sense(definition: &str, hint: ContextHint) -> bool {
    definition
        .lines()
        .any(|line| sense_line_score(line, hint) >= 100)
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
                new_senses.push(format!(
                    "{}. {} {}",
                    num,
                    pos_part,
                    truncated_glosses.join(", ")
                ));
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
    let small_kana = [
        'ゃ', 'ゅ', 'ょ', 'ぁ', 'ぃ', 'ぅ', 'ぇ', 'ぉ', 'ャ', 'ュ', 'ョ', 'ァ', 'ィ', 'ゥ', 'ェ',
        'ォ',
    ];
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
        return (
            reading.to_string(),
            format!("[{}] H (0 morae)", pitch_num),
            0,
        );
    }

    let mut pattern = vec![0; total_morae];
    let k = pitch_num;

    if k == 1 {
        pattern[0] = 1;
    } else if k == 0 || k >= total_morae {
        for val in pattern.iter_mut().take(total_morae).skip(1) {
            *val = 1;
        }
    } else {
        for val in pattern.iter_mut().take(k.min(total_morae)).skip(1) {
            *val = 1;
        }
    }

    let mut hl_str = String::new();

    for (i, _) in morae.iter().enumerate() {
        let is_high = pattern[i] == 1;
        if is_high {
            hl_str.push('H');
        } else {
            hl_str.push('L');
        }
    }

    (
        reading.to_string(),
        format!("[{}] {} ({} morae)", k, hl_str, total_morae),
        total_morae,
    )
}

#[derive(Debug, Clone, Copy)]
pub struct LookupLimits {
    pub max_senses: usize,
    pub max_glosses: usize,
}

pub struct DictionaryService;

impl DictionaryService {
    #[cfg(test)]
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

        let grammar_direct_fallbacks = [
            ("だけで", "だけ"),
            ("についての", "について"),
            ("につきまして", "について"),
            ("に関する", "に関して"),
            ("にかんして", "に関して"),
            ("による", "によって"),
            ("により", "によって"),
            ("によっては", "によって"),
            ("における", "において"),
            ("にあたり", "にあたって"),
            ("にわたり", "にわたって"),
            ("にわたる", "にわたって"),
            ("をはじめとする", "をはじめ"),
            ("をつうじて", "を通じて"),
            ("を通して", "を通じて"),
            ("をとおして", "を通じて"),
            ("にもとづいて", "に基づいて"),
            ("に基づく", "に基づいて"),
            ("と共に", "とともに"),
            ("にはんして", "に反して"),
            ("をこめて", "を込めて"),
            ("に関わらず", "にかかわらず"),
            ("にさきだって", "に先立って"),
            ("を基に", "をもとに"),
            ("を契機に", "をきっかけに"),
            ("にかけては", "にかける"),
            ("に応えて", "に答えて"),
            ("にそって", "に沿って"),
            ("にそくして", "に即して"),
        ];

        for (target, fallback_word) in grammar_direct_fallbacks {
            if word == target {
                if let Ok(fallback_res) =
                    Self::lookup_internal(client, fallback_word, true, max_senses, max_glosses)
                        .await
                {
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

        let complex_fallbacks = [
            ("させられる", "る"),
            ("せられる", "る"),
            ("さされる", "す"),
            ("わされる", "う"),
            ("らされる", "る"),
            ("させられ", "る"),
            ("ちゃった", "つ"),
            ("ちゃう", "つ"),
            ("じゃった", "ぐ"),
            ("じゃう", "ぐ"),
            ("てしまう", "つ"),
            ("でしまう", "ぐ"),
            ("ざるを得ない", "う"),
            ("ざるをえない", "う"),
            ("わけにはいかない", ""),
            ("わけにはいかぬ", ""),
        ];

        for (suffix, verb_end) in complex_fallbacks {
            if let Some(stem) = word.strip_suffix(suffix) {
                let verb_form = format!("{}{}", stem, verb_end);
                if let Ok(fallback_res) =
                    Self::lookup_internal(client, &verb_form, true, max_senses, max_glosses).await
                {
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
            if let Some(stem) = word.strip_suffix(stem_end) {
                let verb_form = format!("{}{}", stem, verb_end);
                if let Ok(fallback_res) =
                    Self::lookup_internal(client, &verb_form, true, max_senses, max_glosses).await
                {
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
        if res.definition == "No dictionary definition found"
            || is_placeholder_definition(&res.definition)
        {
            if let Ok(inexact_res) =
                Self::lookup_internal(client, word, false, max_senses, max_glosses).await
            {
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
        _client: &reqwest::Client,
        word: &str,
        exact_only: bool,
        _max_senses: usize,
        _max_glosses: usize,
    ) -> Result<LookupResult> {
        let cfg = crate::config::AppConfig::load().ok();
        if let Some(c) = cfg {
            if let Ok(db) = crate::db::Database::open(&c.db_path) {
                if let Ok(offline) = db.query_offline_terms(word, exact_only) {
                    if let Some(first) = offline.into_iter().next() {
                        return Ok(first);
                    }
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
        let word_hira = crate::nlp::kata_to_hira(word);
        let matching_form = data["japanese"].as_array().and_then(|forms| {
            forms.iter().find(|j| {
                let w_str = j["word"].as_str().unwrap_or("");
                let r_str = j["reading"].as_str().unwrap_or("");
                w_str == word || r_str == word || crate::nlp::kata_to_hira(r_str) == word_hira
            })
        });

        let kanji_expr = matching_form
            .and_then(|v| v["word"].as_str())
            .or_else(|| {
                data["japanese"]
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(|v| v["word"].as_str())
            })
            .unwrap_or(word)
            .to_string();

        let reading = matching_form
            .and_then(|v| v["reading"].as_str())
            .or_else(|| {
                data["japanese"]
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(|v| v["reading"].as_str())
            })
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
        let resp = client
            .get(&url)
            .header("User-Agent", "kotonoha/0.0.1")
            .send()
            .await?;

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

        results.sort_by_key(|res| {
            let is_exact = res.expression == word || res.reading == word;
            (
                !is_exact,
                (res.expression.chars().count() as i32 - word.chars().count() as i32).abs(),
            )
        });

        Ok(results)
    }

    pub async fn ensure_offline_dictionaries_ready(
        client: &reqwest::Client,
        db: &mut crate::db::Database,
    ) -> Result<()> {
        offline::ensure_offline_dictionaries_ready(client, db).await
    }

    pub async fn lookup_all_candidates_cached(
        client: &reqwest::Client,
        db: Option<&crate::db::Database>,
        word: &str,
        limits: LookupLimits,
    ) -> Result<Vec<LookupResult>> {
        if let Some(db_inst) = db {
            if let Ok(offline) = db_inst.query_offline_terms(word, false) {
                if !offline.is_empty() {
                    return Ok(offline);
                }
            }
            if let Ok(Some(cached)) = db_inst.get_cached_candidates(word) {
                if !cached.is_empty() {
                    return Ok(cached);
                }
            }
        }

        let results =
            Self::lookup_all_candidates(client, word, limits.max_senses, limits.max_glosses)
                .await?;
        if let Some(db_inst) = db {
            if !results.is_empty() {
                let _ = db_inst.cache_candidates(word, &results);
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests;
