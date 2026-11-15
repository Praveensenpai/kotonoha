use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

use crate::ai::AiAnalysisResult;
use crate::config::AppConfig;
use crate::db::{Database, SaveMinedCardParams};
use crate::dict::{self, DictionaryService, LookupLimits, LookupResult};
use crate::media::MediaExtractor;
use crate::miner::CandidateSentence;
use crate::ui::card_dashboard::{DashboardAction, DashboardParams};
use crate::ui::TerminalUi;

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
    pub total_cards: usize,
    pub mined_count: usize,
    pub known_count: usize,
    pub skipped_count: usize,
    pub ignored_count: usize,
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
    let media_stem = MediaExtractor::card_media_stem(
        &ctx.cand.target_word,
        ctx.video_path,
        ctx.cand.sentence.start_ms,
        ctx.cand.sentence.index,
    );
    let audio_path = ctx.cfg.media_dir.join(format!("{}.opus", media_stem));

    if !audio_path.exists() && !ctx.video_path.as_os_str().is_empty() {
        let _ = MediaExtractor::extract_preview_audio(
            ctx.video_path,
            ctx.cand.sentence.start_ms,
            ctx.cand.sentence.end_ms,
            &audio_path,
        );
    }

    let all_candidates = DictionaryService::lookup_all_candidates_cached(
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

    let ai_suggestion: Option<String> = ctx.ai_analysis.and_then(|r| {
        if let Some(ref sug) = r.custom_definition_suggestion {
            Some(sug.clone())
        } else if let Some(cand_idx) = r.recommended_candidate_index {
            all_candidates.get(cand_idx).map(|c| c.definition.clone())
        } else {
            None
        }
    });

    let action = TerminalUi::run_card_dashboard(DashboardParams {
        card_index: ctx.card_idx,
        total_cards: ctx.total_cards,
        mined_count: ctx.mined_count,
        known_count: ctx.known_count,
        skipped_count: ctx.skipped_count,
        ignored_count: ctx.ignored_count,
        cand: ctx.cand,
        dict_info: ctx.dict_info,
        ai_suggestion: ai_suggestion.as_deref(),
        is_ai_selected: ctx.is_ai_selected,
        audio_path: if audio_path.exists() {
            Some(&audio_path)
        } else {
            None
        },
        all_candidates,
    })?;

    match action {
        DashboardAction::Mine => {
            save_card_on_mine(&mut ctx, &media_stem, &audio_path).await?;
            Ok(CardActionResult::Mined)
        }
        DashboardAction::Known => {
            ctx.db
                .add_known_words(std::slice::from_ref(&ctx.cand.target_word))
                .await?;
            Ok(CardActionResult::Known)
        }
        DashboardAction::UnmarkKnown => {
            ctx.db
                .remove_known_words(std::slice::from_ref(&ctx.cand.target_word))
                .await?;
            Ok(CardActionResult::UnmarkedKnown)
        }
        DashboardAction::Ignore => {
            let _ = ctx.db.add_ignored_word(&ctx.cand.target_word).await;
            Ok(CardActionResult::Ignored)
        }
        DashboardAction::Skip => Ok(CardActionResult::Skipped),
        DashboardAction::Quit => Ok(CardActionResult::Quit),
    }
}

async fn save_card_on_mine(
    ctx: &mut CardActionContext<'_>,
    media_stem: &str,
    audio_path: &Path,
) -> Result<()> {
    let senses = dict::parse_senses(&ctx.dict_info.definition);
    let chosen_sense = if senses.len() <= 1 {
        senses.first().cloned()
    } else {
        TerminalUi::select_sense(&senses, &ctx.cand.target_word)?
    };

    if let Some(chosen) = chosen_sense {
        ctx.dict_info.definition = chosen;
    }

    let image_path = ctx.cfg.media_dir.join(format!("{}.jpg", media_stem));
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
        .add_known_words_with_source(std::slice::from_ref(&ctx.cand.target_word), "mined")
        .await;

    Ok(())
}
