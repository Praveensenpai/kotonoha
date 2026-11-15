use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::render::DashboardRenderContext;
use super::theme::Theme;
use crate::miner::CandidateSentence;
use crate::ui::helpers::format_timestamp;

pub fn render_sentence_box(f: &mut Frame<'_>, area: Rect, ctx: &DashboardRenderContext<'_>) {
    let text = &ctx.cand.sentence.text;
    let target = &ctx.cand.target_word;

    let highlighted_line = build_highlighted_sentence(text, target);
    let mut meta_spans = vec![
        Span::styled(
            format!(" ★ {:.2} ", ctx.cand.quality_score),
            Style::default()
                .fg(Theme::YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" [Freq: {}] ", ctx.cand.episode_freq),
            Style::default().fg(Theme::CYAN),
        ),
        Span::styled(
            format!(" [Tier: i+{}] ", ctx.cand.density_tier),
            Style::default().fg(Theme::GREEN),
        ),
        Span::styled(
            format!(" [{}] ", format_timestamp(ctx.cand.sentence.start_ms)),
            Style::default().fg(Theme::SUBTEXT),
        ),
    ];

    if let Some(ref actor) = ctx.cand.sentence.actor {
        meta_spans.push(Span::styled(
            format!(" Speaker: {} ", actor),
            Style::default().fg(Theme::LAVENDER),
        ));
    }

    if ctx.is_ai_selected {
        meta_spans.push(Span::styled(
            " 🤖 [AI Recommended] ",
            Style::default()
                .fg(Theme::CYAN)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let p = Paragraph::new(vec![
        Line::from(""),
        highlighted_line,
        Line::from(""),
        Line::from(meta_spans),
    ])
    .block(
        Block::default()
            .title(" 📝 Sentence ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Theme::BORDER_FOCUSED))
            .style(Style::default().bg(Theme::CARD_BG)),
    );

    f.render_widget(p, area);
}

pub fn build_highlighted_sentence<'a>(text: &'a str, target: &'a str) -> Line<'a> {
    if let Some(pos) = text.find(target) {
        let before = &text[..pos];
        let matched = &text[pos..pos + target.len()];
        let after = &text[pos + target.len()..];
        Line::from(vec![
            Span::styled(format!("   {}", before), Style::default().fg(Theme::TEXT)),
            Span::styled(
                matched,
                Style::default()
                    .fg(Theme::PINK)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ),
            Span::styled(after, Style::default().fg(Theme::TEXT)),
        ])
    } else {
        Line::from(Span::styled(
            format!("   {}", text),
            Style::default().fg(Theme::TEXT),
        ))
    }
}

pub fn render_split_panes(f: &mut Frame<'_>, area: Rect, ctx: &DashboardRenderContext<'_>) {
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    render_context_pane(f, split[0], ctx);
    render_dict_pane(f, split[1], ctx);
}

fn build_dialogue_lines(cand: &CandidateSentence) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if cand.before_context.is_empty() && cand.after_context.is_empty() {
        lines.push(Line::from(Span::styled(
            " (No surrounding dialogue within gap)",
            Style::default().fg(Theme::SUBTEXT),
        )));
        return lines;
    }

    for (idx, prev) in cand.before_context.iter().enumerate() {
        let rel = cand.before_context.len() - idx;
        lines.push(Line::from(vec![
            Span::styled(format!("  -{} ", rel), Style::default().fg(Theme::SUBTEXT)),
            Span::styled(prev.clone(), Style::default().fg(Theme::SUBTEXT)),
        ]));
    }
    lines.push(Line::from(vec![
        Span::styled(
            "  ▶ ",
            Style::default()
                .fg(Theme::GREEN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(cand.sentence.text.clone(), Style::default().fg(Theme::TEXT)),
    ]));
    for (idx, next) in cand.after_context.iter().enumerate() {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  +{} ", idx + 1),
                Style::default().fg(Theme::SUBTEXT),
            ),
            Span::styled(next.clone(), Style::default().fg(Theme::SUBTEXT)),
        ]));
    }
    lines
}

fn build_context_meta_lines(cand: &CandidateSentence, audio_playing: bool) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if !cand.unknown_context_words.is_empty() {
        lines.push(Line::from(""));
        let unk_list = cand.unknown_context_words.join(", ");
        lines.push(Line::from(vec![
            Span::styled("  ❓ Other unknowns: ", Style::default().fg(Theme::YELLOW)),
            Span::styled(unk_list, Style::default().fg(Theme::SUBTEXT)),
        ]));
    }

    if !cand.known_context_words.is_empty() || !cand.ignored_context_words.is_empty() {
        let msg = format!(
            "  Context: {} known, {} ignored",
            cand.known_context_words.len(),
            cand.ignored_context_words.len()
        );
        lines.push(Line::from(Span::styled(
            msg,
            Style::default().fg(Theme::SUBTEXT),
        )));
    }

    lines.push(Line::from(""));
    let audio_label = if audio_playing {
        Span::styled(
            " 🔊 Audio playing... [R to replay]",
            Style::default().fg(Theme::GREEN),
        )
    } else {
        Span::styled(
            " 🔈 Audio ready [Press R to play]",
            Style::default().fg(Theme::SUBTEXT),
        )
    };
    lines.push(Line::from(audio_label));
    lines
}

fn render_context_pane(f: &mut Frame<'_>, area: Rect, ctx: &DashboardRenderContext<'_>) {
    let mut lines = Vec::new();
    if let Some(ref title) = ctx.cand.series_title {
        lines.push(Line::from(vec![
            Span::styled(" 🎬 ", Style::default()),
            Span::styled(
                title.clone(),
                Style::default()
                    .fg(Theme::CYAN)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));
    }

    lines.extend(build_dialogue_lines(ctx.cand));
    lines.extend(build_context_meta_lines(ctx.cand, ctx.audio_playing));

    let p = Paragraph::new(lines).block(
        Block::default()
            .title(" 🎬 Context & Audio ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Theme::BORDER))
            .style(Style::default().bg(Theme::CARD_BG)),
    );
    f.render_widget(p, area);
}

fn render_dict_pane(f: &mut Frame<'_>, area: Rect, ctx: &DashboardRenderContext<'_>) {
    let mut lines = Vec::new();
    let pitch_display = if let Ok(num) = ctx.dict_info.pitch_accent.parse::<usize>() {
        let (_, label, _) = crate::dict::pitch::format_pitch_accent(&ctx.dict_info.reading, num);
        format!(" • {}", label)
    } else if !ctx.dict_info.pitch_accent.is_empty() {
        format!(" • [{}]", ctx.dict_info.pitch_accent)
    } else {
        String::new()
    };

    lines.push(Line::from(vec![
        Span::styled(
            format!(" 語彙: {} ", ctx.dict_info.expression),
            Style::default()
                .fg(Theme::PINK)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("({}){}", ctx.dict_info.reading, pitch_display),
            Style::default().fg(Theme::CYAN),
        ),
    ]));
    lines.push(Line::from(""));

    for d_line in ctx.dict_info.definition.lines() {
        lines.push(Line::from(Span::styled(
            format!("  {}", d_line),
            Style::default().fg(Theme::TEXT),
        )));
    }

    if let Some(sug) = ctx.ai_suggestion {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " 🤖 AI Context Insight:",
            Style::default()
                .fg(Theme::CYAN)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            format!("  {}", sug),
            Style::default().fg(Theme::LAVENDER),
        )));
    }

    let p = Paragraph::new(lines).block(
        Block::default()
            .title(" 📖 Dictionary & AI ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Theme::BORDER))
            .style(Style::default().bg(Theme::CARD_BG)),
    );
    f.render_widget(p, area);
}

fn build_toolbar_line() -> Line<'static> {
    Line::from(vec![
        Span::styled(
            " [Y/Enter] ",
            Style::default()
                .fg(Theme::GREEN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Mine   ", Style::default().fg(Theme::TEXT)),
        Span::styled(
            " [N/Space] ",
            Style::default()
                .fg(Theme::YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Skip   ", Style::default().fg(Theme::TEXT)),
        Span::styled(
            " [K] ",
            Style::default()
                .fg(Theme::CYAN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Known   ", Style::default().fg(Theme::TEXT)),
        Span::styled(
            " [R] ",
            Style::default()
                .fg(Theme::PINK)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Audio   ", Style::default().fg(Theme::TEXT)),
        Span::styled(
            " [C] ",
            Style::default()
                .fg(Theme::LAVENDER)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Candidate   ", Style::default().fg(Theme::TEXT)),
        Span::styled(
            " [F] ",
            Style::default()
                .fg(Theme::LAVENDER)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Furigana   ", Style::default().fg(Theme::TEXT)),
        Span::styled(
            " [I] ",
            Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD),
        ),
        Span::styled("Ignore   ", Style::default().fg(Theme::TEXT)),
        Span::styled(
            " [Q] ",
            Style::default()
                .fg(Theme::SUBTEXT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Quit", Style::default().fg(Theme::SUBTEXT)),
    ])
}

pub fn render_footer_toolbar(f: &mut Frame<'_>, area: Rect) {
    let p = Paragraph::new(build_toolbar_line())
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Theme::BORDER)),
        );
    f.render_widget(p, area);
}
