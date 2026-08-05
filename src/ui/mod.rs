pub mod card;
pub mod config_menu;

pub use card::{box_width, CardRenderParams};

use anyhow::Result;
use console::{measure_text_width, Style};
use inquire::{MultiSelect, Select, Text};
use std::cmp::Ordering;
use std::path::PathBuf;
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
                    format!("#{:02}  {} ({}) — {} occurrences", idx + 1, word, reading, count)
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

    pub fn render_progress(
        current: usize,
        total: usize,
        mined: usize,
        skipped: usize,
        ignored: usize,
    ) {
        card::render_progress(current, total, mined, skipped, ignored);
    }

    pub fn render_card(p: CardRenderParams<'_>) {
        card::render_card(p);
    }

    pub fn ask_action() -> Result<char> {
        let options = vec![
            "⏭️  Skip to next card (n)",
            "⛏️  Mine this card (y)",
            "📖  Change dictionary candidate (c)",
            "🔊  Replay preview audio (r)",
            "🚫  Ignore target word (i)",
            "🚪  Quit (q)",
        ];

        let ans = Select::new("Action?", options).prompt()?;
        if ans.contains("(y)") {
            Ok('y')
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
            let ai_tag = if is_ai_rec { " ✨ [AI Recommended]" } else { "" };
            let first_sense = cand
                .definition
                .lines()
                .next()
                .unwrap_or(&cand.definition);
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
            if let Some(num) = rank_str.strip_prefix('#').and_then(|s| s.parse::<usize>().ok()) {
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

    pub fn select_sense(senses: &[String], target_word: &str) -> Result<String> {
        let options: Vec<String> = senses
            .iter()
            .enumerate()
            .map(|(idx, s)| format!("#{:<2} {}", idx + 1, s))
            .collect();
        let prompt = format!("Select sense for 【{}】:", target_word);
        let ans = Select::new(&prompt, options).prompt()?;
        if let Some(rank_str) = ans.split_whitespace().next() {
            if let Some(num) = rank_str.strip_prefix('#').and_then(|s| s.parse::<usize>().ok()) {
                if let Some(sense) = senses.get(num.saturating_sub(1)) {
                    return Ok(sense.clone());
                }
            }
        }
        Ok(senses.first().cloned().unwrap_or_default())
    }

    pub fn inspect_sentences(
        sentences: &[crate::srt::SubtitleSentence],
        tokenizer: &crate::nlp::JapaneseTokenizer,
        known_words: &std::collections::HashSet<String>,
        ignored_words: &std::collections::HashSet<String>,
    ) {
        let green = Style::new().green().bold();
        let yellow = Style::new().yellow();

        let mut options = Vec::new();
        for s in sentences {
            let tokens = tokenizer.tokenize(&s.text).unwrap_or_default();
            let mut unknown_count = 0;
            let mut formatted_sentence = s.text.clone();

            for t in &tokens {
                if t.is_content_word
                    && !known_words.contains(&t.dictionary_form)
                    && !ignored_words.contains(&t.dictionary_form)
                {
                    unknown_count += 1;
                    formatted_sentence = formatted_sentence.replace(
                        &t.surface,
                        &green.apply_to(&t.surface).to_string(),
                    );
                }
            }

            let is_i1 = unknown_count == 1;
            let mins = s.start_ms / 60_000;
            let secs = (s.start_ms % 60_000) / 1000;
            let hours = mins / 60;
            let mins = mins % 60;

            let ts_str = if hours > 0 {
                format!("{:02}:{:02}:{:02}", hours, mins, secs)
            } else {
                format!("{:02}:{:02}", mins, secs)
            };

            let prefix = if is_i1 {
                format!("★ [{}]", green.apply_to(&ts_str))
            } else {
                format!("  [{}]", yellow.apply_to(&ts_str))
            };

            options.push(format!("{}  {}", prefix, formatted_sentence));
        }

        let _ = Select::new(
            "Inspect Subtitles (↑/↓ scroll, type to search, Enter/Esc to exit):",
            options,
        )
        .with_page_size(18)
        .prompt();
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
        let mut episodes = ["episode-10.mkv", "episode-08.mkv", "episode-01.mkv", "episode-2.mkv"];
        episodes.sort_by(|left, right| natural_cmp(left, right));
        assert_eq!(
            episodes,
            ["episode-01.mkv", "episode-2.mkv", "episode-08.mkv", "episode-10.mkv"]
        );
    }
}
