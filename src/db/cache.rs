use anyhow::Result;
use sea_orm::{
    sea_query::OnConflict, ColumnTrait, Condition, ConnectionTrait, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set,
};

use super::entities::*;
use super::Database;

pub struct GetCachedAiParams<'a> {
    pub sentence: &'a str,
    pub target_word: &'a str,
    pub model: &'a str,
    pub card_index: usize,
    pub ttl_minutes: usize,
}

impl Database {
    pub async fn get_cached_definition(
        &self,
        expression: &str,
    ) -> Result<Option<(String, String, String)>> {
        if let Some(m) = DictionaryCache::find_by_id(expression)
            .one(&self.conn)
            .await?
        {
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
        if let Some(m) = AllCandidatesCache::find_by_id(expression)
            .one(&self.conn)
            .await?
        {
            if let Ok(cands) =
                serde_json::from_str::<Vec<crate::dict::LookupResult>>(&m.candidates_json)
            {
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
        let sql = format!(
            "DELETE FROM ai_analysis_cache WHERE updated_at < datetime('now', '-{} minutes')",
            ttl_minutes
        );
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
            .map(
                |(expr, reading, def, pitch, dict, score)| offline_terms::ActiveModel {
                    rowid: sea_orm::ActiveValue::NotSet,
                    expression: Set(expr.clone()),
                    reading: Set(reading.clone()),
                    definition: Set(def.clone()),
                    pitch_accent: Set(pitch.clone()),
                    dict_name: Set(dict.clone()),
                    score: Set(*score as i32),
                },
            )
            .collect();

        for chunk in models.chunks(500) {
            OfflineTerms::insert_many(chunk.to_vec())
                .exec(&self.conn)
                .await?;
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
}
