use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::state::{FileSelectorState, SelectableFile};

pub fn render_selector(frame: &mut Frame<'_>, state: &FileSelectorState) -> usize {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(frame.area());

    render_header(frame, main_layout[0], state);
    render_search_bar(frame, main_layout[1], state);
    let visible_height = render_file_list(frame, main_layout[2], state);
    render_footer(frame, main_layout[3], state);

    visible_height
}

fn render_header(frame: &mut Frame<'_>, area: Rect, state: &FileSelectorState) {
    let title_line = Line::from(vec![
        Span::styled("🌸 ", Style::default()),
        Span::styled(
            &state.title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    frame.render_widget(Paragraph::new(title_line).block(block), area);
}

fn render_search_bar(frame: &mut Frame<'_>, area: Rect, state: &FileSelectorState) {
    let search_disp = if state.search_query.is_empty() {
        Span::styled(
            "Type to filter (spaces allowed)...",
            Style::default().fg(Color::DarkGray),
        )
    } else {
        Span::styled(
            format!("{}▌", state.search_query),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    };

    let total_matches = state.filtered_indices.len();
    let total_items = state.items.len();
    let selected_count = state.selected_indices.len();

    let count_info = if state.multi {
        format!("  Matches: {total_matches}/{total_items}  |  Selected: {selected_count} ")
    } else {
        format!("  Matches: {total_matches}/{total_items} ")
    };

    let cat_label = state.category_filter.label();
    let cat_color = match state.category_filter {
        super::state::CategoryFilter::All => Color::White,
        super::state::CategoryFilter::Videos => Color::Cyan,
        super::state::CategoryFilter::Bundles => Color::Yellow,
    };

    let search_line = Line::from(vec![
        Span::styled(
            " 🔍 Filter: ",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        search_disp,
        Span::styled("   Category: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("[{cat_label}]"),
            Style::default().fg(cat_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" (Ctrl+F)", Style::default().fg(Color::DarkGray)),
        Span::styled(count_info, Style::default().fg(Color::DarkGray)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(Paragraph::new(search_line).block(block), area);
}

fn render_file_list(frame: &mut Frame<'_>, area: Rect, state: &FileSelectorState) -> usize {
    let list_height = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line<'static>> = state
        .filtered_indices
        .iter()
        .enumerate()
        .skip(state.scroll_offset)
        .take(list_height)
        .map(|(pos, &item_idx)| {
            let is_cursor = pos == state.cursor;
            let is_checked = state.selected_indices.contains(&item_idx);
            let item = &state.items[item_idx];
            build_file_line(item, is_cursor, is_checked, state.multi)
        })
        .collect();

    let cur_display = if state.filtered_indices.is_empty() {
        0
    } else {
        state.cursor + 1
    };
    let title = format!(" Files  {}/{} ", cur_display, state.filtered_indices.len());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(title);

    frame.render_widget(Paragraph::new(lines).block(block), area);
    list_height
}

fn format_short_path(path: &std::path::Path) -> (String, String) {
    let home = dirs::home_dir();
    let path_str = if let Some(home_path) = home {
        if let Ok(rel) = path.strip_prefix(&home_path) {
            format!("~/{}", rel.display())
        } else {
            path.display().to_string()
        }
    } else {
        path.display().to_string()
    };

    if let Some(idx) = path_str.rfind('/') {
        let dir_part = &path_str[..=idx];
        let file_part = &path_str[idx + 1..];
        (dir_part.to_string(), file_part.to_string())
    } else {
        (String::new(), path_str)
    }
}

fn build_file_line(
    item: &SelectableFile,
    is_cursor: bool,
    is_checked: bool,
    multi: bool,
) -> Line<'static> {
    let mut spans = Vec::new();

    if is_cursor {
        spans.push(Span::styled(
            "> ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::raw("  "));
    }

    if multi {
        if is_checked {
            spans.push(Span::styled(
                "[x] ",
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled("[ ] ", Style::default().fg(Color::DarkGray)));
        }
    }

    match item.status {
        super::state::SubtitleStatus::HasSub => {
            spans.push(Span::styled(
                "[✓ SUB]     ",
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        super::state::SubtitleStatus::Bundle => {
            spans.push(Span::styled(
                "📦 [BUNDLE] ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        super::state::SubtitleStatus::NoSub => {
            spans.push(Span::styled(
                "[NO SUB]   ",
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    let (dir_part, file_part) = format_short_path(&item.path);
    let dir_style = if is_cursor {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let file_style = if is_cursor {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else if is_checked {
        Style::default()
            .fg(Color::LightGreen)
            .add_modifier(Modifier::BOLD)
    } else if item.status == super::state::SubtitleStatus::NoSub {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };

    spans.push(Span::styled(dir_part, dir_style));
    spans.push(Span::styled(file_part, file_style));

    let mut line = Line::from(spans);
    if is_cursor {
        line = line.style(Style::default().bg(Color::Rgb(25, 30, 40)));
    }
    line
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, state: &FileSelectorState) {
    let help_spans = if state.multi {
        vec![
            Span::styled(
                " [Tab] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Select  "),
            Span::styled(
                " [Shift+Tab] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Select Prev  "),
            Span::styled(
                " [↑/↓] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Move  "),
            Span::styled(
                " [Ctrl+F] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Category  "),
            Span::styled(
                " [Ctrl+A] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("All  "),
            Span::styled(
                " [Ctrl+D] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Clear  "),
            Span::styled(
                " [Enter] ",
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Confirm  "),
            Span::styled(
                " [Esc] ",
                Style::default()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Cancel"),
        ]
    } else {
        vec![
            Span::styled(
                " [↑/↓] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Move  "),
            Span::styled(
                " [Ctrl+F] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Category  "),
            Span::styled(
                " [Enter] ",
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Confirm  "),
            Span::styled(
                " [Esc] ",
                Style::default()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Cancel"),
        ]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    frame.render_widget(Paragraph::new(Line::from(help_spans)).block(block), area);
}
