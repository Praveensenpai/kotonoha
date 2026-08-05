use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub default_card_limit: usize,
    pub media_dir: PathBuf,
    pub db_path: PathBuf,
    pub audio_padding_secs: f64,
    pub enable_anki_sync: bool,
    pub anki_connect_url: String,
    pub anki_deck_name: String,
    #[serde(default = "default_anki_model_name")]
    pub anki_model_name: String,
    #[serde(default = "default_max_definition_senses")]
    pub max_definition_senses: usize,
    #[serde(default = "default_max_glosses_per_sense")]
    pub max_glosses_per_sense: usize,
    #[serde(default = "default_enable_ai")]
    pub enable_ai: bool,
    #[serde(default)]
    pub gemini_api_key: Option<String>,
    #[serde(default = "default_gemini_model")]
    pub gemini_model: String,
}

fn default_anki_model_name() -> String {
    "Japanese sentences+".to_string()
}

fn default_max_definition_senses() -> usize {
    3
}

fn default_max_glosses_per_sense() -> usize {
    4
}

fn default_enable_ai() -> bool {
    true
}

fn default_gemini_model() -> String {
    "gemini-3.5-flash-lite".to_string()
}

/// Expands a leading `~/` using the home directory of the user running Kotonoha.
/// This keeps paths in `config.toml` portable across user accounts.
fn expand_home_path(path: PathBuf, home: &std::path::Path) -> PathBuf {
    match path.to_str() {
        Some("~") => home.to_path_buf(),
        Some(value) => value
            .strip_prefix("~/")
            .map(|suffix| home.join(suffix))
            .unwrap_or(path),
        None => path,
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let config_dir = dirs::config_dir()
            .map(|p| p.join("kotonoha"))
            .unwrap_or_else(|| home.join(".config/kotonoha"));

        let media_dir = home.join(".local/share/kotonoha/media");

        let api_key = std::env::var("GEMINI_API_KEY").ok();

        Self {
            default_card_limit: 25,
            media_dir,
            db_path: config_dir.join("kotonoha.db"),
            audio_padding_secs: 0.25,
            enable_anki_sync: true,
            anki_connect_url: "http://127.0.0.1:8765".to_string(),
            anki_deck_name: "Anime Mining T1".to_string(),
            anki_model_name: default_anki_model_name(),
            max_definition_senses: 3,
            max_glosses_per_sense: 4,
            enable_ai: true,
            gemini_api_key: api_key,
            gemini_model: default_gemini_model(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let config_dir = dirs::config_dir()
            .map(|p| p.join("kotonoha"))
            .unwrap_or_else(|| home.join(".config/kotonoha"));

        std::fs::create_dir_all(&config_dir)?;
        let config_file = config_dir.join("config.toml");

        if config_file.exists() {
            let content = std::fs::read_to_string(&config_file)?;
            let mut cfg: AppConfig = toml::from_str(&content).unwrap_or_default();
            cfg.media_dir = expand_home_path(cfg.media_dir, &home);
            cfg.db_path = expand_home_path(cfg.db_path, &home);
            if cfg.gemini_api_key.is_none() {
                cfg.gemini_api_key = std::env::var("GEMINI_API_KEY").ok();
            }
            Ok(cfg)
        } else {
            let cfg = AppConfig::default();
            cfg.save()?;
            Ok(cfg)
        }
    }

    pub fn save(&self) -> Result<()> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let config_dir = dirs::config_dir()
            .map(|p| p.join("kotonoha"))
            .unwrap_or_else(|| home.join(".config/kotonoha"));

        std::fs::create_dir_all(&config_dir)?;
        let config_file = config_dir.join("config.toml");

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_file, content)?;
        Ok(())
    }
}
