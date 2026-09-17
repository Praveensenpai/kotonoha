use anyhow::{bail, Result};
use crossterm::{
    cursor::Show,
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use super::render::render_selector;
use super::state::{FileSelectorState, SelectableFile, SelectorAction};

struct TerminalGuard;

impl TerminalGuard {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
    }
}

pub fn run_file_selector(
    title: impl Into<String>,
    items: Vec<SelectableFile>,
    multi: bool,
) -> Result<Vec<PathBuf>> {
    if items.is_empty() {
        bail!("No files available to select.");
    }

    let _guard = TerminalGuard::new()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut state = FileSelectorState::new(title, items, multi);

    loop {
        let mut visible_height = 10;
        terminal.draw(|frame| {
            visible_height = render_selector(frame, &state);
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

        match state.handle_key(key, visible_height) {
            SelectorAction::Continue => {}
            SelectorAction::Confirm => break,
            SelectorAction::Cancel => bail!("No files selected."),
        }
    }

    let result = state.build_result();
    if result.is_empty() {
        bail!("No files selected.");
    }

    Ok(result)
}
