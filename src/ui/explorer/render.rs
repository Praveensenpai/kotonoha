use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style as TuiStyle},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use std::time::Instant;

use super::model::{ExplorerSentence, SentenceStats, SortOrder, Tier, TierFilter};
use crate::ai::AiAnalysisResult;
use crate::dict::LookupResult;
use crate::ui::format_timestamp;

pub struct RenderExplorerContext<'a> {
    pub sentences: &'a [ExplorerSentence],
    pub visible_indices: &'a [usize],
    pub selected: usize,
    pub stats: &'a SentenceStats,
    pub sort_order: SortOrder,
    pub tier_filter: TierFilter,
    pub search: &'a str,
    pub search_active: bool,
    pub auto_play: bool,
    pub playing_row: Option<usize>,
    pub loading_until: Option<Instant>,
    pub active_dict: Option<&'a LookupResult>,
    pub active_ai: Option<&'a AiAnalysisResult>,
    pub status_message: Option<&'a str>,
}

pub fn render_explorer(frame: &mut Frame<'_>, ctx: RenderExplorerContext<'_>) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(frame.area());

    render_header(frame, main_layout[0], &ctx);

    let dual_pane = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(main_layout[1]);

    render_sentence_list(frame, dual_pane[0], &ctx);
    super::inspector_pane::render_word_inspector(frame, dual_pane[1], &ctx);
    render_footer(frame, main_layout[2], &ctx);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, ctx: &RenderExplorerContext<'_>) {
    let auto_play_style = if ctx.auto_play {
        TuiStyle::default()
            .fg(Color::LightGreen)
            .add_modifier(Modifier::BOLD)
    } else {
        TuiStyle::default().fg(Color::DarkGray)
    };

    let search_disp = if ctx.search.is_empty() {
        if ctx.search_active {
            "▌".to_string()
        } else {
            "type to search…".to_string()
        }
    } else if ctx.search_active {
        format!("{}▌", ctx.search)
    } else {
        ctx.search.to_string()
    };

    let line1 = Line::from(vec![
        Span::styled(
            " KOTONOHA ",
            TuiStyle::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "  Sentence Explorer  ",
            TuiStyle::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("Total: {}  •  ", ctx.stats.total),
            TuiStyle::default().fg(Color::Gray),
        ),
        Span::styled(
            format!("[i+0]: {}  ", ctx.stats.i0),
            TuiStyle::default().fg(Color::Cyan),
        ),
        Span::styled(
            format!("[i+1]: {} ★  ", ctx.stats.i1),
            TuiStyle::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("[i+2]: {}  ", ctx.stats.i2),
            TuiStyle::default().fg(Color::Yellow),
        ),
        Span::styled(
            format!("[i+3+]: {}  •  ", ctx.stats.i3_plus),
            TuiStyle::default().fg(Color::LightRed),
        ),
        Span::styled(
            format!(
                "[Auto-Play: {} (a)]",
                if ctx.auto_play { "ON" } else { "OFF" }
            ),
            auto_play_style,
        ),
    ]);

    let line2 = Line::from(vec![
        Span::styled(" Sort: ", TuiStyle::default().fg(Color::DarkGray)),
        Span::styled(
            ctx.sort_order.label(),
            TuiStyle::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" (s)  •  Filter: ", TuiStyle::default().fg(Color::DarkGray)),
        Span::styled(
            ctx.tier_filter.label(),
            TuiStyle::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" (f)  •  Search: ", TuiStyle::default().fg(Color::DarkGray)),
        Span::styled(
            search_disp,
            if ctx.search.is_empty() {
                TuiStyle::default().fg(Color::DarkGray)
            } else {
                TuiStyle::default().fg(Color::Yellow)
            },
        ),
        Span::styled(
            format!("  ({} matches)", ctx.visible_indices.len()),
            TuiStyle::default().fg(Color::DarkGray),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(TuiStyle::default().fg(Color::DarkGray));
    frame.render_widget(Paragraph::new(vec![line1, line2]).block(block), area);
}

fn build_sentence_line(s: &ExplorerSentence, is_selected: bool, is_playing: bool) -> Line<'static> {
    let mut spans = Vec::new();
    let prefix = if is_playing {
        "▶ "
    } else if is_selected {
        "› "
    } else {
        "  "
    };
    let prefix_style = if is_playing {
        TuiStyle::default()
            .fg(Color::LightGreen)
            .add_modifier(Modifier::BOLD)
    } else if is_selected {
        TuiStyle::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        TuiStyle::default()
    };
    spans.push(Span::styled(prefix, prefix_style));

    let (tier_str, tier_color) = match s.tier {
        Tier::I0 => ("[i+0] ", Color::Cyan),
        Tier::I1 => ("[i+1]★", Color::LightGreen),
        Tier::I2 => ("[i+2] ", Color::Yellow),
        Tier::I3Plus => ("[i+3+]", Color::LightRed),
    };
    spans.push(Span::styled(
        tier_str,
        TuiStyle::default()
            .fg(tier_color)
            .add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        format_timestamp(s.sentence.start_ms),
        TuiStyle::default().fg(Color::DarkGray),
    ));
    spans.push(Span::raw("  "));

    let text = &s.sentence.text;
    let mut cursor = 0;
    for (u_idx, u) in s.unknowns.iter().enumerate() {
        if let Some(pos) = text[cursor..].find(&u.surface) {
            let abs_pos = cursor + pos;
            if abs_pos > cursor {
                spans.push(Span::raw(text[cursor..abs_pos].to_string()));
            }
            let is_active_target = is_selected && u_idx == s.selected_unknown;
            let style = if is_active_target {
                TuiStyle::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                TuiStyle::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD)
            };
            spans.push(Span::styled(u.surface.clone(), style));
            cursor = abs_pos + u.surface.len();
        }
    }
    if cursor < text.len() {
        spans.push(Span::raw(text[cursor..].to_string()));
    }

    Line::from(spans)
}

fn render_sentence_list(frame: &mut Frame<'_>, area: Rect, ctx: &RenderExplorerContext<'_>) {
    let list_height = area.height.saturating_sub(2) as usize;
    let scroll_offset = ctx.selected.saturating_sub(list_height.saturating_sub(1));

    let items: Vec<ListItem> = ctx
        .visible_indices
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(list_height)
        .map(|(pos, &s_idx)| {
            let s = &ctx.sentences[s_idx];
            let is_selected = pos == ctx.selected;
            let is_playing = ctx.playing_row == Some(s_idx);
            let line = build_sentence_line(s, is_selected, is_playing);
            let item_style = if is_selected {
                TuiStyle::default().bg(Color::Rgb(25, 30, 40))
            } else if is_playing {
                TuiStyle::default().bg(Color::Rgb(20, 35, 25))
            } else {
                TuiStyle::default()
            };
            ListItem::new(line).style(item_style)
        })
        .collect();

    let cur_display = if ctx.visible_indices.is_empty() {
        0
    } else {
        ctx.selected + 1
    };
    let title = format!(
        " Subtitle Sentences  {}/{} ",
        cur_display,
        ctx.visible_indices.len()
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(TuiStyle::default().fg(Color::DarkGray))
        .title(Span::styled(
            title,
            TuiStyle::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    frame.render_widget(List::new(items).block(block), area);
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, ctx: &RenderExplorerContext<'_>) {
    let status_line = if let Some(msg) = ctx.status_message {
        Line::styled(
            msg,
            TuiStyle::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        )
    } else if ctx
        .loading_until
        .is_some_and(|until| Instant::now() < until)
    {
        Line::styled(
            "⠋ Loading audio snippet…",
            TuiStyle::default().fg(Color::Yellow),
        )
    } else if let Some(s_idx) = ctx.playing_row {
        let s = &ctx.sentences[s_idx];
        Line::styled(
            format!(
                "♫ Playing [{}] {}",
                format_timestamp(s.sentence.start_ms),
                s.sentence.text
            ),
            TuiStyle::default().fg(Color::LightGreen),
        )
    } else {
        Line::styled(
            "Ready. Navigate with ↑↓ to browse and auto-play audio.",
            TuiStyle::default().fg(Color::DarkGray),
        )
    };

    let help_line = Line::styled(
        "↑↓ Scroll  ←→/Tab Unknown Word  m Mine Word  M Mine All (Multi-card)  k Known  K All Known  a Auto-Play  r Replay  s Sort  f Filter  / Search  Esc Exit",
        TuiStyle::default().fg(Color::DarkGray),
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(TuiStyle::default().fg(Color::DarkGray));
    frame.render_widget(
        Paragraph::new(vec![status_line, help_line]).block(block),
        area,
    );
}
