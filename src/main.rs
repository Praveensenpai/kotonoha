mod ai;
mod anki;
mod commands;
mod config;
mod db;
mod dict;
mod media;
mod miner;
mod nlp;
mod session;
mod srt;
mod ui;

use anyhow::Result;
use config::AppConfig;
use console::style;
use db::Database;
use dict::DictionaryService;
use srt::parse_subtitle;
use std::path::PathBuf;
use ui::TerminalUi;

#[tokio::main]
async fn main() -> Result<()> {
    if let Some(arg) = std::env::args().nth(1) {
        if commands::handle_cli_flag(&arg).await? {
            return Ok(());
        }
    }

    TerminalUi::print_banner();

    let cfg = AppConfig::load()?;
    let mut db = Database::open(&cfg.db_path).await?;
    let http_client = reqwest::Client::new();

    // AnkiConnect status
    if anki::anki_connected(&cfg.anki.connect_url).await {
        println!(
            " {}  Anki connected (Deck: {})",
            style("✔").green().bold(),
            style(&cfg.anki.deck_name).cyan().bold()
        );
    } else {
        println!(
            " {}  Anki not connected — cards will be saved locally.\n    Use {} to push them to Anki later.",
            style("⚠").yellow().bold(),
            style("kotonoha --sync").cyan()
        );
    }

    // Gemini AI status
    if cfg.ai.enable_ai {
        if cfg.ai.has_valid_api_key() {
            println!(
                " {}  Gemini AI ready (Model: {})",
                style("✔").green().bold(),
                style(&cfg.ai.gemini_model).cyan().bold()
            );
        } else {
            println!(
                " {}  Gemini API key not set — AI disambiguation & translations disabled.\n    Set {} env var or run {} to configure.",
                style("⚠").yellow().bold(),
                style("GEMINI_API_KEY").yellow().bold(),
                style("kotonoha --config").cyan()
            );
        }
    }

    let unsynced = db.get_unsynced_mined_cards().await.unwrap_or_default();
    if !unsynced.is_empty() {
        println!(
            " {}  {} unsynced card{} in database. Please run {} so old media can be cleaned up.",
            style("⚠").yellow().bold(),
            style(unsynced.len()).yellow().bold(),
            if unsynced.len() == 1 { "" } else { "s" },
            style("kotonoha --sync").cyan()
        );
    }

    if cfg.max_cached_cards > 0 {
        let protected = db.get_unsynced_media_paths().await.unwrap_or_default();
        if let Ok(cleaned) =
            media::MediaExtractor::clean_old_media(&cfg.media_dir, cfg.max_cached_cards, &protected)
        {
            if cleaned > 0 {
                println!(" 🧹 Auto-cleaned {} old cached media file(s).", cleaned);
            }
        }
    }
    println!();

    let _ = DictionaryService::ensure_offline_dictionaries_ready(&http_client, &mut db).await;

    let input_path = match std::env::args().nth(1) {
        Some(arg) => PathBuf::from(arg),
        None => TerminalUi::select_media_file()?,
    };

    let (subtitle_path, video_path) = match commands::find_paired_media(&input_path) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("\n {} {}", style("❌ Error:").red().bold(), e);
            std::process::exit(1);
        }
    };

    println!(
        " ℹ Subtitle File: {}",
        style(subtitle_path.display()).cyan()
    );
    println!(
        " ℹ Video File:    {} ({})",
        style(video_path.display()).cyan(),
        style("✔ Video paired").green().bold()
    );

    let sentences = parse_subtitle(&subtitle_path)?;
    println!(" ✔ Parsed {} subtitle lines", sentences.len());

    session::run_session(sentences, &video_path, &cfg, db, http_client).await
}
