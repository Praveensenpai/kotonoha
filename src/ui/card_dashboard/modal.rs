use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use super::theme::Theme;
use crate::dict::LookupResult;

pub enum ActiveModal {
    None,
    SelectCandidate {
        candidates: Vec<LookupResult>,
        selected_index: usize,
    },
    EditReading {
        buffer: String,
    },
}

pub fn render_candidate_modal(
    f: &mut Frame<'_>,
    area: Rect,
    candidates: &[LookupResult],
    selected_idx: usize,
) {
    let popup_area = centered_rect(area, 70, 60);
    f.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = candidates
        .iter()
        .enumerate()
        .map(|(idx, cand)| {
            let is_sel = idx == selected_idx;
            let prefix = if is_sel { "▶ " } else { "  " };
            let style = if is_sel {
                Style::default()
                    .fg(Theme::PINK)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::TEXT)
            };
            let short_def: String = cand.definition.chars().take(80).collect();
            let line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{} ({})", cand.expression, cand.reading), style),
                Span::styled(
                    format!(" — {}", short_def),
                    Style::default().fg(Theme::SUBTEXT),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" 📖 Select Dictionary Candidate [↑/↓ Navigate, Enter Select, Esc Cancel] ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Theme::BORDER_FOCUSED))
            .style(Style::default().bg(Theme::CARD_BG)),
    );

    f.render_widget(list, popup_area);
}

pub fn render_edit_reading_modal(f: &mut Frame<'_>, area: Rect, buffer: &str) {
    let popup_area = centered_rect(area, 50, 20);
    f.render_widget(Clear, popup_area);

    let text = vec![
        Line::from(Span::styled(
            "Enter target word furigana reading:",
            Style::default().fg(Theme::SUBTEXT),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "❯ ",
                Style::default()
                    .fg(Theme::PINK)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                buffer,
                Style::default()
                    .fg(Theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("█", Style::default().fg(Theme::CYAN)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "[Enter to Confirm, Esc to Cancel]",
            Style::default().fg(Theme::SUBTEXT),
        )),
    ];

    let p = Paragraph::new(text).block(
        Block::default()
            .title(" ✍️ Edit Reading ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Theme::BORDER_FOCUSED))
            .style(Style::default().bg(Theme::CARD_BG)),
    );

    f.render_widget(p, popup_area);
}

fn centered_rect(r: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
