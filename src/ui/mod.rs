use anyhow::Result;
use console::{measure_text_width, Style};
use crossterm::terminal;
use inquire::{MultiSelect, Select, Text};
use std::path::PathBuf;
use walkdir::WalkDir;

/// Returns terminal width capped at 110, minimum 60.
fn box_width() -> usize {
    terminal::size().map(|(w, _)| w as usize).unwrap_or(80).min(110).max(60)
}

/// Wraps content in a full-width box row: "│ <content><padding> │"
fn box_row(content: &str, inner_w: usize) -> String {
    let vis = measure_text_width(content);
    let pad = inner_w.saturating_sub(vis);
    format!("│ {}{} │", content, " ".repeat(pad))
}

/// Empty padded row.
fn box_empty(inner_w: usize) -> String {
    format!("│ {} │", " ".repeat(inner_w))
}

pub struct TerminalUi;

impl TerminalUi {
    /// Prints a full-width box with the app title centered inside.
    pub fn print_banner() {
        let bw = box_width();
        let iw = bw - 4; // inner width between "│ " and " │"

        let title = "🌸  K O T O N O H A  ──  Japanese $i+1$ Sentence Miner";
        let title_vis = measure_text_width(title);

        let total_pad = iw.saturating_sub(title_vis);
        let left_pad  = total_pad / 2;
        let right_pad = total_pad - left_pad;

        let top    = format!("┌{}┐", "─".repeat(bw - 2));
        let middle = format!("│ {}{}{} │", " ".repeat(left_pad), title, " ".repeat(right_pad));
        let bottom = format!("└{}┘", "─".repeat(bw - 2));

        println!("\n{}", top);
        println!("{}", middle);
        println!("{}\n", bottom);
    }

    pub fn select_media_file() -> Result<PathBuf> {

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let search_dirs = vec![home.join("Videos"), PathBuf::from(".")];

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

        files.sort();
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
            let parts: Vec<&str> = item.split_whitespace().collect();
            if parts.len() >= 2 {
                checked_words.push(parts[1].to_string());
            }
        }

        Ok(checked_words)
    }

    pub fn render_progress(current: usize, total: usize, mined: usize, skipped: usize, ignored: usize) {
        let magenta = Style::new().magenta().bold();
        let cyan = Style::new().cyan().bold();
        let green = Style::new().green().bold();
        let yellow = Style::new().yellow();
        let dim = Style::new().dim();

        println!(
            "\n {} Progress [{}]  {}  {} {}  {}  {} {}  {}  {} {}",
            magenta.apply_to("🌸"),
            cyan.apply_to(&format!("{}/{}", current, total)),
            dim.apply_to("───"),
            green.apply_to("✨"),
            green.apply_to(&format!("{} Mined", mined)),
            dim.apply_to("•"),
            yellow.apply_to("⏭️"),
            yellow.apply_to(&format!("{} Skipped", skipped)),
            dim.apply_to("•"),
            dim.apply_to("🙈"),
            dim.apply_to(&format!("{} Ignored", ignored)),
        );
    }

    pub fn render_card(
        rank: usize,
        sentence: &str,
        target_word: &str,
        reading: &str,
        pitch: &str,
        jpdb_rank: Option<u32>,
        definition: &str,
        known_context: &[String],
        unknown_context: &[String],
    ) {
        let cyan = Style::new().cyan().bold();
        let yellow = Style::new().yellow().bold();
        let green = Style::new().green().bold();
        let red = Style::new().red().bold();

        let bw = box_width();      // total box width incl. borders
        let iw = bw - 4;           // inner content width (│ _ _ _ │)
        const LABEL: usize = 15;   // fixed label column display width

        // Top border: ┌─ RANK #N [★ i+1 Candidate] ──────────────────────────────┐
        let rank_label = format!(" RANK #{} ", rank);
        let i1_label = " [★ i+1 Candidate] ";
        let label_width = rank_label.len() + i1_label.len();
        let dash_count = bw.saturating_sub(label_width + 3);
        let top = format!(
            "┌─{}{}{}┐",
            yellow.apply_to(&rank_label),
            green.apply_to(i1_label),
            "─".repeat(dash_count)
        );

        // Bottom border
        let bottom = format!("└{}┘", "─".repeat(bw - 2));

        // Helper: label row  e.g. "Sentence:       <value>"
        let lrow = |label: &str, value: &str| -> String {
            let label_pad = LABEL.saturating_sub(measure_text_width(label));
            let content = format!("{}{}{}", label, " ".repeat(label_pad), value);
            box_row(&content, iw)
        };

        let highlighted = sentence.replace(target_word, &green.apply_to(target_word).to_string());

        let unknown_str = if unknown_context.is_empty() {
            "None (i+1 target)".to_string()
        } else {
            unknown_context.join(", ")
        };
        let known_str = if known_context.is_empty() {
            "None".to_string()
        } else {
            known_context.join(", ")
        };

        println!("\n{}", top);
        println!("{}", box_empty(iw));
        println!("{}", lrow("Sentence:", &highlighted));
        println!("{}", lrow("Target Word:", &format!("{} ({} [Pitch: {}])", green.apply_to(target_word), reading, pitch)));
        if let Some(r) = jpdb_rank {
            println!("{}", lrow("JPDB Rank:", &format!("#{}", r)));
        }
        println!("{}", lrow("Definitions:", definition));
        println!("{}", lrow("Unknown Words:", &red.apply_to(&unknown_str).to_string()));
        println!("{}", lrow("Known Words:", &cyan.apply_to(&known_str).to_string()));
        println!("{}", box_empty(iw));
        println!("{}\n", bottom);
    }

    pub fn ask_action() -> Result<char> {
        let options = vec![
            "⛏️  Mine this card (y)",
            "🔊  Replay preview audio (r)",
            "⏭️  Skip to next card (n)",
            "🚫  Ignore target word (i)",
            "🚪  Quit (q)",
        ];

        let ans = Select::new("Mine this card?", options).prompt()?;
        if ans.contains("(y)") {
            Ok('y')
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

    /// Show all ignored words and let the user multiselect which ones to un-ignore.
    /// Returns the words that were selected for removal.
    pub fn manage_ignored_words(words: &[String]) -> Result<Vec<String>> {
        if words.is_empty() {
            println!("\n 📭 No ignored words yet.\n");
            return Ok(Vec::new());
        }

        let selected = MultiSelect::new(
            "Select words to UN-IGNORE (Space to toggle, Enter to confirm):",
            words.to_vec(),
        )
        .prompt()?;

        Ok(selected)
    }

    /// Prints all subtitle sentences sorted by timestamp with known words in Blue and unknown words in Red.
    pub fn inspect_sentences(
        sentences: &[crate::srt::SubtitleSentence],
        tokenizer: &crate::nlp::JapaneseTokenizer,
        known_words: &std::collections::HashSet<String>,
        ignored_words: &std::collections::HashSet<String>,
    ) {
        let blue = Style::new().blue().bold();
        let red = Style::new().red().bold();
        let green = Style::new().green().bold();
        let dim = Style::new().dim();
        let yellow = Style::new().yellow();

        let mut sorted_sentences = sentences.to_vec();
        sorted_sentences.sort_by_key(|s| s.start_ms);

        let mut file_known = std::collections::HashSet::new();
        let mut file_unknown = std::collections::HashSet::new();
        let mut eligible_i1_count = 0;
        let mut sentence_i1_status = Vec::with_capacity(sorted_sentences.len());

        for sub in &sorted_sentences {
            let mut sentence_unknowns = std::collections::HashSet::new();
            if let Ok(tokens) = tokenizer.tokenize(&sub.text) {
                for t in tokens {
                    if t.is_content_word && !ignored_words.contains(&t.dictionary_form) {
                        if known_words.contains(&t.dictionary_form) {
                            file_known.insert(t.dictionary_form.clone());
                        } else {
                            file_unknown.insert(t.dictionary_form.clone());
                            sentence_unknowns.insert(t.dictionary_form.clone());
                        }
                    }
                }
            }
            let is_i1 = sentence_unknowns.len() == 1;
            if is_i1 {
                eligible_i1_count += 1;
            }
            sentence_i1_status.push(is_i1);
        }

        println!("\n🔍 Inspecting {} subtitle lines (sorted by subtitle timing):", sorted_sentences.len());
        println!(
            "   Stats: {} | {} | {}",
            blue.apply_to(&format!("{} Known Words", file_known.len())),
            red.apply_to(&format!("{} Unknown Words", file_unknown.len())),
            green.apply_to(&format!("{} Eligible i+1 Sentences", eligible_i1_count))
        );
        println!(
            "   Legend: {} | {} | {} | {}\n",
            blue.apply_to("Known Word"),
            red.apply_to("Unknown Word"),
            dim.apply_to("Grammar/Ignored"),
            green.apply_to("★ i+1 Eligible")
        );

        let mut options = Vec::with_capacity(sorted_sentences.len());

        for (idx, sub) in sorted_sentences.iter().enumerate() {
            let is_i1 = sentence_i1_status[idx];
            let mut formatted_sentence = String::new();

            if let Ok(tokens) = tokenizer.tokenize(&sub.text) {
                for t in tokens {
                    if !t.is_content_word || ignored_words.contains(&t.dictionary_form) {
                        formatted_sentence.push_str(&dim.apply_to(&t.surface).to_string());
                    } else if known_words.contains(&t.dictionary_form) {
                        formatted_sentence.push_str(&blue.apply_to(&t.surface).to_string());
                    } else {
                        formatted_sentence.push_str(&red.apply_to(&t.surface).to_string());
                    }
                }
            } else {
                formatted_sentence = sub.text.clone();
            }

            let total_secs = sub.start_ms / 1000;
            let mins = (total_secs / 60) % 60;
            let secs = total_secs % 60;
            let hours = total_secs / 3600;
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
}

