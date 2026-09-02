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
        // Step 1: Count target word frequency across all episode subtitle lines
        let mut episode_word_freq: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for sub in sentences {
            if let Ok(tokens) = self.tokenizer.tokenize(&sub.text) {
                for t in &tokens {
                    if t.is_content_word
                        && !known_words.contains(&t.dictionary_form)
                        && !ignored_words.contains(&t.dictionary_form)
                    {
                        *episode_word_freq
                            .entry(t.dictionary_form.clone())
                            .or_insert(0) += 1;
                    }
                }
            }
        }

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
                let mut ignored_context = Vec::new();
                let mut token_readings = std::collections::HashMap::new();

                for t in &tokens {
                    let dict_form = &t.dictionary_form;
                    token_readings.insert(dict_form.clone(), t.reading.clone());

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
                    } else {
                        if !unknown_words.contains(dict_form) {
                            unknown_words.push(dict_form.clone());
                        }
                    }
                }

                if unknown_words.len() == 1 {
                    let target_word = unknown_words[0].clone();
                    let target_reading = token_readings
                        .get(&target_word)
                        .cloned()
                        .unwrap_or_else(|| target_word.clone());

                    // Deduplicate target words so you only see the single best sentence per word
                    if seen_targets.contains(&target_word) {
                        continue;
                    }

                    seen_targets.insert(target_word.clone());
                    let episode_freq = episode_word_freq.get(&target_word).copied().unwrap_or(1);

                    let total_content_words = known_context.len() + 1;
                    let density_tier = match total_content_words {
                        2 => 1, // Tier 1: 1 Known + 1 Target (Holy Grail of mining!)
                        3 => 2, // Tier 2: 2 Known + 1 Target
                        4 => 3, // Tier 3: 3 Known + 1 Target
                        1 => 4, // Tier 4: Standalone single word
                        n => n, // Tier 5+: 4+ Known + 1 Target
                    };

                    candidates.push(CandidateSentence {
                        sentence: sub.clone(),
                        target_word,
                        target_reading,
                        known_context_words: known_context,
                        unknown_context_words: unknown_words,
                        ignored_context_words: ignored_context,
                        episode_freq,
                        density_tier,
                    });
                }
            }
        }

        // Multi-tier sorting: 1. Episode frequency (desc), 2. Density Tier (asc), 3. Subtitle index (asc)
        candidates.sort_by(|a, b| {
            b.episode_freq
                .cmp(&a.episode_freq)
                .then_with(|| a.density_tier.cmp(&b.density_tier))
                .then_with(|| a.sentence.index.cmp(&b.sentence.index))
        });
        candidates
    }
}
