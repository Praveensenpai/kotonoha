use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

use crate::ai::AiAnalysisResult;
use crate::config::AppConfig;
use crate::db::{Database, SaveMinedCardParams};
use crate::dict::{self, DictionaryService, LookupLimits, LookupResult};
use crate::media::MediaExtractor;
use crate::miner::CandidateSentence;
use crate::ui::{CardRenderParams, TerminalUi};

pub enum CardActionResult {
    Mined,
    Known,
    UnmarkedKnown,
    Ignored,
    Skipped,
    Quit,
}

pub struct CardActionContext<'a> {
    pub card_idx: usize,
    pub cand: &'a CandidateSentence,
    pub dict_info: &'a mut LookupResult,
    pub ai_analysis: Option<&'a AiAnalysisResult>,
    pub is_ai_selected: bool,
    pub video_path: &'a Path,
    pub cfg: &'a AppConfig,
    pub db: &'a mut Database,
    pub http_client: &'a Arc<reqwest::Client>,
}

pub async fn handle_card_interaction(mut ctx: CardActionContext<'_>) -> Result<CardActionResult> {
    let audio_path = ctx.cfg.media_dir.join(format!(
        "{}_{}.opus",
        ctx.cand.target_word, ctx.cand.sentence.index
    ));

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
            println!(" ✍️ Fetching dictionary candidates for furigana reading selection...");
            let candidates = DictionaryService::lookup_all_candidates_cached(
                ctx.http_client,
                Some(ctx.db),
                &ctx.cand.target_word,
                LookupLimits {
                    max_senses: ctx.cfg.dict.max_definition_senses,
                    max_glosses: ctx.cfg.dict.max_glosses_per_sense,
                },
            )
            .await
            .unwrap_or_default();

            if let Ok(new_reading) = TerminalUi::select_or_edit_reading(
                &ctx.dict_info.reading,
                &ctx.cand.target_reading,
                &candidates,
            ) {
                ctx.dict_info.reading = new_reading;
                println!(
                    " ✨ Updated furigana reading: 【{} ({})】",
                    ctx.dict_info.expression, ctx.dict_info.reading
                );
                render_current_card(&ctx);
            }
            continue;
        }

        if action == 'c' {
            println!(" 🔍 Fetching dictionary candidates...");
            let candidates = DictionaryService::lookup_all_candidates_cached(
                ctx.http_client,
                Some(ctx.db),
                &ctx.cand.target_word,
                LookupLimits {
                    max_senses: ctx.cfg.dict.max_definition_senses,
                    max_glosses: ctx.cfg.dict.max_glosses_per_sense,
                },
            )
            .await
            .unwrap_or_default();
            if candidates.is_empty() {
                println!(" ⚠️  No dictionary candidates found — custom definition is available.");
            }
            if let Ok(chosen) = TerminalUi::select_candidate_or_custom(
                &candidates,
                &ctx.cand.target_word,
                &ctx.dict_info.reading,
                &ctx.dict_info.pitch_accent,
                ctx.ai_analysis,
            ) {
                let chosen_is_ai = ctx.ai_analysis.is_some_and(|r| {
                    if let Some(ref sug) = r.custom_definition_suggestion {
                        if chosen.definition == *sug
                            || chosen.definition == format!("1. [AI Suggestion] {}", sug)
                        {
                            return true;
                        }
                    }
                    if let Some(idx) = r.recommended_candidate_index {
                        if candidates
                            .get(idx)
                            .map(|c| (&c.expression, &c.reading, &c.definition))
                            == Some((&chosen.expression, &chosen.reading, &chosen.definition))
                        {
                            return true;
                        }
                    }
                    false
                });
                ctx.is_ai_selected = chosen_is_ai;
                *ctx.dict_info = chosen;
                let _ = ctx
                    .db
                    .cache_definition(
                        &ctx.dict_info.expression,
                        &ctx.dict_info.reading,
                        &ctx.dict_info.definition,
                        &ctx.dict_info.pitch_accent,
                    )
                    .await;
                println!(
                    " ✨ Updated candidate: 【{} ({})】",
                    ctx.dict_info.expression, ctx.dict_info.reading
                );
                render_current_card(&ctx);
            }
            continue;
        }

        if let Some(mut child) = audio_child.take() {
            let _ = child.kill();
        }

        match action {
            'y' => {
                let senses = dict::parse_senses(&ctx.dict_info.definition);
                let chosen_sense = TerminalUi::select_sense(&senses, &ctx.cand.target_word)?;
                let Some(chosen) = chosen_sense else {
                    println!(" ℹ Mining canceled — returning to card menu.");
                    continue;
                };
                ctx.dict_info.definition = chosen;

                let image_path = ctx.cfg.media_dir.join(format!(
                    "{}_{}.jpg",
                    ctx.cand.target_word, ctx.cand.sentence.index
                ));
                let mid_ms = ctx.cand.sentence.start_ms
                    + (ctx
                        .cand
                        .sentence
                        .end_ms
                        .saturating_sub(ctx.cand.sentence.start_ms))
                        / 2;
                let _ = MediaExtractor::extract_screenshot_with_index(
                    ctx.video_path,
                    mid_ms,
                    Some(ctx.cand.sentence.index),
                    &image_path,
                );

                let eng_nat = ctx.ai_analysis.and_then(|r| r.english_natural.as_deref());
                let eng_lit = ctx.ai_analysis.and_then(|r| r.english_literal.as_deref());
                let kan_nat = ctx.ai_analysis.and_then(|r| r.kannada_natural.as_deref());
                let kan_lit = ctx.ai_analysis.and_then(|r| r.kannada_literal.as_deref());

                ctx.db
                    .save_mined_card(SaveMinedCardParams {
                        sentence: &ctx.cand.sentence.text,
                        target_word: &ctx.cand.target_word,
                        reading: &ctx.dict_info.reading,
                        pitch_accent: &ctx.dict_info.pitch_accent,
                        definition: &ctx.dict_info.definition,
                        audio_path: Some(&audio_path.to_string_lossy()),
                        image_path: Some(&image_path.to_string_lossy()),
                        english_natural: eng_nat,
                        english_literal: eng_lit,
                        kannada_natural: kan_nat,
                        kannada_literal: kan_lit,
                    })
                    .await?;

                let _ = ctx
                    .db
                    .add_known_words_with_source(
                        std::slice::from_ref(&ctx.cand.target_word),
                        "mined",
                    )
                    .await;
                println!(" ✔ Card mined successfully!");
                if let Some(mut child) = audio_child.take() {
                    let _ = child.kill();
                }
                return Ok(CardActionResult::Mined);
            }
            'k' => {
                let _ = ctx
                    .db
                    .add_known_words(std::slice::from_ref(&ctx.cand.target_word))
                    .await;
                println!(" 🧠 Target word marked as known!");
                if let Some(mut child) = audio_child.take() {
                    let _ = child.kill();
                }
                return Ok(CardActionResult::Known);
            }
            'u' => {
                let _ = ctx
                    .db
                    .remove_known_words(std::slice::from_ref(&ctx.cand.target_word))
                    .await;
                println!(" 🔓 Target word unmarked as known (moved back to unknown)!");
                if let Some(mut child) = audio_child.take() {
                    let _ = child.kill();
                }
                return Ok(CardActionResult::UnmarkedKnown);
            }
            'i' => {
                let _ = ctx.db.add_ignored_word(&ctx.cand.target_word).await;
                println!(" 🚫 Target word ignored.");
                if let Some(mut child) = audio_child.take() {
                    let _ = child.kill();
                }
                return Ok(CardActionResult::Ignored);
            }
            'n' => {
                println!(" ⏭️ Card skipped.");
                if let Some(mut child) = audio_child.take() {
                    let _ = child.kill();
                }
                return Ok(CardActionResult::Skipped);
            }
            'q' => {
                println!(" 🚪 Exiting mining session.");
                if let Some(mut child) = audio_child.take() {
                    let _ = child.kill();
                }
                return Ok(CardActionResult::Quit);
            }
            _ => {
                if let Some(mut child) = audio_child.take() {
                    let _ = child.kill();
                }
                return Ok(CardActionResult::Skipped);
            }
        }
    }
}

pub fn render_current_card(ctx: &CardActionContext<'_>) {
    TerminalUi::render_card(CardRenderParams {
        rank: ctx.card_idx + 1,
        sentence: &ctx.cand.sentence.text,
        target_word: &ctx.cand.target_word,
        reading: &ctx.dict_info.reading,
        pitch: &ctx.dict_info.pitch_accent,
        episode_freq: ctx.cand.episode_freq,
        density_tier: ctx.cand.density_tier,
        definition: &ctx.dict_info.definition,
        known_context: &ctx.cand.known_context_words,
        unknown_context: &ctx.cand.unknown_context_words,
        ignored_context: &ctx.cand.ignored_context_words,
        ai_warning: ctx
            .ai_analysis
            .as_ref()
            .and_then(|r| r.parsing_warning.as_deref()),
        is_ai_selected: ctx.is_ai_selected,
        translations: None,
    });
}
