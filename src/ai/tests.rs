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
    assert!(!ai_none.has_valid_api_key());

    let ai_empty = AiSettings {
        gemini_api_key: Some("".to_string()),
        ..Default::default()
    };
    assert!(!ai_empty.has_valid_api_key());

    let ai_spaces = AiSettings {
        gemini_api_key: Some("   ".to_string()),
        ..Default::default()
    };
    assert!(!ai_spaces.has_valid_api_key());

    let ai_placeholder1 = AiSettings {
        gemini_api_key: Some("YOUR_GEMINI_API_KEY_HERE".to_string()),
        ..Default::default()
    };
    assert!(!ai_placeholder1.has_valid_api_key());

    let ai_placeholder2 = AiSettings {
        gemini_api_key: Some("your_api_key_here".to_string()),
        ..Default::default()
    };
    assert!(!ai_placeholder2.has_valid_api_key());

    let ai_valid = AiSettings {
        gemini_api_key: Some("AIzaSyValidKey123".to_string()),
        ..Default::default()
    };
    assert!(ai_valid.has_valid_api_key());
}
