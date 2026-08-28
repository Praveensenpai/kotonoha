use anyhow::Result;
use console::style;
use std::path::Path;
use std::sync::Arc;

use super::ai_batch::{collect_ai_results, prepare_ai_batch};
use super::card_actions::{
    handle_card_interaction, render_current_card, CardActionContext, CardActionResult,
};
use super::media_preload::preload_batch_media;
use crate::ai;
use crate::anki;
use crate::config::AppConfig;
use crate::db::Database;
use crate::dict::{self, DictionaryService, LookupLimits};
use crate::miner::CandidateSentence;
use crate::ui::TerminalUi;

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
        let _ = db.clean_expired_ai_cache(cfg.ai.ai_cache_ttl_minutes);

        let ai_prep = prepare_ai_batch(&candidates_to_process, cfg, db, &http_client);

        if !candidates_to_process.is_empty() {
            preload_batch_media(&candidates_to_process, video_path, cfg, db, &http_client).await;
        }

        let ai_results_map =
            collect_ai_results(ai_prep, &candidates_to_process, cfg, db, ai_start).await;

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

            align_contextual_reading(cand, &mut dict_info, cfg, db, &http_client).await;

            apply_ai_gloss_or_candidate(ai_analysis, &mut dict_info, cand, cfg, db, &http_client)
                .await;

            let is_ai_selected = ai_analysis.is_some_and(|r| {
                r.custom_definition_suggestion.is_some()
                    || r.recommended_candidate_index.is_some()
                    || r.recommended_sense_index.is_some()
            });

            let ctx = CardActionContext {
                card_idx: idx,
                cand,
                dict_info: &mut dict_info,
                ai_analysis,
                is_ai_selected,
                video_path,
                cfg,
                db,
                http_client: &http_client,
            };

            render_current_card(&ctx);

            match handle_card_interaction(ctx).await? {
                CardActionResult::Mined => mined_count += 1,
                CardActionResult::Known => known_count += 1,
                CardActionResult::UnmarkedKnown => {}
                CardActionResult::Ignored => ignored_count += 1,
                CardActionResult::Skipped => skipped_count += 1,
                CardActionResult::Quit => {
                    user_quit = true;
                    break;
                }
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
        if anki::anki_connected(&cfg.anki.connect_url).await {
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
            let cands = DictionaryService::lookup_all_candidates_cached(
                http_client,
                Some(db),
                &cand.target_word,
                LookupLimits {
                    max_senses: cfg.dict.max_definition_senses.max(RAW_SENSE_LIMIT),
                    max_glosses: cfg.dict.max_glosses_per_sense,
                },
            )
            .await
            .unwrap_or_default();

            if let Some(first) = cands.into_iter().next() {
                if !dict::is_placeholder_definition(&first.definition)
                    && first.definition != "No dictionary definition found"
                {
                    db.cache_definition(
                        &first.expression,
                        &first.reading,
                        &first.definition,
                        &first.pitch_accent,
                    )?;
                }
                (first.reading, first.definition, first.pitch_accent)
            } else {
                let res = DictionaryService::lookup_with_limits(
                    http_client,
                    &cand.target_word,
                    cfg.dict.max_definition_senses.max(RAW_SENSE_LIMIT),
                    cfg.dict.max_glosses_per_sense,
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
        }
    };

    Ok(dict::LookupResult {
        expression: cand.target_word.clone(),
        reading,
        definition: dict::format_contextual_definition(
            &raw_definition,
            context_hint,
            cfg.dict.max_definition_senses,
            cfg.dict.max_glosses_per_sense,
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
                LookupLimits {
                    max_senses: cfg.dict.max_definition_senses,
                    max_glosses: cfg.dict.max_glosses_per_sense,
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
                *dict_info = rec_cand.clone();
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
            LookupLimits {
                max_senses: cfg.dict.max_definition_senses,
                max_glosses: cfg.dict.max_glosses_per_sense,
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
                    cfg.dict.max_definition_senses,
                    cfg.dict.max_glosses_per_sense,
                );
            }
            let _ = db.cache_definition(
                &dict_info.expression,
                &dict_info.reading,
                &dict_info.definition,
                &dict_info.pitch_accent,
            );
        }
    }
}
