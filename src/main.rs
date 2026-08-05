mod ai;
mod anki;
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

fn words_with_readings(tokenizer: &JapaneseTokenizer, words: Vec<String>) -> Vec<(String, String)> {
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

pub async fn backfill_translations(cfg: &AppConfig, db: &Database) -> Result<()> {
    if !cfg.enable_ai {
        return Ok(());
    }

    let api_key = match cfg.gemini_api_key.as_deref() {
        Some(k) if !k.trim().is_empty() => k,
        _ => return Ok(()),
    };

    let missing = db.get_cards_missing_translations()?;
    if missing.is_empty() {
        return Ok(());
    }

    println!(" 🤖 Backfilling English & Kannada translations for {} past mined card(s)...", missing.len());

    let client = reqwest::Client::new();
    let empty_lookup: Vec<crate::dict::LookupResult> = Vec::new();
    let batch_size = cfg.ai_batch_size.max(1);

    let mut total_backfilled = 0;
    let chunks: Vec<_> = missing.chunks(batch_size).collect();
    let total_chunks = chunks.len();

    for (chunk_idx, chunk) in chunks.iter().enumerate() {
        println!("   ↳ Processing batch {}/{} ({} card(s))...", chunk_idx + 1, total_chunks, chunk.len());

        let batch_inputs: Vec<ai::CardBatchInput<'_>> = chunk
            .iter()
            .enumerate()
            .map(|(idx, card)| ai::CardBatchInput {
                card_index: idx,
                sentence: card.sentence.as_str(),
                target_word: card.target_word.as_str(),
                candidates: empty_lookup.as_slice(),
            })
            .collect();

        match ai::GeminiAiService::analyze_batch(&client, api_key, &cfg.gemini_model, &batch_inputs).await {
            Ok(results) => {
                for res in results {
                    if let Some(card) = chunk.get(res.card_index) {
                        if res.english_natural.is_some() || res.kannada_natural.is_some() {
                            let _ = db.update_card_translations(
                                card.id,
                                res.english_natural.as_deref(),
                                res.english_literal.as_deref(),
                                res.kannada_natural.as_deref(),
                                res.kannada_literal.as_deref(),
                            );
                            total_backfilled += 1;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!(" ⚠️ Backfill batch {}/{} failed: {e}", chunk_idx + 1, total_chunks);
            }
        }
    }

    println!(" ✔ Backfilled translations for {}/{} card(s)!\n", total_backfilled, missing.len());
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
            println!("  kotonoha --config              Interactive TUI configuration manager");
            println!("  kotonoha --show-config         Display active configuration settings");
            println!("  kotonoha --inspect [FILE]      Inspect sentences (Blue=Known, Red=Unknown, ★=i+1)");
            println!("  kotonoha --manage-known        View & remove words from the known database");
            println!("  kotonoha --manage-ignored      View & remove words from the ignore list");
            println!("  kotonoha --clear-cache         Purge all cached dictionary definitions");
            println!("  kotonoha --backfill-translations Generate missing translations for mined cards");
            println!("  kotonoha --sync                Push locally mined cards to Anki");
            println!("  kotonoha --version | -v        Print version information");
            println!("  kotonoha --help    | -h | --h  Show help information");
            return Ok(());
        }
        if arg == "--config" {
            let mut cfg = AppConfig::load()?;
            TerminalUi::configure_interactive(&mut cfg)?;
            return Ok(());
        }
        if arg == "--show-config" {
            let cfg = AppConfig::load()?;
            TerminalUi::show_config(&cfg);
            return Ok(());
        }
        if arg == "--backfill-translations" {
            let cfg = AppConfig::load()?;
            let db = Database::open(&cfg.db_path)?;
            backfill_translations(&cfg, &db).await?;
            return Ok(());
        }
        if arg == "--sync" {
            let cfg = AppConfig::load()?;
            let db = Database::open(&cfg.db_path)?;
            backfill_translations(&cfg, &db).await?;
            anki::sync_to_anki(&cfg, &db).await?;
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
            let tokenizer = JapaneseTokenizer::new()?;
            let words = words_with_readings(&tokenizer, db.get_ignored_words_sorted()?);
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
            let tokenizer = JapaneseTokenizer::new()?;
            let words = words_with_readings(&tokenizer, db.get_known_words_sorted()?);
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

    let http_client = std::sync::Arc::new(reqwest::Client::new());

    let ai_start = std::time::Instant::now();

    // Spawn Gemini AI Context Analysis in the background upfront
    let ai_task_handle = if cfg.enable_ai {
        if let Some(ref api_key) = cfg.gemini_api_key {
            let api_key = api_key.clone();
            let model = cfg.gemini_model.clone();
            let client = std::sync::Arc::clone(&http_client);
            let max_senses = cfg.max_definition_senses;
            let max_glosses = cfg.max_glosses_per_sense;

            let card_targets: Vec<(usize, String, String)> = candidates_to_process
                .iter()
                .enumerate()
                .map(|(idx, cand)| (idx, cand.sentence.text.clone(), cand.target_word.clone()))
                .collect();
            let cfg_ai_batch_size = cfg.ai_batch_size;

            Some(tokio::spawn(async move {
                let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(10));
                let mut all_results = Vec::new();
                let ai_batch_size = cfg_ai_batch_size.max(1);

                for chunk in card_targets.chunks(ai_batch_size) {
                    let mut lookup_futures = Vec::new();

                    for (idx, sentence_text, target_word) in chunk {
                        let sem = std::sync::Arc::clone(&semaphore);
                        let client = std::sync::Arc::clone(&client);
                        let target = target_word.clone();
                        let sentence = sentence_text.clone();
                        let idx = *idx;

                        lookup_futures.push(tokio::spawn(async move {
                            let target_for_lookup = target.clone();
                            let candidates = match tokio::time::timeout(
                                std::time::Duration::from_secs(4),
                                async move {
                                    let _permit = sem.acquire().await;
                                    DictionaryService::lookup_all_candidates(&client, &target_for_lookup, max_senses, max_glosses).await
                                },
                            )
                            .await
                            {
                                Ok(Ok(res)) => res,
                                _ => Vec::new(),
                            };
                            (idx, sentence, target, candidates)
                        }));
                    }

                    let mut batch_inputs_owned = Vec::new();
                    for fut in lookup_futures {
                        if let Ok(item) = fut.await {
                            batch_inputs_owned.push(item);
                        }
                    }

                    let inputs: Vec<ai::CardBatchInput<'_>> = batch_inputs_owned
                        .iter()
                        .map(|(idx, sentence, target_word, candidates)| ai::CardBatchInput {
                            card_index: *idx,
                            sentence: sentence.as_str(),
                            target_word: target_word.as_str(),
                            candidates: candidates.as_slice(),
                        })
                        .collect();

                    match ai::GeminiAiService::analyze_batch(&client, &api_key, &model, &inputs).await {
                        Ok(mut chunk_res) => {
                            all_results.append(&mut chunk_res);
                        }
                        Err(e) => {
                            eprintln!(" ⚠️ Gemini AI batch request error: {e}");
                        }
                    }
                }

                Ok::<Vec<ai::AiAnalysisResult>, anyhow::Error>(all_results)
            }))
        } else {
            None
        }
    } else {
        None
    };

    if !candidates_to_process.is_empty() {
        const RAW_SENSE_LIMIT: usize = 12;
        let total = candidates_to_process.len() as u64;

        // Step 1: Definitions & Pitch Accents (Concurrent 5 at a time with HTTP connection pooling)
        let pb1 = indicatif::ProgressBar::new(total);
        pb1.set_style(
            indicatif::ProgressStyle::default_bar()
                .template(" ℹ [1/4] Definitions & Pitch Accents  [{bar:35.cyan/blue}] {pos}/{len} ({percent}%)")
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

                let max_senses = cfg.max_definition_senses.max(RAW_SENSE_LIMIT);
                let max_glosses = cfg.max_glosses_per_sense;
                tokio::spawn(async move {
                    let _permit = sem.acquire().await;
                    if let Ok(dict_res) = DictionaryService::lookup_with_limits(&client, &word, max_senses, max_glosses).await {
                        let _ = tx.send(dict_res).await;
                    }
                });
            }
            drop(tx);

            while let Some(dict_res) = rx.recv().await {
                if !dict::is_placeholder_definition(&dict_res.definition)
                    && dict_res.definition != "No dictionary definition found"
                {
                    let _ = db.cache_definition(&dict_res.expression, &dict_res.reading, &dict_res.definition, &dict_res.pitch_accent);
                }
                pb1.inc(1);
            }
        }
        pb1.finish();
        println!("\n");

        // Step 2: Audio Preview Clips (.opus)
        let pb2 = indicatif::ProgressBar::new(total);
        pb2.set_style(
            indicatif::ProgressStyle::default_bar()
                .template(" ℹ [2/4] Audio Preview Clips (.opus)   [{bar:35.magenta/blue}] {pos}/{len} ({percent}%)")
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
                .template(" ℹ [3/4] Screenshots 360p (.jpg)       [{bar:35.yellow/blue}] {pos}/{len} ({percent}%)")
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

    // Step 4: Collect / Await Background Gemini AI Results
    let ai_results_map: HashMap<usize, ai::AiAnalysisResult> = match ai_task_handle {
        Some(handle) => {
            println!(" 🤖 [4/4] Gemini AI Context Analysis ({}) ...", cfg.gemini_model);
            match handle.await {
                Ok(Ok(results)) => {
                    let elapsed = ai_start.elapsed().as_secs_f64();
                    println!(" ✔ Gemini AI analysis ready ({} card(s) processed in {:.2}s).\n", results.len(), elapsed);
                    results.into_iter().map(|r| (r.card_index, r)).collect()
                }
                Ok(Err(e)) => {
                    let elapsed = ai_start.elapsed().as_secs_f64();
                    eprintln!(" ⚠️ Gemini AI batch analysis failed after {:.2}s: {e}\n", elapsed);
                    HashMap::new()
                }
                Err(e) => {
                    eprintln!(" ⚠️ Gemini AI task join error: {e}\n");
                    HashMap::new()
                }
            }
        }
        None => HashMap::new(),
    };

    let mut mined_count = 0;
    let mut skipped_count = 0;
    let mut ignored_count = 0;
    let total_cards = candidates_to_process.len();

    for (idx, cand) in candidates_to_process.iter().enumerate() {
        TerminalUi::render_progress(idx + 1, total_cards, mined_count, skipped_count, ignored_count);

        const RAW_SENSE_LIMIT: usize = 12;
        let context_hint = dict::context_hint(&cand.sentence.text, &cand.target_word);
        let cached = db.get_cached_definition(&cand.target_word)?;
        let needs_context_refresh = context_hint
            .is_some_and(|hint| cached.as_ref().is_some_and(|res| !dict::has_contextual_sense(&res.1, hint)));
        let (reading, raw_definition, pitch_accent) = match (cached, needs_context_refresh) {
            (Some(res), false) => res,
            (_, true) | (None, false) => {
                let res = DictionaryService::lookup_with_limits(
                    &http_client,
                    &cand.target_word,
                    cfg.max_definition_senses.max(RAW_SENSE_LIMIT),
                    cfg.max_glosses_per_sense,
                )
                .await?;
                if !dict::is_placeholder_definition(&res.definition)
                    && res.definition != "No dictionary definition found"
                {
                    db.cache_definition(&res.expression, &res.reading, &res.definition, &res.pitch_accent)?;
                }
                (res.reading, res.definition, res.pitch_accent)
            }
        };
        let mut dict_info = dict::LookupResult {
            expression: cand.target_word.clone(),
            reading,
            definition: dict::format_contextual_definition(
                &raw_definition,
                context_hint,
                cfg.max_definition_senses,
                cfg.max_glosses_per_sense,
            ),
            pitch_accent,
        };

        let ai_analysis = ai_results_map.get(&idx);

        if let Some(res) = ai_analysis {
            // A contextual AI gloss is more useful on the card than the raw,
            // multi-sense dictionary entry. Keep the dictionary candidate for
            // reading/base-word data, but let the custom gloss win for meaning.
            if let Some(ref custom_sug) = res.custom_definition_suggestion {
                dict_info.definition = format!("1. [AI Suggestion] {}", custom_sug);
            } else if let Some(cand_idx) = res.recommended_candidate_index {
                let candidates = DictionaryService::lookup_all_candidates(
                    &http_client,
                    &cand.target_word,
                    cfg.max_definition_senses,
                    cfg.max_glosses_per_sense,
                )
                .await
                .unwrap_or_default();

                if let Some(rec_cand) = candidates.get(cand_idx) {
                    dict_info = rec_cand.clone();
                    if let Some(sense_idx) = res.recommended_sense_index {
                        let senses = dict::parse_senses(&dict_info.definition);
                        if let Some(s) = senses.get(sense_idx) {
                            dict_info.definition = s.clone();
                        }
                    }
                }
            }
        }

        let translations_tuple = None;

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
            ai_analysis.as_ref().and_then(|r| r.parsing_warning.as_deref()),
            translations_tuple,
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

            if action == 'c' {
                println!(" 🔍 Fetching dictionary candidates...");
                let candidates = DictionaryService::lookup_all_candidates(
                    &http_client,
                    &cand.target_word,
                    cfg.max_definition_senses,
                    cfg.max_glosses_per_sense,
                )
                .await
                .unwrap_or_default();
                if candidates.is_empty() {
                    println!(" ⚠️  No dictionary candidates found — custom definition is available.");
                }
                if let Ok(chosen) = TerminalUi::select_candidate_or_custom(
                    &candidates,
                    &cand.target_word,
                    &dict_info.reading,
                    &dict_info.pitch_accent,
                    ai_analysis,
                ) {
                    dict_info = chosen;
                    let _ = db.cache_definition(
                        &dict_info.expression,
                        &dict_info.reading,
                        &dict_info.definition,
                        &dict_info.pitch_accent,
                    );
                    println!(" ✨ Updated candidate: 【{} ({})】", dict_info.expression, dict_info.reading);
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
                        ai_analysis.as_ref().and_then(|r| r.parsing_warning.as_deref()),
                        translations_tuple,
                    );
                }
                continue;
            }

            if let Some(mut child) = audio_child.take() {
                let _ = child.kill();
            }

            match action {
                'y' => {
                    let senses = dict::parse_senses(&dict_info.definition);
                    let chosen = TerminalUi::select_sense(&senses, &cand.target_word)?;
                    dict_info.definition = chosen;

                    let image_path = cfg.media_dir.join(format!("{}_{}.jpg", cand.target_word, cand.sentence.index));
                    let _ = MediaExtractor::extract_screenshot(&video_path, cand.sentence.start_ms, &image_path);

                    let eng_nat = ai_analysis.and_then(|r| r.english_natural.as_deref());
                    let eng_lit = ai_analysis.and_then(|r| r.english_literal.as_deref());
                    let kan_nat = ai_analysis.and_then(|r| r.kannada_natural.as_deref());
                    let kan_lit = ai_analysis.and_then(|r| r.kannada_literal.as_deref());

                    db.save_mined_card(
                        &cand.sentence.text,
                        &cand.target_word,
                        &dict_info.reading,
                        &dict_info.pitch_accent,
                        &dict_info.definition,
                        Some(&audio_path.to_string_lossy()),
                        Some(&image_path.to_string_lossy()),
                        eng_nat,
                        eng_lit,
                        kan_nat,
                        kan_lit,
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

    let unsynced = db.get_unsynced_mined_cards()?;
    if !unsynced.is_empty() {
        if anki::anki_connected(&cfg.anki_connect_url).await {
            println!(" 🔄 Auto-syncing mined cards to Anki...");
            if let Err(e) = anki::sync_to_anki(&cfg, &db).await {
                eprintln!(" ❌ Auto-sync error: {e}");
            }
        } else {
            println!(
                " ⚠ Anki is not connected — {} card(s) saved locally in database.\n   Run {} once Anki is open to push them.",
                unsynced.len(),
                style("kotonoha --sync").cyan()
            );
        }
    }

    Ok(())
}


