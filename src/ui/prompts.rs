use anyhow::Result;
use inquire::Select;

use super::SessionMode;

pub fn select_session_mode(i1_count: usize, known_lines_count: usize) -> Result<SessionMode> {
    let options = vec![
        format!("🎯  Mine i+1 Candidate Cards ({} candidates)", i1_count),
        "🧭  Explore Subtitles by Difficulty (i+0 → i+n Browser)".to_string(),
        format!(
            "🔍  Review & Verify Known Sentences ({} lines)",
            known_lines_count
        ),
        "🚪  Exit".to_string(),
    ];

    let choice = Select::new("Select session mode:", options).prompt()?;
    if choice.contains("Mine i+1 Candidate") {
        Ok(SessionMode::MineI1Candidates)
    } else if choice.contains("Explore Subtitles") {
        Ok(SessionMode::Explore)
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
