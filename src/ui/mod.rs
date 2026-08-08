pub mod card;
pub mod config_menu;

pub use card::{box_width, CardRenderParams};

use anyhow::Result;
use console::measure_text_width;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use inquire::{MultiSelect, Select, Text};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style as TuiStyle},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use std::cmp::Ordering;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use walkdir::WalkDir;

/// Compare strings naturally, treating consecutive ASCII digits as a number.
fn natural_cmp(left: &str, right: &str) -> Ordering {
    let mut left_chars = left.chars().peekable();
    let mut right_chars = right.chars().peekable();

    loop {
        match (left_chars.peek(), right_chars.peek()) {
            (Some(left_char), Some(right_char))
                if left_char.is_ascii_digit() && right_char.is_ascii_digit() =>
            {
                let mut left_number = String::new();
                while matches!(left_chars.peek(), Some(c) if c.is_ascii_digit()) {
                    left_number.push(left_chars.next().expect("digit was peeked"));
                }
                let mut right_number = String::new();
                while matches!(right_chars.peek(), Some(c) if c.is_ascii_digit()) {
                    right_number.push(right_chars.next().expect("digit was peeked"));
                }
                let left_trimmed = left_number.trim_start_matches('0');
                let right_trimmed = right_number.trim_start_matches('0');
                let order = left_trimmed
                    .len()
                    .cmp(&right_trimmed.len())
                    .then_with(|| left_trimmed.cmp(right_trimmed));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(left_char), Some(right_char)) => {
                let order = left_char.cmp(right_char);
                if order != Ordering::Equal {
                    return order;
                }
                left_chars.next();
                right_chars.next();
            }
            (None, None) => return left.cmp(right),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
        }
    }
}

fn format_word_with_reading(word: &str, reading: &str) -> String {
    if reading.is_empty() || reading == word {
        word.to_string()
    } else {
        format!("{} ({})", word, reading)
    }
}

fn format_timestamp(start_ms: u64) -> String {
    let total_minutes = start_ms / 60_000;
    let seconds = (start_ms % 60_000) / 1_000;
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

pub struct TerminalUi;

impl TerminalUi {
    /// Prints a full-width box with the app title centered inside.
    pub fn print_banner() {
        let bw = box_width();
        let iw = bw - 4;

        let title = "🌸  K O T O N O H A  ──  Japanese $i+1$ Sentence Miner";
        let title_vis = measure_text_width(title);

        let total_pad = iw.saturating_sub(title_vis);
        let left_pad = total_pad / 2;
        let right_pad = total_pad - left_pad;

        let top = format!("┌{}┐", "─".repeat(bw - 2));
        let middle = format!(
            "│ {}{}{} │",
            " ".repeat(left_pad),
            title,
            " ".repeat(right_pad)
        );
        let bottom = format!("└{}┘", "─".repeat(bw - 2));

        println!("\n{}", top);
        println!("{}", middle);
        println!("{}\n", bottom);
    }

    pub fn select_media_file() -> Result<PathBuf> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let search_dirs = vec![home.join("Videos")];

        let mut files = Vec::new();
        for dir in search_dirs {
            if !dir.exists() {
                continue;
            }
            for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
                let p = entry.path();
                if p.is_file() {
                    if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                        let ext = ext.to_lowercase();
                        if matches!(ext.as_str(), "srt" | "ass" | "mkv" | "mp4") {
                            files.push(p.to_path_buf());
                        }
                    }
                }
            }
        }

        if files.is_empty() {
            let input = Text::new("No media files auto-discovered. Enter file path:").prompt()?;
            return Ok(PathBuf::from(input));
        }

        files.sort_by(|left, right| natural_cmp(&left.to_string_lossy(), &right.to_string_lossy()));
        files.dedup();

        let items: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
        let selected = Select::new("Select Subtitle or Anime Video File:", items).prompt()?;
        Ok(PathBuf::from(selected))
    }

    pub fn bootstrap_known_words(vocab_items: &[(String, usize, String)]) -> Result<Vec<String>> {
        if vocab_items.is_empty() {
            return Ok(Vec::new());
        }

        let options: Vec<String> = vocab_items
            .iter()
            .enumerate()
            .map(|(idx, (word, count, reading))| {
                let has_kanji = word.chars().any(|c| matches!(c, '\u{4E00}'..='\u{9FFF}'));
                if has_kanji && word != reading && !reading.is_empty() {
                    format!(
                        "#{:02}  {} ({}) — {} occurrences",
                        idx + 1,
                        word,
                        reading,
                        count
                    )
                } else {
                    format!("#{:02}  {} — {} occurrences", idx + 1, word, count)
                }
            })
            .collect();

        let prompt_msg = format!(
            "Select words you ALREADY KNOW (Top {} frequent words — Space to toggle, Enter to confirm, type to filter):",
            vocab_items.len()
        );

        let selected_indices = MultiSelect::new(&prompt_msg, options)
            .with_page_size(18)
            .prompt()?;

        let mut checked_words = Vec::new();
        for item in selected_indices {
            let Some(index) = item
                .split_whitespace()
                .next()
                .and_then(|rank| rank.strip_prefix('#'))
                .and_then(|rank| rank.parse::<usize>().ok())
                .and_then(|rank| rank.checked_sub(1))
            else {
                continue;
            };
            if let Some((word, _, _)) = vocab_items.get(index) {
                checked_words.push(word.clone());
            }
        }

        Ok(checked_words)
    }

    pub fn bootstrap_ignored_names(vocab_items: &[(String, usize, String)]) -> Result<Vec<String>> {
        if vocab_items.is_empty() {
            return Ok(Vec::new());
        }

        let options: Vec<String> = vocab_items
            .iter()
            .enumerate()
            .map(|(idx, (word, count, reading))| {
                let has_kanji = word.chars().any(|c| matches!(c, '\u{4E00}'..='\u{9FFF}'));
                if has_kanji && word != reading && !reading.is_empty() {
                    format!(
                        "#{:02}  {} ({}) — {} occurrences",
                        idx + 1,
                        word,
                        reading,
                        count
                    )
                } else {
                    format!("#{:02}  {} — {} occurrences", idx + 1, word, count)
                }
            })
            .collect();

        let prompt_msg = format!(
            "Select CHARACTER NAMES / PROPER NOUNS to IGNORE (Top {} frequent names — Space to toggle, Enter to confirm, type to filter):",
            vocab_items.len()
        );

        let selected_indices = MultiSelect::new(&prompt_msg, options)
            .with_page_size(18)
            .prompt()?;

        let mut checked_words = Vec::new();
        for item in selected_indices {
            let Some(index) = item
                .split_whitespace()
                .next()
                .and_then(|rank| rank.strip_prefix('#'))
                .and_then(|rank| rank.parse::<usize>().ok())
                .and_then(|rank| rank.checked_sub(1))
            else {
                continue;
            };
            if let Some((word, _, _)) = vocab_items.get(index) {
                checked_words.push(word.clone());
            }
        }

        Ok(checked_words)
    }

    pub fn render_progress(
        current: usize,
        total: usize,
        mined: usize,
        known: usize,
        skipped: usize,
        ignored: usize,
    ) {
        card::render_progress(current, total, mined, known, skipped, ignored);
    }

    pub fn render_card(p: CardRenderParams<'_>) {
        card::render_card(p);
    }

    pub fn ask_action() -> Result<char> {
        let options = vec![
            "⏭️  Skip to next card (n)",
            "⛏️  Mine this card (y)",
            "🧠  Mark target word as known (k)",
            "✍️  Edit target word reading / furigana (f)",
            "📖  Change dictionary candidate (c)",
            "🔊  Replay preview audio (r)",
            "🚫  Ignore target word (i)",
            "🚪  Quit (q)",
        ];

        let ans = Select::new("Action?", options).prompt()?;
        if ans.contains("(y)") {
            Ok('y')
        } else if ans.contains("(k)") {
            Ok('k')
        } else if ans.contains("(f)") {
            Ok('f')
        } else if ans.contains("(c)") {
            Ok('c')
        } else if ans.contains("(r)") {
            Ok('r')
        } else if ans.contains("(i)") {
            Ok('i')
        } else if ans.contains("(q)") {
            Ok('q')
        } else {
            Ok('n')
        }
    }

    pub fn select_or_edit_reading(
        current_reading: &str,
        context_reading: &str,
        candidates: &[crate::dict::LookupResult],
    ) -> Result<String> {
        let mut options = Vec::new();

        if !context_reading.is_empty() && context_reading != current_reading {
            options.push(format!(
                "✨ Contextual Reading (Sudachi): \"{}\"",
                context_reading
            ));
        }

        let mut seen_readings = std::collections::HashSet::new();
        if !current_reading.is_empty() {
            seen_readings.insert(current_reading.to_string());
            options.push(format!("📌 Current Reading: \"{}\"", current_reading));
        }
        if !context_reading.is_empty() {
            seen_readings.insert(context_reading.to_string());
        }

        for cand in candidates {
            if !cand.reading.is_empty() && !seen_readings.contains(&cand.reading) {
                seen_readings.insert(cand.reading.clone());
                options.push(format!("📖 Dictionary Candidate: \"{}\"", cand.reading));
            }
        }

        options.push("✍  Enter custom furigana reading text".to_string());

        let ans = Select::new("Select or edit furigana reading:", options).prompt()?;

        if ans.contains("custom furigana reading text") {
            let custom_reading = Text::new("Enter custom furigana reading text:")
                .with_initial_value(current_reading)
                .prompt()?;
            let trimmed = custom_reading.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(trimmed);
            }
        }

        if let Some((_, rest)) = ans.split_once(": \"") {
            if let Some(val) = rest.strip_suffix('"').or_else(|| rest.split('"').next()) {
                return Ok(val.to_string());
            }
        }

        Ok(current_reading.to_string())
    }

    pub fn select_candidate_or_custom(
        candidates: &[crate::dict::LookupResult],
        target_word: &str,
        current_reading: &str,
        current_pitch: &str,
        ai_analysis: Option<&crate::ai::AiAnalysisResult>,
    ) -> Result<crate::dict::LookupResult> {
        let mut options = Vec::new();
        let ai_rec_cand = ai_analysis.and_then(|r| r.recommended_candidate_index);
        let ai_suggested_def = ai_analysis.and_then(|r| r.custom_definition_suggestion.as_deref());

        for (idx, cand) in candidates.iter().enumerate() {
            let is_ai_rec = ai_rec_cand == Some(idx);
            let ai_tag = if is_ai_rec {
                " ✨ [AI Recommended]"
            } else {
                ""
            };
            let first_sense = cand.definition.lines().next().unwrap_or(&cand.definition);
            options.push(format!(
                "#{:<2} 【{} ({})】 {}{}",
                idx + 1,
                cand.expression,
                cand.reading,
                first_sense,
                ai_tag
            ));
        }

        if let Some(sug) = ai_suggested_def {
            options.push(format!("✨  AI Contextual Gloss: \"{}\"", sug));
        }
        options.push("✍  Enter custom definition text".to_string());

        let ans = Select::new("Select dictionary definition:", options)
            .with_page_size(10)
            .prompt()?;

        if ans.contains("AI Contextual Gloss") {
            if let Some(sug) = ai_suggested_def {
                return Ok(crate::dict::LookupResult {
                    expression: target_word.to_string(),
                    reading: current_reading.to_string(),
                    definition: sug.to_string(),
                    pitch_accent: current_pitch.to_string(),
                });
            }
        }

        if ans.contains("custom definition text") {
            let custom_def = Text::new("Enter custom definition text:").prompt()?;
            let custom_def = custom_def.trim().to_string();
            if !custom_def.is_empty() {
                return Ok(crate::dict::LookupResult {
                    expression: target_word.to_string(),
                    reading: current_reading.to_string(),
                    definition: custom_def,
                    pitch_accent: current_pitch.to_string(),
                });
            }
        }

        if let Some(rank_str) = ans.split_whitespace().next() {
            if let Some(num) = rank_str
                .strip_prefix('#')
                .and_then(|s| s.parse::<usize>().ok())
            {
                if let Some(cand) = candidates.get(num.saturating_sub(1)) {
                    return Ok(cand.clone());
                }
            }
        }

        Ok(candidates
            .first()
            .cloned()
            .unwrap_or_else(|| crate::dict::LookupResult {
                expression: target_word.to_string(),
                reading: current_reading.to_string(),
                definition: "No definition".to_string(),
                pitch_accent: "0".to_string(),
            }))
    }

    pub fn select_sense(senses: &[String], target_word: &str) -> Result<Option<String>> {
        if senses.len() <= 1 {
            return Ok(senses.first().cloned());
        }

        let mut options: Vec<String> = senses
            .iter()
            .enumerate()
            .map(|(idx, s)| format!("#{:<2} {}", idx + 1, s))
            .collect();
        options.push("🔙 Cancel / Go back to card menu".to_string());

        let prompt = format!("Select sense for 【{}】:", target_word);
        let ans = Select::new(&prompt, options).prompt();

        match ans {
            Ok(ans_str) => {
                if ans_str.contains("Cancel / Go back") {
                    return Ok(None);
                }
                if let Some(rank_str) = ans_str.split_whitespace().next() {
                    if let Some(num) = rank_str
                        .strip_prefix('#')
                        .and_then(|s| s.parse::<usize>().ok())
                    {
                        if let Some(sense) = senses.get(num.saturating_sub(1)) {
                            return Ok(Some(sense.clone()));
                        }
                    }
                }
                Ok(senses.first().cloned())
            }
            Err(_) => Ok(None),
        }
    }

    pub fn inspect_sentences(
        sentences: &[crate::srt::SubtitleSentence],
        tokenizer: &crate::nlp::JapaneseTokenizer,
        known_words: &std::collections::HashSet<String>,
        ignored_words: &std::collections::HashSet<String>,
        video_path: Option<&Path>,
    ) -> Result<()> {
        let mut rows = Vec::new();
        for s in sentences {
            let tokens = tokenizer.tokenize(&s.text).unwrap_or_default();
            let mut unknown_count = 0;
            let mut spans = Vec::new();
            let mut rest = s.text.as_str();
            for token in &tokens {
                let is_unknown = token.is_content_word
                    && !known_words.contains(&token.dictionary_form)
                    && !ignored_words.contains(&token.dictionary_form);
                if is_unknown {
                    unknown_count += 1;
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

            let is_i1 = unknown_count == 1;
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

        let result = Self::run_inspector(&mut terminal, &rows, video_path);
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
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
                    "Select a line, then press Space to play it.".to_string()
                } else {
                    "Subtitle browsing is available; audio needs a matching video file.".to_string()
                };
                let help = "↑↓ navigate   Space play/replay   type search   Backspace clear   Enter/Esc exit";
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
                KeyCode::Char(' ') if video_path.is_some() && !visible.is_empty() => {
                    if let Some(mut child) = audio_child.take() {
                        let _ = child.kill();
                    }
                    let row_index = visible[selected];
                    let sentence = rows[row_index].0;
                    audio_child = crate::media::MediaExtractor::play_subtitle_segment(
                        video_path.expect("checked above"),
                        sentence.start_ms,
                        sentence.end_ms,
                    );
                    playing_row = audio_child.as_ref().map(|_| row_index);
                    audio_error = audio_child.is_none();
                    loading_until = playing_row.map(|_| Instant::now() + Duration::from_millis(500));
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

    pub fn manage_known_words(words: &[(String, String)]) -> Result<Vec<String>> {
        if words.is_empty() {
            println!(" ℹ No words currently marked as known.");
            return Ok(Vec::new());
        }

        let options: Vec<String> = words
            .iter()
            .map(|(w, r)| format_word_with_reading(w, r))
            .collect();

        let selected = MultiSelect::new(
            "Manage Known Words (Select words to REMOVE from your known list — Space to toggle, Enter to confirm, type to filter):",
            options,
        )
        .with_page_size(18)
        .prompt()?;

        let mut to_remove = Vec::new();
        for sel in selected {
            let word = sel.split_whitespace().next().unwrap_or(&sel);
            to_remove.push(word.to_string());
        }

        Ok(to_remove)
    }

    pub fn manage_ignored_words(words: &[(String, String)]) -> Result<Vec<String>> {
        if words.is_empty() {
            println!(" ℹ No words currently ignored.");
            return Ok(Vec::new());
        }

        let options: Vec<String> = words
            .iter()
            .map(|(w, r)| format_word_with_reading(w, r))
            .collect();

        let selected = MultiSelect::new(
            "Manage Ignored Words (Select words to REMOVE from your ignore list — Space to toggle, Enter to confirm, type to filter):",
            options,
        )
        .with_page_size(18)
        .prompt()?;

        let mut to_remove = Vec::new();
        for sel in selected {
            let word = sel.split_whitespace().next().unwrap_or(&sel);
            to_remove.push(word.to_string());
        }

        Ok(to_remove)
    }

    pub fn manage_mined_words(words: &[(String, String)]) -> Result<Vec<String>> {
        if words.is_empty() {
            println!(" ℹ No words currently marked as mined.");
            return Ok(Vec::new());
        }

        let options: Vec<String> = words
            .iter()
            .map(|(w, r)| format_word_with_reading(w, r))
            .collect();

        let selected = MultiSelect::new(
            "Manage Mined Words (Select words to REMOVE from your mined list — Space to toggle, Enter to confirm, type to filter):",
            options,
        )
        .with_page_size(18)
        .prompt()?;

        let mut to_remove = Vec::new();
        for sel in selected {
            let word = sel.split_whitespace().next().unwrap_or(&sel);
            to_remove.push(word.to_string());
        }

        Ok(to_remove)
    }

    pub fn show_config(cfg: &crate::config::AppConfig) {
        config_menu::show_config(cfg);
    }

    pub fn configure_interactive(cfg: &mut crate::config::AppConfig) -> Result<()> {
        config_menu::configure_interactive(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::natural_cmp;

    #[test]
    fn sorts_episode_numbers_naturally() {
        let mut episodes = [
            "episode-10.mkv",
            "episode-08.mkv",
            "episode-01.mkv",
            "episode-2.mkv",
        ];
        episodes.sort_by(|left, right| natural_cmp(left, right));
        assert_eq!(
            episodes,
            [
                "episode-01.mkv",
                "episode-2.mkv",
                "episode-08.mkv",
                "episode-10.mkv"
            ]
        );
    }
}
