use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Position},
    style::{Color, Modifier, Style as TuiStyle},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::ui::card;
use crate::ui::format_timestamp;

pub struct InspectSentencesParams<'a> {
    pub sentences: &'a [crate::srt::SubtitleSentence],
    pub tokenizer: &'a crate::nlp::JapaneseTokenizer,
    pub known_words: &'a std::collections::HashSet<String>,
    pub ignored_words: &'a std::collections::HashSet<String>,
    pub video_path: Option<&'a Path>,
}

pub fn inspect_sentences(p: InspectSentencesParams<'_>) -> Result<()> {
    let mut rows = Vec::new();
    for s in p.sentences {
        let tokens = p.tokenizer.tokenize(&s.text).unwrap_or_default();
        let mut unknown_set = std::collections::HashSet::new();
        let mut spans = Vec::new();
        let mut rest = s.text.as_str();
        for token in &tokens {
            let is_unknown = token.is_content_word
                && !p.known_words.contains(&token.dictionary_form)
                && !p.ignored_words.contains(&token.dictionary_form);
            if is_unknown {
                unknown_set.insert(token.dictionary_form.clone());
            }
            // Sudachi token surfaces occur in order. Keeping the remaining
            // suffix prevents repeated words from all being highlighted.
            if let Some(offset) = rest.find(&token.surface) {
                let (before, after_before) = rest.split_at(offset);
                if !before.is_empty() {
                    spans.push(Span::raw(before.to_string()));
                }
                let (surface, after) = after_before.split_at(token.surface.len());
                spans.push(if is_unknown {
                    Span::styled(
                        surface.to_string(),
                        TuiStyle::default()
                            .fg(Color::LightGreen)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::raw(surface.to_string())
                });
                rest = after;
            }
        }
        if !rest.is_empty() {
            spans.push(Span::raw(rest.to_string()));
        }

        let is_i1 = unknown_set.len() == 1;
        let ts_str = format_timestamp(s.start_ms);

        let mut line = vec![Span::raw("  ")];
        if is_i1 {
            line.push(Span::styled("★", TuiStyle::default().fg(Color::LightGreen)));
        } else {
            line.push(Span::raw(" "));
        }
        line.push(Span::raw(" ["));
        line.push(Span::styled(ts_str, TuiStyle::default().fg(Color::Yellow)));
        line.push(Span::raw("]  "));
        line.append(&mut spans);
        rows.push((s, Line::from(line)));
    }

    if rows.is_empty() {
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_inspector(&mut terminal, &rows, p.video_path);
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();
    result
}

fn run_inspector(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    rows: &[(&crate::srt::SubtitleSentence, Line<'static>)],
    video_path: Option<&Path>,
) -> Result<()> {
    let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let mut filter = String::new();
    let mut selected = 0usize;
    let mut audio_child: Option<std::process::Child> = None;
    let mut loading_until: Option<Instant> = None;
    let mut playing_row: Option<usize> = None;
    let mut audio_error = false;
    let mut spinner_index = 0usize;

    loop {
        if audio_child
            .as_mut()
            .is_some_and(|child| child.try_wait().ok().flatten().is_some())
        {
            audio_child = None;
            playing_row = None;
        }
        let visible: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter_map(|(idx, (sentence, _))| {
                sentence
                    .text
                    .to_lowercase()
                    .contains(&filter.to_lowercase())
                    .then_some(idx)
            })
            .collect();
        selected = selected.min(visible.len().saturating_sub(1));
        let is_loading = loading_until.is_some_and(|until| Instant::now() < until);

        terminal.draw(|frame| {
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(4),
                    Constraint::Min(3),
                    Constraint::Length(3),
                ])
                .split(frame.area());

            let media_label = if video_path.is_some() {
                "audio ready"
            } else {
                "audio unavailable — matching video not found"
            };
            let search_display = if filter.is_empty() {
                "type to filter…".to_string()
            } else {
                filter.clone()
            };
            let header = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled(
                        " KOTONOHA ",
                        TuiStyle::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("  Subtitle Inspector   {} lines", rows.len()),
                        TuiStyle::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("   • {media_label}"),
                        TuiStyle::default().fg(Color::DarkGray),
                    ),
                ]),
                Line::from(vec![
                    Span::styled(" Search  ", TuiStyle::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::styled(
                        format!("{search_display}▌"),
                        if filter.is_empty() {
                            TuiStyle::default().fg(Color::DarkGray)
                        } else {
                            TuiStyle::default().fg(Color::White)
                        },
                    ),
                    Span::styled(
                        format!("   {} match{}", visible.len(), if visible.len() == 1 { "" } else { "es" }),
                        TuiStyle::default().fg(Color::DarkGray),
                    ),
                ]),
            ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(TuiStyle::default().fg(Color::DarkGray)),
            );
            frame.render_widget(header, layout[0]);

            let cursor_x = layout[0].x + 10 + card::visual_width(&search_display) as u16;
            let cursor_y = layout[0].y + 2;
            frame.set_cursor_position(Position::new(cursor_x, cursor_y));

            let list_height = layout[1].height.saturating_sub(2) as usize;
            let scroll_offset = selected.saturating_sub(list_height.saturating_sub(1));
            let current_match = if visible.is_empty() { 0 } else { selected + 1 };
            let items: Vec<ListItem> = visible
                .iter()
                .enumerate()
                .skip(scroll_offset)
                .take(list_height)
                .map(|(position, idx)| {
                    let mut line = rows[*idx].1.clone();
                    let is_playing = playing_row == Some(*idx);
                    if position == selected {
                        line.spans[0] = Span::styled(
                            if is_playing { "▶ " } else { "› " },
                            TuiStyle::default().fg(Color::Black).add_modifier(Modifier::BOLD),
                        );
                    } else if is_playing {
                        line.spans[0] = Span::styled(
                            "♫ ",
                            TuiStyle::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                        );
                    }
                    let style = if position == selected {
                        TuiStyle::default().bg(Color::Cyan).fg(Color::Black)
                    } else if is_playing {
                        TuiStyle::default().bg(Color::DarkGray)
                    } else {
                        TuiStyle::default()
                    };
                    ListItem::new(line).style(style)
                })
                .collect();
            let list = List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(TuiStyle::default().fg(Color::DarkGray))
                    .title(Span::styled(
                        format!(
                            " Subtitles  {}/{} matches  •  {}/{} ",
                            visible.len(),
                            rows.len(),
                            current_match,
                            visible.len()
                        ),
                        TuiStyle::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    )),
            );
            frame.render_widget(list, layout[1]);

            let footer = if is_loading {
                let idx = playing_row.expect("loading state has a selected row");
                format!(
                    "{} Loading audio  [{}]  {}",
                    spinner[spinner_index],
                    format_timestamp(rows[idx].0.start_ms),
                    rows[idx].0.text
                )
            } else if let Some(idx) = playing_row {
                format!(
                    "♫  Playing  [{}]  {}",
                    format_timestamp(rows[idx].0.start_ms),
                    rows[idx].0.text
                )
            } else if audio_error {
                "Audio player could not be started. Install mpv or ffplay.".to_string()
            } else if video_path.is_some() {
                "Select a line, then press Ctrl+P to play it.".to_string()
            } else {
                "Subtitle browsing is available; audio needs a matching video file.".to_string()
            };
            let help = "↑↓ navigate   Ctrl+P/Tab play audio   type to search   Backspace clear   Enter/Esc exit";
            frame.render_widget(
                Paragraph::new(vec![
                    Line::styled(footer, TuiStyle::default().fg(Color::White)),
                    Line::styled(help, TuiStyle::default().fg(Color::DarkGray)),
                ])
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(TuiStyle::default().fg(Color::DarkGray)),
                ),
                layout[2],
            );
        })?;

        if !event::poll(Duration::from_millis(100))? {
            spinner_index = (spinner_index + 1) % spinner.len();
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match key.code {
            KeyCode::Up => selected = selected.saturating_sub(1),
            KeyCode::Down => {
                if selected + 1 < visible.len() {
                    selected += 1;
                }
            }
            KeyCode::Tab | KeyCode::Char('p')
                if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
            {
                if let Some(video) = video_path {
                    if !visible.is_empty() {
                        if let Some(mut child) = audio_child.take() {
                            let _ = child.kill();
                        }
                        let row_index = visible[selected];
                        let sentence = rows[row_index].0;
                        audio_child = crate::media::MediaExtractor::play_subtitle_segment(
                            video,
                            sentence.start_ms,
                            sentence.end_ms,
                        );
                        playing_row = audio_child.as_ref().map(|_| row_index);
                        audio_error = audio_child.is_none();
                        loading_until =
                            playing_row.map(|_| Instant::now() + Duration::from_millis(500));
                    }
                }
            }
            KeyCode::Enter => break,
            KeyCode::Esc if filter.is_empty() => break,
            KeyCode::Esc => filter.clear(),
            KeyCode::Backspace => {
                filter.pop();
                selected = 0;
            }
            KeyCode::Char(c) => {
                filter.push(c);
                selected = 0;
            }
            _ => {}
        }
    }
    if let Some(mut child) = audio_child {
        let _ = child.kill();
    }
    Ok(())
}
