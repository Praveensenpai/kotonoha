use crate::nlp::JapaneseTokenizer;
use crate::srt::SubtitleSentence;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct CandidateSentence {
    pub sentence: SubtitleSentence,
    pub target_word: String,
    pub target_reading: String,
    pub known_context_words: Vec<String>,
    pub unknown_context_words: Vec<String>,
    pub ignored_context_words: Vec<String>,
    pub episode_freq: usize,
    pub density_tier: usize,
    pub video_path: std::path::PathBuf,
}

fn is_better_candidate(candidate: &CandidateSentence, existing: &CandidateSentence) -> bool {
    if candidate.density_tier != existing.density_tier {
        return candidate.density_tier < existing.density_tier;
    }
    candidate.sentence.text.chars().count() < existing.sentence.text.chars().count()
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
    ) -> Vec<CandidateSentence> {
        let episode_word_freq = self.count_word_frequencies(sentences, known_words, ignored_words);
        let mut best_candidates: std::collections::HashMap<String, CandidateSentence> =
            std::collections::HashMap::new();

        for sub in sentences {
            if sub.text.chars().count() < 4 {
                continue;
            }

            if let Ok(tokens) = self.tokenizer.tokenize(&sub.text) {
                let (unknown_words, known_context, ignored_context, readings) =
                    Self::classify_tokens(&tokens, known_words, ignored_words);

                if unknown_words.len() == 1 {
                    let target_word = unknown_words[0].clone();
                    let target_reading = readings
                        .get(&target_word)
                        .cloned()
                        .unwrap_or_else(|| target_word.clone());
                    let episode_freq = episode_word_freq.get(&target_word).copied().unwrap_or(1);
                    let density_tier = match known_context.len() + 1 {
                        2 => 1,
                        3 => 2,
                        4 => 3,
                        1 => 4,
                        n => n,
                    };

                    let candidate = CandidateSentence {
                        sentence: sub.clone(),
                        target_word: target_word.clone(),
                        target_reading,
                        known_context_words: known_context,
                        unknown_context_words: unknown_words,
                        ignored_context_words: ignored_context,
                        episode_freq,
                        density_tier,
                        video_path: sub.video_path.clone().unwrap_or_default(),
                    };

                    match best_candidates.entry(target_word) {
                        std::collections::hash_map::Entry::Vacant(e) => {
                            e.insert(candidate);
                        }
                        std::collections::hash_map::Entry::Occupied(mut e) => {
                            if is_better_candidate(&candidate, e.get()) {
                                e.insert(candidate);
                            }
                        }
                    }
                }
            }
        }

        let mut candidates: Vec<CandidateSentence> = best_candidates.into_values().collect();
        candidates.sort_by(|a, b| {
            b.episode_freq
                .cmp(&a.episode_freq)
                .then_with(|| a.density_tier.cmp(&b.density_tier))
                .then_with(|| {
                    a.sentence
                        .text
                        .chars()
                        .count()
                        .cmp(&b.sentence.text.chars().count())
                })
        });
        candidates
    }

    fn count_word_frequencies(
        &self,
        sentences: &[SubtitleSentence],
        known_words: &HashSet<String>,
        ignored_words: &HashSet<String>,
    ) -> std::collections::HashMap<String, usize> {
        let mut word_freq = std::collections::HashMap::new();
        for sub in sentences {
            if let Ok(tokens) = self.tokenizer.tokenize(&sub.text) {
                for t in &tokens {
                    if t.is_content_word
                        && !known_words.contains(&t.dictionary_form)
                        && !ignored_words.contains(&t.dictionary_form)
                    {
                        *word_freq.entry(t.dictionary_form.clone()).or_insert(0) += 1;
                    }
                }
            }
        }
        word_freq
    }

    fn classify_tokens(
        tokens: &[crate::nlp::TokenInfo],
        known_words: &HashSet<String>,
        ignored_words: &HashSet<String>,
    ) -> (
        Vec<String>,
        Vec<String>,
        Vec<String>,
        std::collections::HashMap<String, String>,
    ) {
        let mut unknown_words = Vec::new();
        let mut known_context = Vec::new();
        let mut ignored_context = Vec::new();
        let mut readings = std::collections::HashMap::new();

        for t in tokens {
            let dict_form = &t.dictionary_form;
            readings.insert(dict_form.clone(), t.reading.clone());

            if ignored_words.contains(dict_form) {
                let entry = format!("{} (Ignored)", dict_form);
                if !ignored_context.contains(&entry) {
                    ignored_context.push(entry);
                }
                continue;
            }

            if t.is_proper_noun {
                let entry = format!("{} (Name)", dict_form);
                if !ignored_context.contains(&entry) {
                    ignored_context.push(entry);
                }
                continue;
            }

            if !t.is_content_word {
                if matches!(
                    dict_form.as_str(),
                    "ちゃん" | "さん" | "君" | "様" | "殿" | "氏" | "たん" | "先輩"
                ) {
                    let entry = format!("{} (Suffix)", dict_form);
                    if !ignored_context.contains(&entry) {
                        ignored_context.push(entry);
                    }
                }
                continue;
            }

            if known_words.contains(dict_form) {
                if !known_context.contains(dict_form) {
                    known_context.push(dict_form.clone());
                }
            } else if !unknown_words.contains(dict_form) {
                unknown_words.push(dict_form.clone());
            }
        }

        (unknown_words, known_context, ignored_context, readings)
    }
}
