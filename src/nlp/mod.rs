pub mod mergers;

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
    pub is_proper_noun: bool,
}

#[derive(Debug, Clone)]
pub struct SpannedToken {
    pub token: TokenInfo,
    pub begin: usize,
    pub end: usize,
}

pub fn kata_to_hira(s: &str) -> String {
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
            let pos: Vec<String> = node
                .part_of_speech()
                .iter()
                .map(|s| s.to_string())
                .collect();

            let pos_category = pos.first().map(|s| s.as_str()).unwrap_or("");
            let pos_sub = pos.get(1).map(|s| s.as_str()).unwrap_or("");
            let pos_type = pos.get(4).map(|s| s.as_str()).unwrap_or("");
            let pos_form = pos.get(5).map(|s| s.as_str()).unwrap_or("");

            let dictionary_form = mergers::normalize_subsidiary_verb_lemma(
                &surface,
                node.dictionary_form(),
                pos_type,
                pos_form,
            );

            let is_formal_noun = matches!(
                dictionary_form.as_str(),
                "こと" | "もの" | "やつ" | "ため" | "ところ" | "わけ" | "はず" | "つもり"
            );

            let is_conjunction_particle = matches!(
                dictionary_form.as_str(),
                "だって"
                    | "だけど"
                    | "だから"
                    | "なのに"
                    | "けれど"
                    | "けれども"
                    | "でも"
                    | "しかし"
                    | "ただし"
                    | "なお"
                    | "ちなみに"
                    | "および"
                    | "ならびに"
            );

            let is_audio_grunt = matches!(
                dictionary_form.as_str(),
                "おっ"
                    | "あっ"
                    | "えっ"
                    | "うっ"
                    | "はっ"
                    | "ふっ"
                    | "んっ"
                    | "くっ"
                    | "ちっ"
                    | "つっ"
                    | "オッ"
                    | "アッ"
                    | "エッ"
                    | "ウッ"
                    | "ハッ"
                    | "フッ"
                    | "ンッ"
                    | "クッ"
                    | "チッ"
            );

            // Filter symbols, interjections, punctuation, particles, numbers, and non-independent auxiliary verbs
            let is_symbol_or_junk = (is_audio_grunt
                || matches!(
                    pos_category,
                    "記号" | "補助記号" | "感動詞" | "助詞" | "助動詞" | "数詞"
                )
                || matches!(pos_sub, "数詞" | "接尾")
                || pos_sub.contains("非自立")
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
                        | "あ"
                        | "え"
                        | "お"
                        | "う"
                        | "い"
                ))
                && !is_formal_noun
                && !is_conjunction_particle;

            let has_japanese_char = dictionary_form.chars().any(|c| {
                matches!(c, '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}' | '\u{4E00}'..='\u{9FFF}')
            });

            // Single hiragana/katakana are always filler (そ, ぞ, ア…) — block them.
            // Single kanji are legitimate content words (仲, 愛, 心) — allow them.
            let is_single_kana = dictionary_form.chars().count() == 1
                && dictionary_form
                    .chars()
                    .all(|c| matches!(c, '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}'));

            let is_content_word = (matches!(
                pos_category,
                "名詞"
                    | "代名詞"
                    | "接頭辞"
                    | "動詞"
                    | "形容詞"
                    | "形状詞"
                    | "副詞"
                    | "連体詞"
                    | "接続詞"
            ) || is_formal_noun
                || is_conjunction_particle)
                && !is_symbol_or_junk
                && has_japanese_char
                && !is_single_kana;

            let is_proper_noun_pos = pos
                .iter()
                .any(|p| p.contains("固有名詞") || p.contains("人名") || p.contains("地名"));
            let is_katakana_noun = pos_category == "名詞"
                && dictionary_form.chars().count() >= 2
                && dictionary_form
                    .chars()
                    .all(|c| matches!(c, '\u{30A0}'..='\u{30FF}'));
            let is_proper_noun = is_content_word && (is_proper_noun_pos || is_katakana_noun);

            let reading = kata_to_hira(node.reading_form());
            let (dictionary_form, reading) =
                mergers::normalize_colloquial_negative(&surface, dictionary_form, reading);

            tokens.push(SpannedToken {
                token: TokenInfo {
                    surface,
                    dictionary_form,
                    reading,
                    is_content_word,
                    is_proper_noun,
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

        mergers::normalize_colloquial_greetings(&mut normalized_tokens);
        mergers::normalize_ambiguous_imperatives(&mut normalized_tokens, text);
        mergers::normalize_explanatory_nan(&mut normalized_tokens, text);
        let normalized_tokens =
            mergers::merge_fixed_expression(normalized_tokens, "よりにもよって");
        let normalized_tokens = mergers::merge_fixed_expression(normalized_tokens, "もしかして");
        let normalized_tokens = mergers::merge_grammar_expressions(normalized_tokens);
        let normalized_tokens = mergers::merge_complex_verb_inflections(normalized_tokens);
        let normalized_tokens = mergers::merge_adverb_naru(normalized_tokens);
        Ok(mergers::merge_colloquial_small_tsu(normalized_tokens))
    }
}

#[cfg(test)]
mod tests;
