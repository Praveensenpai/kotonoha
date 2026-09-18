use crate::nlp::TokenInfo;

#[cfg(test)]
mod tests;

/// Evaluates sentence quality, grammatical self-containment, and naturalness
/// for Japanese sentence mining (i+1 cards).
pub struct QualityScorer;

impl QualityScorer {
    /// Computes a unified naturalness and completeness score in range [0.0, 1.0].
    pub fn score(text: &str, target_word: &str, tokens: &[TokenInfo]) -> f32 {
        let char_count = text.chars().count();
        if char_count < 4 {
            return 0.0;
        }

        let term_score = score_termination(text);
        let len_score = score_length(char_count);
        let (sem_score, target_conn_score) = score_semantics(text, target_word, tokens);
        let noise_penalty = score_noise(text, tokens);

        let mut composite = (0.35 * term_score)
            + (0.30 * len_score)
            + (0.23 * sem_score)
            + (0.12 * target_conn_score)
            - noise_penalty;

        // If sentence termination is defective (dangling connective/particle/cut-off),
        // gate the score so broken fragments cannot score as high-quality cards.
        if term_score < 0.30 {
            composite *= (term_score / 0.30).max(0.10);
        }

        composite.clamp(0.0, 1.0)
    }
}

/// Trims ending punctuation, quotation marks, and ellipsis from subtitle text.
fn clean_sentence_end(text: &str) -> &str {
    text.trim_end_matches(|c: char| {
        matches!(
            c,
            '。' | '！'
                | '？'
                | '!'
                | '?'
                | '…'
                | '‥'
                | '.'
                | '―'
                | 'ー'
                | '」'
                | '』'
                | ')'
                | '）'
                | ' '
                | '　'
        )
    })
}

/// Evaluates whether the sentence terminates in a complete predicate or clean particle.
fn score_termination(text: &str) -> f32 {
    let clean = clean_sentence_end(text);
    if clean.is_empty() {
        return 0.0;
    }

    let has_trailing_ellipsis = text.ends_with('…') || text.ends_with("...") || text.ends_with('‥');
    let mut base_score = match_termination_pattern(clean);

    if has_trailing_ellipsis {
        base_score = (base_score - 0.20).max(0.05);
    }

    base_score
}

/// Matches sentence suffix against grammatical termination patterns.
fn match_termination_pattern(clean: &str) -> f32 {
    // Severe penalties for dangling connective/subordinate endings
    const DANGLING_CONNECTIVES: &[&str] = &[
        "て",
        "で",
        "けど",
        "けれど",
        "けれども",
        "からさ",
        "のに",
        "たら",
        "なら",
        "ば",
        "たり",
        "だり",
        "ながら",
        "つつ",
        "し",
    ];
    for ending in DANGLING_CONNECTIVES {
        if clean.ends_with(ending) {
            return 0.10;
        }
    }

    // Severe penalties for dangling case/topic particles
    const DANGLING_PARTICLES: &[&str] = &[
        "は", "が", "を", "に", "で", "へ", "と", "も", "より", "って",
    ];
    for ending in DANGLING_PARTICLES {
        if clean.ends_with(ending) {
            return 0.08;
        }
    }

    // High score for polite predicates, copulas, and conclusive endings
    const COMPLETE_PREDICATES: &[&str] = &[
        "です",
        "でした",
        "でしょう",
        "ます",
        "ました",
        "ません",
        "ませんでした",
        "だ",
        "である",
        "だろう",
        "だった",
        "ではない",
        "じゃない",
        "たい",
        "たかった",
        "たくない",
        "ない",
        "なかった",
        "まい",
        "よう",
        "ろう",
        "ましょう",
        "ろ",
    ];
    for ending in COMPLETE_PREDICATES {
        if clean.ends_with(ending) {
            return 1.00;
        }
    }

    // Sentence-ending particles (終助詞)
    const FINAL_PARTICLES: &[&str] = &[
        "よ",
        "ね",
        "わ",
        "ぞ",
        "ぜ",
        "な",
        "さ",
        "もん",
        "っけ",
        "かい",
        "かしら",
        "か",
        "の",
        "んだ",
        "のだ",
    ];
    for ending in FINAL_PARTICLES {
        if clean.ends_with(ending) {
            return 0.95;
        }
    }

    // Plain past/completed forms (た / だ)
    if clean.ends_with('た') || clean.ends_with('だ') {
        return 0.90;
    }

    // Plain non-past verb endings (u-row hiragana)
    if clean.ends_with(['る', 'う', 'く', 'ぐ', 'す', 'つ', 'ぬ', 'ぶ', 'む']) {
        return 0.85;
    }

    // Plain i-adjective ending (い)
    if clean.ends_with('い') {
        return 0.80;
    }

    // Noun-ending (体言止め) without copula
    0.35
}

/// Evaluates sentence length distribution relative to the flashcard sweet-spot.
fn score_length(char_count: usize) -> f32 {
    match char_count {
        0..=5 => 0.15,
        6..=9 => 0.40,
        10..=13 => 0.75,
        14..=32 => 1.00,
        33..=45 => 0.85,
        46..=60 => 0.65,
        _ => 0.40,
    }
}

/// Evaluates presence of case particles and grammatical connection to target word.
fn score_semantics(text: &str, target_word: &str, _tokens: &[TokenInfo]) -> (f32, f32) {
    const CASE_PARTICLES: &[&str] = &[
        "を", "に", "が", "で", "へ", "と", "より", "から", "は", "も",
    ];

    let mut particle_count = 0;
    for p in CASE_PARTICLES {
        if text.contains(p) {
            particle_count += 1;
        }
    }

    let sem_score = match particle_count {
        0 => 0.10,
        1 => 0.55,
        2 => 0.85,
        _ => 1.00,
    };

    let mut target_conn = 0.20;
    for p in CASE_PARTICLES {
        let pattern = format!("{target_word}{p}");
        if text.contains(&pattern) {
            target_conn = 1.00;
            break;
        }
    }

    (sem_score, target_conn)
}

/// Penalizes sentences burdened with audio grunts, interjections, or disjointed starters.
fn score_noise(text: &str, tokens: &[TokenInfo]) -> f32 {
    let mut penalty: f32 = 0.0;

    const ORPHAN_CONJUNCTIONS: &[&str] = &[
        "だけど",
        "だから",
        "でも",
        "しかし",
        "なのに",
        "ところが",
        "それで",
    ];
    for conj in ORPHAN_CONJUNCTIONS {
        if text.starts_with(conj) {
            penalty += 0.15;
            break;
        }
    }

    const INTERJECTIONS: &[&str] = &[
        "あっ", "えっ", "うっ", "おっ", "はっ", "ふっ", "んっ", "くっ", "ちっ", "あー", "えー",
        "うー", "おー", "おい", "ねえ", "ほら", "うわ",
    ];
    for inter in INTERJECTIONS {
        if text.starts_with(inter) {
            penalty += 0.10;
            break;
        }
    }

    if !tokens.is_empty() {
        let non_content_count = tokens.iter().filter(|t| !t.is_content_word).count();
        let non_content_ratio = non_content_count as f32 / tokens.len() as f32;
        if non_content_ratio > 0.80 {
            penalty += 0.15;
        }
    }

    penalty
}
