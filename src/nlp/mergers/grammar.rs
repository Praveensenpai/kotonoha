use crate::nlp::{kata_to_hira, SpannedToken, TokenInfo};

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
