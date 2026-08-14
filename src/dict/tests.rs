use super::*;

#[test]
fn placeholder_definition_is_detected() {
    assert!(is_placeholder_definition("1. [def] vocabulary word"));
    assert!(!is_placeholder_definition("1. [Noun] Monday"));
}

#[tokio::test]
async fn test_serif_lookup() {
    let client = reqwest::Client::new();
    let res = DictionaryService::lookup(&client, "セリフ").await.unwrap();
    println!("LOOKUP RESULT: {:?}", res);
    assert!(res.definition.contains("speech") || res.definition.contains("lines"));
}

#[tokio::test]
async fn test_sakibashiri_lookup() {
    let client = reqwest::Client::new();
    let res = DictionaryService::lookup(&client, "先走り").await.unwrap();
    println!("SAKIBASHIRI RESULT: {:?}", res);
    assert!(res.definition.contains("rash") || res.definition.contains("act") || res.definition.contains("ahead") || res.definition.contains("pre-cum") || res.definition.contains("ejaculate"));
}

#[tokio::test]
async fn test_definition_limits() {
    let client = reqwest::Client::new();
    let res = DictionaryService::lookup_with_limits(&client, "つまり", 2, 3).await.unwrap();
    println!("LIMITED RESULT:\n{}", res.definition);
    let lines: Vec<&str> = res.definition.lines().collect();
    assert!(lines.len() <= 2);
}

#[test]
fn test_truncate_definition() {
    let raw = "1. [Adverb] word1, word2, word3, word4, word5\n│                 2. [Adverb] wordA, wordB, wordC, wordD, wordE\n│                 3. [Noun] wordX, wordY, wordZ\n│                 4. [Noun] extra sense";
    let truncated = truncate_definition(raw, 2, 3);
    assert_eq!(
        truncated,
        "1. [Adverb] word1, word2, word3\n│                 2. [Adverb] wordA, wordB, wordC"
    );
}

#[test]
fn detects_as_stated_context() {
    assert_eq!(context_hint("ひまわりの言うとおり、僕は用事があった", "とおり"), Some(ContextHint::AsStated));
    assert_eq!(context_hint("この通りは広い", "通り"), None);
}

#[test]
fn prioritizes_contextual_toori_sense() {
    let raw = "1. [Noun] street, road, avenue\n│                 2. [Noun] traffic, coming and going\n│                 3. [Noun] in accordance with, according to, just as";
    let result = format_contextual_definition(
        raw,
        Some(ContextHint::AsStated),
        3,
        4,
    );
    assert!(result.starts_with("1. [Noun] in accordance with"));
}

#[tokio::test]
async fn test_hen_lookup_first_result() {
    let client = reqwest::Client::new();
    let res = DictionaryService::lookup(&client, "辺").await.unwrap();
    println!("HEN RESULT: {:?}", res);
    assert_eq!(res.reading, "へん");
    assert!(res.definition.contains("area") || res.definition.contains("vicinity") || res.definition.contains("region"));
}

#[tokio::test]
async fn test_ato_lookup_all_candidates() {
    let client = reqwest::Client::new();
    let candidates = DictionaryService::lookup_all_candidates(&client, "あと", 3, 4).await.unwrap();
    println!("ATO CANDIDATES COUNT: {}", candidates.len());
    assert!(candidates.len() >= 2);
    let has_ato_after = candidates.iter().any(|c| c.expression == "後" || c.definition.contains("behind") || c.definition.contains("after"));
    let has_ato_trace = candidates.iter().any(|c| c.expression == "跡" || c.definition.contains("trace"));
    assert!(has_ato_after && has_ato_trace);
}
