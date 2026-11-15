use super::*;
use std::collections::HashSet;
use std::path::PathBuf;

#[test]
fn test_find_nearest_sentence() {
    let subs = vec![
        SubtitleSentence {
            index: 1,
            start_ms: 1000,
            end_ms: 3000,
            text: "第一文".to_string(),
            video_path: None,
            actor: None,
        },
        SubtitleSentence {
            index: 2,
            start_ms: 5000,
            end_ms: 7000,
            text: "第二文".to_string(),
            video_path: None,
            actor: None,
        },
        SubtitleSentence {
            index: 3,
            start_ms: 10000,
            end_ms: 12000,
            text: "第三文".to_string(),
            video_path: None,
            actor: None,
        },
    ];

    // Near second sentence (6000ms is exactly midpoint of 5000..7000)
    let found = find_nearest_sentence(&subs, 6000).expect("should find nearest");
    assert_eq!(found.index, 2);

    // Near first sentence
    let found_first = find_nearest_sentence(&subs, 2000).expect("should find nearest");
    assert_eq!(found_first.index, 1);

    // Near third sentence
    let found_third = find_nearest_sentence(&subs, 11500).expect("should find nearest");
    assert_eq!(found_third.index, 3);
}

#[test]
fn test_collect_candidate_words_and_build() {
    let tokenizer = JapaneseTokenizer::new().expect("tokenizer init");
    let sentence = SubtitleSentence {
        index: 1,
        start_ms: 1000,
        end_ms: 3000,
        text: "明日新しい本を読む。".to_string(),
        video_path: Some(PathBuf::from("/anime/test.mkv")),
        actor: None,
    };

    let mut known = HashSet::new();
    known.insert("明日".to_string());
    let ignored = HashSet::new();

    let (tokens, words) =
        collect_candidate_words(&sentence, &tokenizer, &known, &ignored).expect("collect words");
    assert!(!words.is_empty());

    // Unknown content words (新しい, 本, 読む) should come before known (明日)
    let first_unknown = &words[0];
    assert!(first_unknown.2, "first word should be unknown");

    let cand = build_single_candidate(
        &sentence,
        std::slice::from_ref(&sentence),
        &first_unknown.0,
        &tokens,
        &known,
        &ignored,
    );
    assert_eq!(cand.target_word, first_unknown.0);
    assert_eq!(cand.sentence.text, sentence.text);
    assert_eq!(cand.video_path, PathBuf::from("/anime/test.mkv"));
}
