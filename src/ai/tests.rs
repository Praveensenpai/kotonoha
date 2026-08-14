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
