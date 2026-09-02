use crate::nlp::{SpannedToken, TokenInfo};

pub fn merge_complex_verb_inflections(tokens: Vec<SpannedToken>) -> Vec<SpannedToken> {
    let aux_morphemes = [
        "させる",
        "させ",
        "られる",
        "られ",
        "れる",
        "れ",
        "さす",
        "さし",
        "わす",
        "わし",
        "ちゃう",
        "ちゃっ",
        "ちゃ",
        "じゃう",
        "じゃっ",
        "じゃ",
        "てしまう",
        "てしまっ",
        "でしまう",
        "でしまっ",
        "ようとする",
        "おうとする",
    ];

    let mut merged = Vec::with_capacity(tokens.len());
    let mut tokens_iter = tokens.into_iter().peekable();

    while let Some(mut current) = tokens_iter.next() {
        if current.token.is_content_word {
            let mut merged_any = false;
            while let Some(next) = tokens_iter.peek() {
                if next.begin == current.end {
                    let next_surf = next.token.surface.as_str();
                    let next_dict = next.token.dictionary_form.as_str();
                    if aux_morphemes.contains(&next_surf) || aux_morphemes.contains(&next_dict) {
                        let next_token = tokens_iter.next().unwrap();
                        let surface =
                            format!("{}{}", current.token.surface, next_token.token.surface);
                        let dictionary_form = surface.clone();
                        let reading =
                            format!("{}{}", current.token.reading, next_token.token.reading);
                        current = SpannedToken {
                            token: TokenInfo {
                                surface,
                                dictionary_form,
                                reading,
                                is_content_word: true,
                                is_proper_noun: false,
                            },
                            begin: current.begin,
                            end: next_token.end,
                        };
                        merged_any = true;
                        continue;
                    }
                }
                break;
            }
            if merged_any
                && (current.token.dictionary_form.ends_with("られ")
                    || current.token.dictionary_form.ends_with("させ"))
            {
                current.token.dictionary_form.push('る');
            }
        }
        merged.push(current);
    }

    merged
}

pub fn normalize_subsidiary_verb_lemma(
    surface: &str,
    raw_dict: &str,
    pos_type: &str,
    _pos_form: &str,
) -> String {
    if surface.starts_with("くれ")
        && (raw_dict == "くる" || raw_dict == "くれる" || raw_dict == "くれ")
    {
        return "くれる".to_string();
    }
    if surface.starts_with("あげ") && (raw_dict == "あげる" || raw_dict == "あげ") {
        return "あげる".to_string();
    }
    if surface.starts_with("もら") && (raw_dict == "もらう" || raw_dict == "もら") {
        return "もらう".to_string();
    }
    if surface == "み" && (raw_dict == "みる" || raw_dict == "み") {
        return "みる".to_string();
    }
    if (surface == "おき" || surface == "おく") && raw_dict == "おく" {
        return "おく".to_string();
    }
    if surface.starts_with("しま") && raw_dict == "しまう" {
        return "しまう".to_string();
    }
    if (surface == "いっ" || surface == "いき") && (raw_dict == "いく" || raw_dict == "行きます")
    {
        return "いく".to_string();
    }

    if (pos_type.contains("下一段") || pos_type.contains("上一段")) && !raw_dict.ends_with('る')
    {
        return format!("{}る", raw_dict);
    }

    raw_dict.to_string()
}

fn is_imperative_following_text(text: &str) -> bool {
    let following = text.trim_start();
    following.is_empty()
        || following.starts_with([
            'よ', 'ね', 'ぞ', 'ぜ', 'な', '！', '!', '？', '?', '。', '…', '♪',
        ])
}

fn imperative_reading_stem(reading: &str) -> Option<&str> {
    [
        "れよ", "れね", "れぞ", "れぜ", "れな", "れ！", "れ!", "れ？", "れ?", "れ。", "れ…",
    ]
    .iter()
    .find_map(|suffix| reading.strip_suffix(suffix))
    .or_else(|| reading.strip_suffix('れ'))
}

pub fn normalize_ambiguous_imperatives(tokens: &mut [SpannedToken], text: &str) {
    for token in tokens {
        if token.token.dictionary_form == "くれる"
            || imperative_reading_stem(&token.token.reading).is_none()
            || !token.token.dictionary_form.ends_with("れる")
            || !is_imperative_following_text(text.get(token.end..).unwrap_or_default())
        {
            continue;
        }

        if let Some(stem) = token.token.dictionary_form.strip_suffix("れる") {
            token.token.dictionary_form = format!("{stem}る");
        }
        if let Some(reading_stem) = imperative_reading_stem(&token.token.reading) {
            token.token.reading = format!("{reading_stem}る");
        }
    }
}
