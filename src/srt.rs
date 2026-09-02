use anyhow::{Context, Result};
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

static RE_HTML: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());
static RE_ASS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{[^}]*\}").unwrap());
static RE_FULL_PARENS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"（[^）]*）").unwrap());
static RE_HALF_PARENS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\([^)]*\)").unwrap());
static RE_SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static RE_SRT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?s)(\d+)\s*\n(\d{2}:\d{2}:\d{2}[,\.]\d{3})\s*-->\s*(\d{2}:\d{2}:\d{2}[,\.]\d{3})\s*\n(.*?)(?:\n\r?\n|\z)",
    )
    .unwrap()
});
static RE_ASS_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)Dialogue:\s*\d+,\s*(\d{1,2}:\d{2}:\d{2}[\.\,]\d{2,3}),\s*(\d{1,2}:\d{2}:\d{2}[\.\,]\d{2,3}),[^,]*,\s*[^,]*,\s*[^,]*,\s*[^,]*,\s*[^,]*,\s*[^,]*,\s*(.*)",
    )
    .unwrap()
});

#[derive(Debug, Clone)]
pub struct SubtitleSentence {
    pub index: usize,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

pub fn parse_subtitle(path: &Path) -> Result<Vec<SubtitleSentence>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read subtitle file at {}", path.display()))?;

    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext == "ass" || ext == "ssa" {
        parse_ass(&content)
    } else {
        parse_srt(&content)
    }
}

fn clean_text(text: &str) -> String {
    let cleaned = RE_HTML.replace_all(text, "");
    let cleaned = RE_ASS.replace_all(&cleaned, "");
    let cleaned = RE_FULL_PARENS.replace_all(&cleaned, "");
    let cleaned = RE_HALF_PARENS.replace_all(&cleaned, "");
    let flattened = cleaned
        .replace("\\N", " ")
        .replace("\\n", " ")
        .replace("\r\n", " ")
        .replace(['\n', '\r'], " ");

    RE_SPACES.replace_all(flattened.trim(), " ").to_string()
}

fn parse_time_srt(s: &str) -> Option<u64> {
    let s = s.replace(',', ".");
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: u64 = parts[0].trim().parse().ok()?;
    let m: u64 = parts[1].trim().parse().ok()?;
    let secs: f64 = parts[2].trim().parse().ok()?;
    Some((h * 3600 + m * 60) * 1000 + (secs * 1000.0) as u64)
}

fn parse_srt(content: &str) -> Result<Vec<SubtitleSentence>> {
    let mut result = Vec::new();

    for cap in RE_SRT.captures_iter(content) {
        let index: usize = cap[1].parse().unwrap_or(0);
        let start_str = &cap[2];
        let end_str = &cap[3];
        let raw_text = &cap[4];

        let text = clean_text(raw_text);
        if text.is_empty() {
            continue;
        }

        if let (Some(start_ms), Some(end_ms)) = (parse_time_srt(start_str), parse_time_srt(end_str))
        {
            result.push(SubtitleSentence {
                index,
                start_ms,
                end_ms,
                text,
            });
        }
    }
    Ok(result)
}

fn parse_ass(content: &str) -> Result<Vec<SubtitleSentence>> {
    let mut result = Vec::new();
    let mut idx = 1;

    for line in content.lines() {
        if let Some(cap) = RE_ASS_LINE.captures(line) {
            let start_str = &cap[1];
            let end_str = &cap[2];
            let raw_text = &cap[3];

            let text = clean_text(raw_text);
            if text.is_empty() {
                continue;
            }

            if let (Some(start_ms), Some(end_ms)) =
                (parse_time_srt(start_str), parse_time_srt(end_str))
            {
                result.push(SubtitleSentence {
                    index: idx,
                    start_ms,
                    end_ms,
                    text,
                });
                idx += 1;
            }
        }
    }
    Ok(result)
}
