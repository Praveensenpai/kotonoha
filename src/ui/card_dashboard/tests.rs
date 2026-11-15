use super::*;
use crate::srt::SubtitleSentence;
use std::path::PathBuf;

fn dummy_candidate() -> CandidateSentence {
    CandidateSentence {
        sentence: SubtitleSentence {
            index: 1,
            start_ms: 1000,
            end_ms: 3000,
            text: "テスト文".to_string(),
            video_path: Some(PathBuf::from("/anime/test.mkv")),
            actor: None,
        },
        target_word: "テスト".to_string(),
        target_reading: "てすと".to_string(),
        known_context_words: Vec::new(),
        unknown_context_words: Vec::new(),
        ignored_context_words: Vec::new(),
        episode_freq: 1,
        density_tier: 1,
        quality_score: 4.5,
        video_path: PathBuf::from("/anime/test.mkv"),
        before_context: Vec::new(),
        after_context: Vec::new(),
        series_title: Some("Test Show".to_string()),
    }
}

fn dummy_dict_info() -> LookupResult {
    LookupResult {
        expression: "テスト".to_string(),
        reading: "てすと".to_string(),
        definition: "1. [Noun] test; trial".to_string(),
        pitch_accent: "1".to_string(),
    }
}

#[test]
fn test_theme_colors() {
    use theme::Theme;
    assert_ne!(Theme::BG, Theme::TEXT);
    assert_ne!(Theme::PINK, Theme::CYAN);
}

#[test]
fn test_handle_main_dashboard_keys() {
    let cand = dummy_candidate();
    let mut dict_info = dummy_dict_info();
    let mut p = DashboardParams {
        card_index: 0,
        total_cards: 10,
        mined_count: 0,
        known_count: 0,
        skipped_count: 0,
        ignored_count: 0,
        cand: &cand,
        dict_info: &mut dict_info,
        ai_suggestion: None,
        is_ai_selected: false,
        audio_path: None,
        all_candidates: Vec::new(),
    };

    let mut state = DashboardState {
        modal: ActiveModal::None,
        audio_child: None,
        audio_playing: false,
    };

    // 'y' -> Mine
    assert!(matches!(
        handle_main_dashboard_key(&mut p, &mut state, KeyCode::Char('y')),
        Some(DashboardAction::Mine)
    ));

    // Enter -> Mine
    assert!(matches!(
        handle_main_dashboard_key(&mut p, &mut state, KeyCode::Enter),
        Some(DashboardAction::Mine)
    ));

    // 'n' -> Skip
    assert!(matches!(
        handle_main_dashboard_key(&mut p, &mut state, KeyCode::Char('n')),
        Some(DashboardAction::Skip)
    ));

    // Space -> Skip
    assert!(matches!(
        handle_main_dashboard_key(&mut p, &mut state, KeyCode::Char(' ')),
        Some(DashboardAction::Skip)
    ));

    // 'k' -> Known
    assert!(matches!(
        handle_main_dashboard_key(&mut p, &mut state, KeyCode::Char('k')),
        Some(DashboardAction::Known)
    ));

    // 'q' -> Quit
    assert!(matches!(
        handle_main_dashboard_key(&mut p, &mut state, KeyCode::Char('q')),
        Some(DashboardAction::Quit)
    ));
}

#[test]
fn test_modal_transitions() {
    let cand = dummy_candidate();
    let mut dict_info = dummy_dict_info();
    let cand_list = vec![dummy_dict_info()];

    let mut p = DashboardParams {
        card_index: 0,
        total_cards: 10,
        mined_count: 0,
        known_count: 0,
        skipped_count: 0,
        ignored_count: 0,
        cand: &cand,
        dict_info: &mut dict_info,
        ai_suggestion: None,
        is_ai_selected: false,
        audio_path: None,
        all_candidates: cand_list,
    };

    let mut state = DashboardState {
        modal: ActiveModal::None,
        audio_child: None,
        audio_playing: false,
    };

    // Press 'c' opens candidate modal
    let _ = handle_main_dashboard_key(&mut p, &mut state, KeyCode::Char('c'));
    assert!(matches!(state.modal, ActiveModal::SelectCandidate { .. }));

    // Press Esc in modal closes it
    let _ = handle_dashboard_key(&mut p, &mut state, KeyCode::Esc, KeyModifiers::empty());
    assert!(matches!(state.modal, ActiveModal::None));

    // Press 'f' opens reading modal
    let _ = handle_main_dashboard_key(&mut p, &mut state, KeyCode::Char('f'));
    assert!(matches!(state.modal, ActiveModal::EditReading { .. }));
}
