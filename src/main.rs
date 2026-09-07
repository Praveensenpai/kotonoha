mod ai;
mod anki;
mod bundle;
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

fn print_service_status(cfg: &AppConfig, anki_connected: bool, unsynced_count: usize) {
    if anki_connected {
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

    if unsynced_count > 0 {
        println!(
            " {}  {} unsynced card{} in database. Please run {} so old media can be cleaned up.",
            style("⚠").yellow().bold(),
            style(unsynced_count).yellow().bold(),
            if unsynced_count == 1 { "" } else { "s" },
            style("kotonoha --sync").cyan()
        );
    }
}

async fn clean_cache_if_needed(cfg: &AppConfig, db: &Database) {
    if cfg.max_cached_cards == 0 {
        return;
    }
    let protected = db.get_unsynced_media_paths().await.unwrap_or_default();
    if let Ok(cleaned) =
        media::MediaExtractor::clean_old_media(&cfg.media_dir, cfg.max_cached_cards, &protected)
    {
        if cleaned > 0 {
            println!(" 🧹 Auto-cleaned {} old cached media file(s).", cleaned);
        }
    }
}

fn load_and_pair_inputs(input_paths: &[PathBuf]) -> Result<Vec<srt::SubtitleSentence>> {
    let mut all_sentences = Vec::new();
    let mut paired_count = 0;

    for input_path in input_paths {
        let (sub_path, vid_path) = match commands::find_paired_media(input_path) {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!(
                    " {} Skipping {}: {}",
                    style("⚠").yellow(),
                    input_path.display(),
                    e
                );
                continue;
            }
        };

        match parse_subtitle(&sub_path) {
            Ok(mut sentences) => {
                for s in &mut sentences {
                    s.video_path = Some(vid_path.clone());
                }
                println!(
                    " ✔ [{}] Parsed {} lines (paired with {})",
                    style(sub_path.file_name().and_then(|n| n.to_str()).unwrap_or("")).cyan(),
                    sentences.len(),
                    style(vid_path.file_name().and_then(|n| n.to_str()).unwrap_or("")).green()
                );
                all_sentences.extend(sentences);
                paired_count += 1;
            }
            Err(e) => {
                eprintln!(
                    " {} Failed to parse subtitle {}: {}",
                    style("⚠").yellow(),
                    sub_path.display(),
                    e
                );
            }
        }
    }

    if all_sentences.is_empty() {
        anyhow::bail!("No valid subtitle lines found from selected file(s).");
    }

    if paired_count > 1 {
        println!(
            "\n ℹ Total: {} files, {} subtitle lines combined.\n",
            style(paired_count).cyan().bold(),
            style(all_sentences.len()).green().bold()
        );
    }

    Ok(all_sentences)
}

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

    let anki_connected = anki::anki_connected(&cfg.anki.connect_url).await;
    let unsynced = db.get_unsynced_mined_cards().await.unwrap_or_default();
    print_service_status(&cfg, anki_connected, unsynced.len());

    clean_cache_if_needed(&cfg, &db).await;
    println!();

    let _ = DictionaryService::ensure_offline_dictionaries_ready(&http_client, &mut db).await;

    let input_paths = {
        let args: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
        if args.is_empty() {
            TerminalUi::select_media_files()?
        } else {
            args
        }
    };

    let sentences = load_and_pair_inputs(&input_paths)?;
    session::run_session(sentences, &cfg, db, http_client).await
}
