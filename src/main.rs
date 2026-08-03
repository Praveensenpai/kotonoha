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
use std::path::PathBuf;
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

    println!(" ℹ Loading media file: {}", input_path.display());

    let ext = input_path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    let (subtitle_path, video_path, has_video) = if ext == "srt" || ext == "ass" {
        let mkv = input_path.with_extension("mkv");
        let mp4 = input_path.with_extension("mp4");
        if mkv.exists() {
            (input_path.clone(), mkv, true)
        } else if mp4.exists() {
            (input_path.clone(), mp4, true)
        } else {
            (input_path.clone(), input_path.clone(), false)
        }
    } else {
        let srt = input_path.with_extension("ja.srt");
        let sub = if srt.exists() { srt } else { input_path.clone() };
        (sub, input_path.clone(), true)
    };

    if !has_video {
        println!(" {} No paired video found — audio preview disabled.", style("⚠").yellow().bold());
    }

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
        print!(" ℹ Pre-processing definitions & audio clips... ");
        let _ = std::io::Write::flush(&mut std::io::stdout());

        for cand in &candidates_to_process {
            if db.get_cached_definition(&cand.target_word).unwrap_or(None).is_none() {
                if let Ok(res) = DictionaryService::lookup(&cand.target_word).await {
                    let _ = db.cache_definition(&res.expression, &res.reading, &res.definition, &res.pitch_accent);
                }
            }
            if has_video {
                let audio_path = cfg.media_dir.join(format!("{}_{}.opus", cand.target_word, cand.sentence.index));
                if !audio_path.exists() {
                    let _ = MediaExtractor::extract_preview_audio(&video_path, cand.sentence.start_ms, cand.sentence.end_ms, &audio_path);
                }
            }
        }
        println!("✔ Ready!\n");
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
        let mut audio_child = if has_video && audio_path.exists() {
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
