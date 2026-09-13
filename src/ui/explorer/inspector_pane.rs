use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style as TuiStyle},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::render::RenderExplorerContext;

pub fn render_word_inspector(frame: &mut Frame<'_>, area: Rect, ctx: &RenderExplorerContext<'_>) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(TuiStyle::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Target Word & Dictionary ",
            TuiStyle::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let Some(&s_idx) = ctx.visible_indices.get(ctx.selected) else {
        frame.render_widget(Paragraph::new("No sentence selected.").block(block), area);
        return;
    };

    let s = &ctx.sentences[s_idx];
    if s.unknowns.is_empty() {
        let lines = vec![
            Line::from(vec![Span::styled(
                "✨ 100% Known Sentence!",
                TuiStyle::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::raw(""),
            Line::from(vec![Span::styled(
                format!("\"{}\"", s.sentence.text),
                TuiStyle::default().fg(Color::White),
            )]),
            Line::raw(""),
            Line::from(vec![Span::styled(
                "All content words in this line are already in your known/mined database.",
                TuiStyle::default().fg(Color::DarkGray),
            )]),
            Line::from(vec![Span::styled(
                "Press 'k' to re-confirm or 's' to sort.",
                TuiStyle::default().fg(Color::DarkGray),
            )]),
        ];
        frame.render_widget(Paragraph::new(lines).block(block), area);
        return;
    }

    let active_word = &s.unknowns[s.selected_unknown];
    let mut lines = Vec::new();

    let reading_disp = if active_word.reading.is_empty() {
        String::new()
    } else {
        format!(" ({})", active_word.reading)
    };

    lines.push(Line::from(vec![
        Span::styled(
            format!("【 {}{} 】", active_word.dictionary_form, reading_disp),
            TuiStyle::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "  [UNKNOWN]",
            TuiStyle::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    if s.unknowns.len() > 1 {
        lines.push(Line::from(vec![Span::styled(
            format!(
                "  ➜ Word {} of {} in this sentence  (←/→ or Tab to switch)",
                s.selected_unknown + 1,
                s.unknowns.len()
            ),
            TuiStyle::default().fg(Color::Cyan),
        )]));
    }
    lines.push(Line::raw(""));

    let is_selected = ctx
        .selected_cards
        .contains(&(s.sentence.index, active_word.dictionary_form.clone()));

    lines.push(Line::styled(
        "🎯 Card Review Selection:",
        TuiStyle::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ));
    if is_selected {
        lines.push(Line::from(vec![
            Span::styled(
                "  [✓] Selected for Card Review ",
                TuiStyle::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "(Space/x to deselect)",
                TuiStyle::default().fg(Color::DarkGray),
            ),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled(
                "  [ ] Not selected ",
                TuiStyle::default().fg(Color::DarkGray),
            ),
            Span::styled(
                "(Space/x to select for review)",
                TuiStyle::default().fg(Color::DarkGray),
            ),
        ]));
    }
    lines.push(Line::raw(""));

    lines.push(Line::styled(
        "📖 Local Dictionary (JMdict):",
        TuiStyle::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ));
    if let Some(dict) = ctx.active_dict {
        for def_line in dict.definition.lines().take(6) {
            lines.push(Line::styled(
                format!("  {def_line}"),
                TuiStyle::default().fg(Color::Gray),
            ));
        }
    } else {
        lines.push(Line::styled(
            "  Looking up definitions…",
            TuiStyle::default().fg(Color::DarkGray),
        ));
    }
    lines.push(Line::raw(""));

    lines.push(Line::styled(
        "💡 Review Workflow:",
        TuiStyle::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::styled(
        "  Select cards with Space/x, then press Enter to launch",
        TuiStyle::default().fg(Color::DarkGray),
    ));
    lines.push(Line::styled(
        "  standard card review with AI, reading edit, & Anki sync.",
        TuiStyle::default().fg(Color::DarkGray),
    ));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}
