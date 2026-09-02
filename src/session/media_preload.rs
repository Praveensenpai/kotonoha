use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::path::Path;
use std::sync::Arc;

use crate::config::AppConfig;
use crate::db::Database;
use crate::dict::{self, DictionaryService};
use crate::media::MediaExtractor;
use crate::miner::CandidateSentence;

const RAW_SENSE_LIMIT: usize = 12;

pub async fn preload_batch_media(
    candidates_to_process: &[CandidateSentence],
    video_path: &Path,
    cfg: &AppConfig,
    db: &mut Database,
    http_client: &Arc<reqwest::Client>,
) {
    let total = candidates_to_process.len() as u64;

    // 1. Prefetch definitions & pitch accents
    let pb1 = ProgressBar::new(total);
    pb1.set_style(
        ProgressStyle::default_bar()
            .template(" ℹ [1/4] Definitions & Pitch Accents  [{bar:35.cyan/blue}] {pos}/{len} ({percent}%)")
            .unwrap()
            .progress_chars("█▓▒░"),
    );

    let mut uncached_words = Vec::new();
    for c in candidates_to_process {
        let w = &c.target_word;
        if db.get_cached_definition(w).await.unwrap_or(None).is_none()
            || db.get_cached_candidates(w).await.unwrap_or(None).is_none()
        {
            uncached_words.push(w.clone());
        }
    }

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

            let max_senses = cfg.dict.max_definition_senses.max(RAW_SENSE_LIMIT);
            let max_glosses = cfg.dict.max_glosses_per_sense;
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
                ).await;
            }
            if !cands_res.is_empty() {
                let _ = db.cache_candidates(&dict_res.expression, &cands_res).await;
            }
            pb1.inc(1);
        }
    }
    pb1.finish();
    println!("\n");

    // 2. Prefetch Opus preview audio clips
    let pb2 = ProgressBar::new(total);
    pb2.set_style(
        ProgressStyle::default_bar()
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

    // 3. Prefetch 360p screenshots
    let pb3 = ProgressBar::new(total);
    pb3.set_style(
        ProgressStyle::default_bar()
            .template(" ℹ [3/4] Screenshots 360p (.jpg)       [{bar:35.yellow/blue}] {pos}/{len} ({percent}%)")
            .unwrap()
            .progress_chars("█▓▒░"),
    );
    candidates_to_process.par_iter().for_each(|cand| {
        let image_path = cfg
            .media_dir
            .join(format!("{}_{}.jpg", cand.target_word, cand.sentence.index));
        if !image_path.exists() {
            let _ = MediaExtractor::extract_screenshot_with_index(
                video_path,
                cand.sentence.start_ms,
                Some(cand.sentence.index),
                &image_path,
            );
        }
        pb3.inc(1);
    });
    pb3.finish();
    println!("\n");
}
