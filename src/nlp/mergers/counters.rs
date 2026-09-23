pub mod numerals;

use crate::nlp::{SpannedToken, TokenInfo};
use numerals::*;

const COUNTER_SUFFIXES: &[&str] = &[
    "人",
    "つ",
    "日",
    "時",
    "分",
    "秒",
    "年",
    "月",
    "回",
    "度",
    "円",
    "匹",
    "頭",
    "羽",
    "本",
    "枚",
    "個",
    "歳",
    "才",
    "階",
    "番",
    "話",
    "冊",
    "着",
    "足",
    "杯",
    "台",
    "点",
    "割",
    "倍",
    "段",
    "曲",
    "発",
    "勝",
    "敗",
    "ヶ月",
    "か月",
    "カ月",
    "箇月",
    "キロ",
    "メートル",
    "センチ",
    "ミリ",
    "グラム",
    "リットル",
    "パック",
    "缶",
    "便",
    "組",
    "件",
    "名",
    "箇所",
    "か所",
];

fn merge_numeric_sequence(tokens: Vec<SpannedToken>) -> Vec<SpannedToken> {
    let mut merged: Vec<SpannedToken> = Vec::with_capacity(tokens.len());
    let mut iter = tokens.into_iter().peekable();

    while let Some(mut current) = iter.next() {
        if is_numeric_token(&current.token.surface) {
            while let Some(next) = iter.peek() {
                if next.begin == current.end && is_numeric_token(&next.token.surface) {
                    let Some(next_token) = iter.next() else { break };
                    let surface = format!("{}{}", current.token.surface, next_token.token.surface);
                    let reading = format!("{}{}", current.token.reading, next_token.token.reading);
                    let surface_reading = format!(
                        "{}{}",
                        current.token.surface_reading, next_token.token.surface_reading
                    );
                    current = SpannedToken {
                        token: TokenInfo {
                            surface: surface.clone(),
                            dictionary_form: surface,
                            reading,
                            surface_reading,
                            is_content_word: false,
                            is_proper_noun: false,
                        },
                        begin: current.begin,
                        end: next_token.end,
                    };
                } else {
                    break;
                }
            }
        }
        merged.push(current);
    }
    merged
}

fn build_counter_token(num_token: SpannedToken, counter_token: SpannedToken) -> SpannedToken {
    let surface = format!("{}{}", num_token.token.surface, counter_token.token.surface);
    let begin = num_token.begin;
    let end = counter_token.end;

    let kanji_num = digits_to_kanji(&num_token.token.surface);
    let dictionary_form = format!("{}{}", kanji_num, counter_token.token.surface);
    let is_content = is_lexical_counter_compound(&kanji_num, &counter_token.token.surface);

    let combined_reading =
        if let Some(spec) = derive_special_reading(&kanji_num, &counter_token.token.surface) {
            spec.to_string()
        } else {
            let num_reading =
                derive_numeric_reading(&num_token.token.surface, &num_token.token.reading);
            let counter_reading = if counter_token.token.surface == "人" {
                "にん"
            } else if counter_token.token.surface == "時" {
                "じ"
            } else {
                &counter_token.token.reading
            };
            format!("{num_reading}{counter_reading}")
        };

    SpannedToken {
        token: TokenInfo {
            surface,
            dictionary_form,
            reading: combined_reading.clone(),
            surface_reading: combined_reading,
            is_content_word: is_content,
            is_proper_noun: false,
        },
        begin,
        end,
    }
}

fn attach_bill_suffix(
    combined: SpannedToken,
    iter: &mut std::iter::Peekable<std::vec::IntoIter<SpannedToken>>,
) -> SpannedToken {
    if let Some(after) = iter.peek() {
        let is_bill_suffix = after.begin == combined.end
            && matches!(after.token.surface.as_str(), "札" | "玉" | "生");
        if is_bill_suffix {
            if let Some(suffix_token) = iter.next() {
                let sub_surf = format!("{}{}", combined.token.surface, suffix_token.token.surface);
                let sub_dict = format!(
                    "{}{}",
                    combined.token.dictionary_form, suffix_token.token.surface
                );
                let sub_read = format!("{}{}", combined.token.reading, suffix_token.token.reading);
                return SpannedToken {
                    token: TokenInfo {
                        surface: sub_surf,
                        dictionary_form: sub_dict,
                        reading: sub_read.clone(),
                        surface_reading: sub_read,
                        is_content_word: true,
                        is_proper_noun: false,
                    },
                    begin: combined.begin,
                    end: suffix_token.end,
                };
            }
        }
    }
    combined
}

pub fn merge_number_counters(tokens: Vec<SpannedToken>) -> Vec<SpannedToken> {
    let tokens = merge_numeric_sequence(tokens);
    let mut merged: Vec<SpannedToken> = Vec::with_capacity(tokens.len());
    let mut iter = tokens.into_iter().peekable();

    while let Some(mut current) = iter.next() {
        if current.token.surface == "何時" {
            current.token.reading = "なんじ".to_string();
            current.token.surface_reading = "なんじ".to_string();
            current.token.is_content_word = true;
        } else if current.token.surface == "時" && current.token.is_content_word {
            current.token.reading = "とき".to_string();
            current.token.surface_reading = "とき".to_string();
        }

        if is_numeric_token(&current.token.surface) {
            if let Some(next) = iter.peek() {
                let is_counter = next.begin == current.end
                    && COUNTER_SUFFIXES.contains(&next.token.surface.as_str());
                if is_counter {
                    if let Some(counter_token) = iter.next() {
                        let combined = build_counter_token(current, counter_token);
                        current = attach_bill_suffix(combined, &mut iter);
                    }
                }
            }
        }
        merged.push(current);
    }
    merged
}

#[cfg(test)]
mod tests;
