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
        ai_translation_natural: None,
        ai_translation_literal: None,
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
        ai_translation_natural: None,
        ai_translation_literal: None,
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

#[test]
fn test_render_dashboard_visual_layout() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let cand = dummy_candidate();
    let dict_info = dummy_dict_info();
    let modal = ActiveModal::None;

    let ctx = DashboardRenderContext {
        card_index: 0,
        total_cards: 10,
        mined_count: 2,
        known_count: 1,
        skipped_count: 0,
        ignored_count: 0,
        cand: &cand,
        dict_info: &dict_info,
        ai_suggestion: Some("Contextual nuance for test"),
        is_ai_selected: true,
        ai_translation_natural: Some("Natural test translation"),
        ai_translation_literal: Some("Literal test translation"),
        audio_playing: false,
        modal: &modal,
    };

    let backend = TestBackend::new(120, 32);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_dashboard(f, &ctx)).unwrap();

    let buffer = terminal.backend().buffer().clone();
    let output = format!("{:?}", buffer);

    // Verify header components
    assert!(output.contains("言の葉"), "Header must contain logo");
    assert!(
        output.contains("Card Mining Dashboard"),
        "Header must contain title"
    );
    assert!(output.contains("1/10"), "Header must show card progression");
    assert!(output.contains("2 mined"), "Header must show mined badge");

    // Verify sentence components
    assert!(output.contains("Sentence"), "Must contain Sentence box");
    assert!(
        output.contains("テスト"),
        "Sentence box must show target word"
    );
    assert!(
        output.contains("AI Recommended"),
        "Sentence box must display AI badge"
    );
    assert!(
        output.contains("Natural test translation"),
        "Sentence box must display natural translation"
    );
    assert!(
        output.contains("Literal test translation"),
        "Sentence box must display literal translation"
    );
    assert!(output.contains("4.50"), "Sentence box must display score");

    // Verify context & dict panes
    assert!(
        output.contains("Context & Audio"),
        "Must contain context pane"
    );
    assert!(
        output.contains("Test Show"),
        "Context pane must contain series title"
    );
    assert!(
        output.contains("Dictionary & AI"),
        "Must contain dictionary pane"
    );
    assert!(
        output.contains("語彙: テスト"),
        "Dictionary pane must contain expression"
    );
    assert!(
        output.contains("[1] HLL"),
        "Dictionary pane must show pitch accent"
    );

    // Print visual representation for inspection
    println!("\n=== Rendered TUI Dashboard Frame ===");
    for y in 0..buffer.area.height {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y)).unwrap();
            line.push_str(cell.symbol());
        }
        println!("{}", line);
    }
    println!("====================================\n");

    // Verify footer toolbar
    assert!(output.contains("[Y/Enter]"), "Toolbar must have Mine key");
    assert!(output.contains("[N/Space]"), "Toolbar must have Skip key");
    assert!(output.contains("[K]"), "Toolbar must have Known key");
    assert!(output.contains("[C]"), "Toolbar must have Candidate key");
}

#[test]
fn test_render_dashboard_modal_overlays() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let cand = dummy_candidate();
    let dict_info = dummy_dict_info();
    let candidates = vec![dummy_dict_info()];
    let candidate_modal = ActiveModal::SelectCandidate {
        candidates,
        selected_index: 0,
    };

    let ctx = DashboardRenderContext {
        card_index: 0,
        total_cards: 10,
        mined_count: 0,
        known_count: 0,
        skipped_count: 0,
        ignored_count: 0,
        cand: &cand,
        dict_info: &dict_info,
        ai_suggestion: None,
        is_ai_selected: false,
        ai_translation_natural: None,
        ai_translation_literal: None,
        audio_playing: false,
        modal: &candidate_modal,
    };

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_dashboard(f, &ctx)).unwrap();

    let buffer = terminal.backend().buffer().clone();
    let output = format!("{:?}", buffer);
    assert!(
        output.contains("Dictionary Candidates"),
        "Modal title must be rendered"
    );

    println!("\n=== Rendered Candidate Modal Overlay ===");
    for y in 0..buffer.area.height {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y)).unwrap();
            line.push_str(cell.symbol());
        }
        println!("{}", line);
    }
    println!("========================================\n");

    // Test edit reading modal overlay
    let edit_modal = ActiveModal::EditReading {
        buffer: "てすと".to_string(),
    };
    let ctx_edit = DashboardRenderContext {
        modal: &edit_modal,
        ..ctx
    };
    terminal.draw(|f| render_dashboard(f, &ctx_edit)).unwrap();
    let buffer2 = terminal.backend().buffer().clone();
    let output2 = format!("{:?}", buffer2);
    assert!(
        output2.contains("Edit Reading"),
        "Edit reading modal must be rendered"
    );

    println!("\n=== Rendered Edit Reading Modal Overlay ===");
    for y in 0..buffer2.area.height {
        let mut line = String::new();
        for x in 0..buffer2.area.width {
            let cell = buffer2.cell((x, y)).unwrap();
            line.push_str(cell.symbol());
        }
        println!("{}", line);
    }
    println!("===========================================\n");
}
