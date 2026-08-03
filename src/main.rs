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
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use config::AppConfig;
use console::style;
use db::Database;
use dict::DictionaryService;
use inquire::Text;
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

async fn anki_request(client: &reqwest::Client, url: &str, action: &str, params: serde_json::Value) -> Result<serde_json::Value> {
    let response = client
        .post(url)
        .json(&serde_json::json!({"action": action, "version": 6, "params": params}))
        .send()
        .await?
        .error_for_status()?;
    let body: serde_json::Value = response.json().await?;
    if let Some(error) = body.get("error").and_then(|value| value.as_str()) {
        anyhow::bail!("AnkiConnect error: {error}");
    }
    Ok(body.get("result").cloned().unwrap_or(serde_json::Value::Null))
}

async fn upload_anki_media(client: &reqwest::Client, url: &str, card_id: i64, path: &str, extension: &str) -> Result<Option<String>> {
    let path = Path::new(path);
    if !path.is_file() {
        return Ok(None);
    }
    let filename = format!("kotonoha-{card_id}.{extension}");
    let data = BASE64.encode(std::fs::read(path)?);
    anki_request(client, url, "storeMediaFile", serde_json::json!({"filename": filename, "data": data})).await?;
    Ok(Some(filename))
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn sentence_with_furigana(tokenizer: &JapaneseTokenizer, sentence: &str, target_word: &str) -> String {
    tokenizer
        .tokenize(sentence)
        .map(|tokens| {
            tokens
                .into_iter()
                .map(|token| {
                    let surface = escape_html(&token.surface);
                    let is_target = token.surface == target_word || token.dictionary_form == target_word;
                    let display = if token.surface.chars().any(|c| matches!(c, '\u{4E00}'..='\u{9FFF}'))
                        && !token.reading.is_empty()
                    {
                        format!("<ruby>{surface}<rt>{}</rt></ruby>", escape_html(&token.reading))
                    } else {
                        surface
                    };
                    if is_target { format!("<b>{display}</b>") } else { display }
                })
                .collect()
        })
        .unwrap_or_else(|_| escape_html(sentence))
}

fn to_katakana(text: &str) -> String {
    text.chars()
        .map(|character| match character {
            '\u{3041}'..='\u{3096}' => std::char::from_u32(character as u32 + 0x60).unwrap_or(character),
            _ => character,
        })
        .collect()
}

fn pitch_number(pitch_accent: &str, mora_count: usize) -> usize {
    if let Ok(number) = pitch_accent.trim().parse::<usize>() {
        return number.min(mora_count);
    }
    let pattern = pitch_accent.trim().to_ascii_uppercase();
    if pattern.starts_with('H') {
        1
    } else if let Some(drop) = pattern.chars().position(|level| level == 'L').filter(|drop| *drop > 1) {
        drop
    } else {
        0
    }
}

fn pitch_pattern(reading: &str, pitch_accent: &str) -> (String, String) {
    let morae = dict::split_morae(&to_katakana(reading));
    let pitch = pitch_number(pitch_accent, morae.len());
    let levels: Vec<bool> = (0..morae.len())
        .map(|index| {
            if pitch == 1 { index == 0 } else if pitch == 0 { index > 0 } else { index > 0 && index < pitch }
        })
        .collect();
    let pattern = morae
        .iter()
        .enumerate()
        .map(|(index, mora)| {
            let current = levels[index];
            let previous = index.checked_sub(1).and_then(|previous| levels.get(previous)).copied();
            let next = levels.get(index + 1).copied();
            let shadow = match (previous, current, next) {
                (_, false, Some(true)) => "inset -2px -2px 0 0 #3366CC",
                (Some(true), true, Some(false)) => "inset -2px 2px 0 0 #3366CC",
                (_, true, _) => "inset 0 2px 0 0 #3366CC",
                _ => "inset 0 -2px 0 0 #3366CC",
            };
            format!("<span style=\"box-shadow: {shadow};\">{mora}</span>")
        })
        .collect::<String>();
    let notation: String = levels.iter().map(|is_high| if *is_high { 'H' } else { 'L' }).collect();
    (
        format!(
            "{pattern} <span class=\"pitch_number\">{pitch}</span> <span class=\"pitch_pattern_text\">[{notation}]</span>"
        ),
        pitch.to_string(),
    )
}

async fn sync_to_anki(cfg: &AppConfig, db: &Database) -> Result<()> {
    let cards = db.get_unsynced_mined_cards()?;
    if cards.is_empty() {
        println!(" ✔ No locally mined cards are waiting to sync.");
        return Ok(());
    }

    let client = reqwest::Client::new();
    let tokenizer = JapaneseTokenizer::new()?;
    anki_request(&client, &cfg.anki_connect_url, "version", serde_json::json!({})).await?;
    anki_request(&client, &cfg.anki_connect_url, "createDeck", serde_json::json!({"deck": cfg.anki_deck_name})).await?;
    let fields = anki_request(
        &client,
        &cfg.anki_connect_url,
        "modelFieldNames",
        serde_json::json!({"modelName": cfg.anki_model_name}),
    )
    .await?;
    let required_fields = [
        "SentKanji", "SentFurigana", "SentEng", "SentAudio", "VocabKanji", "VocabFurigana",
        "VocabPitchPattern", "VocabPitchNum", "VocabDef", "VocabAudio", "Image", "Notes",
        "MakeProductionCard", "Focus",
    ];
    let available_fields = fields.as_array().ok_or_else(|| anyhow::anyhow!("AnkiConnect returned invalid fields for note type: {}", cfg.anki_model_name))?;
    if required_fields.iter().any(|field| !available_fields.iter().any(|value| value.as_str() == Some(field))) {
        anyhow::bail!("Anki note type '{}' does not have the required Japanese sentences+ fields", cfg.anki_model_name);
    }

    let mut synced = 0;
    for card in cards {
        let sentence_furigana = sentence_with_furigana(&tokenizer, &card.sentence, &card.target_word);
        let (pitch_pattern, pitch_number) = pitch_pattern(&card.reading, &card.pitch_accent);
        let audio = match card.audio_path.as_deref() {
            Some(path) => upload_anki_media(&client, &cfg.anki_connect_url, card.id, path, "opus").await?,
            None => None,
        };
        let image = match card.image_path.as_deref() {
            Some(path) => upload_anki_media(&client, &cfg.anki_connect_url, card.id, path, "jpg").await?,
            None => None,
        };
        let sentence_audio = audio.map(|filename| format!("[sound:{filename}]")).unwrap_or_default();
        let image = image.map(|filename| format!("<img src=\"{filename}\">")).unwrap_or_default();
        let note_id = anki_request(&client, &cfg.anki_connect_url, "addNote", serde_json::json!({
            "note": {
                "deckName": cfg.anki_deck_name,
                "modelName": cfg.anki_model_name,
                "fields": {
                    "SentKanji": card.sentence,
                    "SentFurigana": sentence_furigana,
                    "SentEng": "",
                    "SentAudio": sentence_audio,
                    "VocabKanji": card.target_word,
                    "VocabFurigana": card.reading,
                    "VocabPitchPattern": pitch_pattern,
                    "VocabPitchNum": pitch_number,
                    "VocabDef": card.definition,
                    "VocabAudio": "",
                    "Image": image,
                    "Notes": "",
                    "MakeProductionCard": "",
                    "Focus": ""
                },
                "tags": ["jp1k", "kotonoha", "mined"]
            }
        })).await?;
        let note_id = note_id.as_i64().ok_or_else(|| anyhow::anyhow!("AnkiConnect did not return a note ID"))?;
        db.mark_mined_card_synced(card.id, note_id)?;
        synced += 1;
    }
    println!(" ✔ Synced {synced} card(s) to Anki deck: {}", cfg.anki_deck_name);
    Ok(())
}

async fn add_test_card(cfg: &AppConfig) -> Result<()> {
    const TEST_DECK: &str = "kotonohatest";

    let sentence = Text::new("Japanese sentence:").prompt()?;
    let target_word = Text::new("Target word:").prompt()?;
    let audio_path = Text::new("Audio file path (optional):").prompt()?;
    let image_path = Text::new("Screenshot path (optional):").prompt()?;
    let definition = Text::new("Definition (optional):").prompt()?;
    let pitch = Text::new("Pitch number (0 = heiban, optional):").prompt()?;

    if sentence.trim().is_empty() || target_word.trim().is_empty() {
        anyhow::bail!("Sentence and target word are required.");
    }

    let client = reqwest::Client::new();
    let tokenizer = JapaneseTokenizer::new()?;
    anki_request(&client, &cfg.anki_connect_url, "version", serde_json::json!({})).await?;
    anki_request(&client, &cfg.anki_connect_url, "createDeck", serde_json::json!({"deck": TEST_DECK})).await?;

    let token = tokenizer
        .tokenize(&sentence)?
        .into_iter()
        .find(|token| token.surface == target_word || token.dictionary_form == target_word);
    let reading = token.map(|token| token.reading).unwrap_or_else(|| target_word.clone());
    let test_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as i64;
    let audio = if audio_path.trim().is_empty() {
        None
    } else {
        upload_anki_media(&client, &cfg.anki_connect_url, test_id, audio_path.trim(), "opus").await?
    };
    let image = if image_path.trim().is_empty() {
        None
    } else {
        upload_anki_media(&client, &cfg.anki_connect_url, test_id, image_path.trim(), "jpg").await?
    };

    let note_id = anki_request(&client, &cfg.anki_connect_url, "addNote", serde_json::json!({
        "note": {
            "deckName": TEST_DECK,
            "modelName": cfg.anki_model_name,
            "fields": {
                "SentKanji": sentence,
                "SentFurigana": sentence_with_furigana(&tokenizer, &sentence, &target_word),
                "SentEng": "",
                "SentAudio": audio.map(|filename| format!("[sound:{filename}]")).unwrap_or_default(),
                "VocabKanji": target_word,
                "VocabFurigana": reading,
                "VocabPitchPattern": pitch_pattern(&reading, &pitch).0,
                "VocabPitchNum": pitch_pattern(&reading, &pitch).1,
                "VocabDef": definition,
                "VocabAudio": "",
                "Image": image.map(|filename| format!("<img src=\"{filename}\">")),
                "Notes": "Test card created with kotonoha --test-add.",
                "MakeProductionCard": "",
                "Focus": ""
            },
            "tags": ["jp1k", "kotonoha", "test"]
        }
    })).await?;
    let note_id = note_id.as_i64().ok_or_else(|| anyhow::anyhow!("AnkiConnect did not return a note ID"))?;
    println!(" ✔ Test card added to {TEST_DECK} (note ID: {note_id}).");
    Ok(())
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
        if arg == "--help" || arg == "-h" || arg == "--h" {
            println!("🌸 kotonoha {} — Japanese i+1 Sentence Miner", env!("CARGO_PKG_VERSION"));
            println!("\nUSAGE:");
            println!("  kotonoha                       Launch interactive TUI file picker");
            println!("  kotonoha <MEDIA_FILE>          Parse specific subtitle/video file");
            println!("  kotonoha --inspect [FILE]      Inspect sentences (Blue=Known, Red=Unknown, ★=i+1)");
            println!("  kotonoha --manage-known        View & remove words from the known database");
            println!("  kotonoha --manage-ignored      View & remove words from the ignore list");
            println!("  kotonoha --clear-cache         Purge all cached dictionary definitions");
            println!("  kotonoha --sync                Push locally mined cards to Anki");
            println!("  kotonoha --test-add            Add a test card to the kotonohatest deck");
            println!("  kotonoha --version | -v        Print version information");
            println!("  kotonoha --help    | -h | --h  Show help information");
            return Ok(());
        }
        if arg == "--sync" {
            let cfg = AppConfig::load()?;
            let db = Database::open(&cfg.db_path)?;
            sync_to_anki(&cfg, &db).await?;
            return Ok(());
        }
        if arg == "--test-add" {
            let cfg = AppConfig::load()?;
            add_test_card(&cfg).await?;
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
        if arg == "--manage-known" {
            let cfg = AppConfig::load()?;
            let db = Database::open(&cfg.db_path)?;
            let words = db.get_known_words_sorted()?;
            let to_remove = TerminalUi::manage_known_words(&words)?;
            if !to_remove.is_empty() {
                let count = db.remove_known_words(&to_remove)?;
                println!(" ✔ Removed {} word(s) from known database.", count);
            } else {
                println!(" ℹ No changes made.");
            }
            return Ok(());
        }
        if matches!(arg.as_str(), "--clear-cache" | "--clear-dict-cache" | "--clear-jpdb-cache") {
            let cfg = AppConfig::load()?;
            let db = Database::open(&cfg.db_path)?;
            let count = db.clear_dictionary_cache()?;
            println!(" ✔ Cleared dictionary cache ({} cached entries purged).", count);
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
        let http_client = std::sync::Arc::new(reqwest::Client::new());

        // Step 1: Definitions & Pitch Accents (Concurrent 5 at a time with HTTP connection pooling)
        let pb1 = indicatif::ProgressBar::new(total);
        pb1.set_style(
            indicatif::ProgressStyle::default_bar()
                .template(" ℹ [1/3] Definitions & Pitch Accents  [{bar:35.cyan/blue}] {pos}/{len} ({percent}%)")
                .unwrap()
                .progress_chars("█▓▒░"),
        );

        let uncached_words: Vec<String> = candidates_to_process
            .iter()
            .map(|c| c.target_word.clone())
            .filter(|w| db.get_cached_definition(w).unwrap_or(None).is_none())
            .collect();

        let cached_count = (candidates_to_process.len() - uncached_words.len()) as u64;
        pb1.set_position(cached_count);

        if !uncached_words.is_empty() {
            let (tx, mut rx) = tokio::sync::mpsc::channel::<dict::LookupResult>(100);
            let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(5));

            for word in uncached_words {
                let sem = std::sync::Arc::clone(&semaphore);
                let client = std::sync::Arc::clone(&http_client);
                let tx = tx.clone();

                tokio::spawn(async move {
                    let _permit = sem.acquire().await;
                    if let Ok(dict_res) = DictionaryService::lookup(&client, &word).await {
                        let _ = tx.send(dict_res).await;
                    }
                });
            }
            drop(tx);

            while let Some(dict_res) = rx.recv().await {
                let _ = db.cache_definition(&dict_res.expression, &dict_res.reading, &dict_res.definition, &dict_res.pitch_accent);
                pb1.inc(1);
            }
        }
        pb1.finish();
        println!("\n");

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
        println!("\n");

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
        println!("\n");
    }

    let http_client = reqwest::Client::new();
    let mut mined_count = 0;
    let mut skipped_count = 0;
    let mut ignored_count = 0;
    let total_cards = candidates_to_process.len();

    for (idx, cand) in candidates_to_process.iter().enumerate() {
        TerminalUi::render_progress(idx + 1, total_cards, mined_count, skipped_count, ignored_count);

        let dict_info = match db.get_cached_definition(&cand.target_word)? {
            Some(res) => dict::LookupResult {
                expression: cand.target_word.clone(),
                reading: res.0,
                definition: res.1,
                pitch_accent: res.2,
            },
            None => {
                let res = DictionaryService::lookup(&http_client, &cand.target_word).await?;
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
                        &dict_info.pitch_accent,
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
                    ignored_count += 1;
                    println!(" 🚫 Target word ignored.");
                    break;
                }
                'n' => {
                    skipped_count += 1;
                    println!(" ⏭️ Card skipped.");
                    break;
                }
                'q' => {
                    println!(" 🚪 Exiting mining session.");
                    user_quit = true;
                    break;
                }
                _ => {
                    skipped_count += 1;
                    break;
                }
            }
        }

        if user_quit {
            break;
        }
    }

    println!("\n🎉 Mining session finished! Mined {} cards.\n", mined_count);
    Ok(())
}
