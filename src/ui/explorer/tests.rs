use std::collections::HashSet;

use super::model::*;
use crate::nlp::JapaneseTokenizer;
use crate::srt::SubtitleSentence;

fn make_sentence(text: &str, index: usize, start_ms: u64) -> SubtitleSentence {
    SubtitleSentence {
        index,
        start_ms,
        end_ms: start_ms + 2000,
        text: text.to_string(),
        video_path: None,
    }
}

#[test]
fn test_explorer_sentence_tier_categorization() {
    let tokenizer = JapaneseTokenizer::new().expect("tokenizer");
    let mut known = HashSet::new();
    known.insert("私".to_string());
    known.insert("食べる".to_string());
    let ignored = HashSet::new();

    // Sentence 1: "私は食べる" -> 0 unknowns -> Tier::I0
    let s1 = make_sentence("私は食べる", 1, 1000);
    // Sentence 2: "林檎を食べる" -> 1 unknown ("林檎") -> Tier::I1
    let s2 = make_sentence("林檎を食べる", 2, 2000);
    // Sentence 3: "林檎と蜜柑を食べる" -> 2 unknowns ("林檎", "蜜柑") -> Tier::I2
    let s3 = make_sentence("林檎と蜜柑を食べる", 3, 3000);

    let sentences = vec![s3, s1, s2];
    let explorer = build_explorer_sentences(&sentences, &tokenizer, &known, &ignored);

    // By default, sorted by difficulty: i+0, then i+1, then i+2
    assert_eq!(explorer.len(), 3);
    assert_eq!(explorer[0].tier, Tier::I0);
    assert_eq!(explorer[0].unknown_count, 0);

    assert_eq!(explorer[1].tier, Tier::I1);
    assert_eq!(explorer[1].unknown_count, 1);
    assert_eq!(explorer[1].unknowns[0].dictionary_form, "林檎");

    assert_eq!(explorer[2].tier, Tier::I2);
    assert_eq!(explorer[2].unknown_count, 2);
    assert_eq!(explorer[2].unknowns[0].dictionary_form, "林檎");
    assert_eq!(explorer[2].unknowns[1].dictionary_form, "蜜柑");
}

#[test]
fn test_explorer_sort_toggle_and_timeline() {
    let tokenizer = JapaneseTokenizer::new().expect("tokenizer");
    let mut known = HashSet::new();
    known.insert("私".to_string());
    let ignored = HashSet::new();

    let s1 = make_sentence("蜜柑を食べる", 1, 5000); // 2 unknowns ("蜜柑", "食べる") @ 5000ms
    let s2 = make_sentence("私は走る", 2, 1000); // 1 unknown ("走る") @ 1000ms
    let s3 = make_sentence("私", 3, 3000); // 0 unknowns @ 3000ms

    let mut explorer = build_explorer_sentences(&[s1, s2, s3], &tokenizer, &known, &ignored);

    // Default: Difficulty sort (i+0 -> i+1 -> i+2)
    assert_eq!(explorer[0].tier, Tier::I0);
    assert_eq!(explorer[1].tier, Tier::I1);
    assert_eq!(explorer[2].tier, Tier::I2);

    // Toggle to Timeline sort (chronological: 1000ms -> 3000ms -> 5000ms)
    sort_sentences(&mut explorer, SortOrder::Timeline);
    assert_eq!(explorer[0].sentence.start_ms, 1000);
    assert_eq!(explorer[1].sentence.start_ms, 3000);
    assert_eq!(explorer[2].sentence.start_ms, 5000);
}

#[test]
fn test_explorer_filter_and_search() {
    let tokenizer = JapaneseTokenizer::new().expect("tokenizer");
    let known = HashSet::new();
    let ignored = HashSet::new();

    let s1 = make_sentence("林檎を食べる", 1, 1000);
    let s2 = make_sentence("蜜柑を買う", 2, 2000);
    let s3 = make_sentence("走る", 3, 3000);

    let explorer = build_explorer_sentences(&[s1, s2, s3], &tokenizer, &known, &ignored);

    // Filter by search text "林檎"
    let matches = filter_sentences(&explorer, TierFilter::All, "林檎");
    assert_eq!(matches.len(), 1);
    assert_eq!(explorer[matches[0]].sentence.text, "林檎を食べる");

    // Search with case/no match
    let empty_matches = filter_sentences(&explorer, TierFilter::All, "存在しない単語");
    assert!(empty_matches.is_empty());
}

#[test]
fn test_dynamic_refresh_sentence_unknowns() {
    let tokenizer = JapaneseTokenizer::new().expect("tokenizer");
    let mut known = HashSet::new();
    let ignored = HashSet::new();

    let s = make_sentence("林檎と蜜柑を食べる", 1, 1000);
    let mut explorer = build_explorer_sentences(&[s], &tokenizer, &known, &ignored);

    assert_eq!(explorer[0].tier, Tier::I3Plus); // 林檎, 蜜柑, 食べる (3 unknowns)
    assert_eq!(explorer[0].unknown_count, 3);

    // Simulate multi-card mining or marking words as known:
    known.insert("林檎".to_string());
    known.insert("蜜柑".to_string());
    refresh_sentence_unknowns(&mut explorer[0], &known, &ignored);

    // Now only "食べる" remains unknown -> Transitioned to i+1!
    assert_eq!(explorer[0].tier, Tier::I1);
    assert_eq!(explorer[0].unknown_count, 1);
    assert_eq!(explorer[0].unknowns[0].dictionary_form, "食べる");

    // Learn the last word -> Transitioned to i+0!
    known.insert("食べる".to_string());
    refresh_sentence_unknowns(&mut explorer[0], &known, &ignored);

    assert_eq!(explorer[0].tier, Tier::I0);
    assert_eq!(explorer[0].unknown_count, 0);
    assert!(explorer[0].unknowns.is_empty());
}
