use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinHandle;

use crate::ai::{self, GeminiAiService};
use crate::config::AppConfig;
use crate::db::Database;
use crate::dict::{self, DictionaryService, LookupLimits};
use crate::miner::CandidateSentence;

pub struct AiBatchPreparation {
    pub cached_results: Vec<ai::AiAnalysisResult>,
    pub task_handle: Option<JoinHandle<Result<Vec<ai::AiAnalysisResult>>>>,
}

pub async fn prepare_ai_batch(
    candidates_to_process: &[CandidateSentence],
    cfg: &AppConfig,
    db: &Database,
    http_client: &Arc<reqwest::Client>,
) -> AiBatchPreparation {
    let mut cached_results = Vec::new();
    let mut uncached_card_targets = Vec::new();

    if cfg.ai.enable_ai {
        for (idx, cand) in candidates_to_process.iter().enumerate() {
            if let Ok(Some(cached_res)) = db
                .get_cached_ai_analysis(crate::db::GetCachedAiParams {
                    sentence: &cand.sentence.text,
                    target_word: &cand.target_word,
                    model: &cfg.ai.gemini_model,
                    card_index: idx,
                    ttl_minutes: cfg.ai.ai_cache_ttl_minutes,
                })
                .await
            {
                cached_results.push(cached_res);
            } else {
                uncached_card_targets.push((
                    idx,
                    cand.sentence.text.clone(),
                    cand.target_word.clone(),
                    cand.target_reading.clone(),
                    cand.series_title.clone(),
                    cand.before_context.clone(),
                    cand.after_context.clone(),
                ));
            }
        }
    }

    let task_handle = if cfg.ai.enable_ai
        && cfg.ai.has_valid_api_key()
        && !uncached_card_targets.is_empty()
    {
        if let Some(ref api_key) = cfg.ai.gemini_api_key {
            let api_key = api_key.clone();
            let model = cfg.ai.gemini_model.clone();
            let client = Arc::clone(http_client);
            let db_clone = db.clone();
            let max_senses = cfg.dict.max_definition_senses;
            let max_glosses = cfg.dict.max_glosses_per_sense;
            let card_targets = uncached_card_targets;
            let cfg_ai_batch_size = cfg.ai.ai_batch_size;

            Some(tokio::spawn(async move {
                let semaphore = Arc::new(tokio::sync::Semaphore::new(10));
                let mut all_results = Vec::new();
                let ai_batch_size = cfg_ai_batch_size.max(1);

                for chunk in card_targets.chunks(ai_batch_size) {
                    let mut lookup_futures = Vec::new();

                    for (
                        idx,
                        sentence_text,
                        target_word,
                        target_reading,
                        series_title,
                        before_ctx,
                        after_ctx,
                    ) in chunk
                    {
                        let sem = Arc::clone(&semaphore);
                        let client = Arc::clone(&client);
                        let db_for_cand = db_clone.clone();
                        let target = target_word.clone();
                        let reading = target_reading.clone();
                        let sentence = sentence_text.clone();
                        let series = series_title.clone();
                        let before = before_ctx.clone();
                        let after = after_ctx.clone();
                        let idx = *idx;

                        lookup_futures.push(tokio::spawn(async move {
                            let target_for_lookup = target.clone();
                            let mut candidates = match tokio::time::timeout(
                                std::time::Duration::from_secs(4),
                                async move {
                                    let _permit = sem.acquire().await;
                                    DictionaryService::lookup_all_candidates_cached(
                                        &client,
                                        Some(&db_for_cand),
                                        &target_for_lookup,
                                        LookupLimits {
                                            max_senses,
                                            max_glosses,
                                        },
                                    )
                                    .await
                                },
                            )
                            .await
                            {
                                Ok(Ok(res)) => res,
                                _ => Vec::new(),
                            };

                            dict::sort_candidates_for_context(&mut candidates, &target, &reading);

                            (
                                idx,
                                sentence,
                                target,
                                reading,
                                candidates,
                                series,
                                before,
                                after,
                            )
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
                        .map(
                            |(
                                idx,
                                sentence,
                                target_word,
                                target_reading,
                                candidates,
                                series_title,
                                before_context,
                                after_context,
                            )| {
                                ai::CardBatchInput {
                                    card_index: *idx,
                                    sentence: sentence.as_str(),
                                    target_word: target_word.as_str(),
                                    target_reading: target_reading.as_str(),
                                    candidates: candidates.as_slice(),
                                    series_title: series_title.as_deref(),
                                    before_context: before_context.as_slice(),
                                    after_context: after_context.as_slice(),
                                }
                            },
                        )
                        .collect();

                    match GeminiAiService::analyze_batch(&client, &api_key, &model, &inputs).await {
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

    AiBatchPreparation {
        cached_results,
        task_handle,
    }
}

pub async fn collect_ai_results(
    prep: AiBatchPreparation,
    candidates: &[CandidateSentence],
    cfg: &AppConfig,
    db: &Database,
    ai_start: std::time::Instant,
) -> HashMap<usize, ai::AiAnalysisResult> {
    let cached_count = prep.cached_results.len();

    match prep.task_handle {
        Some(handle) => {
            println!(
                " 🤖 [4/4] Gemini AI Context Analysis ({}) ...",
                cfg.ai.gemini_model
            );
            match handle.await {
                Ok(Ok(fresh_results)) => {
                    let elapsed = ai_start.elapsed().as_secs_f64();
                    println!(
                        " ✔ Gemini AI analysis ready ({} from cache, {} fetched from API in {:.2}s).\n",
                        cached_count,
                        fresh_results.len(),
                        elapsed
                    );

                    for res in &fresh_results {
                        let cand = &candidates[res.card_index];
                        let _ = db
                            .cache_ai_analysis(
                                &cand.sentence.text,
                                &cand.target_word,
                                &cfg.ai.gemini_model,
                                res,
                            )
                            .await;
                    }

                    let mut merged = prep.cached_results;
                    merged.extend(fresh_results);
                    merged.into_iter().map(|r| (r.card_index, r)).collect()
                }
                Ok(Err(e)) => {
                    let elapsed = ai_start.elapsed().as_secs_f64();
                    eprintln!(
                        " ⚠️ Gemini AI batch analysis failed after {:.2}s: {e}\n",
                        elapsed
                    );
                    prep.cached_results
                        .into_iter()
                        .map(|r| (r.card_index, r))
                        .collect()
                }
                Err(e) => {
                    eprintln!(" ⚠️ Gemini AI task join error: {e}\n");
                    prep.cached_results
                        .into_iter()
                        .map(|r| (r.card_index, r))
                        .collect()
                }
            }
        }
        None => {
            if cached_count > 0 {
                let elapsed = ai_start.elapsed().as_secs_f64();
                println!(
                    " ✔ Gemini AI analysis ready (all {} card(s) loaded from cache in {:.2}s).\n",
                    cached_count, elapsed
                );
            }
            prep.cached_results
                .into_iter()
                .map(|r| (r.card_index, r))
                .collect()
        }
    }
}
