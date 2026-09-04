use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnkiSettings {
    #[serde(default = "default_enable_anki_sync")]
    pub enable_sync: bool,
    #[serde(default = "default_anki_connect_url")]
    pub connect_url: String,
    #[serde(default = "default_anki_deck_name")]
    pub deck_name: String,
    #[serde(default = "default_anki_model_name")]
    pub model_name: String,
}

fn default_enable_anki_sync() -> bool {
    true
}
fn default_anki_connect_url() -> String {
    "http://127.0.0.1:8765".to_string()
}
fn default_anki_deck_name() -> String {
    "日本語::Mining".to_string()
}
fn default_anki_model_name() -> String {
    "Japanese sentences+".to_string()
}

impl Default for AnkiSettings {
    fn default() -> Self {
        Self {
            enable_sync: default_enable_anki_sync(),
            connect_url: default_anki_connect_url(),
            deck_name: default_anki_deck_name(),
            model_name: default_anki_model_name(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettings {
    #[serde(default = "default_enable_ai")]
    pub enable_ai: bool,
    #[serde(default)]
    pub gemini_api_key: Option<String>,
    #[serde(default = "default_gemini_model")]
    pub gemini_model: String,
    #[serde(default = "default_ai_batch_size")]
    pub ai_batch_size: usize,
    #[serde(default = "default_ai_cache_ttl_minutes")]
    pub ai_cache_ttl_minutes: usize,
}

fn default_enable_ai() -> bool {
    true
}
fn default_gemini_model() -> String {
    "gemini-3.5-flash-lite".to_string()
}
fn default_ai_batch_size() -> usize {
    10
}
fn default_ai_cache_ttl_minutes() -> usize {
    30
}

impl AiSettings {
    pub fn has_valid_api_key(&self) -> bool {
        match self.gemini_api_key.as_deref() {
            Some(key) => {
                let trimmed = key.trim();
                !trimmed.is_empty()
                    && trimmed != "YOUR_GEMINI_API_KEY_HERE"
                    && trimmed != "your_api_key_here"
            }
            None => false,
        }
    }
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            enable_ai: default_enable_ai(),
            gemini_api_key: std::env::var("GEMINI_API_KEY").ok(),
            gemini_model: default_gemini_model(),
            ai_batch_size: default_ai_batch_size(),
            ai_cache_ttl_minutes: default_ai_cache_ttl_minutes(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionarySettings {
    #[serde(default = "default_max_definition_senses")]
    pub max_definition_senses: usize,
    #[serde(default = "default_max_glosses_per_sense")]
    pub max_glosses_per_sense: usize,
}

fn default_max_definition_senses() -> usize {
    3
}
fn default_max_glosses_per_sense() -> usize {
    4
}

impl Default for DictionarySettings {
    fn default() -> Self {
        Self {
            max_definition_senses: default_max_definition_senses(),
            max_glosses_per_sense: default_max_glosses_per_sense(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleStorageStrategy {
    Colocated,
    Central,
    Subfolder,
}

fn default_bundle_storage() -> BundleStorageStrategy {
    BundleStorageStrategy::Colocated
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_card_limit")]
    pub default_card_limit: usize,
    #[serde(default = "default_max_cached_cards")]
    pub max_cached_cards: usize,
    pub media_dir: PathBuf,
    pub db_path: PathBuf,
    #[serde(default = "default_bundle_storage")]
    pub bundle_storage: BundleStorageStrategy,
    #[serde(default = "default_bundles_dir")]
    pub bundles_dir: PathBuf,
    #[serde(default = "default_audio_padding_secs")]
    pub audio_padding_secs: f64,
    #[serde(default)]
    pub anki: AnkiSettings,
    #[serde(default)]
    pub ai: AiSettings,
    #[serde(default)]
    pub dict: DictionarySettings,
}

fn default_bundles_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".local/share/kotonoha/bundles")
}

fn default_card_limit() -> usize {
    25
}
fn default_max_cached_cards() -> usize {
    50
}
fn default_audio_padding_secs() -> f64 {
    0.25
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

        Self {
            default_card_limit: default_card_limit(),
            max_cached_cards: default_max_cached_cards(),
            media_dir,
            db_path: config_dir.join("kotonoha.db"),
            bundle_storage: default_bundle_storage(),
            bundles_dir: default_bundles_dir(),
            audio_padding_secs: default_audio_padding_secs(),
            anki: AnkiSettings::default(),
            ai: AiSettings::default(),
            dict: DictionarySettings::default(),
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
            cfg.bundles_dir = expand_home_path(cfg.bundles_dir, &home);
            if cfg.ai.gemini_api_key.is_none() {
                cfg.ai.gemini_api_key = std::env::var("GEMINI_API_KEY").ok();
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
