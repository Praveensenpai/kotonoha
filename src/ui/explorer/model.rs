use std::collections::HashSet;

use crate::nlp::JapaneseTokenizer;
use crate::srt::SubtitleSentence;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    I0 = 0,
    I1 = 1,
    I2 = 2,
    I3Plus = 3,
}

#[derive(Debug, Clone)]
pub struct UnknownWord {
    pub surface: String,
    pub dictionary_form: String,
    pub reading: String,
}

#[derive(Debug, Clone)]
pub struct ExplorerSentence {
    pub sentence: SubtitleSentence,
    pub tier: Tier,
    pub unknown_count: usize,
    pub unknowns: Vec<UnknownWord>,
    pub selected_unknown: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Difficulty,
    Timeline,
}

impl SortOrder {
    pub fn toggle(self) -> Self {
        match self {
            Self::Difficulty => Self::Timeline,
            Self::Timeline => Self::Difficulty,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Difficulty => "Difficulty (i+0 → i+n)",
            Self::Timeline => "Timeline (Chronological)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierFilter {
    All,
    I0,
    I1,
    I2,
    I3Plus,
}

impl TierFilter {
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::I0,
            Self::I0 => Self::I1,
            Self::I1 => Self::I2,
            Self::I2 => Self::I3Plus,
            Self::I3Plus => Self::All,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::I0 => "i+0 only",
            Self::I1 => "i+1 only",
            Self::I2 => "i+2 only",
            Self::I3Plus => "i+3+ only",
        }
    }
}

pub struct SentenceStats {
    pub total: usize,
    pub i0: usize,
    pub i1: usize,
    pub i2: usize,
    pub i3_plus: usize,
}

pub fn calculate_stats(sentences: &[ExplorerSentence]) -> SentenceStats {
    let mut stats = SentenceStats {
        total: sentences.len(),
        i0: 0,
        i1: 0,
        i2: 0,
        i3_plus: 0,
    };
    for s in sentences {
        match s.tier {
            Tier::I0 => stats.i0 += 1,
            Tier::I1 => stats.i1 += 1,
            Tier::I2 => stats.i2 += 1,
            Tier::I3Plus => stats.i3_plus += 1,
        }
    }
    stats
}

pub fn sort_sentences(sentences: &mut [ExplorerSentence], order: SortOrder) {
    match order {
        SortOrder::Difficulty => {
            sentences.sort_by(|a, b| {
                a.unknown_count
                    .cmp(&b.unknown_count)
                    .then_with(|| a.sentence.start_ms.cmp(&b.sentence.start_ms))
            });
        }
        SortOrder::Timeline => {
            sentences.sort_by_key(|s| s.sentence.start_ms);
        }
    }
}

pub fn filter_sentences(
    sentences: &[ExplorerSentence],
    tier_filter: TierFilter,
    search: &str,
) -> Vec<usize> {
    let search_lower = search.to_lowercase();
    sentences
        .iter()
        .enumerate()
        .filter_map(|(idx, s)| {
            let matches_tier = match tier_filter {
                TierFilter::All => true,
                TierFilter::I0 => s.tier == Tier::I0,
                TierFilter::I1 => s.tier == Tier::I1,
                TierFilter::I2 => s.tier == Tier::I2,
                TierFilter::I3Plus => s.tier == Tier::I3Plus,
            };
            if !matches_tier {
                return None;
            }
            if search.is_empty() || s.sentence.text.to_lowercase().contains(&search_lower) {
                Some(idx)
            } else {
                None
            }
        })
        .collect()
}

pub fn refresh_sentence_unknowns(
    sentence: &mut ExplorerSentence,
    known_words: &HashSet<String>,
    ignored_words: &HashSet<String>,
) {
    sentence.unknowns.retain(|u| {
        !known_words.contains(&u.dictionary_form) && !ignored_words.contains(&u.dictionary_form)
    });
    sentence.unknown_count = sentence.unknowns.len();
    sentence.tier = match sentence.unknown_count {
        0 => Tier::I0,
        1 => Tier::I1,
        2 => Tier::I2,
        _ => Tier::I3Plus,
    };
    if sentence.selected_unknown >= sentence.unknowns.len() {
        sentence.selected_unknown = sentence.unknowns.len().saturating_sub(1);
    }
}

pub fn build_explorer_sentences(
    sentences: &[SubtitleSentence],
    tokenizer: &JapaneseTokenizer,
    known_words: &HashSet<String>,
    ignored_words: &HashSet<String>,
) -> Vec<ExplorerSentence> {
    let mut explorer_list = Vec::with_capacity(sentences.len());

    for s in sentences {
        let tokens = tokenizer.tokenize(&s.text).unwrap_or_default();
        let mut unknowns = Vec::new();
        let mut seen = HashSet::new();

        for t in &tokens {
            if !t.is_content_word {
                continue;
            }
            let dict_form = &t.dictionary_form;
            if known_words.contains(dict_form) || ignored_words.contains(dict_form) {
                continue;
            }
            if seen.insert(dict_form.clone()) {
                unknowns.push(UnknownWord {
                    surface: t.surface.clone(),
                    dictionary_form: dict_form.clone(),
                    reading: t.reading.clone(),
                });
            }
        }

        let unknown_count = unknowns.len();
        let tier = match unknown_count {
            0 => Tier::I0,
            1 => Tier::I1,
            2 => Tier::I2,
            _ => Tier::I3Plus,
        };

        explorer_list.push(ExplorerSentence {
            sentence: s.clone(),
            tier,
            unknown_count,
            unknowns,
            selected_unknown: 0,
        });
    }

    sort_sentences(&mut explorer_list, SortOrder::Difficulty);
    explorer_list
}
