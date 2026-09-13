use anyhow::Result;
use std::path::Path;
use std::process::Child;

use crate::db::Database;
use crate::media::MediaExtractor;

pub fn play_sentence_audio(video_path: &Path, start_ms: u64, end_ms: u64) -> Option<Child> {
    MediaExtractor::play_subtitle_segment(video_path, start_ms, end_ms)
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
