use super::*;
use crate::srt::SubtitleSentence;

#[test]
fn unignored_name_is_counted_as_unknown_and_mineable() {
    let tokenizer = JapaneseTokenizer::new().unwrap();
    let engine = MiningEngine::new(tokenizer);

    let sentence = SubtitleSentence {
        index: 1,
        start_ms: 0,
        end_ms: 1000,
        text: "東京へ行く".to_string(),
        video_path: None,
    };

    let mut known = HashSet::new();
    known.insert("行く".to_string());
    let ignored = HashSet::new();

    let candidates = engine.find_candidates(&[sentence], &known, &ignored);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].target_word, "東京");
}

#[test]
fn ignored_name_is_skipped_and_not_counted_as_unknown() {
    let tokenizer = JapaneseTokenizer::new().unwrap();
    let engine = MiningEngine::new(tokenizer);

    let sentence = SubtitleSentence {
        index: 1,
        start_ms: 0,
        end_ms: 1000,
        text: "東京へ行く".to_string(),
        video_path: None,
    };

    let mut known = HashSet::new();
    known.insert("行く".to_string());
    let mut ignored = HashSet::new();
    ignored.insert("東京".to_string());

    let candidates = engine.find_candidates(&[sentence], &known, &ignored);
    // All words are known or ignored -> 0 unknowns, not i+1
    assert_eq!(candidates.len(), 0);
}

#[test]
fn katakana_loanword_is_mineable_as_i1_candidate() {
    let tokenizer = JapaneseTokenizer::new().unwrap();
    let engine = MiningEngine::new(tokenizer);

    let sentence = SubtitleSentence {
        index: 1,
        start_ms: 0,
        end_ms: 1000,
        text: "温かいコーヒーを飲む".to_string(),
        video_path: None,
    };

    let mut known = HashSet::new();
    known.insert("温かい".to_string());
    known.insert("飲む".to_string());
    let ignored = HashSet::new();

    let candidates = engine.find_candidates(&[sentence], &known, &ignored);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].target_word, "コーヒー");
}

#[test]
fn complete_sentence_preferred_over_fragment_for_same_target() {
    let tokenizer = JapaneseTokenizer::new().unwrap();
    let engine = MiningEngine::new(tokenizer);

    let fragment = SubtitleSentence {
        index: 1,
        start_ms: 0,
        end_ms: 1000,
        text: "そうか、コーヒー。".to_string(),
        video_path: None,
    };

    let rich = SubtitleSentence {
        index: 2,
        start_ms: 2000,
        end_ms: 4000,
        text: "温かいコーヒーを飲むよ。".to_string(),
        video_path: None,
    };

    let mut known = HashSet::new();
    known.insert("温かい".to_string());
    known.insert("飲む".to_string());
    let ignored = HashSet::new();

    let candidates = engine.find_candidates(&[fragment, rich], &known, &ignored);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].target_word, "コーヒー");
    assert_eq!(
        candidates[0].sentence.text, "温かいコーヒーを飲むよ。",
        "MiningEngine must pick the complete context sentence over the fragment"
    );
    assert!(candidates[0].quality_score > 0.70);
}

#[test]
fn verbs_like_machigau_affect_i1_candidate_generation() {
    let tokenizer = JapaneseTokenizer::new().unwrap();
    let engine = MiningEngine::new(tokenizer);

    let sentence = SubtitleSentence {
        index: 1,
        start_ms: 0,
        end_ms: 1000,
        text: "なでしこ 私が間違ってたぞ！".to_string(),
        video_path: None,
    };

    // Scenario 1: Only "私" is known -> sentence has 2 unknowns ("なでしこ", "間違う")
    // It is i+2, so it MUST NOT be yielded as an i+1 candidate!
    let mut known = HashSet::new();
    known.insert("私".to_string());
    let ignored = HashSet::new();
    let candidates = engine.find_candidates(std::slice::from_ref(&sentence), &known, &ignored);
    assert_eq!(
        candidates.len(),
        0,
        "When 間違う is unknown, sentence has 2 unknowns and must NOT be an i+1 candidate"
    );

    // Scenario 2: Both "私" and "間違う" are known -> sentence has 1 unknown ("なでしこ")
    known.insert("間違う".to_string());
    let candidates = engine.find_candidates(std::slice::from_ref(&sentence), &known, &ignored);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].target_word, "なでしこ");
    assert!(candidates[0]
        .known_context_words
        .contains(&"間違う".to_string()));
    assert!(candidates[0]
        .known_context_words
        .contains(&"私".to_string()));

    // Scenario 3: "私" and "なでしこ" are known -> "間違う" is the single unknown target!
    let mut known3 = HashSet::new();
    known3.insert("私".to_string());
    known3.insert("なでしこ".to_string());
    let candidates = engine.find_candidates(std::slice::from_ref(&sentence), &known3, &ignored);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].target_word, "間違う");
    assert!(candidates[0]
        .known_context_words
        .contains(&"なでしこ".to_string()));
    assert!(candidates[0]
        .known_context_words
        .contains(&"私".to_string()));
}
