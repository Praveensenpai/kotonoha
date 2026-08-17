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
    let mut db = Database::open(&cfg.db_path)?;
    let http_client = reqwest::Client::new();

    // AnkiConnect status
    if anki::anki_connected(&cfg.anki_connect_url).await {
        println!(
            " {}  Anki connected (Deck: {})",
            style("✔").green().bold(),
            style(&cfg.anki_deck_name).cyan().bold()
        );
    } else {
        println!(
            " {}  Anki not connected — cards will be saved locally.\n    Use {} to push them to Anki later.",
            style("⚠").yellow().bold(),
            style("kotonoha --sync").cyan()
        );
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
