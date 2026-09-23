use crate::nlp::JapaneseTokenizer;

#[test]
fn test_merges_futari_counter() {
    let tokenizer = JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("あ～ ２人で行ってきたんやね").unwrap();
    let futari = tokens
        .iter()
        .find(|t| t.surface == "２人")
        .expect("２人 token must exist");
    assert_eq!(futari.dictionary_form, "二人");
    assert_eq!(futari.reading, "ふたり");
    assert_eq!(futari.surface_reading, "ふたり");
    assert!(futari.is_content_word, "二人 must be a content word");
}

#[test]
fn test_merges_hitori_counter() {
    let tokenizer = JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer
        .tokenize("この子 １人でキャンプしてるのかな")
        .unwrap();
    let hitori = tokens
        .iter()
        .find(|t| t.surface == "１人")
        .expect("１人 token must exist");
    assert_eq!(hitori.dictionary_form, "一人");
    assert_eq!(hitori.reading, "ひとり");
    assert!(hitori.is_content_word, "一人 must be a content word");
}

#[test]
fn test_merges_rokuji_time() {
    let tokenizer = JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("６時くらいかな").unwrap();
    let rokuji = tokens
        .iter()
        .find(|t| t.surface == "６時")
        .expect("６時 token must exist");
    assert_eq!(rokuji.reading, "ろくじ");
    assert!(
        !rokuji.is_content_word,
        "arbitrary time should not be isolated content word"
    );
    assert!(!tokens
        .iter()
        .any(|t| t.surface == "時" && t.is_content_word));
}

#[test]
fn test_merges_senen_satsu() {
    let tokenizer = JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("千円札の絵にもなってるって―").unwrap();
    let bill = tokens
        .iter()
        .find(|t| t.surface == "千円札")
        .expect("千円札 token must exist");
    assert_eq!(bill.dictionary_form, "千円札");
    assert_eq!(bill.reading, "せんえんさつ");
    assert!(bill.is_content_word);
}

#[test]
fn test_merges_three_people_counter() {
    let tokenizer = JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("３人で行こう").unwrap();
    let sannin = tokens
        .iter()
        .find(|t| t.surface == "３人")
        .expect("３人 token must exist");
    assert_eq!(sannin.reading, "さんにん");
    assert!(!sannin.is_content_word);
}

#[test]
fn test_merges_hitotsu_futatsu() {
    let tokenizer = JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("２つ買ってきて").unwrap();
    let futatsu = tokens
        .iter()
        .find(|t| t.surface == "２つ")
        .expect("２つ token must exist");
    assert_eq!(futatsu.dictionary_form, "二つ");
    assert_eq!(futatsu.reading, "ふたつ");
    assert!(futatsu.is_content_word);
}

#[test]
fn test_merges_hatachi_age() {
    let tokenizer = JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("もう 20歳になったんだ").unwrap();
    let hatachi = tokens
        .iter()
        .find(|t| t.surface == "20歳")
        .expect("20歳 token must exist");
    assert_eq!(hatachi.dictionary_form, "二十歳");
    assert_eq!(hatachi.reading, "はたち");
    assert!(hatachi.is_content_word);
}

#[test]
fn test_merges_interrogatives() {
    let tokenizer = JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("今 何時？").unwrap();
    let nanji = tokens
        .iter()
        .find(|t| t.surface == "何時")
        .expect("何時 token must exist");
    assert_eq!(nanji.dictionary_form, "何時");
    assert_eq!(nanji.reading, "なんじ");
    assert!(nanji.is_content_word);
}

#[test]
fn test_formal_noun_toki() {
    let tokenizer = JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer.tokenize("空気を送る時は").unwrap();
    let toki = tokens
        .iter()
        .find(|t| t.surface == "時")
        .expect("時 token must exist");
    assert_eq!(toki.reading, "とき");
    assert_eq!(toki.surface_reading, "とき");
}

#[test]
fn test_filters_screams_and_interjection_noises() {
    let tokenizer = JapaneseTokenizer::new().unwrap();
    let tokens = tokenizer
        .tokenize("ぐっ… ひいいっ！ わあっ こりゃああっ！ ぐほっ")
        .unwrap();
    for t in &tokens {
        assert!(
            !t.is_content_word,
            "Scream noise '{}' must not be a content word",
            t.surface
        );
    }
}
