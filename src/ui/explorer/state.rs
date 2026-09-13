use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::process::Child;
use std::time::{Duration, Instant};

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
    pub selected_cards: HashSet<(usize, String)>,
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
            selected_cards: HashSet::new(),
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
        let Some((_, _, word)) = self.current_sentence_and_word() else {
            return;
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
    }

    pub fn toggle_selection(&mut self) {
        let Some((s, _, target_word)) = self.current_sentence_and_word() else {
            return;
        };
        let key = (s.sentence.index, target_word.clone());
        if self.selected_cards.contains(&key) {
            self.selected_cards.remove(&key);
            self.set_status(format!(
                "Deselected '{}' ({} selected)",
                target_word,
                self.selected_cards.len()
            ));
        } else {
            self.selected_cards.insert(key);
            self.set_status(format!(
                "✔ Selected '{}' ({} total)",
                target_word,
                self.selected_cards.len()
            ));
        }
    }

    pub fn toggle_all_in_current_sentence(&mut self) {
        let visible = self.visible_indices();
        let Some(&s_idx) = visible.get(self.selected) else {
            return;
        };
        let s = &self.sentences[s_idx];
        if s.unknowns.is_empty() {
            return;
        }
        let all_selected = s.unknowns.iter().all(|u| {
            self.selected_cards
                .contains(&(s.sentence.index, u.dictionary_form.clone()))
        });
        if all_selected {
            for u in &s.unknowns {
                self.selected_cards
                    .remove(&(s.sentence.index, u.dictionary_form.clone()));
            }
            self.set_status(format!(
                "Deselected all words in line ({} selected)",
                self.selected_cards.len()
            ));
        } else {
            for u in &s.unknowns {
                self.selected_cards
                    .insert((s.sentence.index, u.dictionary_form.clone()));
            }
            self.set_status(format!(
                "✔ Selected all {} words in line ({} total)",
                s.unknowns.len(),
                self.selected_cards.len()
            ));
        }
    }

    pub fn clear_selection(&mut self) {
        let prev = self.selected_cards.len();
        self.selected_cards.clear();
        self.set_status(format!("Cleared {prev} selections"));
    }

    pub fn build_selected_candidates(&self) -> Vec<crate::miner::CandidateSentence> {
        if self.selected_cards.is_empty() {
            if let Some((s, _, target_word)) = self.current_sentence_and_word() {
                let tokens = self
                    .p
                    .tokenizer
                    .tokenize(&s.sentence.text)
                    .unwrap_or_default();
                return vec![crate::miner::MiningEngine::build_candidate(
                    crate::miner::BuildCandidateParams {
                        sentence: &s.sentence,
                        target_word: &target_word,
                        tokens: &tokens,
                        known_words: self.p.known_words,
                        ignored_words: self.p.ignored_words,
                    },
                )];
            }
            return Vec::new();
        }

        let mut candidates = Vec::new();
        for s in &self.sentences {
            let tokens = self
                .p
                .tokenizer
                .tokenize(&s.sentence.text)
                .unwrap_or_default();
            for u in &s.unknowns {
                if self
                    .selected_cards
                    .contains(&(s.sentence.index, u.dictionary_form.clone()))
                {
                    candidates.push(crate::miner::MiningEngine::build_candidate(
                        crate::miner::BuildCandidateParams {
                            sentence: &s.sentence,
                            target_word: &u.dictionary_form,
                            tokens: &tokens,
                            known_words: self.p.known_words,
                            ignored_words: self.p.ignored_words,
                        },
                    ));
                }
            }
        }
        candidates
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
}
