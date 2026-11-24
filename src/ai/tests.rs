use super::build_prompt;

#[test]
fn prompt_requires_contextual_custom_gloss_for_sei() {
    let prompt = build_prompt(
        r#"Card Index #0:
Sentence: "私のせいですか？"
Target Word: "せい"
Candidates:
Candidate #1: Expression: せい, Reading: せい
Definitions:
1. [Noun] consequence, outcome, result, blame"#,
    );

    assert!(prompt.contains("Always provide a concise, custom English"));
    assert!(prompt.contains(
        "For example, for `私のせいですか？`, return exactly `fault; blame; cause of a bad result`"
    ));
    assert!(prompt.contains("Sentence: \"私のせいですか？\""));
}

#[test]
fn test_prompt_includes_dialogue_context_and_show() {
    let summary = r#"Card Index #0:
Show / Episode: "Sousou no Frieren - Episode 08"
Dialogue Context:
    [Prev -2]: 魔法使いの戦いは魔力の制限で決まる。
    [Prev -1]: だからお前はまだ未熟なんだ。
  >>> [TARGET SENTENCE]: 私のせいですか？ <<<
    [Next +1]: そうじゃない。
Target Word: "せい"
Candidates:
Candidate #1: Expression: せい, Reading: せい"#;

    let prompt = build_prompt(summary);
    assert!(prompt.contains("Show / Episode: \"Sousou no Frieren - Episode 08\""));
    assert!(prompt.contains(">>> [TARGET SENTENCE]: 私のせいですか？ <<<"));
    assert!(prompt.contains("[Prev -1]: だからお前はまだ未熟なんだ。"));
    assert!(prompt.contains("[Next +1]: そうじゃない。"));
    assert!(prompt.contains("surrounding conversation and show theme"));
}

#[test]
fn test_ai_index_normalization() {
    let json_text = r#"{
        "results": [
            {
                "card_index": 0,
                "recommended_candidate_index": 1,
                "recommended_sense_index": 1,
                "parsing_warning": null,
                "custom_definition_suggestion": null,
                "explanation": null
            }
        ]
    }"#;
    let mut parsed: super::BatchAiAnalysisResponse = serde_json::from_str(json_text).unwrap();
    for res in &mut parsed.results {
        if let Some(cand_idx) = res.recommended_candidate_index {
            if cand_idx > 0 {
                res.recommended_candidate_index = Some(cand_idx - 1);
            }
        }
        if let Some(sense_idx) = res.recommended_sense_index {
            if sense_idx > 0 {
                res.recommended_sense_index = Some(sense_idx - 1);
            }
        }
    }
    assert_eq!(parsed.results[0].recommended_candidate_index, Some(0));
    assert_eq!(parsed.results[0].recommended_sense_index, Some(0));
}

#[test]
fn test_has_valid_api_key() {
    use crate::config::AiSettings;

    let ai_none = AiSettings {
        gemini_api_key: None,
        ..Default::default()
    };
    assert!(!ai_none.has_valid_gemini_key());
    assert!(!ai_none.has_valid_api_key());

    let ai_empty = AiSettings {
        gemini_api_key: Some("".to_string()),
        ..Default::default()
    };
    assert!(!ai_empty.has_valid_gemini_key());
    assert!(!ai_empty.has_valid_api_key());

    let ai_spaces = AiSettings {
        gemini_api_key: Some("   ".to_string()),
        ..Default::default()
    };
    assert!(!ai_spaces.has_valid_gemini_key());

    let ai_placeholder1 = AiSettings {
        gemini_api_key: Some("YOUR_GEMINI_API_KEY_HERE".to_string()),
        ..Default::default()
    };
    assert!(!ai_placeholder1.has_valid_gemini_key());

    let ai_placeholder2 = AiSettings {
        gemini_api_key: Some("your_api_key_here".to_string()),
        ..Default::default()
    };
    assert!(!ai_placeholder2.has_valid_gemini_key());

    let ai_valid = AiSettings {
        gemini_api_key: Some("AIzaSyValidKey123".to_string()),
        ..Default::default()
    };
    assert!(ai_valid.has_valid_gemini_key());
    assert!(ai_valid.has_valid_api_key());
}

#[test]
fn test_prompt_requests_dual_translations() {
    let prompt = build_prompt("dummy cards");
    assert!(prompt.contains("english_natural"));
    assert!(prompt.contains("english_literal"));
    assert!(prompt.contains("TWO distinct English translations"));
}

#[test]
fn test_json_deserialization_includes_dual_translations() {
    let json_text = r#"{
        "results": [
            {
                "card_index": 0,
                "recommended_candidate_index": null,
                "recommended_sense_index": null,
                "recommended_reading": null,
                "parsing_warning": null,
                "custom_definition_suggestion": "senior; superior",
                "explanation": null,
                "english_natural": "Let's do it too, senpai.",
                "english_literal": "Senpai too, let's do it."
            }
        ]
    }"#;
    let parsed: super::BatchAiAnalysisResponse = serde_json::from_str(json_text).unwrap();
    assert_eq!(
        parsed.results[0].english_natural.as_deref(),
        Some("Let's do it too, senpai.")
    );
    assert_eq!(
        parsed.results[0].english_literal.as_deref(),
        Some("Senpai too, let's do it.")
    );
}

#[test]
fn test_parse_gemini_response_valid() {
    let body = serde_json::json!({
        "candidates": [{
            "content": {
                "parts": [{
                    "text": "```json\n{\"results\": [{\"card_index\": 0, \"recommended_candidate_index\": 1, \"recommended_sense_index\": 2, \"recommended_reading\": \"さき\", \"parsing_warning\": null, \"custom_definition_suggestion\": \"point; tip\", \"explanation\": null, \"english_natural\": \"Natural\", \"english_literal\": \"Literal\"}]}\n```"
                }]
            }
        }]
    });
    let results = super::parse_gemini_response(body).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].card_index, 0);
    assert_eq!(results[0].recommended_candidate_index, Some(0));
    assert_eq!(results[0].recommended_sense_index, Some(1));
    assert_eq!(
        results[0].custom_definition_suggestion.as_deref(),
        Some("point; tip")
    );
}

#[test]
fn test_parse_gemini_response_invalid_structure() {
    let body = serde_json::json!({
        "error": {
            "code": 503,
            "message": "Service unavailable"
        }
    });
    let result = super::parse_gemini_response(body);
    assert!(result.is_err());
}

use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn spawn_mock_gemini_server(
    responses: Vec<(&'static str, &'static str)>,
) -> (String, tokio::task::JoinHandle<usize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    let handle = tokio::spawn(async move {
        let mut count = 0;
        for (status_line, body) in responses {
            if let Ok((mut stream, _)) = listener.accept().await {
                count += 1;
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf).await;
                let resp = format!(
                    "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    status_line,
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        }
        count
    });

    (base_url, handle)
}

#[tokio::test]
async fn test_gemini_retries_on_503_and_succeeds() {
    let valid_json = r#"{"candidates":[{"content":{"parts":[{"text":"{\"results\":[{\"card_index\":0,\"recommended_candidate_index\":null,\"recommended_sense_index\":null,\"recommended_reading\":null,\"parsing_warning\":null,\"custom_definition_suggestion\":\"test\",\"explanation\":null,\"english_natural\":null,\"english_literal\":null}]}"}]}}]}"#;
    let err_503 = r#"{"error":{"code":503,"message":"Service Unavailable"}}"#;

    let (base_url, server_handle) = spawn_mock_gemini_server(vec![
        ("503 Service Unavailable", err_503),
        ("200 OK", valid_json),
    ])
    .await;

    let client = reqwest::Client::new();
    let opts = super::GeminiRequestOptions {
        base_url: &base_url,
        api_key: "dummy_key",
        model: "gemini-3.5-flash-lite",
        retry_delays: &[0, 0],
    };
    let cards = [super::CardBatchInput {
        card_index: 0,
        sentence: "テスト",
        target_word: "テスト",
        target_reading: "てすと",
        candidates: &[],
        series_title: None,
        before_context: &[],
        after_context: &[],
    }];

    let res = super::GeminiAiService::analyze_batch_internal(&client, &opts, &cards).await;
    assert!(res.is_ok(), "Expected success after retry: {:?}", res);
    let attempts = server_handle.await.unwrap();
    assert_eq!(
        attempts, 2,
        "Server should have received 2 requests (1 failure + 1 retry)"
    );
}

#[tokio::test]
async fn test_gemini_fast_fails_on_401_without_retrying() {
    let err_401 = r#"{"error":{"code":401,"message":"API key not valid"}}"#;

    let (base_url, server_handle) =
        spawn_mock_gemini_server(vec![("401 Unauthorized", err_401), ("200 OK", "{}")]).await;

    let client = reqwest::Client::new();
    let opts = super::GeminiRequestOptions {
        base_url: &base_url,
        api_key: "invalid_key",
        model: "gemini-3.5-flash-lite",
        retry_delays: &[0, 0, 0, 0, 0],
    };
    let cards = [super::CardBatchInput {
        card_index: 0,
        sentence: "テスト",
        target_word: "テスト",
        target_reading: "てすと",
        candidates: &[],
        series_title: None,
        before_context: &[],
        after_context: &[],
    }];

    let res = super::GeminiAiService::analyze_batch_internal(&client, &opts, &cards).await;
    assert!(res.is_err(), "Expected 401 error");
    let err_msg = res.unwrap_err().to_string();
    assert!(err_msg.contains("401"), "Error message should contain 401");
    server_handle.abort();
}
