use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

use super::MediaExtractor;

static RE_BRACKETS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[[^\]]*\]").unwrap());
static RE_PARENS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\([^)]*\)").unwrap());
static RE_SEPARATORS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[._]+").unwrap());
static RE_SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static RE_EP_DASH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s*-\s*(\d{1,4})(?:v\d+)?\b").unwrap());
static RE_EP_SEASON: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:S\d+)?E(\d{1,4})\b").unwrap());
static RE_EP_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bEP?(\d{1,4})\b").unwrap());
static RE_MEDIA_TAGS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(1080p|720p|480p|2160p|4k|x264|x265|hevc|avc|aac|flac|web-dl|bluray|bdrip|remux)\b")
        .unwrap()
});

/// Cleans and extracts a user- and AI-friendly show and episode title from a video path.
///
/// Strips release group tags, resolution codes, bracketed hashes, and standardizes
/// episode numbering (e.g. `[SubsPlease] Sousou no Frieren - 08 (1080p).mkv` -> `Sousou no Frieren - Episode 08`).
pub fn extract_clean_show_context(video_path: &Path) -> String {
    let raw_stem = MediaExtractor::media_source_stem(video_path);
    if raw_stem.trim().is_empty() || raw_stem == "media" {
        return "Unknown Show".to_string();
    }

    let without_brackets = RE_BRACKETS.replace_all(&raw_stem, " ");
    let without_parens = RE_PARENS.replace_all(&without_brackets, " ");
    let without_tags = RE_MEDIA_TAGS.replace_all(&without_parens, " ");
    let spaced = RE_SEPARATORS.replace_all(&without_tags, " ");
    let normalized = RE_SPACES.replace_all(spaced.trim(), " ").to_string();

    if let Some(caps) = RE_EP_DASH.captures(&normalized) {
        let ep = &caps[1];
        let ep_idx = caps.get(0).map(|m| m.start()).unwrap_or(normalized.len());
        let show = normalized[..ep_idx].trim();
        if !show.is_empty() {
            return format!("{} - Episode {}", show, ep);
        }
    }

    if let Some(caps) = RE_EP_SEASON.captures(&normalized) {
        let ep = &caps[1];
        let ep_idx = caps.get(0).map(|m| m.start()).unwrap_or(normalized.len());
        let show = normalized[..ep_idx].trim();
        if !show.is_empty() {
            return format!("{} - Episode {}", show, ep);
        }
    }

    if let Some(caps) = RE_EP_PREFIX.captures(&normalized) {
        let ep = &caps[1];
        let ep_idx = caps.get(0).map(|m| m.start()).unwrap_or(normalized.len());
        let show = normalized[..ep_idx].trim();
        if !show.is_empty() {
            return format!("{} - Episode {}", show, ep);
        }
    }

    if normalized.is_empty() {
        raw_stem
    } else {
        normalized
    }
}
