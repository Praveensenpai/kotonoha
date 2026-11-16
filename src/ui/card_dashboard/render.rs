use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::modal::{render_candidate_modal, render_edit_reading_modal, ActiveModal};
use super::panes::{render_footer_toolbar, render_sentence_box, render_split_panes};
use super::theme::Theme;
use crate::dict::LookupResult;
use crate::miner::CandidateSentence;

pub struct DashboardRenderContext<'a> {
    pub card_index: usize,
    pub total_cards: usize,
    pub mined_count: usize,
    pub known_count: usize,
    pub skipped_count: usize,
    pub ignored_count: usize,
    pub cand: &'a CandidateSentence,
    pub dict_info: &'a LookupResult,
    pub ai_suggestion: Option<&'a str>,
    pub is_ai_selected: bool,
    pub ai_translation_natural: Option<&'a str>,
    pub ai_translation_literal: Option<&'a str>,
    pub audio_playing: bool,
    pub modal: &'a ActiveModal,
}

pub fn render_dashboard(f: &mut Frame<'_>, ctx: &DashboardRenderContext<'_>) {
    let background = Block::default().style(Style::default().bg(Theme::BG));
    f.render_widget(background, f.area());

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header & progress
            Constraint::Length(9), // Japanese sentence box
            Constraint::Min(10),   // Split pane (Context vs Dict)
            Constraint::Length(3), // Keybind toolbar
        ])
        .split(f.area());

    render_header(f, main_layout[0], ctx);
    render_sentence_box(f, main_layout[1], ctx);
    render_split_panes(f, main_layout[2], ctx);
    render_footer_toolbar(f, main_layout[3]);

    match ctx.modal {
        ActiveModal::SelectCandidate {
            candidates,
            selected_index,
        } => {
            render_candidate_modal(f, f.area(), candidates, *selected_index);
        }
        ActiveModal::EditReading { buffer } => {
            render_edit_reading_modal(f, f.area(), buffer);
        }
        ActiveModal::None => {}
    }
}

fn build_header_title(ctx: &DashboardRenderContext<'_>) -> Line<'static> {
    let percent = ((ctx.card_index + 1) * 100)
        .checked_div(ctx.total_cards)
        .unwrap_or(100);

    Line::from(vec![
        Span::styled(
            " 🌸 言の葉 ",
            Style::default()
                .fg(Theme::PINK)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "── Card Mining Dashboard ",
            Style::default().fg(Theme::TEXT),
        ),
        Span::styled(
            format!(
                "[{}/{} • {}%]",
                ctx.card_index + 1,
                ctx.total_cards,
                percent
            ),
            Style::default()
                .fg(Theme::CYAN)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn build_header_badges(ctx: &DashboardRenderContext<'_>, compact: bool) -> Line<'static> {
    if compact {
        Line::from(vec![
            Span::styled(
                format!(" ⛏ {} ", ctx.mined_count),
                Style::default().fg(Theme::GREEN),
            ),
            Span::styled(
                format!("✔ {} ", ctx.known_count),
                Style::default().fg(Theme::CYAN),
            ),
            Span::styled(
                format!("⏭ {} ", ctx.skipped_count),
                Style::default().fg(Theme::YELLOW),
            ),
            Span::styled(
                format!("🚫 {} ", ctx.ignored_count),
                Style::default().fg(Theme::RED),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                format!(" ⛏ {} mined  ", ctx.mined_count),
                Style::default().fg(Theme::GREEN),
            ),
            Span::styled(
                format!("✔ {} known  ", ctx.known_count),
                Style::default().fg(Theme::CYAN),
            ),
            Span::styled(
                format!("⏭ {} skipped  ", ctx.skipped_count),
                Style::default().fg(Theme::YELLOW),
            ),
            Span::styled(
                format!("🚫 {} ignored ", ctx.ignored_count),
                Style::default().fg(Theme::RED),
            ),
        ])
    }
}

fn render_header(f: &mut Frame<'_>, area: Rect, ctx: &DashboardRenderContext<'_>) {
    let is_compact = area.width < 105;
    let right_width = if is_compact { 28 } else { 48 };

    let header_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(35), Constraint::Length(right_width)])
        .split(area);

    let border = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Theme::BORDER));

    let p_left = Paragraph::new(build_header_title(ctx)).block(border.clone());
    let p_right = Paragraph::new(build_header_badges(ctx, is_compact))
        .alignment(ratatui::layout::Alignment::Right)
        .block(border);

    f.render_widget(p_left, header_layout[0]);
    f.render_widget(p_right, header_layout[1]);
}
