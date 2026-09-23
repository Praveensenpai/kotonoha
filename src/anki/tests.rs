use super::client::*;
use super::formatter::*;

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

#[test]
fn test_sentence_with_furigana_highlights_full_predicate() {
    let tokenizer = crate::nlp::JapaneseTokenizer::new().unwrap();

    let res = sentence_with_furigana(&tokenizer, "私はご飯を食べたい", "食べる");
    assert!(res.contains("<b><ruby>食べ<rt>たべ</rt></ruby>たい</b>"));

    let res2 = sentence_with_furigana(&tokenizer, "本を読んだ", "読む");
    assert!(res2.contains("<b><ruby>読ん<rt>よん</rt></ruby>だ</b>"));

    let res3 = sentence_with_furigana(&tokenizer, "映画を見ていたよ", "見る");
    assert!(res3.contains("<b><ruby>見<rt>み</rt></ruby>ていた</b>よ"));

    let res4 = sentence_with_furigana(&tokenizer, "学校に行く", "学校");
    assert_eq!(
        res4,
        "<b><ruby>学校<rt>がっこう</rt></ruby></b>に<ruby>行く<rt>いく</rt></ruby>"
    );
}
