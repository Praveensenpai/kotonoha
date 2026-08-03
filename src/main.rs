mod config;
mod db;
mod dict;
mod jpdb;
mod media;
mod miner;
mod nlp;
mod srt;
mod ui;

use anyhow::Result;
use config::AppConfig;
use console::style;
use db::Database;
use dict::DictionaryService;
use jpdb::JpdbVocabList;
use media::MediaExtractor;
use miner::MiningEngine;
use nlp::JapaneseTokenizer;
use srt::parse_subtitle;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use ui::TerminalUi;

async fn anki_connected(url: &str) -> bool {
    let body = serde_json::json!({"action": "version", "version": 6});
    reqwest::Client::new()
        .post(url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
        .is_ok()
}

fn find_paired_media(input_path: &Path) -> Result<(PathBuf, PathBuf)> {
    let parent = input_path.parent().unwrap_or_else(|| Path::new("."));
    let ext = input_path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();

    let is_sub = matches!(ext.as_str(), "srt" | "ass" | "vtt");
    let is_vid = matches!(ext.as_str(), "mkv" | "mp4" | "webm" | "avi");

    let stem = input_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
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
                let p_ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                if matches!(p_ext.as_str(), "mkv" | "mp4" | "webm" | "avi") {
                    let p_stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    if p_stem == stem || p_stem == clean_stem || stem.starts_with(p_stem) || p_stem.starts_with(clean_stem) {
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
                let p_ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                if matches!(p_ext.as_str(), "srt" | "ass" | "vtt") {
                    let p_stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    let p_clean = p_stem
                        .trim_end_matches(".ja")
                        .trim_end_matches(".jp")
                        .trim_end_matches(".ja-JP")
                        .trim_end_matches(".japanese");
                    if p_stem == stem || p_clean == stem || p_stem.starts_with(stem) || stem.starts_with(p_clean) {
                        return Ok((p, vid_path));
                    }
                }
            }
        }
        anyhow::bail!(
            "No matching Japanese subtitle file (.srt, .ass) found for video: {}\n   Place the subtitle file in the same folder to mine cards.",
            input_path.display()
        );
    } else {
        anyhow::bail!("Unsupported file format: {}", input_path.display());
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    if let Some(arg) = std::env::args().nth(1) {
        if arg == "--version" || arg == "-v" {
            println!("kotonoha {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        if arg == "--help" || arg == "-h" {
            println!("🌸 kotonoha {} — Japanese i+1 Sentence Miner", env!("CARGO_PKG_VERSION"));
            println!("\nUSAGE:");
            println!("  kotonoha                       Launch interactive TUI file picker");
            println!("  kotonoha <MEDIA_FILE>          Parse specific subtitle/video file");
            println!("  kotonoha --inspect [FILE]      Inspect sentences (Blue=Known, Red=Unknown)");
            println!("  kotonoha --manage-ignored      View & remove words from the ignore list");
            println!("  kotonoha --version | -v        Print version information");
            println!("  kotonoha --help    | -h        Show help information");
            return Ok(());
        }
        if arg == "--inspect" {
            let cfg = AppConfig::load()?;
            let db = Database::open(&cfg.db_path)?;
            let input_path = match std::env::args().nth(2) {
                Some(p) => PathBuf::from(p),
                None => TerminalUi::select_media_file()?,
            };
            let ext = input_path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
            let subtitle_path = if ext == "srt" || ext == "ass" {
                input_path
            } else {
                let srt = input_path.with_extension("ja.srt");
                if srt.exists() { srt } else { input_path }
            };
            let sentences = parse_subtitle(&subtitle_path)?;
            let tokenizer = JapaneseTokenizer::new()?;
            let known_words = db.get_known_words()?;
            let ignored_words = db.get_ignored_words()?;
            TerminalUi::inspect_sentences(&sentences, &tokenizer, &known_words, &ignored_words);
            return Ok(());
        }
        if arg == "--manage-ignored" {
            let cfg = AppConfig::load()?;
            let db = Database::open(&cfg.db_path)?;
            let words = db.get_ignored_words_sorted()?;
            let to_remove = TerminalUi::manage_ignored_words(&words)?;
            if !to_remove.is_empty() {
                let count = db.remove_ignored_words(&to_remove)?;
                println!(" ✔ Removed {} word(s) from the ignore list.", count);
            } else {
                println!(" ℹ No changes made.");
            }
            return Ok(());
        }
    }

    TerminalUi::print_banner();

    let cfg = AppConfig::load()?;
    let db = Database::open(&cfg.db_path)?;

    // AnkiConnect status
    if anki_connected(&cfg.anki_connect_url).await {
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

    let input_path = match std::env::args().nth(1) {
        Some(arg) => PathBuf::from(arg),
        None => TerminalUi::select_media_file()?,
    };

    let (subtitle_path, video_path) = match find_paired_media(&input_path) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("\n {} {}", style("❌ Error:").red().bold(), e);
            std::process::exit(1);
        }
    };

    println!(" ℹ Subtitle File: {}", style(subtitle_path.display()).cyan());
    println!(" ℹ Video File:    {} ({})", style(video_path.display()).cyan(), style("✔ Video paired").green().bold());

    let sentences = parse_subtitle(&subtitle_path)?;
    println!(" ✔ Parsed {} subtitle lines", sentences.len());

    let tokenizer = JapaneseTokenizer::new()?;

    let known_words = db.get_known_words()?;
    let ignored_words = db.get_ignored_words()?;

    // Bootstrap Vocabulary: Extract top unknown content words by frequency
    let mut word_counts: HashMap<String, (usize, String)> = HashMap::new();
    for sub in &sentences {
        if let Ok(tokens) = tokenizer.tokenize(&sub.text) {
            for t in tokens {
                if t.is_content_word && !known_words.contains(&t.dictionary_form) && !ignored_words.contains(&t.dictionary_form) {
                    let entry = word_counts.entry(t.dictionary_form.clone()).or_insert((0, t.reading.clone()));
                    entry.0 += 1;
                }
            }
        }
    }

    let mut top_vocab: Vec<(String, usize, String)> = word_counts
        .into_iter()
        .map(|(word, (count, reading))| (word, count, reading))
        .filter(|(_, count, _)| *count >= 2)
        .collect();
    top_vocab.sort_by(|a, b| b.1.cmp(&a.1));
    let bootstrap_candidates: Vec<(String, usize, String)> = top_vocab.into_iter().take(100).collect();

    if !bootstrap_candidates.is_empty() {
        let newly_known = TerminalUi::bootstrap_known_words(&bootstrap_candidates)?;
        if !newly_known.is_empty() {
            let count = db.add_known_words(&newly_known)?;
            println!(" ✔ Marked {} words as known!", count);
        }
    }

    let known_words = db.get_known_words()?;

    let mut file_known = std::collections::HashSet::new();
    let mut file_unknown = std::collections::HashSet::new();

    for sub in &sentences {
        if let Ok(tokens) = tokenizer.tokenize(&sub.text) {
            for t in tokens {
                if t.is_content_word && !ignored_words.contains(&t.dictionary_form) {
                    if known_words.contains(&t.dictionary_form) {
                        file_known.insert(t.dictionary_form);
                    } else {
                        file_unknown.insert(t.dictionary_form);
                    }
                }
            }
        }
    }

    let engine = MiningEngine::new(tokenizer);
    let jpdb_list = JpdbVocabList::load_or_fetch("https://jpdb.io/vocabulary-list")?;

    let candidates = engine.find_candidates(&sentences, &known_words, &ignored_words, &jpdb_list.ranks);
    println!(
        " ✔ Found {} $i+1$ candidate sentences (Stats: {} | {} | {})\n",
        candidates.len(),
        style(format!("{} Known Words", file_known.len())).blue().bold(),
        style(format!("{} Unknown Words", file_unknown.len())).red().bold(),
        style(format!("{} Eligible i+1 Sentences", candidates.len())).green().bold(),
    );

    let candidates_to_process: Vec<_> = candidates.into_iter().take(cfg.default_card_limit).collect();

    if !candidates_to_process.is_empty() {
        let total = candidates_to_process.len() as u64;

        // Step 1: Definitions & Pitch Accents
        let pb1 = indicatif::ProgressBar::new(total);
        pb1.set_style(
            indicatif::ProgressStyle::default_bar()
                .template(" ℹ [1/3] Definitions & Pitch Accents  [{bar:35.cyan/blue}] {pos}/{len} ({percent}%)")
                .unwrap()
                .progress_chars("█▓▒░"),
        );
        for cand in &candidates_to_process {
            if db.get_cached_definition(&cand.target_word).unwrap_or(None).is_none() {
                if let Ok(res) = DictionaryService::lookup(&cand.target_word).await {
                    let _ = db.cache_definition(&res.expression, &res.reading, &res.definition, &res.pitch_accent);
                }
            }
            pb1.inc(1);
        }
        pb1.finish();

        // Step 2: Audio Preview Clips (.opus)
        let pb2 = indicatif::ProgressBar::new(total);
        pb2.set_style(
            indicatif::ProgressStyle::default_bar()
                .template(" ℹ [2/3] Audio Preview Clips (.opus)   [{bar:35.magenta/blue}] {pos}/{len} ({percent}%)")
                .unwrap()
                .progress_chars("█▓▒░"),
        );
        for cand in &candidates_to_process {
            let audio_path = cfg.media_dir.join(format!("{}_{}.opus", cand.target_word, cand.sentence.index));
            if !audio_path.exists() {
                let _ = MediaExtractor::extract_preview_audio(&video_path, cand.sentence.start_ms, cand.sentence.end_ms, &audio_path);
            }
            pb2.inc(1);
        }
        pb2.finish();

        // Step 3: Screenshots (.jpg)
        let pb3 = indicatif::ProgressBar::new(total);
        pb3.set_style(
            indicatif::ProgressStyle::default_bar()
                .template(" ℹ [3/3] Screenshots 360p (.jpg)       [{bar:35.yellow/blue}] {pos}/{len} ({percent}%)")
                .unwrap()
                .progress_chars("█▓▒░"),
        );
        for cand in &candidates_to_process {
            let image_path = cfg.media_dir.join(format!("{}_{}.jpg", cand.target_word, cand.sentence.index));
            if !image_path.exists() {
                let _ = MediaExtractor::extract_screenshot(&video_path, cand.sentence.start_ms, &image_path);
            }
            pb3.inc(1);
        }
        pb3.finish();
        println!();
    }

    let mut mined_count = 0;
    for (idx, cand) in candidates_to_process.iter().enumerate() {
        let dict_info = match db.get_cached_definition(&cand.target_word)? {
            Some(res) => dict::LookupResult {
                expression: cand.target_word.clone(),
                reading: res.0,
                definition: res.1,
                pitch_accent: res.2,
            },
            None => {
                let res = DictionaryService::lookup(&cand.target_word).await?;
                db.cache_definition(&res.expression, &res.reading, &res.definition, &res.pitch_accent)?;
                res
            }
        };

        TerminalUi::render_card(
            idx + 1,
            &cand.sentence.text,
            &cand.target_word,
            &dict_info.reading,
            &dict_info.pitch_accent,
            cand.jpdb_rank,
            &dict_info.definition,
            &cand.known_context_words,
            &cand.unknown_context_words,
        );

        let audio_path = cfg.media_dir.join(format!("{}_{}.opus", cand.target_word, cand.sentence.index));
        let mut audio_child = if audio_path.exists() {
            MediaExtractor::play_preview_audio(&audio_path)
        } else {
            None
        };

        let mut user_quit = false;
        loop {
            let action = TerminalUi::ask_action()?;

            if action == 'r' {
                if let Some(mut child) = audio_child.take() {
                    let _ = child.kill();
                }
                audio_child = MediaExtractor::play_preview_audio(&audio_path);
                continue;
            }

            if let Some(mut child) = audio_child.take() {
                let _ = child.kill();
            }

            match action {
                'y' => {
                    let image_path = cfg.media_dir.join(format!("{}_{}.jpg", cand.target_word, cand.sentence.index));
                    let _ = MediaExtractor::extract_screenshot(&video_path, cand.sentence.start_ms, &image_path);

                    db.save_mined_card(
                        &cand.sentence.text,
                        &cand.target_word,
                        &dict_info.reading,
                        &dict_info.definition,
                        Some(&audio_path.to_string_lossy()),
                        Some(&image_path.to_string_lossy()),
                    )?;

                    let _ = db.add_known_words(&[cand.target_word.clone()]);
                    mined_count += 1;
                    println!(" ✔ Card mined successfully!");
                    break;
                }
                'i' => {
                    let _ = db.add_ignored_word(&cand.target_word);
                    println!(" 🚫 Target word ignored.");
                    break;
                }
                'q' => {
                    println!(" 🚪 Exiting mining session.");
                    user_quit = true;
                    break;
                }
                _ => break,
            }
        }

        if user_quit {
            break;
        }
    }

    println!("\n🎉 Mining session finished! Mined {} cards.\n", mined_count);
    Ok(())
}
