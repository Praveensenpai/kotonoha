mod ai;
mod anki;
mod commands;
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

#[tokio::main]
async fn main() -> Result<()> {
    if let Some(arg) = std::env::args().nth(1) {
        if commands::handle_cli_flag(&arg).await? {
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

    let (subtitle_path, video_path) = match commands::find_paired_media(&input_path) {
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
    let mut word_counts: HashMap<String, (usize, String, bool)> = HashMap::new();
    for sub in &sentences {
        if let Ok(tokens) = tokenizer.tokenize(&sub.text) {
            for t in tokens {
                if t.is_content_word && !known_words.contains(&t.dictionary_form) && !ignored_words.contains(&t.dictionary_form) {
                    let entry = word_counts.entry(t.dictionary_form.clone()).or_insert((0, t.reading.clone(), t.is_proper_noun));
                    entry.0 += 1;
                    if t.is_proper_noun {
                        entry.2 = true;
                    }
                }
            }
        }
    }

    let mut top_vocab: Vec<(String, usize, String, bool)> = word_counts
        .into_iter()
        .map(|(word, (count, reading, is_pn))| (word, count, reading, is_pn))
        .filter(|(_, count, _, _)| *count >= 2)
        .collect();
    top_vocab.sort_by_key(|b| std::cmp::Reverse(b.1));

    let general_candidates: Vec<(String, usize, String)> = top_vocab
        .iter()
        .filter(|(_, _, _, is_pn)| !*is_pn)
        .map(|(w, c, r, _)| (w.clone(), *c, r.clone()))
        .take(100)
        .collect();

    let name_candidates: Vec<(String, usize, String)> = top_vocab
        .iter()
        .filter(|(_, _, _, is_pn)| *is_pn)
        .map(|(w, c, r, _)| (w.clone(), *c, r.clone()))
        .take(50)
        .collect();

    if !name_candidates.is_empty() {
        let newly_ignored = TerminalUi::bootstrap_ignored_names(&name_candidates)?;
        if !newly_ignored.is_empty() {
            for w in &newly_ignored {
                let _ = db.add_ignored_word(w);
            }
            println!(" 🚫 Marked {} character names/proper nouns as ignored!", newly_ignored.len());
        }
    }

    if !general_candidates.is_empty() {
        let newly_known = TerminalUi::bootstrap_known_words(&general_candidates)?;
        if !newly_known.is_empty() {
            let count = db.add_known_words(&newly_known)?;
            println!(" ✔ Marked {} words as known!", count);
        }
    }

    let known_words = db.get_known_words()?;
    let ignored_words = db.get_ignored_words()?;
    let already_known_set = db.get_known_words_by_source("known")?;
    let mined_set = db.get_known_words_by_source("mined")?;

    let mut file_already_known = std::collections::HashSet::new();
    let mut file_mined = std::collections::HashSet::new();
    let mut file_unknown = std::collections::HashSet::new();

    let mut known_lines_count = 0usize;
    let mut i1_lines_count = 0usize;
    let mut i2_plus_lines_count = 0usize;

    for sub in &sentences {
        let mut line_unknown_words = 0usize;
        if let Ok(tokens) = tokenizer.tokenize(&sub.text) {
            for t in tokens {
                if t.is_content_word && !ignored_words.contains(&t.dictionary_form) {
                    if already_known_set.contains(&t.dictionary_form) {
                        file_already_known.insert(t.dictionary_form.clone());
                    } else if mined_set.contains(&t.dictionary_form) {
                        file_mined.insert(t.dictionary_form.clone());
                    } else {
                        file_unknown.insert(t.dictionary_form.clone());
                        line_unknown_words += 1;
                    }
                }
            }
        }
        if line_unknown_words == 0 {
            known_lines_count += 1;
        } else if line_unknown_words == 1 {
            i1_lines_count += 1;
        } else {
            i2_plus_lines_count += 1;
        }
    }

    let total_lines = sentences.len();
    let comp_ratio = if total_lines > 0 {
        (known_lines_count as f64 / total_lines as f64) * 100.0
    } else {
        0.0
    };

    let engine = MiningEngine::new(tokenizer);
    let jpdb_list = JpdbVocabList::load_or_fetch("https://jpdb.io/vocabulary-list")?;

    let candidates = engine.find_candidates(&sentences, &known_words, &ignored_words, &jpdb_list.ranks);
    let total_mined_cards = db.get_all_mined_cards().map(|v| v.len()).unwrap_or(0);
    println!(
        " 📊 Subtitle Line Comprehension Stats:\n   • Lines Known: {} / {} ({:.1}% comprehension ratio)\n   • i+1 Candidate Lines: {}\n   • Hard Lines (2+ Unknowns): {}\n   • Vocab Stats: {} | {} | {}\n",
        style(known_lines_count).green().bold(),
        style(total_lines).cyan().bold(),
        style(format!("{:.1}%", comp_ratio)).yellow().bold(),
        style(format!("{} i+1 lines ({} eligible candidates)", i1_lines_count, candidates.len())).green().bold(),
        style(format!("{} hard lines", i2_plus_lines_count)).red().bold(),
        style(format!("{} Known Words", file_already_known.len())).blue().bold(),
        style(format!("{} Mined Cards", total_mined_cards)).magenta().bold(),
        style(format!("{} Unknown Words", file_unknown.len())).red().bold(),
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

            let card_targets: Vec<(usize, String, String, String)> = candidates_to_process
                .iter()
                .enumerate()
                .map(|(idx, cand)| (idx, cand.sentence.text.clone(), cand.target_word.clone(), cand.target_reading.clone()))
                .collect();
            let cfg_ai_batch_size = cfg.ai_batch_size;

            Some(tokio::spawn(async move {
                let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(10));
                let mut all_results = Vec::new();
                let ai_batch_size = cfg_ai_batch_size.max(1);

                for chunk in card_targets.chunks(ai_batch_size) {
                    let mut lookup_futures = Vec::new();

                    for (idx, sentence_text, target_word, target_reading) in chunk {
                        let sem = std::sync::Arc::clone(&semaphore);
                        let client = std::sync::Arc::clone(&client);
                        let target = target_word.clone();
                        let reading = target_reading.clone();
                        let sentence = sentence_text.clone();
                        let idx = *idx;

                        lookup_futures.push(tokio::spawn(async move {
                            let target_for_lookup = target.clone();
                            let mut candidates = match tokio::time::timeout(
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

                            if !reading.is_empty() {
                                candidates.sort_by_key(|c| {
                                    let is_reading_match = c.reading == reading;
                                    let is_expr_match = c.expression == target;
                                    (!is_reading_match, !is_expr_match)
                                });
                            }

                            (idx, sentence, target, reading, candidates)
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
                        .map(|(idx, sentence, target_word, target_reading, candidates)| ai::CardBatchInput {
                            card_index: *idx,
                            sentence: sentence.as_str(),
                            target_word: target_word.as_str(),
                            target_reading: target_reading.as_str(),
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
    let mut known_count = 0;
    let mut skipped_count = 0;
    let mut ignored_count = 0;
    let total_cards = candidates_to_process.len();

    for (idx, cand) in candidates_to_process.iter().enumerate() {
        TerminalUi::render_progress(idx + 1, total_cards, mined_count, known_count, skipped_count, ignored_count);

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
                    let mut rec_def = rec_cand.definition.clone();
                    if let Some(sense_idx) = res.recommended_sense_index {
                        let senses = dict::parse_senses(&rec_def);
                        if let Some(s) = senses.get(sense_idx) {
                            rec_def = s.clone();
                        }
                    }
                    dict_info.definition = rec_def;
                }
            }
        }

        // Automatic Contextual Reading Alignment:
        // Prioritize dictionary candidates whose reading matches Sudachi's contextual sentence reading (e.g. "あさ" for "朝")
        if !cand.target_reading.is_empty() {
            let all_cands = DictionaryService::lookup_all_candidates(
                &http_client,
                &cand.target_word,
                cfg.max_definition_senses,
                cfg.max_glosses_per_sense,
            )
            .await
            .unwrap_or_default();

            if let Some(matched_cand) = all_cands.iter().find(|c| c.reading == cand.target_reading) {
                let current_def = dict_info.definition.clone();
                dict_info = matched_cand.clone();
                if current_def.starts_with("1. [AI Suggestion]") {
                    dict_info.definition = current_def;
                } else {
                    dict_info.definition = dict::format_contextual_definition(
                        &dict_info.definition,
                        context_hint,
                        cfg.max_definition_senses,
                        cfg.max_glosses_per_sense,
                    );
                }
            } else {
                dict_info.reading = cand.target_reading.clone();
            }

            let _ = db.cache_definition(&dict_info.expression, &dict_info.reading, &dict_info.definition, &dict_info.pitch_accent);
        }

        let translations_tuple = None;

        TerminalUi::render_card(ui::CardRenderParams {
            rank: idx + 1,
            sentence: &cand.sentence.text,
            target_word: &cand.target_word,
            reading: &dict_info.reading,
            pitch: &dict_info.pitch_accent,
            jpdb_rank: cand.jpdb_rank,
            definition: &dict_info.definition,
            known_context: &cand.known_context_words,
            unknown_context: &cand.unknown_context_words,
            ai_warning: ai_analysis.as_ref().and_then(|r| r.parsing_warning.as_deref()),
            translations: translations_tuple,
        });

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

            if action == 'f' {
                println!(" ✍️ Fetching dictionary candidates for furigana reading selection...");
                let candidates = DictionaryService::lookup_all_candidates(
                    &http_client,
                    &cand.target_word,
                    cfg.max_definition_senses,
                    cfg.max_glosses_per_sense,
                )
                .await
                .unwrap_or_default();

                if let Ok(new_reading) = TerminalUi::select_or_edit_reading(
                    &dict_info.reading,
                    &cand.target_reading,
                    &candidates,
                ) {
                    dict_info.reading = new_reading;
                    println!(" ✨ Updated furigana reading: 【{} ({})】", dict_info.expression, dict_info.reading);
                    TerminalUi::render_card(ui::CardRenderParams {
                        rank: idx + 1,
                        sentence: &cand.sentence.text,
                        target_word: &cand.target_word,
                        reading: &dict_info.reading,
                        pitch: &dict_info.pitch_accent,
                        jpdb_rank: cand.jpdb_rank,
                        definition: &dict_info.definition,
                        known_context: &cand.known_context_words,
                        unknown_context: &cand.unknown_context_words,
                        ai_warning: ai_analysis.as_ref().and_then(|r| r.parsing_warning.as_deref()),
                        translations: translations_tuple,
                    });
                }
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
                    TerminalUi::render_card(ui::CardRenderParams {
                        rank: idx + 1,
                        sentence: &cand.sentence.text,
                        target_word: &cand.target_word,
                        reading: &dict_info.reading,
                        pitch: &dict_info.pitch_accent,
                        jpdb_rank: cand.jpdb_rank,
                        definition: &dict_info.definition,
                        known_context: &cand.known_context_words,
                        unknown_context: &cand.unknown_context_words,
                        ai_warning: ai_analysis.as_ref().and_then(|r| r.parsing_warning.as_deref()),
                        translations: translations_tuple,
                    });
                }
                continue;
            }

            if let Some(mut child) = audio_child.take() {
                let _ = child.kill();
            }

            match action {
                'y' => {
                    let senses = dict::parse_senses(&dict_info.definition);
                    let chosen_sense = TerminalUi::select_sense(&senses, &cand.target_word)?;
                    let Some(chosen) = chosen_sense else {
                        println!(" ℹ Mining canceled — returning to card menu.");
                        continue;
                    };
                    dict_info.definition = chosen;

                    let image_path = cfg.media_dir.join(format!("{}_{}.jpg", cand.target_word, cand.sentence.index));
                    let _ = MediaExtractor::extract_screenshot(&video_path, cand.sentence.start_ms, &image_path);

                    let eng_nat = ai_analysis.and_then(|r| r.english_natural.as_deref());
                    let eng_lit = ai_analysis.and_then(|r| r.english_literal.as_deref());
                    let kan_nat = ai_analysis.and_then(|r| r.kannada_natural.as_deref());
                    let kan_lit = ai_analysis.and_then(|r| r.kannada_literal.as_deref());

                    db.save_mined_card(crate::db::SaveMinedCardParams {
                        sentence: &cand.sentence.text,
                        target_word: &cand.target_word,
                        reading: &dict_info.reading,
                        pitch_accent: &dict_info.pitch_accent,
                        definition: &dict_info.definition,
                        audio_path: Some(&audio_path.to_string_lossy()),
                        image_path: Some(&image_path.to_string_lossy()),
                        english_natural: eng_nat,
                        english_literal: eng_lit,
                        kannada_natural: kan_nat,
                        kannada_literal: kan_lit,
                    })?;

                    let _ = db.add_known_words_with_source(std::slice::from_ref(&cand.target_word), "mined");
                    mined_count += 1;
                    println!(" ✔ Card mined successfully!");
                    break;
                }
                'k' => {
                    let _ = db.add_known_words(std::slice::from_ref(&cand.target_word));
                    known_count += 1;
                    println!(" 🧠 Target word marked as known!");
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


