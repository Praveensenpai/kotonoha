use anyhow::{Context, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, Database as SeaDatabase,
    DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
    sea_query::OnConflict,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub mod entities;
use entities::*;

#[derive(Debug, Clone)]
pub struct Database {
    conn: DatabaseConnection,
}

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

pub struct GetCachedAiParams<'a> {
    pub sentence: &'a str,
    pub target_word: &'a str,
    pub model: &'a str,
    pub card_index: usize,
    pub ttl_minutes: usize,
}

impl Database {
    pub async fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let abs_path = if db_path.is_absolute() {
            db_path.to_path_buf()
        } else {
            std::env::current_dir()?.join(db_path)
        };

        let db_url = format!("sqlite://{}?mode=rwc", abs_path.to_string_lossy());
        let conn = SeaDatabase::connect(&db_url)
            .await
            .with_context(|| format!("Failed to open SQLite database at {}", db_path.display()))?;

        let db = Database { conn };
        db.init_schema().await?;
        Ok(db)
    }

    async fn init_schema(&self) -> Result<()> {
        let builder = self.conn.get_database_backend();
        let schema = sea_orm::Schema::new(builder);

        let table_stmts = [
            schema.create_table_from_entity(KnownWords).if_not_exists().take(),
            schema.create_table_from_entity(IgnoredWords).if_not_exists().take(),
            schema.create_table_from_entity(DictionaryCache).if_not_exists().take(),
            schema.create_table_from_entity(AllCandidatesCache).if_not_exists().take(),
            schema.create_table_from_entity(MinedCards).if_not_exists().take(),
            schema.create_table_from_entity(AiAnalysisCache).if_not_exists().take(),
            schema.create_table_from_entity(OfflineTerms).if_not_exists().take(),
            schema.create_table_from_entity(BundledMedia).if_not_exists().take(),
        ];

        for stmt in table_stmts {
            self.conn.execute(builder.build(&stmt)).await?;
        }

        self.conn.execute_unprepared(
            "
            CREATE INDEX IF NOT EXISTS idx_offline_terms_expr ON offline_terms(expression);
            CREATE INDEX IF NOT EXISTS idx_offline_terms_reading ON offline_terms(reading);
            CREATE INDEX IF NOT EXISTS idx_bundled_media_fps ON bundled_media(video_fingerprint, subtitle_fingerprint);
            DELETE FROM dictionary_cache WHERE definition LIKE '%[Noun] serif%' OR definition LIKE '%[Wikipedia definition] Serif%';
            ",
        ).await?;

        // Run migrations for any existing databases that might miss new columns
        let _ = self.conn.execute_unprepared("ALTER TABLE mined_cards ADD COLUMN anki_note_id INTEGER").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE mined_cards ADD COLUMN pitch_accent TEXT").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE mined_cards ADD COLUMN english_natural TEXT").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE mined_cards ADD COLUMN english_literal TEXT").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE mined_cards ADD COLUMN kannada_natural TEXT").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE mined_cards ADD COLUMN kannada_literal TEXT").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE ai_analysis_cache ADD COLUMN recommended_candidate_index INTEGER").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE ai_analysis_cache ADD COLUMN recommended_sense_index INTEGER").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE ai_analysis_cache ADD COLUMN custom_definition_suggestion TEXT").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE ai_analysis_cache ADD COLUMN explanation TEXT").await;
        let _ = self.conn.execute_unprepared("ALTER TABLE known_words ADD COLUMN source TEXT DEFAULT 'known'").await;

        Ok(())
    }

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

    pub async fn get_cached_definition(
        &self,
        expression: &str,
    ) -> Result<Option<(String, String, String)>> {
        if let Some(m) = DictionaryCache::find_by_id(expression).one(&self.conn).await? {
            let reading = m.reading.unwrap_or_default();
            let definition = m.definition.unwrap_or_default();
            let pitch = m.pitch_accent.unwrap_or_default();
            if definition.trim() == "1. [def] vocabulary word"
                || definition.trim() == "No dictionary definition found"
            {
                Ok(None)
            } else {
                Ok(Some((reading, definition, pitch)))
            }
        } else {
            Ok(None)
        }
    }

    pub async fn cache_definition(
        &self,
        expression: &str,
        reading: &str,
        definition: &str,
        pitch: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let active = dictionary_cache::ActiveModel {
            expression: Set(expression.to_string()),
            reading: Set(Some(reading.to_string())),
            definition: Set(Some(definition.to_string())),
            pitch_accent: Set(Some(pitch.to_string())),
            updated_at: Set(now),
        };
        DictionaryCache::insert(active)
            .on_conflict(
                OnConflict::column(dictionary_cache::Column::Expression)
                    .update_columns([
                        dictionary_cache::Column::Reading,
                        dictionary_cache::Column::Definition,
                        dictionary_cache::Column::PitchAccent,
                        dictionary_cache::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec(&self.conn)
            .await?;
        Ok(())
    }

    pub async fn get_cached_candidates(
        &self,
        expression: &str,
    ) -> Result<Option<Vec<crate::dict::LookupResult>>> {
        if let Some(m) = AllCandidatesCache::find_by_id(expression).one(&self.conn).await? {
            if let Ok(cands) = serde_json::from_str::<Vec<crate::dict::LookupResult>>(&m.candidates_json) {
                return Ok(Some(cands));
            }
        }
        Ok(None)
    }

    pub async fn cache_candidates(
        &self,
        expression: &str,
        candidates: &[crate::dict::LookupResult],
    ) -> Result<()> {
        if let Ok(json_str) = serde_json::to_string(candidates) {
            let now = chrono::Utc::now().to_rfc3339();
            let active = all_candidates_cache::ActiveModel {
                expression: Set(expression.to_string()),
                candidates_json: Set(json_str),
                updated_at: Set(now),
            };
            AllCandidatesCache::insert(active)
                .on_conflict(
                    OnConflict::column(all_candidates_cache::Column::Expression)
                        .update_columns([
                            all_candidates_cache::Column::CandidatesJson,
                            all_candidates_cache::Column::UpdatedAt,
                        ])
                        .to_owned(),
                )
                .exec(&self.conn)
                .await?;
        }
        Ok(())
    }

    pub async fn save_mined_card(&self, p: SaveMinedCardParams<'_>) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let active = mined_cards::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            sentence: Set(p.sentence.to_string()),
            target_word: Set(p.target_word.to_string()),
            reading: Set(Some(p.reading.to_string())),
            pitch_accent: Set(Some(p.pitch_accent.to_string())),
            definition: Set(Some(p.definition.to_string())),
            audio_path: Set(p.audio_path.map(|s| s.to_string())),
            image_path: Set(p.image_path.map(|s| s.to_string())),
            english_natural: Set(p.english_natural.map(|s| s.to_string())),
            english_literal: Set(p.english_literal.map(|s| s.to_string())),
            kannada_natural: Set(p.kannada_natural.map(|s| s.to_string())),
            kannada_literal: Set(p.kannada_literal.map(|s| s.to_string())),
            anki_note_id: Set(None),
            created_at: Set(now),
        };
        MinedCards::insert(active).exec(&self.conn).await?;
        Ok(())
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

    pub async fn get_all_mined_cards(&self) -> Result<Vec<(MinedCard, Option<i64>)>> {
        let items = MinedCards::find()
            .order_by_asc(mined_cards::Column::Id)
            .all(&self.conn)
            .await?;

        Ok(items
            .into_iter()
            .map(|m| {
                let note_id = m.anki_note_id;
                let card = MinedCard {
                    id: m.id,
                    sentence: m.sentence,
                    target_word: m.target_word,
                    reading: m.reading.unwrap_or_default(),
                    pitch_accent: m.pitch_accent.unwrap_or_default(),
                    definition: m.definition.unwrap_or_default(),
                    audio_path: m.audio_path,
                    image_path: m.image_path,
                    english_natural: m.english_natural,
                };
                (card, note_id)
            })
            .collect())
    }

    pub async fn get_cached_ai_analysis(
        &self,
        p: GetCachedAiParams<'_>,
    ) -> Result<Option<crate::ai::AiAnalysisResult>> {
        if p.ttl_minutes == 0 {
            return Ok(None);
        }
        let key = format!("{}:{}:{}", p.sentence, p.target_word, p.model);
        if let Some(m) = AiAnalysisCache::find_by_id(&key).one(&self.conn).await? {
            let res = crate::ai::AiAnalysisResult {
                card_index: p.card_index,
                recommended_candidate_index: m.recommended_candidate_index.map(|v| v as usize),
                recommended_sense_index: m.recommended_sense_index.map(|v| v as usize),
                custom_definition_suggestion: m.custom_definition_suggestion,
                explanation: m.explanation,
                english_natural: m.english_natural,
                english_literal: m.english_literal,
                kannada_natural: m.kannada_natural,
                kannada_literal: m.kannada_literal,
                parsing_warning: m.parsing_warning,
            };
            return Ok(Some(res));
        }
        Ok(None)
    }

    pub async fn cache_ai_analysis(
        &self,
        sentence: &str,
        target_word: &str,
        model: &str,
        res: &crate::ai::AiAnalysisResult,
    ) -> Result<()> {
        let key = format!("{}:{}:{}", sentence, target_word, model);
        let now = chrono::Utc::now().to_rfc3339();
        let active = ai_analysis_cache::ActiveModel {
            cache_key: Set(key),
            english_natural: Set(res.english_natural.clone()),
            english_literal: Set(res.english_literal.clone()),
            kannada_natural: Set(res.kannada_natural.clone()),
            kannada_literal: Set(res.kannada_literal.clone()),
            parsing_warning: Set(res.parsing_warning.clone()),
            recommended_candidate_index: Set(res.recommended_candidate_index.map(|v| v as i64)),
            recommended_sense_index: Set(res.recommended_sense_index.map(|v| v as i64)),
            custom_definition_suggestion: Set(res.custom_definition_suggestion.clone()),
            explanation: Set(res.explanation.clone()),
            updated_at: Set(now),
        };
        AiAnalysisCache::insert(active)
            .on_conflict(
                OnConflict::column(ai_analysis_cache::Column::CacheKey)
                    .update_columns([
                        ai_analysis_cache::Column::EnglishNatural,
                        ai_analysis_cache::Column::EnglishLiteral,
                        ai_analysis_cache::Column::KannadaNatural,
                        ai_analysis_cache::Column::KannadaLiteral,
                        ai_analysis_cache::Column::ParsingWarning,
                        ai_analysis_cache::Column::RecommendedCandidateIndex,
                        ai_analysis_cache::Column::RecommendedSenseIndex,
                        ai_analysis_cache::Column::CustomDefinitionSuggestion,
                        ai_analysis_cache::Column::Explanation,
                        ai_analysis_cache::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec(&self.conn)
            .await?;
        Ok(())
    }

    pub async fn clean_expired_ai_cache(&self, ttl_minutes: usize) -> Result<usize> {
        if ttl_minutes == 0 {
            let res = AiAnalysisCache::delete_many().exec(&self.conn).await?;
            return Ok(res.rows_affected as usize);
        }
        let sql = format!("DELETE FROM ai_analysis_cache WHERE updated_at < datetime('now', '-{} minutes')", ttl_minutes);
        let res = self.conn.execute_unprepared(&sql).await?;
        Ok(res.rows_affected() as usize)
    }

    pub async fn is_offline_dict_indexed(&self) -> Result<bool> {
        let count = OfflineTerms::find().count(&self.conn).await?;
        Ok(count > 0)
    }

    pub async fn insert_offline_terms_batch(
        &mut self,
        terms: &[(String, String, String, String, String, i64)],
    ) -> Result<usize> {
        if terms.is_empty() {
            return Ok(0);
        }
        let total = terms.len();
        let models: Vec<offline_terms::ActiveModel> = terms
            .iter()
            .map(|(expr, reading, def, pitch, dict, score)| offline_terms::ActiveModel {
                rowid: sea_orm::ActiveValue::NotSet,
                expression: Set(expr.clone()),
                reading: Set(reading.clone()),
                definition: Set(def.clone()),
                pitch_accent: Set(pitch.clone()),
                dict_name: Set(dict.clone()),
                score: Set(*score as i32),
            })
            .collect();

        for chunk in models.chunks(500) {
            OfflineTerms::insert_many(chunk.to_vec()).exec(&self.conn).await?;
        }
        Ok(total)
    }

    pub async fn query_offline_terms(
        &self,
        word: &str,
        exact_only: bool,
    ) -> Result<Vec<crate::dict::LookupResult>> {
        let word_hira = crate::nlp::kata_to_hira(word);
        let is_short_hiragana =
            word.chars().all(|c| matches!(c, '\u{3040}'..='\u{309F}')) && word.chars().count() <= 3;
        let like_param = format!("{}%", word);

        let query = OfflineTerms::find();
        let query = if exact_only {
            query.filter(
                Condition::any()
                    .add(offline_terms::Column::Expression.eq(word))
                    .add(offline_terms::Column::Reading.eq(word))
                    .add(offline_terms::Column::Reading.eq(&word_hira)),
            )
        } else if is_short_hiragana {
            query.filter(
                Condition::any()
                    .add(offline_terms::Column::Expression.eq(word))
                    .add(offline_terms::Column::Reading.eq(word))
                    .add(offline_terms::Column::Reading.eq(&word_hira))
                    .add(offline_terms::Column::Expression.like(&like_param)),
            )
        } else {
            query.filter(
                Condition::any()
                    .add(offline_terms::Column::Expression.eq(word))
                    .add(offline_terms::Column::Reading.eq(word))
                    .add(offline_terms::Column::Reading.eq(&word_hira))
                    .add(offline_terms::Column::Expression.like(&like_param))
                    .add(offline_terms::Column::Reading.like(&like_param)),
            )
        };

        let items = query
            .order_by_desc(offline_terms::Column::Score)
            .limit(10)
            .all(&self.conn)
            .await?;

        let mut results: Vec<crate::dict::LookupResult> = items
            .into_iter()
            .map(|m| crate::dict::LookupResult {
                expression: m.expression,
                reading: m.reading,
                definition: m.definition,
                pitch_accent: m.pitch_accent,
            })
            .collect();

        results.sort_by_key(|res| {
            let is_exact =
                res.expression == word || res.reading == word || res.reading == word_hira;
            let is_uk_kana = is_short_hiragana
                && res.reading == word
                && res.definition.contains("[")
                && (res.definition.contains("uk]") || res.definition.contains("uk "));
            (!is_exact, !is_uk_kana, res.expression != word)
        });

        Ok(results)
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

    pub async fn find_existing_bundle(
        &self,
        video_fingerprint: &str,
        subtitle_fingerprint: &str,
    ) -> Result<Option<PathBuf>> {
        let match_record = BundledMedia::find()
            .filter(bundled_media::Column::VideoFingerprint.eq(video_fingerprint))
            .filter(bundled_media::Column::SubtitleFingerprint.eq(subtitle_fingerprint))
            .order_by_desc(bundled_media::Column::Id)
            .one(&self.conn)
            .await?;

        if let Some(record) = match_record {
            let path = PathBuf::from(&record.bundle_path);
            if path.exists() {
                return Ok(Some(path));
            }
        }

        Ok(None)
    }

    pub async fn record_bundle(
        &self,
        bundle_path: &Path,
        source_video: &str,
        source_subtitle: &str,
        video_fingerprint: &str,
        subtitle_fingerprint: &str,
    ) -> Result<()> {
        let active = bundled_media::ActiveModel {
            bundle_path: Set(bundle_path.to_string_lossy().to_string()),
            source_video: Set(source_video.to_string()),
            source_subtitle: Set(source_subtitle.to_string()),
            video_fingerprint: Set(video_fingerprint.to_string()),
            subtitle_fingerprint: Set(subtitle_fingerprint.to_string()),
            created_at: Set(chrono::Utc::now().to_rfc3339()),
            ..Default::default()
        };

        BundledMedia::insert(active)
            .on_conflict(
                OnConflict::column(bundled_media::Column::BundlePath)
                    .update_columns([
                        bundled_media::Column::SourceVideo,
                        bundled_media::Column::SourceSubtitle,
                        bundled_media::Column::VideoFingerprint,
                        bundled_media::Column::SubtitleFingerprint,
                        bundled_media::Column::CreatedAt,
                    ])
                    .to_owned(),
            )
            .exec(&self.conn)
            .await?;

        Ok(())
    }

    pub async fn get_all_bundled_media(&self) -> Result<Vec<bundled_media::Model>> {
        BundledMedia::find()
            .order_by_desc(bundled_media::Column::Id)
            .all(&self.conn)
            .await
            .context("Failed to load bundled media list")
    }

    pub async fn delete_bundled_media_by_id(&self, id: i32) -> Result<()> {
        BundledMedia::delete_by_id(id)
            .exec(&self.conn)
            .await
            .context("Failed to delete bundled media by id")?;
        Ok(())
    }

    pub async fn delete_bundled_media_by_path(&self, bundle_path: &str) -> Result<()> {
        BundledMedia::delete_many()
            .filter(bundled_media::Column::BundlePath.eq(bundle_path))
            .exec(&self.conn)
            .await
            .context("Failed to delete bundled media by path")?;
        Ok(())
    }

    pub async fn prune_missing_bundles(&self) -> Result<usize> {
        let all = self.get_all_bundled_media().await?;
        let mut pruned = 0;
        for record in all {
            if !Path::new(&record.bundle_path).exists() {
                self.delete_bundled_media_by_id(record.id).await?;
                pruned += 1;
            }
        }
        Ok(pruned)
    }
}
