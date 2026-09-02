use crate::nlp::{kata_to_hira, SpannedToken, TokenInfo};

pub fn normalize_colloquial_negative(
    surface: &str,
    dictionary_form: String,
    reading: String,
) -> (String, String) {
    let Some(stem) = ["ねえ", "ねぇ", "ねー"]
        .iter()
        .find_map(|suffix| surface.strip_suffix(suffix).filter(|stem| !stem.is_empty()))
    else {
        return (dictionary_form, reading);
    };

    if !dictionary_form.ends_with('る') {
        return (dictionary_form, reading);
    }

    let normalized_dictionary_form = format!("{stem}ない");
    let normalized_reading = if let Some(prefix) = reading.strip_suffix('る') {
        format!("{prefix}ない")
    } else if let Some(prefix) = reading
        .strip_suffix("ねえ")
        .or_else(|| reading.strip_suffix("ねぇ"))
        .or_else(|| reading.strip_suffix("ねー"))
    {
        format!("{prefix}ない")
    } else {
        reading
    };

    (normalized_dictionary_form, normalized_reading)
}

fn is_kana(c: char) -> bool {
    matches!(c, '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}')
}

fn is_kana_word(surface: &str) -> bool {
    !surface.is_empty() && surface.chars().all(is_kana)
}

fn ends_in_small_tsu(surface: &str) -> bool {
    surface.ends_with(['っ', 'ッ'])
}

pub fn merge_colloquial_small_tsu(tokens: Vec<SpannedToken>) -> Vec<TokenInfo> {
    let mut merged: Vec<SpannedToken> = Vec::with_capacity(tokens.len());
    let mut tokens = tokens.into_iter().peekable();

    while let Some(mut token) = tokens.next() {
        let has_kana_continuation = tokens
            .peek()
            .is_some_and(|next| next.begin == token.end && is_kana_word(&next.token.surface));

        if ends_in_small_tsu(&token.token.surface)
            && is_kana_word(&token.token.surface)
            && !has_kana_continuation
        {
            while let Some(previous) = merged.last() {
                if previous.end != token.begin || !is_kana_word(&previous.token.surface) {
                    break;
                }

                let previous = merged.pop().expect("merge candidate exists");
                let surface = format!("{}{}", previous.token.surface, token.token.surface);
                token = SpannedToken {
                    token: TokenInfo {
                        dictionary_form: surface.clone(),
                        reading: kata_to_hira(&surface),
                        surface,
                        is_content_word: previous.token.is_content_word
                            || token.token.is_content_word,
                        is_proper_noun: previous.token.is_proper_noun || token.token.is_proper_noun,
                    },
                    begin: previous.begin,
                    end: token.end,
                };
            }
        }

        merged.push(token);
    }

    merged.into_iter().map(|token| token.token).collect()
}

pub fn normalize_explanatory_nan(tokens: &mut [SpannedToken], text: &str) {
    for token in tokens {
        if token.token.surface != "なん" {
            continue;
        }

        let before = text.get(..token.begin).unwrap_or_default().trim_end();
        let after = text.get(token.end..).unwrap_or_default().trim_start();
        let follows_explanatory_connector = before.ends_with("から") || before.ends_with('の');
        let starts_copula = after.starts_with('だ') || after.starts_with("です");

        if follows_explanatory_connector && starts_copula {
            token.token.is_content_word = false;
        }
    }
}

pub fn normalize_colloquial_greetings(tokens: &mut Vec<SpannedToken>) {
    let mut i = 0;
    while i < tokens.len() {
        let is_ok_prefix = tokens[i].token.surface == "おっ" || tokens[i].token.surface == "お";
        if is_ok_prefix && i + 1 < tokens.len() {
            let next_surf = &tokens[i + 1].token.surface;
            if next_surf.starts_with("はよ") || next_surf.starts_with("はよう") {
                let combined_surface = format!("{}{}", tokens[i].token.surface, next_surf);
                tokens[i].token.surface = combined_surface;
                tokens[i].token.dictionary_form = "おはよう".to_string();
                tokens[i].token.reading = "おはよう".to_string();
                tokens[i].token.is_content_word = true;
                tokens[i].end = tokens[i + 1].end;
                tokens.remove(i + 1);
            } else if next_surf == "は"
                && i + 2 < tokens.len()
                && tokens[i + 2].token.surface.starts_with('よ')
            {
                let combined_surface = format!(
                    "{}{}{}",
                    tokens[i].token.surface,
                    tokens[i + 1].token.surface,
                    tokens[i + 2].token.surface
                );
                tokens[i].token.surface = combined_surface;
                tokens[i].token.dictionary_form = "おはよう".to_string();
                tokens[i].token.reading = "おはよう".to_string();
                tokens[i].token.is_content_word = true;
                tokens[i].end = tokens[i + 2].end;
                tokens.remove(i + 2);
                tokens.remove(i + 1);
            } else if next_surf == "す" && tokens[i].token.surface == "おっ" {
                tokens[i].token.surface = "おっす".to_string();
                tokens[i].token.dictionary_form = "おっす".to_string();
                tokens[i].token.reading = "おっす".to_string();
                tokens[i].token.is_content_word = false;
                tokens[i].end = tokens[i + 1].end;
                tokens.remove(i + 1);
            }
        }
        i += 1;
    }
}
