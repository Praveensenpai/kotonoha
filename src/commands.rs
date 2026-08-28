use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::anki;
use crate::config::AppConfig;
use crate::db::Database;
use crate::nlp::JapaneseTokenizer;
use crate::srt::parse_subtitle;
use crate::ui::TerminalUi;

pub fn words_with_readings(
    tokenizer: &JapaneseTokenizer,
    words: Vec<String>,
) -> Vec<(String, String)> {
    words
        .into_iter()
        .map(|word| {
            let reading = tokenizer
                .tokenize(&word)
                .ok()
                .and_then(|tokens| {
                    tokens
                        .iter()
                        .find(|token| token.dictionary_form == word)
                        .or_else(|| (tokens.len() == 1).then(|| &tokens[0]))
                        .map(|token| token.reading.clone())
                })
                .unwrap_or_else(|| word.clone());
            (word, reading)
        })
        .collect()
}

pub fn find_paired_media(input_path: &Path) -> Result<(PathBuf, PathBuf)> {
    let parent = input_path.parent().unwrap_or_else(|| Path::new("."));
    let ext = input_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    let is_sub = matches!(ext.as_str(), "srt" | "ass" | "vtt");
    let is_vid = matches!(ext.as_str(), "mkv" | "mp4" | "webm" | "avi");

    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let clean_stem = stem
        .trim_end_matches(".ja")
        .trim_end_matches(".jp")
        .trim_end_matches(".ja-JP")
        .trim_end_matches(".japanese")
        .trim_end_matches(".en");

    if is_sub {
        let sub_path = input_path.to_path_buf();
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                let p = entry.path();
                let p_ext = p
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if matches!(p_ext.as_str(), "mkv" | "mp4" | "webm" | "avi") {
                    let p_stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    if p_stem == stem
                        || p_stem == clean_stem
                        || stem.starts_with(p_stem)
                        || p_stem.starts_with(clean_stem)
                    {
                        return Ok((sub_path, p));
                    }
                }
            }
        }
        anyhow::bail!(
            "No matching video file (.mkv, .mp4) found for subtitle: {}\n   Place the video file in the same folder to mine cards.",
            input_path.display()
        );
    } else if is_vid {
        let vid_path = input_path.to_path_buf();
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                let p = entry.path();
                let p_ext = p
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if matches!(p_ext.as_str(), "srt" | "ass" | "vtt") {
                    let p_stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    let p_clean = p_stem
                        .trim_end_matches(".ja")
                        .trim_end_matches(".jp")
                        .trim_end_matches(".ja-JP")
                        .trim_end_matches(".japanese");
                    if p_stem == stem
                        || p_clean == stem
                        || p_stem.starts_with(stem)
                        || stem.starts_with(p_clean)
                    {
                        return Ok((p, vid_path));
                    }
                }
            }
        }
        anyhow::bail!(
            "No matching Japanese subtitle file (.srt, .ass) found for video: {}\n   Place the subtitle file in the same folder to mine cards.\n\n   Need to generate an .srt subtitle? Try SubSink:\n   https://github.com/Praveensenpai/subsink",
            input_path.display()
        );
    } else {
        anyhow::bail!("Unsupported file format: {}", input_path.display());
    }
}

pub async fn handle_cli_flag(arg: &str) -> Result<bool> {
    if arg == "--version" || arg == "-v" {
        println!("kotonoha {}", env!("CARGO_PKG_VERSION"));
        return Ok(true);
    }
    if arg == "--help" || arg == "-h" || arg == "--h" {
        println!(
            "🌸 kotonoha {} — Japanese i+1 Sentence Miner",
            env!("CARGO_PKG_VERSION")
        );
        println!("\nUSAGE:");
        println!("  kotonoha                       Launch interactive TUI file picker");
        println!("  kotonoha <MEDIA_FILE>          Parse specific subtitle/video file");
        println!("  kotonoha --config              Interactive TUI configuration manager");
        println!("  kotonoha --show-config         Display active configuration settings");
        println!(
            "  kotonoha --inspect [FILE]      Inspect sentences (Space plays selected audio; ★=i+1)"
        );
        println!("  kotonoha --manage-known        View & remove words from the known database");
        println!("  kotonoha --manage-mined        View & remove words from the mined list");
        println!("  kotonoha --manage-ignored      View & remove words from the ignore list");
        println!("  kotonoha --sync                Push locally mined cards to Anki");
        println!("  kotonoha --version | -v        Print version information");
        println!("  kotonoha --help    | -h | --h  Show help information");
        return Ok(true);
    }
    if arg == "--config" {
        let mut cfg = AppConfig::load()?;
        TerminalUi::configure_interactive(&mut cfg)?;
        return Ok(true);
    }
    if arg == "--show-config" {
        let cfg = AppConfig::load()?;
        TerminalUi::show_config(&cfg);
        return Ok(true);
    }
    if arg == "--sync" {
        let cfg = AppConfig::load()?;
        let db = Database::open(&cfg.db_path).await?;
        anki::sync_to_anki(&cfg, &db).await?;
        return Ok(true);
    }
    if arg == "--inspect" {
        let cfg = AppConfig::load()?;
        let db = Database::open(&cfg.db_path).await?;
        let input_path = match std::env::args().nth(2) {
            Some(p) => PathBuf::from(p),
            None => TerminalUi::select_media_file()?,
        };
        let video_path = find_paired_media(&input_path).ok().map(|(_, video)| video);
        let ext = input_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let subtitle_path = if ext == "srt" || ext == "ass" {
            input_path
        } else {
            let srt = input_path.with_extension("ja.srt");
            if srt.exists() {
                srt
            } else {
                input_path
            }
        };
        let sentences = parse_subtitle(&subtitle_path)?;
        let tokenizer = JapaneseTokenizer::new()?;
        let known_words = db.get_known_words().await?;
        let ignored_words = db.get_ignored_words().await?;
        TerminalUi::inspect_sentences(crate::ui::InspectSentencesParams {
            sentences: &sentences,
            tokenizer: &tokenizer,
            known_words: &known_words,
            ignored_words: &ignored_words,
            video_path: video_path.as_deref(),
        })?;
        return Ok(true);
    }
    if arg == "--manage-ignored" {
        let cfg = AppConfig::load()?;
        let db = Database::open(&cfg.db_path).await?;
        let tokenizer = JapaneseTokenizer::new()?;
        let words = words_with_readings(&tokenizer, db.get_ignored_words_sorted().await?);
        let to_remove = TerminalUi::manage_ignored_words(&words)?;
        if !to_remove.is_empty() {
            let count = db.remove_ignored_words(&to_remove).await?;
            println!(" ✔ Removed {} word(s) from the ignore list.", count);
        } else {
            println!(" ℹ No changes made.");
        }
        return Ok(true);
    }
    if arg == "--manage-known" {
        let cfg = AppConfig::load()?;
        let db = Database::open(&cfg.db_path).await?;
        let tokenizer = JapaneseTokenizer::new()?;
        let words = words_with_readings(&tokenizer, db.get_known_words_sorted_by_source("known").await?);
        let to_remove = TerminalUi::manage_known_words(&words)?;
        if !to_remove.is_empty() {
            let count = db.remove_known_words(&to_remove).await?;
            println!(" ✔ Removed {} word(s) from your known list.", count);
        } else {
            println!(" ℹ No changes made.");
        }
        return Ok(true);
    }
    if arg == "--manage-mined" {
        let cfg = AppConfig::load()?;
        let db = Database::open(&cfg.db_path).await?;
        let tokenizer = JapaneseTokenizer::new()?;
        let words = words_with_readings(&tokenizer, db.get_known_words_sorted_by_source("mined").await?);
        let to_remove = TerminalUi::manage_mined_words(&words)?;
        if !to_remove.is_empty() {
            let count = db.remove_known_words(&to_remove).await?;
            println!(" ✔ Removed {} word(s) from your mined list.", count);
        } else {
            println!(" ℹ No changes made.");
        }
        return Ok(true);
    }

    Ok(false)
}
