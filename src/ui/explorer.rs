pub mod actions;
pub mod inspector_pane;
pub mod model;
pub mod render;
pub mod state;

#[cfg(test)]
mod tests;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::config::AppConfig;
use crate::db::Database;
use crate::nlp::JapaneseTokenizer;
use crate::srt::SubtitleSentence;

use model::{calculate_stats, sort_sentences};
use render::{render_explorer, RenderExplorerContext};
use state::ExplorerController;

pub struct ExplorerParams<'a> {
    pub sentences: &'a [SubtitleSentence],
    pub tokenizer: &'a JapaneseTokenizer,
    pub known_words: &'a mut HashSet<String>,
    pub ignored_words: &'a mut HashSet<String>,
    pub video_path: Option<&'a Path>,
    pub cfg: &'a AppConfig,
    pub db: &'a Database,
    pub http_client: &'a reqwest::Client,
}

enum KeyAction {
    Continue,
    Quit,
    ConfirmReview,
}

fn handle_key_press(ctrl: &mut ExplorerController<'_>, key: KeyEvent) -> KeyAction {
    if ctrl.search_active {
        match key.code {
            KeyCode::Enter | KeyCode::Esc => ctrl.search_active = false,
            KeyCode::Backspace => {
                ctrl.search.pop();
                ctrl.selected = 0;
            }
            KeyCode::Char(c) => {
                ctrl.search.push(c);
                ctrl.selected = 0;
            }
            _ => {}
        }
        return KeyAction::Continue;
    }

    let visible_len = ctrl.visible_indices().len();
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            ctrl.selected = ctrl.selected.saturating_sub(1);
            ctrl.pending_audio = true;
            ctrl.last_scroll_time = Instant::now();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if ctrl.selected + 1 < visible_len {
                ctrl.selected += 1;
                ctrl.pending_audio = true;
                ctrl.last_scroll_time = Instant::now();
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if let Some((_, s_idx, _)) = ctrl.current_sentence_and_word() {
                let s = &mut ctrl.sentences[s_idx];
                s.selected_unknown = s.selected_unknown.saturating_sub(1);
            }
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
            if let Some((_, s_idx, _)) = ctrl.current_sentence_and_word() {
                let s = &mut ctrl.sentences[s_idx];
                if s.selected_unknown + 1 < s.unknowns.len() {
                    s.selected_unknown += 1;
                } else {
                    s.selected_unknown = 0;
                }
            }
        }
        KeyCode::BackTab => {
            if let Some((_, s_idx, _)) = ctrl.current_sentence_and_word() {
                let s = &mut ctrl.sentences[s_idx];
                if s.selected_unknown == 0 {
                    s.selected_unknown = s.unknowns.len().saturating_sub(1);
                } else {
                    s.selected_unknown -= 1;
                }
            }
        }
        KeyCode::Char(' ') | KeyCode::Char('x') => ctrl.toggle_selection(),
        KeyCode::Char('X') => ctrl.toggle_all_in_current_sentence(),
        KeyCode::Char('C') => ctrl.clear_selection(),
        KeyCode::Char('a') => {
            ctrl.auto_play = !ctrl.auto_play;
            ctrl.set_status(format!(
                "Auto-Play {}",
                if ctrl.auto_play {
                    "Enabled"
                } else {
                    "Disabled"
                }
            ));
        }
        KeyCode::Char('r') => ctrl.trigger_audio_for_current(),
        KeyCode::Char('s') => {
            ctrl.sort_order = ctrl.sort_order.toggle();
            sort_sentences(&mut ctrl.sentences, ctrl.sort_order);
            ctrl.selected = 0;
        }
        KeyCode::Char('f') => {
            ctrl.tier_filter = ctrl.tier_filter.next();
            ctrl.selected = 0;
        }
        KeyCode::Char('/') => ctrl.search_active = true,
        KeyCode::Enter | KeyCode::Char('m') => return KeyAction::ConfirmReview,
        KeyCode::Char('q') | KeyCode::Esc => return KeyAction::Quit,
        _ => {}
    }
    KeyAction::Continue
}

async fn handle_action_shortcut(ctrl: &mut ExplorerController<'_>, key: KeyEvent) -> Result<bool> {
    if ctrl.search_active {
        return Ok(false);
    }
    if key.code == KeyCode::Char('k') && !key.modifiers.contains(KeyModifiers::SHIFT) {
        ctrl.handle_mark_known().await?;
        return Ok(true);
    }
    if key.code == KeyCode::Char('K')
        || (key.code == KeyCode::Char('k') && key.modifiers.contains(KeyModifiers::SHIFT))
    {
        ctrl.handle_mark_all_known().await?;
        return Ok(true);
    }
    if key.code == KeyCode::Char('i') {
        ctrl.handle_mark_ignored().await?;
        return Ok(true);
    }
    Ok(false)
}

pub async fn run_explorer(p: ExplorerParams<'_>) -> Result<Vec<crate::miner::CandidateSentence>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut ctrl = ExplorerController::new(p);

    let should_review = loop {
        if ctrl
            .audio_child
            .as_mut()
            .is_some_and(|c| c.try_wait().ok().flatten().is_some())
        {
            ctrl.audio_child = None;
            ctrl.playing_row = None;
        }

        if ctrl.auto_play
            && ctrl.pending_audio
            && ctrl.last_scroll_time.elapsed() >= Duration::from_millis(150)
        {
            ctrl.trigger_audio_for_current();
        }

        ctrl.ensure_active_word_cached().await;

        let visible = ctrl.visible_indices();
        ctrl.selected = ctrl.selected.min(visible.len().saturating_sub(1));
        let stats = calculate_stats(&ctrl.sentences);

        let active_dict = ctrl
            .current_sentence_and_word()
            .and_then(|(_, _, w)| ctrl.cached_dict.get(&w));

        let status_text = ctrl.status_msg.as_ref().and_then(|(msg, expires)| {
            if Instant::now() < *expires {
                Some(msg.as_str())
            } else {
                None
            }
        });

        terminal.draw(|f| {
            render_explorer(
                f,
                RenderExplorerContext {
                    sentences: &ctrl.sentences,
                    visible_indices: &visible,
                    selected: ctrl.selected,
                    stats: &stats,
                    sort_order: ctrl.sort_order,
                    tier_filter: ctrl.tier_filter,
                    search: &ctrl.search,
                    search_active: ctrl.search_active,
                    auto_play: ctrl.auto_play,
                    playing_row: ctrl.playing_row,
                    loading_until: ctrl.loading_until,
                    active_dict,
                    selected_cards: &ctrl.selected_cards,
                    status_message: status_text,
                },
            );
        })?;

        if !event::poll(Duration::from_millis(50))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        if handle_action_shortcut(&mut ctrl, key).await? {
            continue;
        }

        match handle_key_press(&mut ctrl, key) {
            KeyAction::Continue => {}
            KeyAction::Quit => break false,
            KeyAction::ConfirmReview => break true,
        }
    };

    if let Some(mut child) = ctrl.audio_child.take() {
        let _ = child.kill();
    }

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    if should_review {
        Ok(ctrl.build_selected_candidates())
    } else {
        Ok(Vec::new())
    }
}
