use crate::nlp::JapaneseTokenizer;
use crate::srt::SubtitleSentence;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct CandidateSentence {
    pub sentence: SubtitleSentence,
    pub target_word: String,
    pub known_context_words: Vec<String>,
    pub unknown_context_words: Vec<String>,
    pub jpdb_rank: Option<u32>,
    pub score: f64,
}

pub struct MiningEngine {
    tokenizer: JapaneseTokenizer,
}

impl MiningEngine {
    pub fn new(tokenizer: JapaneseTokenizer) -> Self {
        Self { tokenizer }
    }

    pub fn find_candidates(
        &self,
        sentences: &[SubtitleSentence],
        known_words: &HashSet<String>,
        ignored_words: &HashSet<String>,
        jpdb_ranks: &std::collections::HashMap<String, u32>,
    ) -> Vec<CandidateSentence> {
        let mut candidates = Vec::new();
        let mut seen_targets = HashSet::new();

        for sub in sentences {
            // Require sentence length >= 4 characters to ignore single-word grunts (あ…, ん？)
            if sub.text.chars().count() < 4 {
                continue;
            }

            if let Ok(tokens) = self.tokenizer.tokenize(&sub.text) {
                let mut unknown_words = Vec::new();
                let mut known_context = Vec::new();

                for t in &tokens {
                    if !t.is_content_word {
                        continue;
                    }
                    let dict_form = &t.dictionary_form;
                    if ignored_words.contains(dict_form) {
                        continue;
                    }

                    if known_words.contains(dict_form) {
                        if !known_context.contains(dict_form) {
                            known_context.push(dict_form.clone());
                        }
                    } else {
                        if !unknown_words.contains(dict_form) {
                            unknown_words.push(dict_form.clone());
                        }
                    }
                }

                if unknown_words.len() == 1 {
                    let target_word = unknown_words[0].clone();

                    // Deduplicate target words so you only see the single best sentence per word
                    if seen_targets.contains(&target_word) {
                        continue;
                    }

                    seen_targets.insert(target_word.clone());
                    let rank = jpdb_ranks.get(&target_word).copied();

                    let base_score = rank.unwrap_or(5000) as f64;
                    let len_penalty = sub.text.chars().count() as f64 * 2.0;
                    let score = base_score + len_penalty;

                    candidates.push(CandidateSentence {
                        sentence: sub.clone(),
                        target_word,
                        known_context_words: known_context,
                        unknown_context_words: unknown_words,
                        jpdb_rank: rank,
                        score,
                    });
                }
            }
        }

        candidates.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap());
        candidates
    }
}
