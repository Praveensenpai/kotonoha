use anyhow::Result;
use std::path::PathBuf;
use sudachi::analysis::stateless_tokenizer::StatelessTokenizer;
use sudachi::analysis::Mode;
use sudachi::analysis::Tokenize;
use sudachi::config::Config;
use sudachi::dic::dictionary::JapaneseDictionary;

#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub surface: String,
    pub dictionary_form: String,
    pub reading: String,
    pub is_content_word: bool,
}

#[derive(Debug, Clone)]
struct SpannedToken {
    token: TokenInfo,
    begin: usize,
    end: usize,
}

fn kata_to_hira(s: &str) -> String {
    s.chars()
        .map(|c| {
            if matches!(c, '\u{30A1}'..='\u{30F6}') {
                std::char::from_u32(c as u32 - 0x60).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

fn normalize_colloquial_negative(
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

    // Limit the correction to the common Sudachi misparse where the form is
    // incorrectly classified as a godan/ichidan verb ending in る. This keeps
    // slang adjectives such as すげえ from being rewritten as すげない.
    if !dictionary_form.ends_with('る') {
        return (dictionary_form, reading);
    }

    // Sudachi can analyse rough negative forms such as いけねえ as the
    // unrelated verb いける. In this construction, ねえ/ねぇ/ねー is the
    // colloquial equivalent of ない.
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

/// Sudachi can prefer a dictionary entry inside colloquial, clipped words.
/// For example, it may split キモッ so that モッ is analysed as the unrelated
/// noun もっこく. Merge only adjacent kana fragments ending in small ッ/っ;
/// this preserves ordinary boundaries while keeping slang as one lookup word.
fn merge_colloquial_small_tsu(tokens: Vec<SpannedToken>) -> Vec<TokenInfo> {
    let mut merged: Vec<SpannedToken> = Vec::with_capacity(tokens.len());
    let mut tokens = tokens.into_iter().peekable();

    while let Some(mut token) = tokens.next() {
        let has_kana_continuation = tokens.peek().is_some_and(|next| {
            next.begin == token.end && is_kana_word(&next.token.surface)
        });

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

fn merge_adverb_naru(tokens: Vec<SpannedToken>) -> Vec<SpannedToken> {
    let adverbs = ["こう", "そう", "ああ", "どう"];
    let mut merged = Vec::with_capacity(tokens.len());

    for token in tokens {
        let should_merge = token.token.dictionary_form == "なる"
            && merged.last().is_some_and(|previous: &SpannedToken| {
                previous.end == token.begin
                    && adverbs.contains(&previous.token.surface.as_str())
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

fn merge_fixed_expression(tokens: Vec<SpannedToken>, expression: &str) -> Vec<SpannedToken> {
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

        if surface == expression {
            let first = &tokens[index];
            let last = &tokens[end_index];
            merged.push(SpannedToken {
                token: TokenInfo {
                    surface: expression.to_string(),
                    dictionary_form: expression.to_string(),
                    reading: expression.to_string(),
                    is_content_word: true,
                },
                begin: first.begin,
                end: last.end,
            });
            index = end_index + 1;
        } else {
            merged.push(tokens[index].clone());
            index += 1;
        }
    }

    merged
}

fn is_imperative_following_text(text: &str) -> bool {
    let following = text.trim_start();
    following.is_empty()
        || following.starts_with(['よ', 'ね', 'ぞ', 'ぜ', 'な', '！', '!', '？', '?', '。', '…', '♪'])
}

fn imperative_reading_stem(reading: &str) -> Option<&str> {
    ["れよ", "れね", "れぞ", "れぜ", "れな", "れ！", "れ!", "れ？", "れ?", "れ。", "れ…"]
        .iter()
        .find_map(|suffix| reading.strip_suffix(suffix))
        .or_else(|| reading.strip_suffix('れ'))
}

/// Sudachi can resolve an imperative such as 頑張れ as the ambiguous
/// potential-form lemma 頑張れる. Sentence-final context strongly favors the
/// godan imperative reading, so prefer the corresponding る-form there.
fn normalize_ambiguous_imperatives(tokens: &mut [SpannedToken], text: &str) {
    for token in tokens {
        if imperative_reading_stem(&token.token.reading).is_none()
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

fn normalize_explanatory_nan(tokens: &mut [SpannedToken], text: &str) {
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

pub struct JapaneseTokenizer {
    dict: JapaneseDictionary,
}

impl JapaneseTokenizer {
    pub fn new() -> Result<Self> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let kotonoha_dir = home.join(".config/kotonoha");
        std::fs::create_dir_all(&kotonoha_dir)?;

        let dict_path = kotonoha_dir.join("system.dic");
        if !dict_path.exists() {
            let uv_dic = PathBuf::from("/home/paisen/.cache/uv/archive-v0/xaAwtzGbATmrfCxj/sudachidict_core/resources/system.dic");
            if uv_dic.exists() {
                let _ = std::fs::copy(uv_dic, &dict_path);
            }
        }

        let res_dir = PathBuf::from(
            "/home/paisen/.cargo/git/checkouts/sudachi.rs-f754f73973769f6e/f4dd8f2/resources",
        );
        for def_file in &["char.def", "rewrite.def", "unk.def"] {
            let dst = kotonoha_dir.join(def_file);
            if !dst.exists() {
                let src = res_dir.join(def_file);
                if src.exists() {
                    let _ = std::fs::copy(src, dst);
                }
            }
        }

        let config = Config::minimal_at(&kotonoha_dir).with_system_dic(&dict_path);
        let dict = JapaneseDictionary::from_cfg(&config)?;
        Ok(Self { dict })
    }

    pub fn tokenize(&self, text: &str) -> Result<Vec<TokenInfo>> {
        let tokenizer = StatelessTokenizer::new(&self.dict);
        let morphemes = tokenizer.tokenize(text, Mode::C, false)?;

        let mut tokens = Vec::new();
        for node in morphemes.iter() {
            let surface = node.surface().to_string();
            let dictionary_form = node.dictionary_form().to_string();
            let pos: Vec<String> = node
                .part_of_speech()
                .iter()
                .map(|s| s.to_string())
                .collect();

            let pos_category = pos.first().map(|s| s.as_str()).unwrap_or("");
            let pos_sub = pos.get(1).map(|s| s.as_str()).unwrap_or("");

            // Filter symbols, interjections, punctuation, particles, numbers
            let is_symbol_or_junk = matches!(
                pos_category,
                "記号" | "補助記号" | "感動詞" | "助詞" | "助動詞" | "数詞"
            ) || matches!(pos_sub, "数詞" | "非自立" | "接尾")
                || matches!(
                    dictionary_form.as_str(),
                    "…" | "？"
                        | "！"
                        | "♪"
                        | "―"
                        | "ー"
                        | "、"
                        | "。"
                        | "～"
                        | "する"
                        | "いる"
                        | "ある"
                        | "なる"
                        | "の"
                        | "ん"
                        | "よう"
                        | "こと"
                        | "もの"
                        | "あ"
                        | "え"
                        | "お"
                        | "う"
                        | "い"
                );

            let has_japanese_char = dictionary_form.chars().any(|c| {
                matches!(c, '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}' | '\u{4E00}'..='\u{9FFF}')
            });

            // Single hiragana/katakana are always filler (そ, ぞ, ア…) — block them.
            // Single kanji are legitimate content words (仲, 愛, 心) — allow them.
            let is_single_kana = dictionary_form.chars().count() == 1
                && dictionary_form
                    .chars()
                    .all(|c| matches!(c, '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}'));

            let is_content_word = matches!(
                pos_category,
                "名詞" | "代名詞" | "接頭辞" | "動詞" | "形容詞" | "形状詞" | "副詞" | "連体詞"
            ) && !is_symbol_or_junk
                && has_japanese_char
                && !is_single_kana;

            let reading = kata_to_hira(node.reading_form());
            let (dictionary_form, reading) =
                normalize_colloquial_negative(&surface, dictionary_form, reading);

            tokens.push(SpannedToken {
                token: TokenInfo {
                    surface,
                    dictionary_form,
                    reading,
                    is_content_word,
                },
                begin: node.begin(),
                end: node.end(),
            });
        }

        let mut normalized_tokens = Vec::with_capacity(tokens.len());
        for token in tokens {
            let is_rough_negative_suffix =
                matches!(token.token.surface.as_str(), "ねえ" | "ねぇ" | "ねー")
                    && !token.token.is_content_word;
            if is_rough_negative_suffix
                && normalized_tokens
                    .last()
                    .is_some_and(|previous: &SpannedToken| {
                        previous.token.dictionary_form.ends_with('る')
                    })
            {
                let previous = normalized_tokens.last_mut().expect("previous token exists");
                previous.token.dictionary_form = format!("{}ない", previous.token.surface);
                previous.token.reading = format!("{}ない", previous.token.reading);
                previous.end = token.end;
            } else {
                normalized_tokens.push(token);
            }
        }

        normalize_ambiguous_imperatives(&mut normalized_tokens, text);
        normalize_explanatory_nan(&mut normalized_tokens, text);
        let normalized_tokens = merge_fixed_expression(normalized_tokens, "よりにもよって");
        let normalized_tokens = merge_adverb_naru(normalized_tokens);
        Ok(merge_colloquial_small_tsu(normalized_tokens))
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_colloquial_negative;

    #[test]
    fn normalizes_rough_negative_form() {
        assert_eq!(
            normalize_colloquial_negative("いけねえ", "いける".into(), "いける".into()),
            ("いけない".into(), "いけない".into())
        );
    }

    #[test]
    fn leaves_non_negative_forms_unchanged() {
        assert_eq!(
            normalize_colloquial_negative("ねえ", "ねえ".into(), "ねえ".into()),
            ("ねえ".into(), "ねえ".into())
        );
    }

    #[test]
    fn normalizes_split_rough_negative_sentence() {
        let tokenizer = super::JapaneseTokenizer::new().unwrap();
        let tokens = tokenizer.tokenize("バカ言っちゃいけねえよ！").unwrap();
        let token = tokens.iter().find(|token| token.surface == "いけ").unwrap();
        assert_eq!(token.dictionary_form, "いけない");
        assert_eq!(token.reading, "いけない");
    }

    #[test]
    fn keeps_colloquial_small_tsu_words_together() {
        let tokenizer = super::JapaneseTokenizer::new().unwrap();
        let tokens = tokenizer.tokenize("うわ キモッ むっ…").unwrap();
        let token = tokens
            .iter()
            .find(|token| token.surface == "キモッ")
            .unwrap();

        assert_eq!(token.dictionary_form, "キモッ");
        assert_eq!(token.reading, "きもっ");
        assert!(!tokens.iter().any(|token| token.dictionary_form == "モッ"));
    }

    #[test]
    fn does_not_merge_small_tsu_across_whitespace() {
        let tokenizer = super::JapaneseTokenizer::new().unwrap();
        let tokens = tokenizer.tokenize("キモ ッ").unwrap();

        assert!(!tokens.iter().any(|token| token.surface == "キモッ"));
    }

    #[test]
    fn leaves_regular_words_unchanged() {
        let tokenizer = super::JapaneseTokenizer::new().unwrap();
        let tokens = tokenizer.tokenize("ササササ サンちゃん").unwrap();
        let surfaces: String = tokens.iter().map(|token| token.surface.as_str()).collect();

        assert_eq!(surfaces, "ササササ サンちゃん");
        assert!(!tokens.iter().any(|token| token.dictionary_form == "キモッ"));
    }

    #[test]
    fn resolves_sentence_final_imperative() {
        let tokenizer = super::JapaneseTokenizer::new().unwrap();
        let tokens = tokenizer.tokenize("ジョーロも生徒会 頑張れよ").unwrap();
        let token = tokens.iter().find(|token| token.surface == "頑張れよ").unwrap();

        assert_eq!(token.dictionary_form, "頑張る");
        assert_eq!(token.reading, "がんばる");
    }

    #[test]
    fn preserves_non_imperative_potential_form() {
        let tokenizer = super::JapaneseTokenizer::new().unwrap();
        let tokens = tokenizer.tokenize("彼は頑張れる").unwrap();
        let token = tokens.iter().find(|token| token.surface == "頑張れる").unwrap();

        assert_eq!(token.dictionary_form, "頑張れる");
    }

    #[test]
    fn keeps_adverb_naru_phrase_together() {
        let tokenizer = super::JapaneseTokenizer::new().unwrap();
        let tokens = tokenizer.tokenize("何がどうして こうなった!? ").unwrap();
        let token = tokens.iter().find(|token| token.dictionary_form == "こうなる").unwrap();

        assert_eq!(token.surface, "こうなっ");
        assert_eq!(token.reading, "こうなる");
        assert!(!tokens.iter().any(|token| token.dictionary_form == "こうなっ"));
    }

    #[test]
    fn keeps_yori_ni_mo_yotte_expression_together() {
        let tokenizer = super::JapaneseTokenizer::new().unwrap();
        let tokens = tokenizer.tokenize("よりにもよって こいつだけ！").unwrap();
        let token = tokens
            .iter()
            .find(|token| token.dictionary_form == "よりにもよって")
            .unwrap();

        assert_eq!(token.surface, "よりにもよって");
        assert_eq!(token.reading, "よりにもよって");
        assert!(!tokens.iter().any(|token| token.dictionary_form == "よりにもよっ"));
    }

    #[test]
    fn filters_explanatory_nan() {
        let tokenizer = super::JapaneseTokenizer::new().unwrap();
        let tokens = tokenizer
            .tokenize("君に伝えたいことが あるからなんだ")
            .unwrap();
        let token = tokens.iter().find(|token| token.surface == "なん").unwrap();

        assert!(!token.is_content_word);
    }
}
