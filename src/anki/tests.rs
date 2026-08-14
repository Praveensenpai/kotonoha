use super::*;

#[tokio::test]
async fn test_anki_not_connected_error() {
    let client = reqwest::Client::new();
    let err = anki_request(
        &client,
        "http://127.0.0.1:18765",
        "version",
        serde_json::json!({}),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("Anki is not connected"));
    assert!(err
        .to_string()
        .contains("Please open Anki and make sure AnkiConnect is installed"));
}

#[test]
fn test_format_definition_for_anki() {
    let raw = "1. [Pre-noun adjectival (rentaishi)] such, that sort of, that kind of, like that\n│                 2. [Vocab] no way!, never!";
    assert_eq!(
        format_definition_for_anki(raw),
        "such, that sort of, that kind of, like that"
    );

    let raw2 = "1. [Noun] I, me (a neutral pronoun)\n│                 2. [Noun] some other sense";
    assert_eq!(
        format_definition_for_anki(raw2),
        "I, me (a neutral pronoun)"
    );

    let raw3 = "1. [Godan verb with 'ru' ending] to do, to undertake, to perform, to play (a game)\n│                 2. [Godan verb with 'ru' ending] to send";
    assert_eq!(
        format_definition_for_anki(raw3),
        "to do, to undertake, to perform, to play (a game)"
    );
}

#[test]
fn test_anki_search_text_escapes_query_delimiters() {
    assert_eq!(anki_search_text(r#"a\"b"#), r#"a\\\"b"#);
}
