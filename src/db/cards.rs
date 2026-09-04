use anyhow::Result;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use std::collections::HashSet;
use std::path::PathBuf;

use super::entities::*;
use super::Database;

#[derive(Debug, Clone)]
pub struct MinedCard {
    pub id: i64,
    pub sentence: String,
    pub target_word: String,
    pub reading: String,
    pub pitch_accent: String,
    pub definition: String,
    pub audio_path: Option<String>,
    pub image_path: Option<String>,
    pub english_natural: Option<String>,
}

pub struct SaveMinedCardParams<'a> {
    pub sentence: &'a str,
    pub target_word: &'a str,
    pub reading: &'a str,
    pub pitch_accent: &'a str,
    pub definition: &'a str,
    pub audio_path: Option<&'a str>,
    pub image_path: Option<&'a str>,
    pub english_natural: Option<&'a str>,
    pub english_literal: Option<&'a str>,
    pub kannada_natural: Option<&'a str>,
    pub kannada_literal: Option<&'a str>,
}

impl Database {
    pub async fn save_mined_card(&self, params: SaveMinedCardParams<'_>) -> Result<i64> {
        let now = chrono::Utc::now().to_rfc3339();
        let active = mined_cards::ActiveModel {
            sentence: Set(params.sentence.to_string()),
            target_word: Set(params.target_word.to_string()),
            reading: Set(Some(params.reading.to_string())),
            pitch_accent: Set(Some(params.pitch_accent.to_string())),
            definition: Set(Some(params.definition.to_string())),
            audio_path: Set(params.audio_path.map(str::to_string)),
            image_path: Set(params.image_path.map(str::to_string)),
            english_natural: Set(params.english_natural.map(str::to_string)),
            english_literal: Set(params.english_literal.map(str::to_string)),
            kannada_natural: Set(params.kannada_natural.map(str::to_string)),
            kannada_literal: Set(params.kannada_literal.map(str::to_string)),
            anki_note_id: Set(None),
            created_at: Set(now),
            ..Default::default()
        };
        let res = MinedCards::insert(active).exec(&self.conn).await?;
        let _ = self
            .add_known_words_with_source(&[params.target_word.to_string()], "mined")
            .await;
        Ok(res.last_insert_id)
    }

    pub async fn get_all_mined_cards(&self) -> Result<Vec<(MinedCard, Option<i64>)>> {
        let items = MinedCards::find()
            .order_by_asc(mined_cards::Column::Id)
            .all(&self.conn)
            .await?;

        let res = items
            .into_iter()
            .map(|m| {
                (
                    MinedCard {
                        id: m.id,
                        sentence: m.sentence,
                        target_word: m.target_word,
                        reading: m.reading.unwrap_or_default(),
                        pitch_accent: m.pitch_accent.unwrap_or_default(),
                        definition: m.definition.unwrap_or_default(),
                        audio_path: m.audio_path,
                        image_path: m.image_path,
                        english_natural: m.english_natural,
                    },
                    m.anki_note_id,
                )
            })
            .collect();
        Ok(res)
    }

    pub async fn get_unsynced_mined_cards(&self) -> Result<Vec<MinedCard>> {
        let items = MinedCards::find()
            .filter(mined_cards::Column::AnkiNoteId.is_null())
            .order_by_asc(mined_cards::Column::Id)
            .all(&self.conn)
            .await?;

        Ok(items
            .into_iter()
            .map(|m| MinedCard {
                id: m.id,
                sentence: m.sentence,
                target_word: m.target_word,
                reading: m.reading.unwrap_or_default(),
                pitch_accent: m.pitch_accent.unwrap_or_default(),
                definition: m.definition.unwrap_or_default(),
                audio_path: m.audio_path,
                image_path: m.image_path,
                english_natural: m.english_natural,
            })
            .collect())
    }

    pub async fn mark_mined_card_synced(&self, card_id: i64, anki_note_id: i64) -> Result<()> {
        if let Some(card) = MinedCards::find_by_id(card_id).one(&self.conn).await? {
            let mut active: mined_cards::ActiveModel = card.into();
            active.anki_note_id = Set(Some(anki_note_id));
            active.update(&self.conn).await?;
        }
        Ok(())
    }

    pub async fn get_unsynced_media_paths(&self) -> Result<HashSet<PathBuf>> {
        let cards = MinedCards::find()
            .filter(mined_cards::Column::AnkiNoteId.is_null())
            .all(&self.conn)
            .await?;
        let mut set = HashSet::new();
        for r in cards {
            if let Some(a) = r.audio_path {
                set.insert(PathBuf::from(a));
            }
            if let Some(i) = r.image_path {
                set.insert(PathBuf::from(i));
            }
        }
        Ok(set)
    }
}
