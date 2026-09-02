use anyhow::Result;
use inquire::{Select, Text};

use super::SessionMode;

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
