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
use db::Database;
use dict::DictionaryService;
use jpdb::JpdbVocabList;
use media::MediaExtractor;
use miner::MiningEngine;
use nlp::JapaneseTokenizer;
use srt::parse_subtitle;
use std::path::{Path, PathBuf};
use ui::TerminalUi;

#[tokio::main]
async fn main() -> Result<()> {
    println!("\n┌───────────────────────────────────────────────────────────────┐");
    println!("│      🌸  K O T O N O H A  ──  Japanese $i+1$ Sentence Miner    │");
    println!("└───────────────────────────────────────────────────────────────┘\n");

    let cfg = AppConfig::load()?;
    let db = Database::open(&cfg.db_path)?;

    let input_path = match std::env::args().nth(1) {
        Some(arg) => PathBuf::from(arg),
        None => TerminalUi::select_media_file()?,
    };

    println!(" ℹ Loading media file: {}", input_path.display());

    let ext = input_path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    let (subtitle_path, video_path) = if ext == "srt" || ext == "ass" {
        let mkv = input_path.with_extension("mkv");
        let mp4 = input_path.with_extension("mp4");
        let vid = if mkv.exists() {
            mkv
        } else if mp4.exists() {
            mp4
        } else {
            input_path.clone()
        };
        (input_path.clone(), vid)
    } else {
        let srt = input_path.with_extension("ja.srt");
        let sub = if srt.exists() { srt } else { input_path.clone() };
        (sub, input_path.clone())
    };

    let sentences = parse_subtitle(&subtitle_path)?;
    println!(" ✔ Parsed {} subtitle lines", sentences.len());

    let tokenizer = JapaneseTokenizer::new()?;
    let engine = MiningEngine::new(tokenizer);

    let known_words = db.get_known_words()?;
    let ignored_words = db.get_ignored_words()?;
    let jpdb_list = JpdbVocabList::load_or_fetch("https://jpdb.io/vocabulary-list")?;

    let candidates = engine.find_candidates(&sentences, &known_words, &ignored_words, &jpdb_list.ranks);
    println!(" ✔ Found {} $i+1$ candidate sentences\n", candidates.len());

    let mut mined_count = 0;
    for (idx, cand) in candidates.iter().enumerate() {
        if mined_count >= cfg.default_card_limit {
            break;
        }

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
        );

        let audio_path = cfg.media_dir.join(format!("{}_{}.mp3", cand.target_word, cand.sentence.index));
        let _ = MediaExtractor::extract_preview_audio(&video_path, cand.sentence.start_ms, cand.sentence.end_ms, &audio_path);

        let mut audio_child = MediaExtractor::play_preview_audio(&audio_path);

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
