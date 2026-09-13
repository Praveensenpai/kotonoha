use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style as TuiStyle},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use std::time::Instant;

use super::model::{ExplorerSentence, SentenceStats, SortOrder, Tier, TierFilter};
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
    pub selected_cards: &'a std::collections::HashSet<(usize, String)>,
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
    let line1 = build_header_stats_line(ctx);
    let line2 = build_header_controls_line(ctx);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(TuiStyle::default().fg(Color::DarkGray));
    frame.render_widget(Paragraph::new(vec![line1, line2]).block(block), area);
}

fn build_header_stats_line(ctx: &RenderExplorerContext<'_>) -> Line<'static> {
    let auto_style = if ctx.auto_play {
        TuiStyle::default()
            .fg(Color::LightGreen)
            .add_modifier(Modifier::BOLD)
    } else {
        TuiStyle::default().fg(Color::DarkGray)
    };

    let mut spans = vec![
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
    ];

    let tiers = [
        ("[i+0]", ctx.stats.i0, Color::Cyan),
        ("[i+1] ★", ctx.stats.i1, Color::LightGreen),
        ("[i+2]", ctx.stats.i2, Color::Yellow),
        ("[i+3+]", ctx.stats.i3_plus, Color::LightRed),
    ];
    for (name, count, color) in tiers {
        spans.push(Span::styled(
            format!("{name}: {count}  "),
            TuiStyle::default().fg(color).add_modifier(Modifier::BOLD),
        ));
    }
    spans.push(Span::styled(
        format!(
            "[Auto-Play: {} (a)]",
            if ctx.auto_play { "ON" } else { "OFF" }
        ),
        auto_style,
    ));
    Line::from(spans)
}

fn build_header_controls_line(ctx: &RenderExplorerContext<'_>) -> Line<'static> {
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

    let mut spans = vec![
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
    ];

    if !ctx.selected_cards.is_empty() {
        spans.push(Span::styled(
            format!(
                "  •  Selected: {} cards [Enter to Review]",
                ctx.selected_cards.len()
            ),
            TuiStyle::default()
                .fg(Color::Black)
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        ));
    }

    Line::from(spans)
}

fn build_checkbox_span(
    s: &ExplorerSentence,
    selected: &std::collections::HashSet<(usize, String)>,
) -> Span<'static> {
    let any = s
        .unknowns
        .iter()
        .any(|u| selected.contains(&(s.sentence.index, u.dictionary_form.clone())));
    let all = !s.unknowns.is_empty()
        && s.unknowns
            .iter()
            .all(|u| selected.contains(&(s.sentence.index, u.dictionary_form.clone())));

    let (label, color) = if all {
        ("[✓] ", Color::LightGreen)
    } else if any {
        ("[~] ", Color::Yellow)
    } else {
        return Span::styled("[ ] ", TuiStyle::default().fg(Color::DarkGray));
    };
    Span::styled(
        label,
        TuiStyle::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

fn build_word_span(surface: &str, is_active: bool, is_card_selected: bool) -> Span<'static> {
    let text = if is_card_selected {
        format!("{surface}[✓]")
    } else {
        surface.to_string()
    };
    let (fg, bg) = match (is_active, is_card_selected) {
        (true, true) => (Color::Black, Some(Color::LightGreen)),
        (true, false) => (Color::Black, Some(Color::Cyan)),
        (false, true) => (Color::LightGreen, None),
        (false, false) => (Color::Yellow, None),
    };
    let mut style = TuiStyle::default().fg(fg).add_modifier(Modifier::BOLD);
    if let Some(bg) = bg {
        style = style.bg(bg);
    }
    Span::styled(text, style)
}

fn build_sentence_line(
    s: &ExplorerSentence,
    is_selected: bool,
    is_playing: bool,
    selected_cards: &std::collections::HashSet<(usize, String)>,
) -> Line<'static> {
    let mut spans = Vec::new();
    let (prefix, prefix_style) = if is_playing {
        (
            "▶ ",
            TuiStyle::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        )
    } else if is_selected {
        (
            "› ",
            TuiStyle::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        ("  ", TuiStyle::default())
    };
    spans.push(Span::styled(prefix, prefix_style));
    spans.push(build_checkbox_span(s, selected_cards));

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
            let is_active = is_selected && u_idx == s.selected_unknown;
            let is_card_sel =
                selected_cards.contains(&(s.sentence.index, u.dictionary_form.clone()));
            spans.push(build_word_span(&u.surface, is_active, is_card_sel));
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
            let line = build_sentence_line(s, is_selected, is_playing, ctx.selected_cards);
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

    let count_disp = if ctx.selected_cards.is_empty() {
        "Enter Review (Current)".to_string()
    } else {
        format!("Enter Review ({})", ctx.selected_cards.len())
    };
    let help_line = Line::styled(
        format!(
            "↑↓ Scroll  ←→/Tab Target  Space/x Select  X Line  C Clear  {}  k Known  i Ignore  r Replay  a Auto-Play  s Sort  f Filter  / Search  Esc Exit",
            count_disp
        ),
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
