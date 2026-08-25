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
fn test_ai_index_normalization() {
    let json_text = r#"{
        "results": [
            {
                "card_index": 0,
                "recommended_candidate_index": 1,
                "recommended_sense_index": 0,
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
    }
    assert_eq!(parsed.results[0].recommended_candidate_index, Some(0));
}

#[test]
fn test_has_valid_api_key() {
    use crate::config::AiSettings;

    let mut ai = AiSettings::default();
    ai.gemini_api_key = None;
    assert!(!ai.has_valid_api_key());

    ai.gemini_api_key = Some("".to_string());
    assert!(!ai.has_valid_api_key());

    ai.gemini_api_key = Some("   ".to_string());
    assert!(!ai.has_valid_api_key());

    ai.gemini_api_key = Some("YOUR_GEMINI_API_KEY_HERE".to_string());
    assert!(!ai.has_valid_api_key());

    ai.gemini_api_key = Some("your_api_key_here".to_string());
    assert!(!ai.has_valid_api_key());

    ai.gemini_api_key = Some("AIzaSyValidKey123".to_string());
    assert!(ai.has_valid_api_key());
}
