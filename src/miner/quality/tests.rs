use super::*;
use crate::nlp::TokenInfo;

fn make_dummy_token(surface: &str, is_content: bool) -> TokenInfo {
    TokenInfo {
        surface: surface.to_string(),
        dictionary_form: surface.to_string(),
        reading: surface.to_string(),
        is_content_word: is_content,
        is_proper_noun: false,
    }
}

#[test]
fn complete_sentence_scores_substantially_higher_than_fragments() {
    let rich_tokens = vec![
        make_dummy_token("明日", true),
        make_dummy_token("の", false),
        make_dummy_token("朝", true),
        make_dummy_token("まで", false),
        make_dummy_token("に", false),
        make_dummy_token("宿題", true),
        make_dummy_token("を", false),
        make_dummy_token("終わらせる", true),
        make_dummy_token("と", false),
        make_dummy_token("怒られる", true),
        make_dummy_token("よ", false),
    ];
    let rich_score = QualityScorer::score(
        "明日の朝までに宿題を終わらせないと先生に怒られるよ。",
        "宿題",
        &rich_tokens,
    );

    let fragment_tokens = vec![
        make_dummy_token("そうか", false),
        make_dummy_token("、", false),
        make_dummy_token("宿題", true),
    ];
    let fragment_score = QualityScorer::score("そうか、宿題。", "宿題", &fragment_tokens);

    let dangling_tokens = vec![
        make_dummy_token("宿題", true),
        make_dummy_token("だけど", false),
        make_dummy_token("…", false),
    ];
    let dangling_score = QualityScorer::score("宿題だけど…", "宿題", &dangling_tokens);

    assert!(
        rich_score > 0.80,
        "Rich sentence score expected > 0.80, got {rich_score}"
    );
    assert!(
        fragment_score < 0.40,
        "Fragment score expected < 0.40, got {fragment_score}"
    );
    assert!(
        dangling_score < 0.25,
        "Dangling score expected < 0.25, got {dangling_score}"
    );
    assert!(rich_score > fragment_score * 2.0);
}

#[test]
fn dangling_particles_and_connectives_are_severely_penalized() {
    let t_te = vec![make_dummy_token("待って", true)];
    let score_te = QualityScorer::score("ちょっと待って…", "待つ", &t_te);

    let t_ga = vec![make_dummy_token("彼", true)];
    let score_ga = QualityScorer::score("本当は彼が…", "彼", &t_ga);

    assert!(
        score_te < 0.35,
        "Te-form ending should be penalized: {score_te}"
    );
    assert!(
        score_ga < 0.30,
        "Dangling 'ga' ending should be penalized: {score_ga}"
    );
}

#[test]
fn polite_and_sentence_ending_particles_score_highly() {
    let t_desu = vec![
        make_dummy_token("東京", true),
        make_dummy_token("に", false),
        make_dummy_token("行く", true),
        make_dummy_token("予定", true),
        make_dummy_token("です", false),
    ];
    let score = QualityScorer::score("来週東京に行く予定です。", "東京", &t_desu);
    assert!(
        score > 0.80,
        "Expected high score for polite sentence, got {score}"
    );
}

#[test]
fn sub_four_character_sentence_returns_zero() {
    let t = vec![make_dummy_token("うん", false)];
    assert_eq!(QualityScorer::score("うん", "うん", &t), 0.0);
}

#[test]
fn target_word_case_marker_connection_boosts_score() {
    let tokens = vec![
        make_dummy_token("温かい", true),
        make_dummy_token("コーヒー", true),
        make_dummy_token("を", false),
        make_dummy_token("飲む", true),
    ];
    let with_case = QualityScorer::score("温かいコーヒーを飲む。", "コーヒー", &tokens);
    let without_case = QualityScorer::score("温かいコーヒー、飲む。", "コーヒー", &tokens);
    assert!(
        with_case > without_case,
        "Sentence with direct case particle connection should score higher: {with_case} vs {without_case}"
    );
}

#[test]
fn test_real_world_anime_subtitles_quality_scoring() {
    let sub_path =
        std::path::Path::new("/home/paisen/Videos/Anime/Yuru Camp/Yuru Camp - 01.ja.srt");
    if !sub_path.exists() {
        return;
    }

    let sentences = crate::srt::parse_subtitle(sub_path).expect("failed to parse Yuru Camp sub");
    let tokenizer = crate::nlp::JapaneseTokenizer::new().expect("failed to load tokenizer");
    let engine = crate::miner::MiningEngine::new(tokenizer);

    let mut known = std::collections::HashSet::new();
    // Simulate knowing common basic words
    for w in &[
        "行く",
        "見る",
        "する",
        "いる",
        "私",
        "これ",
        "それ",
        "今日",
        "富士山",
    ] {
        known.insert(w.to_string());
    }
    let ignored = std::collections::HashSet::new();

    let candidates = engine.find_candidates(&sentences, &known, &ignored);
    assert!(!candidates.is_empty(), "Candidates should be mined");

    println!("\n=== TOP MINED CANDIDATES FROM YURU CAMP EP 01 ===");
    for (i, c) in candidates.iter().take(5).enumerate() {
        println!(
            "#{}: [{:.2}] target: '{}' (tier {}) => \"{}\"",
            i + 1,
            c.quality_score,
            c.target_word,
            c.density_tier,
            c.sentence.text
        );
        assert!(
            c.quality_score > 0.0,
            "Quality score must be positive: {}",
            c.sentence.text
        );
    }
}
