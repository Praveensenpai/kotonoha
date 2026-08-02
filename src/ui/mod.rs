use anyhow::Result;
use console::Style;
use inquire::{Select, Text};
use std::path::PathBuf;
use walkdir::WalkDir;

pub struct TerminalUi;

impl TerminalUi {
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

        let rank_str = format!("RANK #{}", rank);
        let border = "─".repeat(50);

        let highlighted_sentence = sentence.replace(target_word, &green.apply_to(target_word).to_string());

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

        println!("\n┌─ {} {} ┐", yellow.apply_to(&rank_str), border);
        println!("│ ");
        println!("│  Sentence:       {}", highlighted_sentence);
        println!("│  Target Word:    {} ({} [Pitch: {}])", green.apply_to(target_word), reading, pitch);
        if let Some(r) = jpdb_rank {
            println!("│  JPDB Rank:      #{}", r);
        }
        println!("│  Definitions:    {}", definition);
        println!("│  Unknown Words:  {}", red.apply_to(&unknown_str));
        println!("│  Known Words:    {}", cyan.apply_to(&known_str));
        println!("│ ");
        println!("└{}┘\n", "─".repeat(54));
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
}
