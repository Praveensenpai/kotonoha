use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Child;

use super::model::ExplorerSentence;
use crate::ai::AiAnalysisResult;
use crate::config::AppConfig;
use crate::db::{Database, SaveMinedCardParams};
use crate::dict::LookupResult;
use crate::media::MediaExtractor;

pub struct MineWordParams<'a> {
    pub explorer_sentence: &'a ExplorerSentence,
    pub word_index: usize,
    pub dict_info: &'a LookupResult,
    pub ai_analysis: Option<&'a AiAnalysisResult>,
    pub video_path: Option<&'a Path>,
    pub cfg: &'a AppConfig,
    pub db: &'a Database,
}

pub fn play_sentence_audio(video_path: &Path, start_ms: u64, end_ms: u64) -> Option<Child> {
    MediaExtractor::play_subtitle_segment(video_path, start_ms, end_ms)
}

fn prepare_media_files(
    video_path: Option<&Path>,
    target_word: &str,
    sentence_idx: usize,
    start_ms: u64,
    end_ms: u64,
    media_dir: &Path,
) -> (Option<PathBuf>, Option<PathBuf>) {
    let Some(vid) = video_path else {
        return (None, None);
    };

    let audio_path = media_dir.join(format!("{target_word}_{sentence_idx}.opus"));
    if !audio_path.exists() {
        let _ = MediaExtractor::extract_preview_audio(vid, start_ms, end_ms, &audio_path);
    }

    let image_path = media_dir.join(format!("{target_word}_{sentence_idx}.jpg"));
    if !image_path.exists() {
        let mid_ms = start_ms + (end_ms.saturating_sub(start_ms)) / 2;
        let _ = MediaExtractor::extract_screenshot_with_index(
            vid,
            mid_ms,
            Some(sentence_idx),
            &image_path,
        );
    }

    let a_opt = if audio_path.exists() {
        Some(audio_path)
    } else {
        None
    };
    let i_opt = if image_path.exists() {
        Some(image_path)
    } else {
        None
    };
    (a_opt, i_opt)
}

pub async fn mine_single_word(p: MineWordParams<'_>) -> Result<i64> {
    let target = p
        .explorer_sentence
        .unknowns
        .get(p.word_index)
        .ok_or_else(|| anyhow::anyhow!("Unknown word index out of bounds"))?;

    let s = &p.explorer_sentence.sentence;
    let (audio_path, image_path) = prepare_media_files(
        p.video_path,
        &target.dictionary_form,
        s.index,
        s.start_ms,
        s.end_ms,
        &p.cfg.media_dir,
    );

    let audio_str = audio_path.as_ref().map(|p| p.to_string_lossy());
    let image_str = image_path.as_ref().map(|p| p.to_string_lossy());

    let eng_nat = p.ai_analysis.and_then(|r| r.english_natural.as_deref());
    let eng_lit = p.ai_analysis.and_then(|r| r.english_literal.as_deref());
    let kan_nat = p.ai_analysis.and_then(|r| r.kannada_natural.as_deref());
    let kan_lit = p.ai_analysis.and_then(|r| r.kannada_literal.as_deref());

    let definition = p
        .ai_analysis
        .and_then(|r| r.custom_definition_suggestion.as_deref())
        .filter(|sug| !sug.is_empty())
        .unwrap_or(&p.dict_info.definition);

    let card_id =
        p.db.save_mined_card(SaveMinedCardParams {
            sentence: &s.text,
            target_word: &target.dictionary_form,
            reading: &p.dict_info.reading,
            pitch_accent: &p.dict_info.pitch_accent,
            definition,
            audio_path: audio_str.as_deref(),
            image_path: image_str.as_deref(),
            english_natural: eng_nat,
            english_literal: eng_lit,
            kannada_natural: kan_nat,
            kannada_literal: kan_lit,
        })
        .await?;

    let _ =
        p.db.add_known_words_with_source(std::slice::from_ref(&target.dictionary_form), "mined")
            .await;

    Ok(card_id)
}

pub struct MineAllParams<'a> {
    pub explorer_sentence: &'a ExplorerSentence,
    pub video_path: Option<&'a Path>,
    pub cfg: &'a AppConfig,
    pub db: &'a Database,
    pub http_client: &'a reqwest::Client,
}

pub async fn mine_all_unknowns_in_sentence(p: MineAllParams<'_>) -> Result<Vec<String>> {
    let s = &p.explorer_sentence.sentence;
    if p.explorer_sentence.unknowns.is_empty() {
        return Ok(Vec::new());
    }

    let first_target = &p.explorer_sentence.unknowns[0].dictionary_form;
    let (audio_path, image_path) = prepare_media_files(
        p.video_path,
        first_target,
        s.index,
        s.start_ms,
        s.end_ms,
        &p.cfg.media_dir,
    );

    let audio_str = audio_path.as_ref().map(|p| p.to_string_lossy());
    let image_str = image_path.as_ref().map(|p| p.to_string_lossy());

    let mut mined_words = Vec::new();

    for target in &p.explorer_sentence.unknowns {
        let dict_info = crate::dict::DictionaryService::lookup_all_candidates_cached(
            p.http_client,
            Some(p.db),
            &target.dictionary_form,
            crate::dict::LookupLimits {
                max_senses: p.cfg.dict.max_definition_senses,
                max_glosses: p.cfg.dict.max_glosses_per_sense,
            },
        )
        .await
        .ok()
        .and_then(|cands| cands.into_iter().next())
        .unwrap_or_else(|| LookupResult {
            expression: target.dictionary_form.clone(),
            reading: target.reading.clone(),
            definition: "Unknown term".to_string(),
            pitch_accent: String::new(),
        });

        p.db.save_mined_card(SaveMinedCardParams {
            sentence: &s.text,
            target_word: &target.dictionary_form,
            reading: &dict_info.reading,
            pitch_accent: &dict_info.pitch_accent,
            definition: &dict_info.definition,
            audio_path: audio_str.as_deref(),
            image_path: image_str.as_deref(),
            english_natural: None,
            english_literal: None,
            kannada_natural: None,
            kannada_literal: None,
        })
        .await?;

        p.db.add_known_words_with_source(std::slice::from_ref(&target.dictionary_form), "mined")
            .await?;

        mined_words.push(target.dictionary_form.clone());
    }

    Ok(mined_words)
}

pub async fn mark_word_known(word: &str, db: &Database) -> Result<()> {
    db.add_known_words(std::slice::from_ref(&word.to_string()))
        .await?;
    Ok(())
}

pub async fn mark_all_known(words: &[String], db: &Database) -> Result<()> {
    db.add_known_words(words).await?;
    Ok(())
}

pub async fn mark_word_ignored(word: &str, db: &Database) -> Result<()> {
    db.add_ignored_word(word).await?;
    Ok(())
}
