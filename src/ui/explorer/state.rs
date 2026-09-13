use anyhow::Result;
use std::collections::HashMap;
use std::process::Child;
use std::time::{Duration, Instant};

use crate::ai::{AiAnalysisResult, CardBatchInput, GeminiAiService};
use crate::dict::{DictionaryService, LookupLimits, LookupResult};

use super::actions;
use super::model::{
    build_explorer_sentences, filter_sentences, refresh_sentence_unknowns, ExplorerSentence,
    SortOrder, TierFilter,
};
use super::ExplorerParams;

pub struct ExplorerController<'a> {
    pub p: ExplorerParams<'a>,
    pub sentences: Vec<ExplorerSentence>,
    pub selected: usize,
    pub sort_order: SortOrder,
    pub tier_filter: TierFilter,
    pub search: String,
    pub search_active: bool,
    pub auto_play: bool,
    pub audio_child: Option<Child>,
    pub playing_row: Option<usize>,
    pub loading_until: Option<Instant>,
    pub last_scroll_time: Instant,
    pub pending_audio: bool,
    pub cached_dict: HashMap<String, LookupResult>,
    pub cached_ai: HashMap<(String, String), AiAnalysisResult>,
    pub status_msg: Option<(String, Instant)>,
}

impl<'a> ExplorerController<'a> {
    pub fn new(p: ExplorerParams<'a>) -> Self {
        let sentences =
            build_explorer_sentences(p.sentences, p.tokenizer, p.known_words, p.ignored_words);
        Self {
            p,
            sentences,
            selected: 0,
            sort_order: SortOrder::Difficulty,
            tier_filter: TierFilter::All,
            search: String::new(),
            search_active: false,
            auto_play: true,
            audio_child: None,
            playing_row: None,
            loading_until: None,
            last_scroll_time: Instant::now(),
            pending_audio: false,
            cached_dict: HashMap::new(),
            cached_ai: HashMap::new(),
            status_msg: None,
        }
    }

    pub fn set_status(&mut self, text: impl Into<String>) {
        self.status_msg = Some((text.into(), Instant::now() + Duration::from_secs(3)));
    }

    pub fn visible_indices(&self) -> Vec<usize> {
        filter_sentences(&self.sentences, self.tier_filter, &self.search)
    }

    pub fn current_sentence_and_word(&self) -> Option<(&ExplorerSentence, usize, String)> {
        let visible = self.visible_indices();
        let &s_idx = visible.get(self.selected)?;
        let s = &self.sentences[s_idx];
        let word = s.unknowns.get(s.selected_unknown)?.dictionary_form.clone();
        Some((s, s_idx, word))
    }

    pub fn trigger_audio_for_current(&mut self) {
        let visible = self.visible_indices();
        let Some(&s_idx) = visible.get(self.selected) else {
            return;
        };
        let Some(video) = self.p.video_path else {
            return;
        };

        if let Some(mut child) = self.audio_child.take() {
            let _ = child.kill();
        }

        let s = &self.sentences[s_idx].sentence;
        self.audio_child = actions::play_sentence_audio(video, s.start_ms, s.end_ms);
        self.playing_row = self.audio_child.as_ref().map(|_| s_idx);
        self.loading_until = self
            .playing_row
            .map(|_| Instant::now() + Duration::from_millis(250));
        self.pending_audio = false;
    }

    pub async fn ensure_active_word_cached(&mut self) {
        let (sentence_text, word) = {
            let Some((s, _, word)) = self.current_sentence_and_word() else {
                return;
            };
            (s.sentence.text.clone(), word)
        };

        if !self.cached_dict.contains_key(&word) {
            let dict_res = DictionaryService::lookup_all_candidates_cached(
                self.p.http_client,
                Some(self.p.db),
                &word,
                LookupLimits {
                    max_senses: self.p.cfg.dict.max_definition_senses,
                    max_glosses: self.p.cfg.dict.max_glosses_per_sense,
                },
            )
            .await
            .ok()
            .and_then(|cands| cands.into_iter().next())
            .unwrap_or_else(|| LookupResult {
                expression: word.clone(),
                reading: String::new(),
                definition: "No definition found".to_string(),
                pitch_accent: String::new(),
            });
            self.cached_dict.insert(word.clone(), dict_res);
        }

        let ai_key = (sentence_text.clone(), word.clone());
        if !self.cached_ai.contains_key(&ai_key) {
            if let Ok(Some(cached_ai)) = self
                .p
                .db
                .get_cached_ai_analysis(crate::db::GetCachedAiParams {
                    sentence: &sentence_text,
                    target_word: &word,
                    model: &self.p.cfg.ai.gemini_model,
                    card_index: 0,
                    ttl_minutes: self.p.cfg.ai.ai_cache_ttl_minutes,
                })
                .await
            {
                self.cached_ai.insert(ai_key, cached_ai);
            }
        }
    }

    pub async fn handle_mine_word(&mut self) -> Result<()> {
        let Some((s, s_idx, target_word)) = self.current_sentence_and_word() else {
            return Ok(());
        };
        let dict_info = self
            .cached_dict
            .get(&target_word)
            .cloned()
            .unwrap_or_else(|| LookupResult {
                expression: target_word.clone(),
                reading: String::new(),
                definition: "Mined word".to_string(),
                pitch_accent: String::new(),
            });
        let ai_res = self
            .cached_ai
            .get(&(s.sentence.text.clone(), target_word.clone()));

        let res = actions::mine_single_word(actions::MineWordParams {
            explorer_sentence: s,
            word_index: s.selected_unknown,
            dict_info: &dict_info,
            ai_analysis: ai_res,
            video_path: self.p.video_path,
            cfg: self.p.cfg,
            db: self.p.db,
        })
        .await;

        match res {
            Ok(_) => {
                self.p.known_words.insert(target_word.clone());
                refresh_sentence_unknowns(
                    &mut self.sentences[s_idx],
                    self.p.known_words,
                    self.p.ignored_words,
                );
                self.set_status(format!("✔ Mined Anki card for '{target_word}'!"));
            }
            Err(e) => self.set_status(format!("✖ Failed to mine card: {e}")),
        }
        Ok(())
    }

    pub async fn handle_mine_all(&mut self) -> Result<()> {
        let visible = self.visible_indices();
        let Some(&s_idx) = visible.get(self.selected) else {
            return Ok(());
        };
        let s = &self.sentences[s_idx];
        if s.unknowns.is_empty() {
            self.set_status("Line has 0 unknown words.");
            return Ok(());
        }

        let res = actions::mine_all_unknowns_in_sentence(actions::MineAllParams {
            explorer_sentence: s,
            video_path: self.p.video_path,
            cfg: self.p.cfg,
            db: self.p.db,
            http_client: self.p.http_client,
        })
        .await;

        match res {
            Ok(words) => {
                for w in &words {
                    self.p.known_words.insert(w.clone());
                }
                refresh_sentence_unknowns(
                    &mut self.sentences[s_idx],
                    self.p.known_words,
                    self.p.ignored_words,
                );
                self.set_status(format!(
                    "✔ Generated {} individual cards: {:?}",
                    words.len(),
                    words
                ));
            }
            Err(e) => self.set_status(format!("✖ Multi-card mining failed: {e}")),
        }
        Ok(())
    }

    pub async fn handle_mark_known(&mut self) -> Result<()> {
        let Some((_, s_idx, target_word)) = self.current_sentence_and_word() else {
            return Ok(());
        };
        actions::mark_word_known(&target_word, self.p.db).await?;
        self.p.known_words.insert(target_word.clone());
        refresh_sentence_unknowns(
            &mut self.sentences[s_idx],
            self.p.known_words,
            self.p.ignored_words,
        );
        self.set_status(format!("🧠 Marked '{target_word}' as known."));
        Ok(())
    }

    pub async fn handle_mark_all_known(&mut self) -> Result<()> {
        let visible = self.visible_indices();
        let Some(&s_idx) = visible.get(self.selected) else {
            return Ok(());
        };
        let words: Vec<String> = self.sentences[s_idx]
            .unknowns
            .iter()
            .map(|u| u.dictionary_form.clone())
            .collect();
        if words.is_empty() {
            return Ok(());
        }

        actions::mark_all_known(&words, self.p.db).await?;
        for w in &words {
            self.p.known_words.insert(w.clone());
        }
        refresh_sentence_unknowns(
            &mut self.sentences[s_idx],
            self.p.known_words,
            self.p.ignored_words,
        );
        self.set_status(format!("🧠 Marked all {} words as known!", words.len()));
        Ok(())
    }

    pub async fn handle_mark_ignored(&mut self) -> Result<()> {
        let Some((_, s_idx, target_word)) = self.current_sentence_and_word() else {
            return Ok(());
        };
        actions::mark_word_ignored(&target_word, self.p.db).await?;
        self.p.ignored_words.insert(target_word.clone());
        refresh_sentence_unknowns(
            &mut self.sentences[s_idx],
            self.p.known_words,
            self.p.ignored_words,
        );
        self.set_status(format!("🚫 Marked '{target_word}' as ignored."));
        Ok(())
    }

    pub async fn handle_request_ai(&mut self) -> Result<()> {
        let (sentence_text, target_word) = {
            let Some((s, _, target_word)) = self.current_sentence_and_word() else {
                return Ok(());
            };
            (s.sentence.text.clone(), target_word)
        };
        if !self.p.cfg.ai.enable_ai || !self.p.cfg.ai.has_valid_api_key() {
            self.set_status("⚠ Gemini AI is disabled or GEMINI_API_KEY is not set.");
            return Ok(());
        }
        let dict = self
            .cached_dict
            .get(&target_word)
            .cloned()
            .unwrap_or_else(|| LookupResult {
                expression: target_word.clone(),
                reading: String::new(),
                definition: String::new(),
                pitch_accent: String::new(),
            });
        let input = [CardBatchInput {
            card_index: 0,
            sentence: &sentence_text,
            target_word: &target_word,
            target_reading: &dict.reading,
            candidates: std::slice::from_ref(&dict),
        }];
        self.set_status("🤖 Requesting Gemini AI contextual analysis…");
        if let Some(ref key) = self.p.cfg.ai.gemini_api_key {
            match GeminiAiService::analyze_batch(
                self.p.http_client,
                key,
                &self.p.cfg.ai.gemini_model,
                &input,
            )
            .await
            {
                Ok(results) => {
                    if let Some(res) = results.into_iter().next() {
                        let _ = self
                            .p
                            .db
                            .cache_ai_analysis(
                                &sentence_text,
                                &target_word,
                                &self.p.cfg.ai.gemini_model,
                                &res,
                            )
                            .await;
                        self.cached_ai.insert((sentence_text, target_word), res);
                        self.set_status("✨ AI analysis loaded successfully!");
                    } else {
                        self.set_status("⚠ Gemini AI returned empty response.");
                    }
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    let short_err = if err_msg.contains("429") {
                        "⚠ Gemini rate-limited (429). Please wait a moment.".to_string()
                    } else {
                        format!("⚠ Gemini: {}", err_msg.chars().take(60).collect::<String>())
                    };
                    self.set_status(short_err);
                }
            }
        }
        Ok(())
    }
}
