use anyhow::Result;
use console::style;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::ai::{self, GeminiAiService};
use crate::anki;
use crate::config::AppConfig;
use crate::db::Database;
use crate::dict::{self, DictionaryService};
use crate::media::MediaExtractor;
use crate::miner::CandidateSentence;
use crate::ui::{CardRenderParams, TerminalUi};

pub async fn run_mining_loop(
    all_candidates: Vec<CandidateSentence>,
    video_path: &Path,
    cfg: &AppConfig,
    db: &mut Database,
    http_client: reqwest::Client,
) -> Result<()> {
    let batch_size = cfg.default_card_limit.max(1);
    let total_all_candidates = all_candidates.len();
    let batch_chunks: Vec<Vec<_>> = all_candidates
        .chunks(batch_size)
        .map(|c| c.to_vec())
        .collect();
    let total_batches = batch_chunks.len();

    let http_client = Arc::new(http_client);
    let mut mined_count = 0;
    let mut known_count = 0;
    let mut skipped_count = 0;
    let mut ignored_count = 0;
    let mut user_quit = false;

    for (batch_idx, candidates_to_process) in batch_chunks.into_iter().enumerate() {
        if user_quit {
            break;
        }

        let remaining_lines = total_all_candidates.saturating_sub((batch_idx + 1) * batch_size);
        if total_batches > 1 {
            println!(
                "\n 📦 Processing Batch {}/{} ({} candidates in batch, {} lines remaining)...",
                batch_idx + 1,
                total_batches,
                candidates_to_process.len(),
                remaining_lines
            );
        }

        let ai_start = std::time::Instant::now();
        let _ = db.clean_expired_ai_cache(cfg.ai_cache_ttl_minutes);

        let mut cached_ai_results = Vec::new();
        let mut uncached_card_targets = Vec::new();

        if cfg.enable_ai {
            for (idx, cand) in candidates_to_process.iter().enumerate() {
                if let Ok(Some(cached_res)) =
                    db.get_cached_ai_analysis(crate::db::GetCachedAiParams {
                        sentence: &cand.sentence.text,
                        target_word: &cand.target_word,
                        model: &cfg.gemini_model,
                        card_index: idx,
                        ttl_minutes: cfg.ai_cache_ttl_minutes,
                    })
                {
                    cached_ai_results.push(cached_res);
                } else {
                    uncached_card_targets.push((
                        idx,
                        cand.sentence.text.clone(),
                        cand.target_word.clone(),
                        cand.target_reading.clone(),
                    ));
                }
            }
        }

        let ai_task_handle = if cfg.enable_ai && !uncached_card_targets.is_empty() {
            if let Some(ref api_key) = cfg.gemini_api_key {
                let api_key = api_key.clone();
                let model = cfg.gemini_model.clone();
                let client = Arc::clone(&http_client);
                let max_senses = cfg.max_definition_senses;
                let max_glosses = cfg.max_glosses_per_sense;
                let card_targets = uncached_card_targets.clone();
                let cfg_ai_batch_size = cfg.ai_batch_size;

                Some(tokio::spawn(async move {
                    let semaphore = Arc::new(tokio::sync::Semaphore::new(10));
                    let mut all_results = Vec::new();
                    let ai_batch_size = cfg_ai_batch_size.max(1);

                    for chunk in card_targets.chunks(ai_batch_size) {
                        let mut lookup_futures = Vec::new();

                        for (idx, sentence_text, target_word, target_reading) in chunk {
                            let sem = Arc::clone(&semaphore);
                            let client = Arc::clone(&client);
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
                                        DictionaryService::lookup_all_candidates(
                                            &client,
                                            &target_for_lookup,
                                            max_senses,
                                            max_glosses,
                                        )
                                        .await
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
                            .map(|(idx, sentence, target_word, target_reading, candidates)| {
                                ai::CardBatchInput {
                                    card_index: *idx,
                                    sentence: sentence.as_str(),
                                    target_word: target_word.as_str(),
                                    target_reading: target_reading.as_str(),
                                    candidates: candidates.as_slice(),
                                }
                            })
                            .collect();

                        match GeminiAiService::analyze_batch(&client, &api_key, &model, &inputs)
                            .await
                        {
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
            preload_batch_media(&candidates_to_process, video_path, cfg, db, &http_client).await;
        }

        let cached_count_ai = cached_ai_results.len();
        let ai_results_map: HashMap<usize, ai::AiAnalysisResult> = match ai_task_handle {
            Some(handle) => {
                println!(
                    " 🤖 [4/4] Gemini AI Context Analysis ({}) ...",
                    cfg.gemini_model
                );
                match handle.await {
                    Ok(Ok(fresh_results)) => {
                        let elapsed = ai_start.elapsed().as_secs_f64();
                        println!(
                            " ✔ Gemini AI analysis ready ({} from cache, {} fetched from API in {:.2}s).\n",
                            cached_count_ai,
                            fresh_results.len(),
                            elapsed
                        );

                        for res in &fresh_results {
                            let cand = &candidates_to_process[res.card_index];
                            let _ = db.cache_ai_analysis(
                                &cand.sentence.text,
                                &cand.target_word,
                                &cfg.gemini_model,
                                res,
                            );
                        }

                        let mut merged = cached_ai_results;
                        merged.extend(fresh_results);
                        merged.into_iter().map(|r| (r.card_index, r)).collect()
                    }
                    Ok(Err(e)) => {
                        let elapsed = ai_start.elapsed().as_secs_f64();
                        eprintln!(
                            " ⚠️ Gemini AI batch analysis failed after {:.2}s: {e}\n",
                            elapsed
                        );
                        cached_ai_results
                            .into_iter()
                            .map(|r| (r.card_index, r))
                            .collect()
                    }
                    Err(e) => {
                        eprintln!(" ⚠️ Gemini AI task join error: {e}\n");
                        cached_ai_results
                            .into_iter()
                            .map(|r| (r.card_index, r))
                            .collect()
                    }
                }
            }
            None => {
                if cached_count_ai > 0 {
                    let elapsed = ai_start.elapsed().as_secs_f64();
                    println!(
                        " ✔ Gemini AI analysis ready (all {} card(s) loaded from cache in {:.2}s).\n",
                        cached_count_ai, elapsed
                    );
                }
                cached_ai_results
                    .into_iter()
                    .map(|r| (r.card_index, r))
                    .collect()
            }
        };

        let total_cards = candidates_to_process.len();

        for (idx, cand) in candidates_to_process.iter().enumerate() {
            TerminalUi::render_progress(
                idx + 1,
                total_cards,
                mined_count,
                known_count,
                skipped_count,
                ignored_count,
            );

            let mut dict_info = resolve_dict_info(cand, cfg, db, &http_client).await?;
            let ai_analysis = ai_results_map.get(&idx);

            apply_ai_gloss_or_candidate(ai_analysis, &mut dict_info, cand, cfg, db, &http_client)
                .await;

            align_contextual_reading(cand, &mut dict_info, cfg, db, &http_client).await;

            TerminalUi::render_card(CardRenderParams {
                rank: idx + 1,
                sentence: &cand.sentence.text,
                target_word: &cand.target_word,
                reading: &dict_info.reading,
                pitch: &dict_info.pitch_accent,
                episode_freq: cand.episode_freq,
                density_tier: cand.density_tier,
                definition: &dict_info.definition,
                known_context: &cand.known_context_words,
                unknown_context: &cand.unknown_context_words,
                ignored_context: &cand.ignored_context_words,
                ai_warning: ai_analysis
                    .as_ref()
                    .and_then(|r| r.parsing_warning.as_deref()),
                translations: None,
            });

            let audio_path = cfg
                .media_dir
                .join(format!("{}_{}.opus", cand.target_word, cand.sentence.index));
            let mut audio_child = if audio_path.exists() {
                MediaExtractor::play_preview_audio(&audio_path)
            } else {
                None
            };

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
                    println!(
                        " ✍️ Fetching dictionary candidates for furigana reading selection..."
                    );
                    let candidates = DictionaryService::lookup_all_candidates_cached(
                        &http_client,
                        Some(db),
                        &cand.target_word,
                        dict::LookupLimits {
                            max_senses: cfg.max_definition_senses,
                            max_glosses: cfg.max_glosses_per_sense,
                        },
                    )
                    .await
                    .unwrap_or_default();

                    if let Ok(new_reading) = TerminalUi::select_or_edit_reading(
                        &dict_info.reading,
                        &cand.target_reading,
                        &candidates,
                    ) {
                        dict_info.reading = new_reading;
                        println!(
                            " ✨ Updated furigana reading: 【{} ({})】",
                            dict_info.expression, dict_info.reading
                        );
                        TerminalUi::render_card(CardRenderParams {
                            rank: idx + 1,
                            sentence: &cand.sentence.text,
                            target_word: &cand.target_word,
                            reading: &dict_info.reading,
                            pitch: &dict_info.pitch_accent,
                            episode_freq: cand.episode_freq,
                            density_tier: cand.density_tier,
                            definition: &dict_info.definition,
                            known_context: &cand.known_context_words,
                            unknown_context: &cand.unknown_context_words,
                            ignored_context: &cand.ignored_context_words,
                            ai_warning: ai_analysis
                                .as_ref()
                                .and_then(|r| r.parsing_warning.as_deref()),
                            translations: None,
                        });
                    }
                    continue;
                }

                if action == 'c' {
                    println!(" 🔍 Fetching dictionary candidates...");
                    let candidates = DictionaryService::lookup_all_candidates_cached(
                        &http_client,
                        Some(db),
                        &cand.target_word,
                        dict::LookupLimits {
                            max_senses: cfg.max_definition_senses,
                            max_glosses: cfg.max_glosses_per_sense,
                        },
                    )
                    .await
                    .unwrap_or_default();
                    if candidates.is_empty() {
                        println!(
                            " ⚠️  No dictionary candidates found — custom definition is available."
                        );
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
                        println!(
                            " ✨ Updated candidate: 【{} ({})】",
                            dict_info.expression, dict_info.reading
                        );
                        TerminalUi::render_card(CardRenderParams {
                            rank: idx + 1,
                            sentence: &cand.sentence.text,
                            target_word: &cand.target_word,
                            reading: &dict_info.reading,
                            pitch: &dict_info.pitch_accent,
                            episode_freq: cand.episode_freq,
                            density_tier: cand.density_tier,
                            definition: &dict_info.definition,
                            known_context: &cand.known_context_words,
                            unknown_context: &cand.unknown_context_words,
                            ignored_context: &cand.ignored_context_words,
                            ai_warning: ai_analysis
                                .as_ref()
                                .and_then(|r| r.parsing_warning.as_deref()),
                            translations: None,
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

                        let image_path = cfg
                            .media_dir
                            .join(format!("{}_{}.jpg", cand.target_word, cand.sentence.index));
                        let _ = MediaExtractor::extract_screenshot(
                            video_path,
                            cand.sentence.start_ms,
                            &image_path,
                        );

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

                        let _ = db.add_known_words_with_source(
                            std::slice::from_ref(&cand.target_word),
                            "mined",
                        );
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
                    'u' => {
                        let _ = db.remove_known_words(std::slice::from_ref(&cand.target_word));
                        println!(" 🔓 Target word unmarked as known (moved back to unknown)!");
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

        if user_quit {
            break;
        }

        if batch_idx + 1 < total_batches {
            let next_batch_num = batch_idx + 2;
            if !TerminalUi::ask_next_batch(next_batch_num, total_batches, remaining_lines)? {
                println!(
                    " 🚪 Finishing mining session after Batch {}/{}.",
                    batch_idx + 1,
                    total_batches
                );
                break;
            }
        }
    }

    println!(
        "\n🎉 Mining session finished! Mined {} cards.\n",
        mined_count
    );

    let unsynced = db.get_unsynced_mined_cards()?;
    if !unsynced.is_empty() {
        if anki::anki_connected(&cfg.anki_connect_url).await {
            println!(" 🔄 Auto-syncing mined cards to Anki...");
            if let Err(e) = anki::sync_to_anki(cfg, db).await {
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

async fn preload_batch_media(
    candidates_to_process: &[CandidateSentence],
    video_path: &Path,
    cfg: &AppConfig,
    db: &mut Database,
    http_client: &Arc<reqwest::Client>,
) {
    const RAW_SENSE_LIMIT: usize = 12;
    let total = candidates_to_process.len() as u64;

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
        .filter(|w| {
            db.get_cached_definition(w).unwrap_or(None).is_none()
                || db.get_cached_candidates(w).unwrap_or(None).is_none()
        })
        .collect();

    let cached_count = (candidates_to_process.len() - uncached_words.len()) as u64;
    pb1.set_position(cached_count);

    if !uncached_words.is_empty() {
        let (tx, mut rx) =
            tokio::sync::mpsc::channel::<(dict::LookupResult, Vec<dict::LookupResult>)>(100);
        let semaphore = Arc::new(tokio::sync::Semaphore::new(5));

        for word in uncached_words {
            let sem = Arc::clone(&semaphore);
            let client = Arc::clone(http_client);
            let tx = tx.clone();

            let max_senses = cfg.max_definition_senses.max(RAW_SENSE_LIMIT);
            let max_glosses = cfg.max_glosses_per_sense;
            tokio::spawn(async move {
                let _permit = sem.acquire().await;
                let dict_res =
                    DictionaryService::lookup_with_limits(&client, &word, max_senses, max_glosses)
                        .await;
                let cands_res = DictionaryService::lookup_all_candidates(
                    &client,
                    &word,
                    max_senses,
                    max_glosses,
                )
                .await
                .unwrap_or_default();
                if let Ok(dict_res) = dict_res {
                    let _ = tx.send((dict_res, cands_res)).await;
                }
            });
        }
        drop(tx);

        while let Some((dict_res, cands_res)) = rx.recv().await {
            if !dict::is_placeholder_definition(&dict_res.definition)
                && dict_res.definition != "No dictionary definition found"
            {
                let _ = db.cache_definition(
                    &dict_res.expression,
                    &dict_res.reading,
                    &dict_res.definition,
                    &dict_res.pitch_accent,
                );
            }
            if !cands_res.is_empty() {
                let _ = db.cache_candidates(&dict_res.expression, &cands_res);
            }
            pb1.inc(1);
        }
    }
    pb1.finish();
    println!("\n");

    let pb2 = indicatif::ProgressBar::new(total);
    pb2.set_style(
        indicatif::ProgressStyle::default_bar()
            .template(" ℹ [2/4] Audio Preview Clips (.opus)   [{bar:35.magenta/blue}] {pos}/{len} ({percent}%)")
            .unwrap()
            .progress_chars("█▓▒░"),
    );
    candidates_to_process.par_iter().for_each(|cand| {
        let audio_path = cfg
            .media_dir
            .join(format!("{}_{}.opus", cand.target_word, cand.sentence.index));
        if !audio_path.exists() {
            let _ = MediaExtractor::extract_preview_audio(
                video_path,
                cand.sentence.start_ms,
                cand.sentence.end_ms,
                &audio_path,
            );
        }
        pb2.inc(1);
    });
    pb2.finish();
    println!("\n");

    let pb3 = indicatif::ProgressBar::new(total);
    pb3.set_style(
        indicatif::ProgressStyle::default_bar()
            .template(" ℹ [3/4] Screenshots 360p (.jpg)       [{bar:35.yellow/blue}] {pos}/{len} ({percent}%)")
            .unwrap()
            .progress_chars("█▓▒░"),
    );
    candidates_to_process.par_iter().for_each(|cand| {
        let image_path = cfg
            .media_dir
            .join(format!("{}_{}.jpg", cand.target_word, cand.sentence.index));
        if !image_path.exists() {
            let _ =
                MediaExtractor::extract_screenshot(video_path, cand.sentence.start_ms, &image_path);
        }
        pb3.inc(1);
    });
    pb3.finish();
    println!("\n");
}

async fn resolve_dict_info(
    cand: &CandidateSentence,
    cfg: &AppConfig,
    db: &mut Database,
    http_client: &Arc<reqwest::Client>,
) -> Result<dict::LookupResult> {
    const RAW_SENSE_LIMIT: usize = 12;
    let context_hint = dict::context_hint(&cand.sentence.text, &cand.target_word);
    let cached = db.get_cached_definition(&cand.target_word)?;
    let needs_context_refresh = context_hint.is_some_and(|hint| {
        cached
            .as_ref()
            .is_some_and(|res| !dict::has_contextual_sense(&res.1, hint))
    });
    let (reading, raw_definition, pitch_accent) = match (cached, needs_context_refresh) {
        (Some(res), false) => res,
        (_, true) | (None, false) => {
            let res = DictionaryService::lookup_with_limits(
                http_client,
                &cand.target_word,
                cfg.max_definition_senses.max(RAW_SENSE_LIMIT),
                cfg.max_glosses_per_sense,
            )
            .await?;
            if !dict::is_placeholder_definition(&res.definition)
                && res.definition != "No dictionary definition found"
            {
                db.cache_definition(
                    &res.expression,
                    &res.reading,
                    &res.definition,
                    &res.pitch_accent,
                )?;
            }
            (res.reading, res.definition, res.pitch_accent)
        }
    };

    Ok(dict::LookupResult {
        expression: cand.target_word.clone(),
        reading,
        definition: dict::format_contextual_definition(
            &raw_definition,
            context_hint,
            cfg.max_definition_senses,
            cfg.max_glosses_per_sense,
        ),
        pitch_accent,
    })
}

async fn apply_ai_gloss_or_candidate(
    ai_analysis: Option<&ai::AiAnalysisResult>,
    dict_info: &mut dict::LookupResult,
    cand: &CandidateSentence,
    cfg: &AppConfig,
    db: &Database,
    http_client: &Arc<reqwest::Client>,
) {
    if let Some(res) = ai_analysis {
        if let Some(ref custom_sug) = res.custom_definition_suggestion {
            dict_info.definition = format!("1. [AI Suggestion] {}", custom_sug);
        } else if let Some(cand_idx) = res.recommended_candidate_index {
            let candidates = DictionaryService::lookup_all_candidates_cached(
                http_client,
                Some(db),
                &cand.target_word,
                dict::LookupLimits {
                    max_senses: cfg.max_definition_senses,
                    max_glosses: cfg.max_glosses_per_sense,
                },
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
}

async fn align_contextual_reading(
    cand: &CandidateSentence,
    dict_info: &mut dict::LookupResult,
    cfg: &AppConfig,
    db: &mut Database,
    http_client: &Arc<reqwest::Client>,
) {
    let context_hint = dict::context_hint(&cand.sentence.text, &cand.target_word);
    if !cand.target_reading.is_empty() {
        let all_cands = DictionaryService::lookup_all_candidates_cached(
            http_client,
            Some(db),
            &cand.target_word,
            dict::LookupLimits {
                max_senses: cfg.max_definition_senses,
                max_glosses: cfg.max_glosses_per_sense,
            },
        )
        .await
        .unwrap_or_default();

        if let Some(matched_cand) = all_cands.iter().find(|c| c.reading == cand.target_reading) {
            let current_def = dict_info.definition.clone();
            *dict_info = matched_cand.clone();
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

        let _ = db.cache_definition(
            &dict_info.expression,
            &dict_info.reading,
            &dict_info.definition,
            &dict_info.pitch_accent,
        );
    }
}
