pub mod card;
pub mod config_menu;
pub mod inspector;

pub use card::{box_width, CardRenderParams};
pub use inspector::InspectSentencesParams;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMode {
    MineI1Candidates,
    ReviewKnownLines,
    Exit,
}

use anyhow::Result;
use console::measure_text_width;
use indicatif::{ProgressBar, ProgressStyle};
use inquire::{MultiSelect, Select, Text};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
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

    pub fn is_hidden_or_ignored_entry(entry: &walkdir::DirEntry) -> bool {
        let name = entry.file_name().to_string_lossy();
        // Skip hidden dot-files and dot-directories (e.g. .cache, .config, .cargo, .local, .git)
        if entry.depth() > 0 && name.starts_with('.') {
            return false;
        }
        // Skip common large non-media build/cache directories
        if matches!(
            name.as_ref(),
            "node_modules"
                | "target"
                | "venv"
                | ".venv"
                | "env"
                | "collection.media"
                | "__pycache__"
                | "vendor"
        ) {
            return false;
        }
        true
    }

    pub fn discover_media_files(allowed_exts: &[&str], spinner_msg: &str) -> Result<Vec<PathBuf>> {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template(" {spinner:.green} {msg}")
                .unwrap(),
        );
        pb.set_message(spinner_msg.to_string());
        pb.enable_steady_tick(std::time::Duration::from_millis(80));

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let mut search_dirs = Vec::new();

        // 1. Current working directory
        search_dirs.push(PathBuf::from("."));

        // 2. Standard user media folders if they exist
        let videos = home.join("Videos");
        if videos.exists() && !search_dirs.contains(&videos) {
            search_dirs.push(videos);
        }
        let downloads = home.join("Downloads");
        if downloads.exists() && !search_dirs.contains(&downloads) {
            search_dirs.push(downloads);
        }
        let anime = home.join("Anime");
        if anime.exists() && !search_dirs.contains(&anime) {
            search_dirs.push(anime);
        }

        let is_cwd_home = std::env::current_dir().map(|cwd| cwd == home).unwrap_or(false);
        let mut files = Vec::new();

        for dir in search_dirs {
            if !dir.exists() {
                continue;
            }

            let max_depth = if dir == Path::new(".") && is_cwd_home {
                // Prevent deep scanning whole home tree when run directly at ~
                2
            } else {
                6
            };

            for entry in WalkDir::new(&dir)
                .max_depth(max_depth)
                .into_iter()
                .filter_entry(Self::is_hidden_or_ignored_entry)
                .filter_map(|e| e.ok())
            {
                let p = entry.path();
                if p.is_file() {
                    if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                        let ext = ext.to_lowercase();
                        if allowed_exts.contains(&ext.as_str()) {
                            files.push(p.to_path_buf());
                        }
                    }
                }
            }
        }

        pb.finish_and_clear();

        files.sort_by(|left, right| natural_cmp(&left.to_string_lossy(), &right.to_string_lossy()));
        files.dedup();

        Ok(files)
    }

    pub fn select_media_file() -> Result<PathBuf> {
        let files = Self::discover_media_files(
            &["srt", "ass", "vtt", "mkv", "mp4", "webm", "koto"],
            "Scanning for media and subtitle files...",
        )?;

        if files.is_empty() {
            let input = Text::new("No media files auto-discovered. Enter file path:").prompt()?;
            return Ok(PathBuf::from(input));
        }

        let items: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
        let selected = Select::new("Select Subtitle or Anime Video File:", items).prompt()?;
        Ok(PathBuf::from(selected))
    }

    pub fn select_bundle_source_files() -> Result<Vec<PathBuf>> {
        let files = Self::discover_media_files(
            &["srt", "ass", "vtt", "mkv", "mp4", "webm", "avi"],
            "Scanning for unbundled video and subtitle files...",
        )?;

        if files.is_empty() {
            let input = Text::new("No unbundled media files discovered. Enter file path:").prompt()?;
            return Ok(vec![PathBuf::from(input)]);
        }

        let items: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
        let selected = MultiSelect::new(
            "📦 Select Video or Subtitle File(s) to Bundle into .koto (Space to select, Enter to bundle):",
            items,
        )
        .with_page_size(15)
        .prompt()?;

        if selected.is_empty() {
            anyhow::bail!("No files selected for bundling.");
        }

        Ok(selected.into_iter().map(PathBuf::from).collect())
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

    pub fn select_session_mode(i1_count: usize, known_lines_count: usize) -> Result<SessionMode> {
        let options = vec![
            format!("🎯  Mine i+1 Candidate Cards ({} candidates)", i1_count),
            format!(
                "🔍  Review & Verify Known Sentences ({} lines)",
                known_lines_count
            ),
            "🚪  Exit".to_string(),
        ];

        let choice = Select::new("Select session mode:", options).prompt()?;
        if choice.contains("Mine i+1 Candidate") {
            Ok(SessionMode::MineI1Candidates)
        } else if choice.contains("Review & Verify Known") {
            Ok(SessionMode::ReviewKnownLines)
        } else {
            Ok(SessionMode::Exit)
        }
    }

    pub fn ask_next_batch(
        next_batch: usize,
        total_batches: usize,
        remaining_lines: usize,
    ) -> Result<bool> {
        let prompt = format!(
            "Proceed to Batch {}/{} ({} candidate lines remaining)?",
            next_batch, total_batches, remaining_lines
        );
        let options = vec![
            format!(
                "🚀  Continue to Batch {}/{} ({} remaining)",
                next_batch, total_batches, remaining_lines
            ),
            "🚪  Finish & Exit mining session".to_string(),
        ];
        let choice = Select::new(&prompt, options).prompt()?;
        Ok(choice.contains("Continue"))
    }

    pub fn ask_action() -> Result<char> {
        let options = vec![
            "⏭️  Skip to next card (n)",
            "⛏️  Mine this card (y)",
            "🧠  Mark target word as known (k)",
            "🔓  Unmark target word as known / move to unknown (u)",
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
        } else if ans.contains("(u)") {
            Ok('u')
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

        let mut select = Select::new("Select dictionary definition:", options).with_page_size(10);
        if let Some(idx) = ai_rec_cand {
            if idx < candidates.len() {
                select = select.with_starting_cursor(idx);
            }
        } else if ai_suggested_def.is_some() {
            select = select.with_starting_cursor(candidates.len());
        }

        let ans = select.prompt()?;

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

    pub fn inspect_sentences(p: inspector::InspectSentencesParams<'_>) -> Result<()> {
        inspector::inspect_sentences(p)
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
mod tests;
