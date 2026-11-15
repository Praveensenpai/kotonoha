pub mod modal;
pub mod panes;
pub mod render;
pub mod theme;

#[cfg(test)]
mod tests;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::Path;
use std::time::Duration;

use crate::dict::LookupResult;
use crate::media::MediaExtractor;
use crate::miner::CandidateSentence;
use modal::ActiveModal;
use render::{render_dashboard, DashboardRenderContext};

pub enum DashboardAction {
    Mine,
    Skip,
    Known,
    UnmarkKnown,
    Ignore,
    Quit,
}

pub struct DashboardParams<'a> {
    pub card_index: usize,
    pub total_cards: usize,
    pub mined_count: usize,
    pub known_count: usize,
    pub skipped_count: usize,
    pub ignored_count: usize,
    pub cand: &'a CandidateSentence,
    pub dict_info: &'a mut LookupResult,
    pub ai_suggestion: Option<&'a str>,
    pub is_ai_selected: bool,
    pub audio_path: Option<&'a Path>,
    pub all_candidates: Vec<LookupResult>,
}

struct DashboardState {
    modal: ActiveModal,
    audio_child: Option<std::process::Child>,
    audio_playing: bool,
}

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

pub fn run_card_dashboard(mut p: DashboardParams<'_>) -> Result<DashboardAction> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = DashboardState {
        modal: ActiveModal::None,
        audio_child: p.audio_path.and_then(MediaExtractor::play_preview_audio),
        audio_playing: p.audio_path.is_some(),
    };

    loop {
        terminal.draw(|f| {
            render_dashboard(
                f,
                &DashboardRenderContext {
                    card_index: p.card_index,
                    total_cards: p.total_cards,
                    mined_count: p.mined_count,
                    known_count: p.known_count,
                    skipped_count: p.skipped_count,
                    ignored_count: p.ignored_count,
                    cand: p.cand,
                    dict_info: p.dict_info,
                    ai_suggestion: p.ai_suggestion,
                    is_ai_selected: p.is_ai_selected,
                    audio_playing: state.audio_playing,
                    modal: &state.modal,
                },
            );
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if let Some(action) =
                        handle_dashboard_key(&mut p, &mut state, key.code, key.modifiers)
                    {
                        if let Some(mut child) = state.audio_child.take() {
                            let _ = child.kill();
                        }
                        return Ok(action);
                    }
                }
            }
        }

        if let Some(ref mut child) = state.audio_child {
            if let Ok(Some(_)) = child.try_wait() {
                state.audio_playing = false;
            }
        }
    }
}

enum ModalOutcome {
    Continue,
    Close,
    SelectCandidate(LookupResult),
    UpdateReading(String),
}

fn handle_dashboard_key(
    p: &mut DashboardParams<'_>,
    state: &mut DashboardState,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Option<DashboardAction> {
    if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
        return Some(DashboardAction::Quit);
    }

    match state.modal {
        ActiveModal::SelectCandidate {
            ref candidates,
            ref mut selected_index,
        } => {
            match handle_candidate_modal_key(code, candidates, selected_index) {
                ModalOutcome::SelectCandidate(chosen) => {
                    *p.dict_info = chosen;
                    state.modal = ActiveModal::None;
                }
                ModalOutcome::Close => {
                    state.modal = ActiveModal::None;
                }
                ModalOutcome::Continue | ModalOutcome::UpdateReading(_) => {}
            }
            None
        }
        ActiveModal::EditReading { ref mut buffer } => {
            match handle_reading_modal_key(code, buffer) {
                ModalOutcome::UpdateReading(reading) => {
                    p.dict_info.reading = reading;
                    state.modal = ActiveModal::None;
                }
                ModalOutcome::Close => {
                    state.modal = ActiveModal::None;
                }
                ModalOutcome::Continue | ModalOutcome::SelectCandidate(_) => {}
            }
            None
        }
        ActiveModal::None => handle_main_dashboard_key(p, state, code),
    }
}

fn handle_main_dashboard_key(
    p: &mut DashboardParams<'_>,
    state: &mut DashboardState,
    code: KeyCode,
) -> Option<DashboardAction> {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => Some(DashboardAction::Mine),
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char(' ') => Some(DashboardAction::Skip),
        KeyCode::Char('k') | KeyCode::Char('K') => Some(DashboardAction::Known),
        KeyCode::Char('u') | KeyCode::Char('U') => Some(DashboardAction::UnmarkKnown),
        KeyCode::Char('i') | KeyCode::Char('I') => Some(DashboardAction::Ignore),
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => Some(DashboardAction::Quit),
        KeyCode::Char('r') | KeyCode::Char('R') => {
            replay_audio(p.audio_path, state);
            None
        }
        KeyCode::Char('c') | KeyCode::Char('C') => {
            if !p.all_candidates.is_empty() {
                state.modal = ActiveModal::SelectCandidate {
                    candidates: p.all_candidates.clone(),
                    selected_index: 0,
                };
            }
            None
        }
        KeyCode::Char('f') | KeyCode::Char('F') => {
            state.modal = ActiveModal::EditReading {
                buffer: p.dict_info.reading.clone(),
            };
            None
        }
        _ => None,
    }
}

fn handle_candidate_modal_key(
    code: KeyCode,
    candidates: &[LookupResult],
    selected_index: &mut usize,
) -> ModalOutcome {
    match code {
        KeyCode::Up => {
            if *selected_index > 0 {
                *selected_index -= 1;
            }
            ModalOutcome::Continue
        }
        KeyCode::Down => {
            if *selected_index + 1 < candidates.len() {
                *selected_index += 1;
            }
            ModalOutcome::Continue
        }
        KeyCode::Enter => {
            if let Some(cand) = candidates.get(*selected_index) {
                ModalOutcome::SelectCandidate(cand.clone())
            } else {
                ModalOutcome::Close
            }
        }
        KeyCode::Esc => ModalOutcome::Close,
        _ => ModalOutcome::Continue,
    }
}

fn handle_reading_modal_key(code: KeyCode, buffer: &mut String) -> ModalOutcome {
    match code {
        KeyCode::Char(c) => {
            buffer.push(c);
            ModalOutcome::Continue
        }
        KeyCode::Backspace => {
            buffer.pop();
            ModalOutcome::Continue
        }
        KeyCode::Enter => {
            let trimmed = buffer.trim();
            if trimmed.is_empty() {
                ModalOutcome::Close
            } else {
                ModalOutcome::UpdateReading(trimmed.to_string())
            }
        }
        KeyCode::Esc => ModalOutcome::Close,
        _ => ModalOutcome::Continue,
    }
}

fn replay_audio(audio_path: Option<&Path>, state: &mut DashboardState) {
    if let Some(mut child) = state.audio_child.take() {
        let _ = child.kill();
    }
    if let Some(path) = audio_path {
        state.audio_child = MediaExtractor::play_preview_audio(path);
        state.audio_playing = state.audio_child.is_some();
    }
}
