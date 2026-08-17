use super::{kata_to_hira, SpannedToken, TokenInfo};

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

pub fn merge_adverb_naru(tokens: Vec<SpannedToken>) -> Vec<SpannedToken> {
    let adverbs = ["こう", "そう", "ああ", "どう"];
    let mut merged = Vec::with_capacity(tokens.len());

    for token in tokens {
        let should_merge = token.token.dictionary_form == "なる"
            && merged.last().is_some_and(|previous: &SpannedToken| {
                previous.end == token.begin && adverbs.contains(&previous.token.surface.as_str())
            });

        if should_merge {
            let previous = merged.pop().expect("merge candidate exists");
            let dictionary_form = format!("{}なる", previous.token.surface);
            let reading = format!("{}なる", previous.token.reading);
            merged.push(SpannedToken {
                token: TokenInfo {
                    surface: format!("{}{}", previous.token.surface, token.token.surface),
                    dictionary_form,
                    reading,
                    is_content_word: true,
                    is_proper_noun: false,
                },
                begin: previous.begin,
                end: token.end,
            });
        } else {
            merged.push(token);
        }
    }

    merged
}

pub fn merge_fixed_expression(tokens: Vec<SpannedToken>, expression: &str) -> Vec<SpannedToken> {
    let mut merged = Vec::with_capacity(tokens.len());
    let mut index = 0;

    while index < tokens.len() {
        let mut surface = String::new();
        let mut end_index = index;

        while end_index < tokens.len() {
            let token = &tokens[end_index];
            if end_index > index && tokens[end_index - 1].end != token.begin {
                break;
            }
            surface.push_str(&token.token.surface);
            if surface.chars().count() >= expression.chars().count() {
                break;
            }
            end_index += 1;
        }

        let is_exact_match = surface == expression;
        let is_prefix_match = surface.chars().count() >= expression.chars().count()
            && surface.starts_with(expression);

        if is_exact_match || is_prefix_match {
            let first = &tokens[index];
            let last = &tokens[end_index];
            let expression_end = if is_exact_match {
                last.end
            } else {
                let preceding_chars: usize = tokens[index..end_index]
                    .iter()
                    .map(|token| token.token.surface.chars().count())
                    .sum();
                let chars_in_last = expression.chars().count() - preceding_chars;
                let split_bytes = last
                    .token
                    .surface
                    .char_indices()
                    .nth(chars_in_last)
                    .map(|(offset, _)| offset)
                    .unwrap_or_else(|| last.token.surface.len());
                last.begin + split_bytes
            };
            merged.push(SpannedToken {
                token: TokenInfo {
                    surface: expression.to_string(),
                    dictionary_form: expression.to_string(),
                    reading: expression.to_string(),
                    is_content_word: true,
                    is_proper_noun: false,
                },
                begin: first.begin,
                end: expression_end,
            });

            if is_prefix_match && !is_exact_match {
                let split_offset = expression_end - last.begin;
                let remainder = &last.token.surface[split_offset..];
                if !remainder.is_empty() {
                    merged.push(SpannedToken {
                        token: TokenInfo {
                            surface: remainder.to_string(),
                            dictionary_form: remainder.to_string(),
                            reading: kata_to_hira(remainder),
                            is_content_word: false,
                            is_proper_noun: false,
                        },
                        begin: expression_end,
                        end: last.end,
                    });
                }
            }
            index = end_index + 1;
        } else {
            merged.push(tokens[index].clone());
            index += 1;
        }
    }

    merged
}

pub fn merge_grammar_expressions(mut tokens: Vec<SpannedToken>) -> Vec<SpannedToken> {
    const GRAMMAR_PATTERNS: &[&str] = &[
        "だって",
        "だけど",
        "だから",
        "なのに",
        "けれども",
        "ですが",
        "だけで",
        "について",
        "についての",
        "につきまして",
        "に関して",
        "にかんして",
        "に関する",
        "によって",
        "により",
        "による",
        "によっては",
        "において",
        "における",
        "にあたって",
        "にあたり",
        "にわたって",
        "にわたり",
        "にわたる",
        "をはじめ",
        "をはじめとする",
        "を通じて",
        "をつうじて",
        "を通して",
        "をとおして",
        "に基づいて",
        "にもとづいて",
        "に基づく",
        "とともに",
        "と共に",
        "にしては",
        "に反して",
        "にはんして",
        "を込めて",
        "をこめて",
        "にかかわらず",
        "に関わらず",
        "に先立って",
        "にさきだって",
        "をもとに",
        "を基に",
        "をきっかけに",
        "を契機に",
        "にかける",
        "にかけては",
        "に答えて",
        "に応えて",
        "に沿って",
        "にそって",
        "に即して",
        "にそくして",
        "わけにはいかない",
        "わけにはいかぬ",
        "わけにはいかん",
        "わけがない",
        "わけもない",
        "ざるを得ない",
        "ざるをえない",
        "に違いない",
        "にちがいない",
        "そうになる",
        "っこない",
        "かねない",
        "そうにない",
        "そうもない",
        "にすぎない",
        "に過ぎない",
        "にほかならない",
        "に他ならない",
        "かねる",
        "てはいけない",
        "ではいけない",
        "じゃいけない",
        "っちゃいけない",
        "なきゃいけない",
        "なくちゃいけない",
        "なければならない",
        "なくてはならない",
        "に決まっている",
        "にきまっている",
        "よりほかはない",
        "よりほかない",
        "てしょうがない",
        "てたまらない",
        "てしかたない",
    ];

    for &expr in GRAMMAR_PATTERNS {
        tokens = merge_fixed_expression(tokens, expr);
    }
    tokens
}

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
