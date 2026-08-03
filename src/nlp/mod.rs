use anyhow::Result;
use sudachi::analysis::stateless_tokenizer::StatelessTokenizer;
use sudachi::analysis::Tokenize;
use sudachi::analysis::Mode;
use sudachi::config::Config;
use sudachi::dic::dictionary::JapaneseDictionary;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub surface: String,
    pub dictionary_form: String,
    pub reading: String,
    pub is_content_word: bool,
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

        let res_dir = PathBuf::from("/home/paisen/.cargo/git/checkouts/sudachi.rs-f754f73973769f6e/f4dd8f2/resources");
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
            let pos: Vec<String> = node.part_of_speech().iter().map(|s| s.to_string()).collect();

            let pos_category = pos.first().map(|s| s.as_str()).unwrap_or("");
            let pos_sub = pos.get(1).map(|s| s.as_str()).unwrap_or("");

            // Filter symbols, interjections, punctuation, particles, numbers
            let is_symbol_or_junk = matches!(pos_category, "記号" | "補助記号" | "感動詞" | "助詞" | "助動詞" | "数詞")
                || matches!(pos_sub, "数詞" | "非自立" | "接尾")
                || matches!(dictionary_form.as_str(), "…" | "？" | "！" | "♪" | "―" | "ー" | "、" | "。" | "～" | "する" | "いる" | "ある" | "なる" | "の" | "ん" | "よう" | "こと" | "もの" | "あ" | "え" | "お" | "う" | "い");

            let has_japanese_char = dictionary_form.chars().any(|c| {
                matches!(c, '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}' | '\u{4E00}'..='\u{9FFF}')
            });

            // Single hiragana/katakana are always filler (そ, ぞ, ア…) — block them.
            // Single kanji are legitimate content words (仲, 愛, 心) — allow them.
            let is_single_kana = dictionary_form.chars().count() == 1
                && dictionary_form.chars().all(|c| {
                    matches!(c, '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}')
                });

            let is_content_word = matches!(pos_category, "名詞" | "代名詞" | "接頭辞" | "動詞" | "形容詞" | "形状詞" | "副詞" | "連体詞")
                && !is_symbol_or_junk
                && has_japanese_char
                && !is_single_kana;

            let reading = kata_to_hira(node.reading_form());
            let (dictionary_form, reading) =
                normalize_colloquial_negative(&surface, dictionary_form, reading);

            tokens.push(TokenInfo {
                surface,
                dictionary_form,
                reading,
                is_content_word,
            });
        }

        Ok(tokens)
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
}
