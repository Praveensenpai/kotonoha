use super::normalize_colloquial_negative;

#[test]
fn normalizes_rough_negative_form() {
    assert_eq!(
        normalize_colloquial_negative("いけねえ", "いける".into(), "いける".into()),
        ("いけない".into(), "いけない".into())
    );
}

#[test]
fn leaves_non_negative_forms_unchanged() {
    assert_eq!(
        normalize_colloquial_negative("ねえ", "ねえ".into(), "ねえ".into()),
        ("ねえ".into(), "ねえ".into())
    );
}

#[test]
fn normalizes_split_rough_negative_sentence() {
    let tokenizer = super::JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("バカ言っちゃいけねえよ！").unwrap();
    let token = tokens.iter().find(|token| token.surface == "いけ").unwrap();
    assert_eq!(token.dictionary_form, "いけない");
    assert_eq!(token.reading, "いけない");
}

#[test]
fn normalizes_kurenai_to_kureru() {
    let tokenizer = super::JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("考えてきてくれたの？").unwrap();
    let kangaeru = tokens.iter().find(|token| token.surface.contains("考え")).unwrap();
    assert!(kangaeru.is_content_word);
    assert_eq!(kangaeru.dictionary_form, "考える");

    let aux_ki = tokens.iter().find(|token| token.surface == "き").unwrap();
    assert!(!aux_ki.is_content_word);
}

#[test]
fn keeps_colloquial_small_tsu_words_together() {
    let tokenizer = super::JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("うわ キモッ むっ…").unwrap();
    let token = tokens
        .iter()
        .find(|token| token.surface == "キモッ")
        .unwrap();

    assert_eq!(token.dictionary_form, "キモッ");
    assert_eq!(token.reading, "きもっ");
    assert!(!tokens.iter().any(|token| token.dictionary_form == "モッ"));
}

#[test]
fn does_not_merge_small_tsu_across_whitespace() {
    let tokenizer = super::JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("キモ ッ").unwrap();

    assert!(!tokens.iter().any(|token| token.surface == "キモッ"));
}

#[test]
fn leaves_regular_words_unchanged() {
    let tokenizer = super::JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("ササササ サンちゃん").unwrap();
    let surfaces: String = tokens.iter().map(|token| token.surface.as_str()).collect();

    assert_eq!(surfaces, "ササササ サンちゃん");
    assert!(!tokens.iter().any(|token| token.dictionary_form == "キモッ"));
}

#[test]
fn resolves_sentence_final_imperative() {
    let tokenizer = super::JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("ジョーロも生徒会 頑張れよ").unwrap();
    let token = tokens.iter().find(|token| token.surface == "頑張れよ").unwrap();

    assert_eq!(token.dictionary_form, "頑張る");
    assert_eq!(token.reading, "がんばる");
}

#[test]
fn preserves_non_imperative_potential_form() {
    let tokenizer = super::JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("彼は頑張れる").unwrap();
    let token = tokens.iter().find(|token| token.surface == "頑張れる").unwrap();

    assert_eq!(token.dictionary_form, "頑張れる");
}

#[test]
fn keeps_adverb_naru_phrase_together() {
    let tokenizer = super::JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("何がどうして こうなった!? ").unwrap();
    let token = tokens.iter().find(|token| token.dictionary_form == "こうなる").unwrap();

    assert_eq!(token.surface, "こうなっ");
    assert_eq!(token.reading, "こうなる");
    assert!(!tokens.iter().any(|token| token.dictionary_form == "こうなっ"));
}

#[test]
fn keeps_yori_ni_mo_yotte_expression_together() {
    let tokenizer = super::JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("よりにもよって こいつだけ！").unwrap();
    let token = tokens
        .iter()
        .find(|token| token.dictionary_form == "よりにもよって")
        .unwrap();

    assert_eq!(token.surface, "よりにもよって");
    assert_eq!(token.reading, "よりにもよって");
    assert!(!tokens.iter().any(|token| token.dictionary_form == "よりにもよっ"));
}

#[test]
fn filters_explanatory_nan() {
    let tokenizer = super::JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer
        .tokenize("君に伝えたいことが あるからなんだ")
        .unwrap();
    let token = tokens.iter().find(|token| token.surface == "なん").unwrap();

    assert!(!token.is_content_word);
}

#[test]
fn keeps_moshikashite_expression_together() {
    let tokenizer = super::JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer
        .tokenize("だから もしかしてって 思ってたけど―")
        .unwrap();
    let token = tokens
        .iter()
        .find(|token| token.dictionary_form == "もしかして")
        .unwrap();

    assert_eq!(token.surface, "もしかして");
    assert_eq!(token.reading, "もしかして");
    assert!(!tokens.iter().any(|token| token.dictionary_form == "もし"));
}

#[test]
fn recognizes_grammar_expressions_as_targets() {
    let tokenizer = super::JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer
        .tokenize("言わざるを得ない事態になった")
        .unwrap();
    let grammar_token = tokens
        .iter()
        .find(|token| token.dictionary_form == "ざるを得ない")
        .unwrap();

    assert_eq!(grammar_token.surface, "ざるを得ない");
    assert!(grammar_token.is_content_word);
}

#[test]
fn merges_complex_causative_passive_inflection() {
    let tokenizer = super::JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer
        .tokenize("ピーマンを食べさせられた")
        .unwrap();
    let verb_token = tokens
        .iter()
        .find(|token| token.dictionary_form == "食べさせられる" || token.dictionary_form == "食べさせられた")
        .unwrap();

    assert!(verb_token.is_content_word);
}

#[test]
fn recognizes_dakede_particle_grammar_as_target() {
    let tokenizer = super::JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer
        .tokenize("会えるだけで幸せだ")
        .unwrap();
    let dakede_token = tokens
        .iter()
        .find(|token| token.dictionary_form == "だけで")
        .unwrap();

    assert_eq!(dakede_token.surface, "だけで");
    assert!(dakede_token.is_content_word);
}

#[test]
fn recognizes_formal_nouns_as_content_words() {
    let tokenizer = super::JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("そのことを教えて").unwrap();
    let koto_token = tokens
        .iter()
        .find(|token| token.dictionary_form == "こと")
        .unwrap();

    assert_eq!(koto_token.surface, "こと");
    assert!(koto_token.is_content_word);
}

#[test]
fn recognizes_datte_conjunction_as_content_word() {
    let tokenizer = super::JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("だって私 あなたのことを...").unwrap();
    let datte = tokens.iter().find(|t| t.surface == "だって").unwrap();
    assert!(datte.is_content_word);
}

#[test]
fn normalizes_colloquial_ohayou_greeting() {
    let tokenizer = super::JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("おっはよー 諸君！").unwrap();
    let ohayou = tokens.iter().find(|t| t.dictionary_form == "おはよう").unwrap();
    assert_eq!(ohayou.surface, "おっはよー");
    assert!(ohayou.is_content_word);
}

#[test]
fn filters_standalone_audio_grunt() {
    let tokenizer = super::JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("おっ！誰か来た").unwrap();
    let otsu = tokens.iter().find(|t| t.surface == "おっ").unwrap();
    assert!(!otsu.is_content_word);
}
