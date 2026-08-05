use anyhow::Result;
use console::{measure_text_width, Style};
use crossterm::terminal;
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
            (Some(left_char), Some(right_char)) if left_char.is_ascii_digit() && right_char.is_ascii_digit() => {
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
            // Recover the original vocabulary entry by its stable numbered
            // option. Parsing the rendered label is fragile because labels
            // contain readings, punctuation, and whitespace.
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
        ai_warning: Option<&str>,
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

        let max_val_w = iw.saturating_sub(LABEL);

        let format_val = |val: &str| -> String {
            let vis = measure_text_width(val);
            if vis > max_val_w && max_val_w > 3 {
                let mut truncated = String::new();
                let mut current_w = 0;
                for c in val.chars() {
                    let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(1);
                    if current_w + cw + 3 > max_val_w {
                        truncated.push_str("...");
                        break;
                    }
                    truncated.push(c);
                    current_w += cw;
                }
                truncated
            } else {
                val.to_string()
            }
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
        let pitch_num = pitch.parse::<usize>().unwrap_or_else(|_| {
            if pitch == "HLL" || pitch == "1" { 1 } else { 0 }
        });
        let (pitch_overbar, pitch_tag, _morae_cnt) = crate::dict::format_pitch_accent(reading, pitch_num);

        println!("{}", lrow("Target Word:", &format!("{} ({} [Pitch: {}])", green.apply_to(target_word), yellow.apply_to(&pitch_overbar), cyan.apply_to(&pitch_tag))));
        if let Some(r) = jpdb_rank {
            println!("{}", lrow("JPDB Rank:", &format!("#{}", r)));
        }

        if let Some(warn) = ai_warning {
            let warn_styled = format!("⚠️  AI Parsing Warning: {}", warn);
            println!("{}", lrow("AI Notice:", &red.apply_to(&format_val(&warn_styled)).to_string()));
        }

        let def_lines: Vec<&str> = definition.lines().collect();
        if def_lines.is_empty() {
            println!("{}", lrow("Definitions:", "No definition"));
        } else {
            for (idx, line) in def_lines.iter().enumerate() {
                let clean_line = line.trim();
                let clean_line = clean_line.strip_prefix('│').unwrap_or(clean_line).trim();
                let truncated_line = format_val(clean_line);
                if idx == 0 {
                    println!("{}", lrow("Definitions:", &truncated_line));
                } else {
                    println!("{}", lrow("", &truncated_line));
                }
            }
        }

        println!("{}", lrow("Unknown Words:", &red.apply_to(&format_val(&unknown_str)).to_string()));
        println!("{}", lrow("Known Words:", &cyan.apply_to(&format_val(&known_str)).to_string()));
        println!("{}", box_empty(iw));
        println!("{}\n", bottom);
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
        expression: &str,
        reading: &str,
        pitch_accent: &str,
        ai_res: Option<&crate::ai::AiAnalysisResult>,
    ) -> Result<crate::dict::LookupResult> {
        let rec_idx = ai_res.and_then(|r| r.recommended_candidate_index);
        let mut options: Vec<String> = candidates
            .iter()
            .enumerate()
            .map(|(idx, res)| {
                let first_line = res.definition.lines().next().unwrap_or("");
                let clean_line = first_line.trim().strip_prefix('│').unwrap_or(first_line).trim();
                let badge = if rec_idx == Some(idx) { " ✨ [AI Recommended]" } else { "" };
                format!("#{} 【{} ({})】 {}{}", idx + 1, res.expression, res.reading, clean_line, badge)
            })
            .collect();

        let ai_suggested_def = ai_res.and_then(|r| r.custom_definition_suggestion.as_deref());
        if let Some(sug) = ai_suggested_def {
            options.push(format!("✨ Use AI Suggested Definition: \"{}\"", sug));
        }

        options.push("✍ Enter custom definition".to_string());

        let default_selection_idx = rec_idx.unwrap_or(0).min(options.len().saturating_sub(1));
        let selected = Select::new("Select dictionary definition:", options.clone())
            .with_starting_cursor(default_selection_idx)
            .with_page_size(10)
            .prompt()?;

        if let Some(sug) = ai_suggested_def {
            if selected.starts_with("✨ Use AI Suggested Definition:") {
                return Ok(crate::dict::LookupResult {
                    expression: expression.to_string(),
                    reading: reading.to_string(),
                    definition: format!("1. [AI Suggestion] {}", sug),
                    pitch_accent: pitch_accent.to_string(),
                });
            }
        }

        let custom_index = options.len() - 1;
        if options[custom_index] == selected {
            let definition = Text::new(&format!("Definition for {expression} (context:):")).prompt()?;
            if definition.trim().is_empty() {
                anyhow::bail!("Custom definition cannot be empty.");
            }
            return Ok(crate::dict::LookupResult {
                expression: expression.to_string(),
                reading: reading.to_string(),
                definition,
                pitch_accent: pitch_accent.to_string(),
            });
        }

        let selected_index = options.iter().position(|option| option == &selected).unwrap_or(0);
        Ok(candidates[selected_index].clone())
    }

    /// Ask the user to pick a sense. Auto-selects if only one sense exists.
    pub fn select_sense(senses: &[String], word: &str) -> Result<String> {
        if senses.len() == 1 {
            return Ok(senses[0].clone());
        }
        let selected = Select::new(
            &format!("Which sense for 【{}】?", word),
            senses.to_vec(),
        )
        .with_page_size(10)
        .prompt()?;
        Ok(selected)
    }

    /// Show all known words in DB with their readings and let the user multiselect which ones to remove/forget.
    pub fn manage_known_words(words: &[(String, String)]) -> Result<Vec<String>> {
        if words.is_empty() {
            println!("\n 📭 No known words in database yet.\n");
            return Ok(Vec::new());
        }

        let prompt_msg = format!(
            "Select known words to REMOVE / FORGET ({} words in DB — Space to toggle, Enter to confirm, type to filter):",
            words.len()
        );

        let options: Vec<String> = words
            .iter()
            .map(|(word, reading)| format_word_with_reading(word, reading))
            .collect();
        let selected = MultiSelect::new(&prompt_msg, options.clone())
            .with_page_size(18)
            .prompt()?;

        Ok(selected
            .into_iter()
            .filter_map(|selected| options.iter().position(|option| option == &selected))
            .map(|index| words[index].0.clone())
            .collect())
    }

    /// Show all ignored words and let the user multiselect which ones to un-ignore.
    /// Returns the words that were selected for removal.
    pub fn manage_ignored_words(words: &[(String, String)]) -> Result<Vec<String>> {
        if words.is_empty() {
            println!("\n 📭 No ignored words yet.\n");
            return Ok(Vec::new());
        }

        let options: Vec<String> = words
            .iter()
            .map(|(word, reading)| format_word_with_reading(word, reading))
            .collect();
        let selected = MultiSelect::new(
            "Select words to UN-IGNORE (Space to toggle, Enter to confirm):",
            options.clone(),
        )
        .prompt()?;

        Ok(selected
            .into_iter()
            .filter_map(|selected| options.iter().position(|option| option == &selected))
            .map(|index| words[index].0.clone())
            .collect())
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

    pub fn show_config(cfg: &crate::config::AppConfig) {
        let cyan = Style::new().cyan().bold();
        let yellow = Style::new().yellow().bold();
        let green = Style::new().green().bold();
        let dim = Style::new().dim();

        println!("\n📋  K O T O N O H A   C O N F I G U R A T I O N\n");
        println!("  • Card Limit:            {}", cyan.apply_to(cfg.default_card_limit));
        println!("  • Media Directory:       {}", dim.apply_to(cfg.media_dir.display()));
        println!("  • Database Path:         {}", dim.apply_to(cfg.db_path.display()));
        println!("  • Enable AI:             {}", if cfg.enable_ai { green.apply_to("true") } else { yellow.apply_to("false") });
        println!("  • Gemini Model:          {}", cyan.apply_to(&cfg.gemini_model));
        let key_status = match cfg.gemini_api_key.as_deref() {
            Some(k) if !k.trim().is_empty() && k != "YOUR_GEMINI_API_KEY_HERE" => {
                let mask_len = k.len().saturating_sub(6);
                format!("{}{} (Set)", &k[..3.min(k.len())], "*".repeat(mask_len))
            }
            _ => "Not set (Set GEMINI_API_KEY env var or in config)".to_string(),
        };
        println!("  • Gemini API Key:        {}", yellow.apply_to(key_status));
        println!("  • Anki Sync Enabled:     {}", if cfg.enable_anki_sync { green.apply_to("true") } else { yellow.apply_to("false") });
        println!("  • AnkiConnect URL:       {}", dim.apply_to(&cfg.anki_connect_url));
        println!("  • Anki Deck Name:        {}", cyan.apply_to(&cfg.anki_deck_name));
        println!("  • Anki Model Name:       {}", dim.apply_to(&cfg.anki_model_name));
        println!("  • Max Definition Senses: {}", cyan.apply_to(cfg.max_definition_senses));
        println!("  • Max Glosses / Sense:   {}", cyan.apply_to(cfg.max_glosses_per_sense));
        println!();
    }

    pub fn configure_interactive(cfg: &mut crate::config::AppConfig) -> Result<()> {
        let cyan = Style::new().cyan().bold();
        let green = Style::new().green().bold();

        loop {
            let key_display = match cfg.gemini_api_key.as_deref() {
                Some(k) if !k.trim().is_empty() && k != "YOUR_GEMINI_API_KEY_HERE" => {
                    format!("{}...", &k[..3.min(k.len())])
                }
                _ => "Not Set".to_string(),
            };

            let options = vec![
                format!("🎴  Default Card Limit         [Current: {}]", cfg.default_card_limit),
                format!("🔑  Gemini API Key             [Current: {}]", key_display),
                format!("🤖  Gemini Model               [Current: {}]", cfg.gemini_model),
                format!("⚡  Enable AI Disambiguation   [Current: {}]", cfg.enable_ai),
                format!("📦  Anki Deck Name             [Current: {}]", cfg.anki_deck_name),
                format!("🔌  AnkiConnect URL            [Current: {}]", cfg.anki_connect_url),
                format!("📖  Max Definition Senses      [Current: {}]", cfg.max_definition_senses),
                format!("📝  Max Glosses per Sense      [Current: {}]", cfg.max_glosses_per_sense),
                "🔄  Reset to Default Values".to_string(),
                "💾  Save & Exit".to_string(),
            ];

            let choice = Select::new("Select configuration option to edit:", options)
                .with_page_size(12)
                .prompt()?;

            if choice.contains("Default Card Limit") {
                let input = Text::new("Enter default card limit per session:")
                    .with_default(&cfg.default_card_limit.to_string())
                    .prompt()?;
                if let Ok(num) = input.trim().parse::<usize>() {
                    cfg.default_card_limit = num;
                    println!(" ✔ Card limit set to {}", cyan.apply_to(num));
                }
            } else if choice.contains("Gemini API Key") {
                let current_key = cfg.gemini_api_key.clone().unwrap_or_default();
                let input = Text::new("Enter Gemini API key:")
                    .with_default(&current_key)
                    .prompt()?;
                let trimmed = input.trim();
                if !trimmed.is_empty() {
                    cfg.gemini_api_key = Some(trimmed.to_string());
                    println!(" ✔ Gemini API key updated.");
                }
            } else if choice.contains("Gemini Model") {
                let models = vec![
                    "gemini-3.5-flash-lite",
                    "gemini-2.5-flash",
                    "gemini-2.5-pro",
                    "✍ Enter custom model",
                ];
                let selected_model = Select::new("Select Gemini model:", models).prompt()?;
                if selected_model.contains("custom model") {
                    let custom = Text::new("Enter custom model name:").prompt()?;
                    if !custom.trim().is_empty() {
                        cfg.gemini_model = custom.trim().to_string();
                    }
                } else {
                    cfg.gemini_model = selected_model.to_string();
                }
                println!(" ✔ Model set to {}", cyan.apply_to(&cfg.gemini_model));
            } else if choice.contains("Enable AI Disambiguation") {
                cfg.enable_ai = !cfg.enable_ai;
                println!(" ✔ AI Disambiguation {}", if cfg.enable_ai { green.apply_to("Enabled") } else { cyan.apply_to("Disabled") });
            } else if choice.contains("Anki Deck Name") {
                let input = Text::new("Enter Anki deck name:")
                    .with_default(&cfg.anki_deck_name)
                    .prompt()?;
                if !input.trim().is_empty() {
                    cfg.anki_deck_name = input.trim().to_string();
                    println!(" ✔ Anki deck set to {}", cyan.apply_to(&cfg.anki_deck_name));
                }
            } else if choice.contains("AnkiConnect URL") {
                let input = Text::new("Enter AnkiConnect URL:")
                    .with_default(&cfg.anki_connect_url)
                    .prompt()?;
                if !input.trim().is_empty() {
                    cfg.anki_connect_url = input.trim().to_string();
                }
            } else if choice.contains("Max Definition Senses") {
                let input = Text::new("Enter max definition senses:")
                    .with_default(&cfg.max_definition_senses.to_string())
                    .prompt()?;
                if let Ok(num) = input.trim().parse::<usize>() {
                    cfg.max_definition_senses = num;
                }
            } else if choice.contains("Max Glosses per Sense") {
                let input = Text::new("Enter max glosses per sense:")
                    .with_default(&cfg.max_glosses_per_sense.to_string())
                    .prompt()?;
                if let Ok(num) = input.trim().parse::<usize>() {
                    cfg.max_glosses_per_sense = num;
                }
            } else if choice.contains("Reset to Default Values") {
                *cfg = crate::config::AppConfig::default();
                println!(" 🔄 Config reset to default values.");
            } else if choice.contains("Save & Exit") {
                cfg.save()?;
                println!(" ✨ Configuration saved to ~/.config/kotonoha/config.toml!");
                break;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::natural_cmp;

    #[test]
    fn sorts_episode_numbers_naturally() {
        let mut episodes = ["episode-10.mkv", "episode-08.mkv", "episode-01.mkv", "episode-2.mkv"];
        episodes.sort_by(|left, right| natural_cmp(left, right));
        assert_eq!(episodes, ["episode-01.mkv", "episode-2.mkv", "episode-08.mkv", "episode-10.mkv"]);
    }
}
