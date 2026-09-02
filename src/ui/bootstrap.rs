use anyhow::Result;
use inquire::MultiSelect;

use super::helpers::format_word_with_reading;

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
