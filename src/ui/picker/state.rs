use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubtitleStatus {
    HasSub,
    Bundle,
    NoSub,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectableFile {
    pub path: PathBuf,
    pub status: SubtitleStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorAction {
    Continue,
    Confirm,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CategoryFilter {
    #[default]
    All,
    Videos,
    Bundles,
}

impl CategoryFilter {
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Videos,
            Self::Videos => Self::Bundles,
            Self::Bundles => Self::All,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Videos => "Videos",
            Self::Bundles => "Bundles",
        }
    }

    pub fn matches(self, item: &SelectableFile) -> bool {
        match self {
            Self::All => true,
            Self::Videos => item.status != SubtitleStatus::Bundle,
            Self::Bundles => item.status == SubtitleStatus::Bundle,
        }
    }
}

pub struct FileSelectorState {
    pub title: String,
    pub items: Vec<SelectableFile>,
    pub search_query: String,
    pub category_filter: CategoryFilter,
    pub filtered_indices: Vec<usize>,
    pub selected_indices: HashSet<usize>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub multi: bool,
}

impl FileSelectorState {
    pub fn new(title: impl Into<String>, items: Vec<SelectableFile>, multi: bool) -> Self {
        let filtered_indices = (0..items.len()).collect();
        Self {
            title: title.into(),
            items,
            search_query: String::new(),
            category_filter: CategoryFilter::All,
            filtered_indices,
            selected_indices: HashSet::new(),
            cursor: 0,
            scroll_offset: 0,
            multi,
        }
    }

    pub fn recompute_filter(&mut self) {
        let terms: Vec<String> = self
            .search_query
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();

        self.filtered_indices = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                if !self.category_filter.matches(item) {
                    return false;
                }
                if terms.is_empty() {
                    return true;
                }
                let path_str = item.path.to_string_lossy().to_lowercase();
                let status_str = match item.status {
                    SubtitleStatus::HasSub => "sub",
                    SubtitleStatus::Bundle => "bundle koto sub",
                    SubtitleStatus::NoSub => "nosub no sub",
                };
                let searchable = format!("{path_str} {status_str}");
                terms.iter().all(|term| searchable.contains(term))
            })
            .map(|(idx, _)| idx)
            .collect();

        self.cursor = 0;
        self.scroll_offset = 0;
    }

    pub fn toggle_and_advance(&mut self) {
        if !self.multi || self.filtered_indices.is_empty() {
            return;
        }
        let real_idx = self.filtered_indices[self.cursor];
        if self.selected_indices.contains(&real_idx) {
            self.selected_indices.remove(&real_idx);
        } else {
            self.selected_indices.insert(real_idx);
        }
        let max_idx = self.filtered_indices.len().saturating_sub(1);
        self.cursor = (self.cursor + 1).min(max_idx);
    }

    pub fn toggle_and_retreat(&mut self) {
        if !self.multi || self.filtered_indices.is_empty() {
            return;
        }
        let real_idx = self.filtered_indices[self.cursor];
        if self.selected_indices.contains(&real_idx) {
            self.selected_indices.remove(&real_idx);
        } else {
            self.selected_indices.insert(real_idx);
        }
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn ensure_cursor_visible(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        } else if self.cursor >= self.scroll_offset + visible_height {
            self.scroll_offset = self.cursor - visible_height + 1;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, visible_height: usize) -> SelectorAction {
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('c')) | (_, KeyCode::Esc) => {
                SelectorAction::Cancel
            }
            (_, KeyCode::Enter) => SelectorAction::Confirm,
            (KeyModifiers::NONE, KeyCode::Tab) => {
                self.toggle_and_advance();
                self.ensure_cursor_visible(visible_height);
                SelectorAction::Continue
            }
            (KeyModifiers::SHIFT, KeyCode::BackTab)
            | (KeyModifiers::NONE, KeyCode::BackTab)
            | (KeyModifiers::SHIFT, KeyCode::Tab) => {
                self.toggle_and_retreat();
                self.ensure_cursor_visible(visible_height);
                SelectorAction::Continue
            }
            (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::CONTROL, KeyCode::Char('p')) => {
                self.cursor = self.cursor.saturating_sub(1);
                self.ensure_cursor_visible(visible_height);
                SelectorAction::Continue
            }
            (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::CONTROL, KeyCode::Char('n')) => {
                let max_idx = self.filtered_indices.len().saturating_sub(1);
                self.cursor = (self.cursor + 1).min(max_idx);
                self.ensure_cursor_visible(visible_height);
                SelectorAction::Continue
            }
            (KeyModifiers::NONE, KeyCode::PageUp) => {
                self.cursor = self.cursor.saturating_sub(visible_height);
                self.ensure_cursor_visible(visible_height);
                SelectorAction::Continue
            }
            (KeyModifiers::NONE, KeyCode::PageDown) => {
                let max_idx = self.filtered_indices.len().saturating_sub(1);
                self.cursor = (self.cursor + visible_height).min(max_idx);
                self.ensure_cursor_visible(visible_height);
                SelectorAction::Continue
            }
            (KeyModifiers::NONE, KeyCode::Home) => {
                self.cursor = 0;
                self.scroll_offset = 0;
                SelectorAction::Continue
            }
            (KeyModifiers::NONE, KeyCode::End) => {
                self.cursor = self.filtered_indices.len().saturating_sub(1);
                self.ensure_cursor_visible(visible_height);
                SelectorAction::Continue
            }
            (KeyModifiers::CONTROL, KeyCode::Char('a')) => {
                if self.multi {
                    for &idx in &self.filtered_indices {
                        self.selected_indices.insert(idx);
                    }
                }
                SelectorAction::Continue
            }
            (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
                if self.multi {
                    self.selected_indices.clear();
                }
                SelectorAction::Continue
            }
            (KeyModifiers::CONTROL, KeyCode::Char('f')) | (_, KeyCode::F(2)) => {
                self.category_filter = self.category_filter.next();
                self.recompute_filter();
                SelectorAction::Continue
            }
            (_, KeyCode::Backspace) => {
                self.search_query.pop();
                self.recompute_filter();
                SelectorAction::Continue
            }
            (_, KeyCode::Char(c)) => {
                self.search_query.push(c);
                self.recompute_filter();
                SelectorAction::Continue
            }
            _ => SelectorAction::Continue,
        }
    }

    pub fn build_result(self) -> Vec<PathBuf> {
        if self.multi && !self.selected_indices.is_empty() {
            return self
                .items
                .into_iter()
                .enumerate()
                .filter(|(idx, _)| self.selected_indices.contains(idx))
                .map(|(_, item)| item.path)
                .collect();
        }

        if let Some(&idx) = self.filtered_indices.get(self.cursor) {
            if let Some(item) = self.items.get(idx) {
                return vec![item.path.clone()];
            }
        }

        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_files() -> Vec<SelectableFile> {
        vec![
            SelectableFile {
                path: PathBuf::from("/anime/Yuru Camp/Yuru Camp - 01.mkv"),
                status: SubtitleStatus::HasSub,
            },
            SelectableFile {
                path: PathBuf::from("/anime/Yuru Camp/Yuru Camp - 01.ja.srt"),
                status: SubtitleStatus::HasSub,
            },
            SelectableFile {
                path: PathBuf::from("/anime/Bocchi/Bocchi - 01.mkv"),
                status: SubtitleStatus::NoSub,
            },
        ]
    }

    #[test]
    fn multi_token_filter_with_space() {
        let mut state = FileSelectorState::new("Test", mock_files(), true);
        state.search_query = "yuru camp".to_string();
        state.recompute_filter();
        assert_eq!(state.filtered_indices.len(), 2);
        assert_eq!(state.filtered_indices, vec![0, 1]);
    }

    #[test]
    fn tab_toggles_and_advances() {
        let mut state = FileSelectorState::new("Test", mock_files(), true);
        state.toggle_and_advance();
        assert!(state.selected_indices.contains(&0));
        assert_eq!(state.cursor, 1);
    }

    #[test]
    fn enter_without_tab_selects_highlighted() {
        let state = FileSelectorState::new("Test", mock_files(), true);
        let res = state.build_result();
        assert_eq!(
            res,
            vec![PathBuf::from("/anime/Yuru Camp/Yuru Camp - 01.mkv")]
        );
    }

    #[test]
    fn category_filter_cycles_and_filters() {
        let mut state = FileSelectorState::new(
            "Test",
            vec![
                SelectableFile {
                    path: PathBuf::from("/anime/Yuru.mkv"),
                    status: SubtitleStatus::HasSub,
                },
                SelectableFile {
                    path: PathBuf::from("/anime/Yuru.koto"),
                    status: SubtitleStatus::Bundle,
                },
            ],
            true,
        );
        assert_eq!(state.filtered_indices.len(), 2);

        state.category_filter = state.category_filter.next();
        state.recompute_filter();
        assert_eq!(state.filtered_indices.len(), 1);
        assert_eq!(state.filtered_indices, vec![0]);

        state.category_filter = state.category_filter.next();
        state.recompute_filter();
        assert_eq!(state.filtered_indices.len(), 1);
        assert_eq!(state.filtered_indices, vec![1]);

        state.category_filter = state.category_filter.next();
        state.recompute_filter();
        assert_eq!(state.filtered_indices.len(), 2);
    }
}
