pub mod dictionary;
pub mod filters;
pub mod mergers;

pub use filters::{is_predicate_lemma, is_predicate_suffix};

use anyhow::Result;
use filters::{is_conjunction_particle, is_formal_noun, is_symbol_or_junk, MorphemeMeta};
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
    pub surface_reading: String,
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

static CHAR_DEF: &str = include_str!("nlp/resources/char.def");
static REWRITE_DEF: &str = include_str!("nlp/resources/rewrite.def");
static UNK_DEF: &str = include_str!("nlp/resources/unk.def");

pub struct JapaneseTokenizer {
    dict: JapaneseDictionary,
}

fn locate_or_migrate_system_dic(
    sudachi_dir: &std::path::Path,
    legacy_config_dir: &std::path::Path,
) -> Result<PathBuf> {
    let dict_path = sudachi_dir.join("system.dic");
    if dict_path.exists() {
        return Ok(dict_path);
    }

    let legacy_dic = legacy_config_dir.join("system.dic");
    if legacy_dic.exists() && std::fs::rename(&legacy_dic, &dict_path).is_ok() {
        return Ok(dict_path);
    }

    dictionary::ensure_system_dict(&dict_path)?;
    Ok(dict_path)
}

fn clean_legacy_config_defs(legacy_config_dir: &std::path::Path) {
    for def in ["char.def", "rewrite.def", "unk.def", "system.dic"] {
        let p = legacy_config_dir.join(def);
        if p.exists() {
            let _ = std::fs::remove_file(p);
        }
    }
}

impl JapaneseTokenizer {
    pub fn new() -> Result<Self> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let data_dir = dirs::data_dir()
            .map(|p| p.join("kotonoha"))
            .unwrap_or_else(|| home.join(".local/share/kotonoha"));
        let sudachi_dir = data_dir.join("sudachi");
        std::fs::create_dir_all(&sudachi_dir)?;

        let legacy_config_dir = dirs::config_dir()
            .map(|p| p.join("kotonoha"))
            .unwrap_or_else(|| home.join(".config/kotonoha"));

        let dict_path = locate_or_migrate_system_dic(&sudachi_dir, &legacy_config_dir)?;
        clean_legacy_config_defs(&legacy_config_dir);

        let char_dst = sudachi_dir.join("char.def");
        if !char_dst.exists() {
            let _ = std::fs::write(&char_dst, CHAR_DEF);
        }

        let rewrite_dst = sudachi_dir.join("rewrite.def");
        if !rewrite_dst.exists() {
            let _ = std::fs::write(&rewrite_dst, REWRITE_DEF);
        }

        let unk_dst = sudachi_dir.join("unk.def");
        if !unk_dst.exists() {
            let _ = std::fs::write(&unk_dst, UNK_DEF);
        }

        let config = Config::minimal_at(&sudachi_dir).with_system_dic(&dict_path);
        let dict = JapaneseDictionary::from_cfg(&config)?;
        Ok(Self { dict })
    }
}

impl JapaneseTokenizer {
    pub fn tokenize(&self, text: &str) -> Result<Vec<TokenInfo>> {
        let tokenizer = StatelessTokenizer::new(&self.dict);
        let raw_tokens = self.extract_morphemes(&tokenizer, text)?;
        let mut final_tokens = self.apply_mergers(raw_tokens, text);
        self.resolve_lemma_readings(&tokenizer, &mut final_tokens);
        Ok(final_tokens)
    }

    fn extract_morphemes(
        &self,
        tokenizer: &StatelessTokenizer<&JapaneseDictionary>,
        text: &str,
    ) -> Result<Vec<SpannedToken>> {
        let morphemes = tokenizer.tokenize(text, Mode::C, false)?;
        let mut prev_is_te_or_de = false;
        let mut tokens = Vec::with_capacity(morphemes.len());

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

            let is_subsidiary_verb =
                pos_category == "動詞" && pos_sub == "非自立可能" && prev_is_te_or_de;

            if pos_category != "空白" {
                prev_is_te_or_de = (pos_category == "助詞"
                    && matches!(surface.as_str(), "て" | "で"))
                    || (pos_category == "動詞"
                        && (surface.ends_with('て') || surface.ends_with('で')));
            }

            let dictionary_form = mergers::normalize_subsidiary_verb_lemma(
                &surface,
                node.dictionary_form(),
                pos_type,
                pos_form,
            );

            let meta = MorphemeMeta {
                pos_category,
                pos_sub,
                dictionary_form: &dictionary_form,
                surface: &surface,
                is_subsidiary_verb,
            };

            let is_symbol_or_junk = is_symbol_or_junk(&meta);
            let has_japanese_char = dictionary_form.chars().any(|c| {
                matches!(
                    c,
                    '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}' | '\u{4E00}'..='\u{9FFF}'
                )
            });
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
            ) || is_formal_noun(&dictionary_form)
                || is_conjunction_particle(&dictionary_form))
                && !is_symbol_or_junk
                && has_japanese_char
                && !is_single_kana;

            let is_proper_noun = is_content_word
                && pos
                    .iter()
                    .any(|p| p.contains("固有名詞") || p.contains("人名") || p.contains("地名"));

            let surface_reading = kata_to_hira(node.reading_form());
            let reading = surface_reading.clone();
            let (dictionary_form, reading) =
                mergers::normalize_colloquial_negative(&surface, dictionary_form, reading);

            tokens.push(SpannedToken {
                token: TokenInfo {
                    surface,
                    dictionary_form,
                    reading,
                    surface_reading,
                    is_content_word,
                    is_proper_noun,
                },
                begin: node.begin(),
                end: node.end(),
            });
        }

        Ok(tokens)
    }

    fn apply_mergers(&self, tokens: Vec<SpannedToken>, text: &str) -> Vec<TokenInfo> {
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
                if let Some(previous) = normalized_tokens.last_mut() {
                    previous.token.dictionary_form = format!("{}ない", previous.token.surface);
                    previous.token.reading = format!("{}ない", previous.token.reading);
                    previous.token.surface_reading =
                        format!("{}ない", previous.token.surface_reading);
                    previous.end = token.end;
                }
            } else {
                normalized_tokens.push(token);
            }
        }

        mergers::normalize_colloquial_greetings(&mut normalized_tokens);
        mergers::normalize_ambiguous_imperatives(&mut normalized_tokens, text);
        mergers::normalize_explanatory_nan(&mut normalized_tokens, text);
        mergers::normalize_explanatory_njanai(&mut normalized_tokens);
        mergers::normalize_kansai_negative(&mut normalized_tokens);
        let normalized_tokens =
            mergers::merge_fixed_expression(normalized_tokens, "よりにもよって");
        let normalized_tokens = mergers::merge_fixed_expression(normalized_tokens, "もしかして");
        let normalized_tokens = mergers::merge_grammar_expressions(normalized_tokens);
        let normalized_tokens = mergers::merge_complex_verb_inflections(normalized_tokens);
        let normalized_tokens = mergers::merge_compound_verbs(normalized_tokens);
        let normalized_tokens = mergers::merge_adverb_naru(normalized_tokens);
        mergers::merge_colloquial_small_tsu(normalized_tokens)
    }

    fn resolve_lemma_readings(
        &self,
        tokenizer: &StatelessTokenizer<&JapaneseDictionary>,
        tokens: &mut [TokenInfo],
    ) {
        for token in tokens {
            if token.is_content_word && token.surface != token.dictionary_form {
                if let Ok(lemma_morphemes) =
                    tokenizer.tokenize(&token.dictionary_form, Mode::C, false)
                {
                    let r: String = lemma_morphemes
                        .iter()
                        .map(|m| kata_to_hira(m.reading_form()))
                        .collect();
                    if !r.is_empty() {
                        token.reading = r;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
