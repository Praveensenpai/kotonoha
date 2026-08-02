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
    pub pos: Vec<String>,
    pub is_content_word: bool,
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
            let is_content_word = matches!(pos_category, "名詞" | "動詞" | "形容詞" | "副詞")
                && !matches!(dictionary_form.as_str(), "する" | "いる" | "ある" | "なる" | "の" | "ん" | "よう" | "こと" | "もの");

            tokens.push(TokenInfo {
                surface,
                dictionary_form,
                pos,
                is_content_word,
            });
        }

        Ok(tokens)
    }
}
