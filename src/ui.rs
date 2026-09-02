pub mod bootstrap;
pub mod bundles;
pub mod card;
pub mod config_menu;
pub mod helpers;
pub mod inspector;
pub mod picker;
pub mod prompts;

#[cfg(test)]
mod tests;

pub use card::CardRenderParams;
pub use helpers::*;
pub use inspector::InspectSentencesParams;

use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMode {
    MineI1Candidates,
    ReviewKnownLines,
    Exit,
}

pub struct TerminalUi;

impl TerminalUi {
    pub fn print_banner() {
        helpers::print_banner();
    }

    pub fn select_media_file() -> Result<PathBuf> {
        picker::select_media_file()
    }

    pub fn select_bundle_source_files() -> Result<Vec<PathBuf>> {
        picker::select_bundle_source_files()
    }

    pub fn bootstrap_known_words(vocab_items: &[(String, usize, String)]) -> Result<Vec<String>> {
        bootstrap::bootstrap_known_words(vocab_items)
    }

    pub fn bootstrap_ignored_names(vocab_items: &[(String, usize, String)]) -> Result<Vec<String>> {
        bootstrap::bootstrap_ignored_names(vocab_items)
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

    pub fn select_session_mode(i1_count: usize, known_lines_count: usize) -> Result<SessionMode> {
        prompts::select_session_mode(i1_count, known_lines_count)
    }

    pub fn ask_next_batch(
        next_batch: usize,
        total_batches: usize,
        remaining_lines: usize,
    ) -> Result<bool> {
        prompts::ask_next_batch(next_batch, total_batches, remaining_lines)
    }

    pub fn ask_action() -> Result<char> {
        prompts::ask_action()
    }

    pub fn select_or_edit_reading(
        current_reading: &str,
        context_reading: &str,
        candidates: &[crate::dict::LookupResult],
    ) -> Result<String> {
        prompts::select_or_edit_reading(current_reading, context_reading, candidates)
    }

    pub fn select_candidate_or_custom(
        candidates: &[crate::dict::LookupResult],
        target_word: &str,
        current_reading: &str,
        current_pitch: &str,
        ai_analysis: Option<&crate::ai::AiAnalysisResult>,
    ) -> Result<crate::dict::LookupResult> {
        prompts::select_candidate_or_custom(
            candidates,
            target_word,
            current_reading,
            current_pitch,
            ai_analysis,
        )
    }

    pub fn select_sense(senses: &[String], target_word: &str) -> Result<Option<String>> {
        prompts::select_sense(senses, target_word)
    }

    pub fn inspect_sentences(p: inspector::InspectSentencesParams<'_>) -> Result<()> {
        inspector::inspect_sentences(p)
    }

    pub fn manage_known_words(words: &[(String, String)]) -> Result<Vec<String>> {
        bootstrap::manage_known_words(words)
    }

    pub fn manage_ignored_words(words: &[(String, String)]) -> Result<Vec<String>> {
        bootstrap::manage_ignored_words(words)
    }

    pub fn manage_mined_words(words: &[(String, String)]) -> Result<Vec<String>> {
        bootstrap::manage_mined_words(words)
    }

    pub fn show_config(cfg: &crate::config::AppConfig) {
        config_menu::show_config(cfg);
    }

    pub fn configure_interactive(cfg: &mut crate::config::AppConfig) -> Result<()> {
        config_menu::configure_interactive(cfg)
    }
}
