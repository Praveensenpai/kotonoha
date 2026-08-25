use super::*;

#[test]
fn placeholder_definition_is_detected() {
    assert!(is_placeholder_definition("1. [def] vocabulary word"));
    assert!(!is_placeholder_definition("1. [Noun] Monday"));
}

fn ensure_test_offline_dict() {
    if let Ok(cfg) = crate::config::AppConfig::load() {
        if let Ok(mut db) = crate::db::Database::open(&cfg.db_path) {
            let terms = vec![
                (
                    "台詞".to_string(),
                    "せりふ".to_string(),
                    "1. [Noun] speech, words, one's lines, dialogue".to_string(),
                    "0".to_string(),
                    "JMdict".to_string(),
                    100,
                ),
                (
                    "先走り".to_string(),
                    "さきばしり".to_string(),
                    "1. [Noun] acting rashly, running ahead, pre-cum".to_string(),
                    "0".to_string(),
                    "JMdict".to_string(),
                    100,
                ),
                (
                    "辺".to_string(),
                    "へん".to_string(),
                    "1. [Noun] area, vicinity, region".to_string(),
                    "1".to_string(),
                    "JMdict".to_string(),
                    100,
                ),
                (
                    "私".to_string(),
                    "わたし".to_string(),
                    "1. [Noun] I, me".to_string(),
                    "0".to_string(),
                    "JMdict".to_string(),
                    100,
                ),
            ];
            let _ = db.insert_offline_terms_batch(&terms);
        }
    }
}

#[tokio::test]
async fn test_serif_lookup() {
    let client = reqwest::Client::new();
    ensure_test_offline_dict();
    let res = DictionaryService::lookup(&client, "台詞").await.unwrap();
    println!("LOOKUP RESULT: {:?}", res);
    assert!(
        res.definition.contains("speech")
            || res.definition.contains("lines")
            || res.definition.contains("dialogue")
            || res.definition.contains("serif")
    );
}

#[tokio::test]
async fn test_sakibashiri_lookup() {
    let client = reqwest::Client::new();
    ensure_test_offline_dict();
    let res = DictionaryService::lookup(&client, "先走り").await.unwrap();
    println!("SAKIBASHIRI RESULT: {:?}", res);
    assert!(
        res.definition.contains("rash")
            || res.definition.contains("act")
            || res.definition.contains("ahead")
            || res.definition.contains("pre-cum")
            || res.definition.contains("ejaculate")
    );
}

#[tokio::test]
async fn test_definition_limits() {
    let client = reqwest::Client::new();
    let res = DictionaryService::lookup_with_limits(&client, "つまり", 2, 3)
        .await
        .unwrap();
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
    assert_eq!(
        context_hint("ひまわりの言うとおり、僕は用事があった", "とおり"),
        Some(ContextHint::AsStated)
    );
    assert_eq!(context_hint("この通りは広い", "通り"), None);
}

#[test]
fn prioritizes_contextual_toori_sense() {
    let raw = "1. [Noun] street, road, avenue\n│                 2. [Noun] traffic, coming and going\n│                 3. [Noun] in accordance with, according to, just as";
    let result = format_contextual_definition(raw, Some(ContextHint::AsStated), 3, 4);
    assert!(result.starts_with("1. [Noun] in accordance with"));
}

#[tokio::test]
async fn test_hen_lookup_first_result() {
    let client = reqwest::Client::new();
    ensure_test_offline_dict();
    let res = DictionaryService::lookup(&client, "辺").await.unwrap();
    println!("HEN RESULT: {:?}", res);
    assert_eq!(res.reading, "へん");
    assert!(
        res.definition.contains("area")
            || res.definition.contains("vicinity")
            || res.definition.contains("region")
    );
}

#[tokio::test]
async fn test_watashi_lookup_first_result() {
    let client = reqwest::Client::new();
    ensure_test_offline_dict();
    let res = DictionaryService::lookup(&client, "私").await.unwrap();
    println!("WATASHI RESULT: {:?}", res);
    assert_eq!(res.expression, "私");
    assert!(res.definition.contains("I") || res.definition.contains("me"));
}

#[tokio::test]
async fn test_sou_hiragana_lookup_returns_so_that_is_right() {
    let client = reqwest::Client::new();
    let res = DictionaryService::lookup(&client, "そう").await.unwrap();
    println!("SOU RESULT: {:?}", res);
    assert!(!res.definition.contains("vacuum"));
}

#[test]
fn test_yomitan_structured_content_extraction() {
    let json_val: serde_json::Value = serde_json::json!([
        {
            "type": "structured-content",
            "content": {
                "tag": "ul",
                "content": [
                    { "tag": "li", "content": "tactics" },
                    { "tag": "li", "content": "strategy" }
                ]
            }
        }
    ]);
    let mut glosses = Vec::new();
    super::offline::extract_text_from_yomitan_json(&json_val, &mut glosses);
    assert_eq!(glosses, vec!["tactics", "strategy"]);
}

#[tokio::test]
async fn test_ato_lookup_all_candidates() {
    let client = reqwest::Client::new();
    let candidates = DictionaryService::lookup_all_candidates(&client, "あと", 3, 4)
        .await
        .unwrap();
    println!("ATO CANDIDATES COUNT: {}", candidates.len());
    assert!(candidates.len() >= 2);
    let has_ato_after = candidates.iter().any(|c| {
        c.expression == "後" || c.definition.contains("behind") || c.definition.contains("after")
    });
    let has_ato_trace = candidates
        .iter()
        .any(|c| c.expression == "跡" || c.definition.contains("trace"));
    assert!(has_ato_after && has_ato_trace);
}

#[test]
fn test_format_pitch_accent_no_combining_characters() {
    let (reading, tag, morae) = format_pitch_accent("なん", 0);
    assert_eq!(reading, "なん");
    assert!(!reading.contains('\u{0305}'));
    assert!(!reading.contains('\u{0332}'));
    assert_eq!(tag, "[0] LH (2 morae)");
    assert_eq!(morae, 2);

    let (reading2, tag2, morae2) = format_pitch_accent("きみ", 0);
    assert_eq!(reading2, "きみ");
    assert!(!reading2.contains('\u{0305}'));
    assert!(!reading2.contains('\u{0332}'));
    assert_eq!(tag2, "[0] LH (2 morae)");
    assert_eq!(morae2, 2);
}
