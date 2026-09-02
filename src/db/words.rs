use anyhow::Result;
use sea_orm::{
    ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set,
    sea_query::OnConflict,
};
use std::collections::HashSet;

use super::entities::*;
use super::Database;

impl Database {
    pub async fn get_known_words(&self) -> Result<HashSet<String>> {
        let items = KnownWords::find().all(&self.conn).await?;
        Ok(items.into_iter().map(|m| m.word).collect())
    }

    pub async fn get_known_words_by_source(&self, source: &str) -> Result<HashSet<String>> {
        let items = KnownWords::find()
            .filter(known_words::Column::Source.eq(source))
            .all(&self.conn)
            .await?;
        Ok(items.into_iter().map(|m| m.word).collect())
    }

    pub async fn add_known_words(&self, words: &[String]) -> Result<usize> {
        self.add_known_words_with_source(words, "known").await
    }

    pub async fn add_known_words_with_source(&self, words: &[String], source: &str) -> Result<usize> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut added = 0;
        for w in words {
            let trimmed = w.trim();
            if trimmed.is_empty() {
                continue;
            }
            let active = known_words::ActiveModel {
                word: Set(trimmed.to_string()),
                source: Set(source.to_string()),
                added_at: Set(now.clone()),
            };
            KnownWords::insert(active)
                .on_conflict(
                    OnConflict::column(known_words::Column::Word)
                        .update_column(known_words::Column::Source)
                        .to_owned(),
                )
                .exec(&self.conn)
                .await?;
            added += 1;
        }
        Ok(added)
    }

    pub async fn get_known_words_sorted_by_source(&self, source: &str) -> Result<Vec<String>> {
        let items = KnownWords::find()
            .filter(known_words::Column::Source.eq(source))
            .order_by_asc(known_words::Column::Word)
            .all(&self.conn)
            .await?;
        Ok(items.into_iter().map(|m| m.word).collect())
    }

    pub async fn remove_known_words(&self, words: &[String]) -> Result<usize> {
        let clean_words: Vec<&str> = words.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        if clean_words.is_empty() {
            return Ok(0);
        }
        let res = KnownWords::delete_many()
            .filter(known_words::Column::Word.is_in(clean_words))
            .exec(&self.conn)
            .await?;
        Ok(res.rows_affected as usize)
    }

    pub async fn get_ignored_words(&self) -> Result<HashSet<String>> {
        let items = IgnoredWords::find().all(&self.conn).await?;
        Ok(items.into_iter().map(|m| m.word).collect())
    }

    pub async fn add_ignored_word(&self, word: &str) -> Result<()> {
        let trimmed = word.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        let now = chrono::Utc::now().to_rfc3339();
        let active = ignored_words::ActiveModel {
            word: Set(trimmed.to_string()),
            added_at: Set(now),
        };
        let _ = IgnoredWords::insert(active)
            .on_conflict(OnConflict::column(ignored_words::Column::Word).do_nothing().to_owned())
            .exec(&self.conn)
            .await;
        Ok(())
    }

    pub async fn get_ignored_words_sorted(&self) -> Result<Vec<String>> {
        let items = IgnoredWords::find()
            .order_by_asc(ignored_words::Column::Word)
            .all(&self.conn)
            .await?;
        Ok(items.into_iter().map(|m| m.word).collect())
    }

    pub async fn remove_ignored_words(&self, words: &[String]) -> Result<usize> {
        let clean_words: Vec<&str> = words.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        if clean_words.is_empty() {
            return Ok(0);
        }
        let res = IgnoredWords::delete_many()
            .filter(ignored_words::Column::Word.is_in(clean_words))
            .exec(&self.conn)
            .await?;
        Ok(res.rows_affected as usize)
    }
}
