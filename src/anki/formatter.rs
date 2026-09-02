use crate::dict;
use crate::nlp::JapaneseTokenizer;

pub fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn anki_search_text(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn format_definition_for_anki(def: &str) -> String {
    // Take the first non-empty sense line only
    let first_line = def
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            trimmed.strip_prefix('│').unwrap_or(trimmed).trim()
        })
        .find(|line| !line.is_empty())
        .unwrap_or(def.trim());

    // Strip leading "N. " numbering
    let after_num = first_line
        .find(". ")
        .map(|i| first_line[i + 2..].trim())
        .unwrap_or(first_line);

    // Strip leading "[Grammar Tag] " part
    if after_num.starts_with('[') {
        if let Some(close) = after_num.find(']') {
            return after_num[close + 1..].trim().to_string();
        }
    }

    after_num.to_string()
}

pub fn sentence_with_furigana(
    tokenizer: &JapaneseTokenizer,
    sentence: &str,
    target_word: &str,
) -> String {
    tokenizer
        .tokenize(sentence)
        .map(|tokens| {
            tokens
                .into_iter()
                .map(|token| {
                    let surface = escape_html(&token.surface);
                    let is_target =
                        token.surface == target_word || token.dictionary_form == target_word;
                    let display = if token
                        .surface
                        .chars()
                        .any(|c| matches!(c, '\u{4E00}'..='\u{9FFF}'))
                        && !token.reading.is_empty()
                    {
                        format!(
                            "<ruby>{surface}<rt>{}</rt></ruby>",
                            escape_html(&token.reading)
                        )
                    } else {
                        surface
                    };
                    if is_target {
                        format!("<b>{display}</b>")
                    } else {
                        display
                    }
                })
                .collect()
        })
        .unwrap_or_else(|_| escape_html(sentence))
}

pub fn to_katakana(text: &str) -> String {
    text.chars()
        .map(|character| match character {
            '\u{3041}'..='\u{3096}' => {
                std::char::from_u32(character as u32 + 0x60).unwrap_or(character)
            }
            _ => character,
        })
        .collect()
}

pub fn pitch_number(pitch_accent: &str, mora_count: usize) -> usize {
    if let Ok(number) = pitch_accent.trim().parse::<usize>() {
        return number.min(mora_count);
    }
    let pattern = pitch_accent.trim().to_ascii_uppercase();
    if pattern.starts_with('H') {
        1
    } else {
        pattern
            .chars()
            .position(|level| level == 'L')
            .filter(|drop| *drop > 1)
            .unwrap_or_default()
    }
}

pub fn pitch_pattern(reading: &str, pitch_accent: &str) -> (String, String) {
    let morae = dict::split_morae(&to_katakana(reading));
    let pitch = pitch_number(pitch_accent, morae.len());
    let levels: Vec<bool> = (0..morae.len())
        .map(|index| {
            if pitch == 1 {
                index == 0
            } else if pitch == 0 {
                index > 0
            } else {
                index > 0 && index < pitch
            }
        })
        .collect();
    let pattern = morae
        .iter()
        .enumerate()
        .map(|(index, mora)| {
            let current = levels[index];
            let previous = index
                .checked_sub(1)
                .and_then(|previous| levels.get(previous))
                .copied();
            let next = levels.get(index + 1).copied();
            let shadow = match (previous, current, next) {
                (_, false, Some(true)) => "inset -2px -2px 0 0 #3366CC",
                (Some(true), true, Some(false)) => "inset -2px 2px 0 0 #3366CC",
                (_, true, _) => "inset 0 2px 0 0 #3366CC",
                _ => "inset 0 -2px 0 0 #3366CC",
            };
            format!("<span style=\"box-shadow: {shadow};\">{mora}</span>")
        })
        .collect::<String>();
    let notation: String = levels
        .iter()
        .map(|is_high| if *is_high { 'H' } else { 'L' })
        .collect();
    (
        format!(
            "{pattern} <span class=\"pitch_number\">{pitch}</span> <span class=\"pitch_pattern_text\">[{notation}]</span>"
        ),
        pitch.to_string(),
    )
}
