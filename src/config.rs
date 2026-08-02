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
}

impl Default for AppConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let config_dir = dirs::config_dir()
            .map(|p| p.join("kotonoha"))
            .unwrap_or_else(|| home.join(".config/kotonoha"));

        let media_dir = home.join(".local/share/kotonoha/media");

        Self {
            default_card_limit: 25,
            media_dir,
            db_path: config_dir.join("kotonoha.db"),
            audio_padding_secs: 0.25,
            enable_anki_sync: true,
            anki_connect_url: "http://127.0.0.1:8765".to_string(),
            anki_deck_name: "Japanese::Mining".to_string(),
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
            let cfg: AppConfig = toml::from_str(&content).unwrap_or_default();
            Ok(cfg)
        } else {
            let cfg = AppConfig::default();
            let content = toml::to_string_pretty(&cfg)?;
            std::fs::write(&config_file, content)?;
            Ok(cfg)
        }
    }
}
