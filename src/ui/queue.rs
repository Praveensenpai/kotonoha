use anyhow::Result;
use console::style;
use inquire::{MultiSelect, Select};
use std::path::Path;

use crate::queue::QueueEntry;
use crate::ui::helpers::format_timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueNoSubChoice {
    Keep,
    Skip,
}

pub fn select_queue_entries(entries: &[QueueEntry]) -> Result<Vec<usize>> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }

    let labels: Vec<String> = entries
        .iter()
        .map(|e| {
            let file_name = e
                .video_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            format!("{:<8}  {}", format_timestamp(e.timestamp_ms), file_name)
        })
        .collect();

    let defaults: Vec<usize> = (0..entries.len()).collect();
    let selected_labels = MultiSelect::new("Select queued timestamps to process:", labels.clone())
        .with_default(&defaults)
        .prompt()?;

    let selected_indices = selected_labels
        .iter()
        .filter_map(|sel| labels.iter().position(|l| l == sel))
        .collect();

    Ok(selected_indices)
}

pub fn ask_no_subtitle_action(video_path: &Path, timestamp_ms: u64) -> Result<QueueNoSubChoice> {
    let name = video_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("video");
    let ts_str = format_timestamp(timestamp_ms);

    println!();
    println!(
        " {}  No subtitle found for video: {}",
        style("⚠").yellow().bold(),
        style(name).cyan().bold()
    );
    println!("    Timestamp: {}", style(ts_str).yellow());
    println!();
    println!("    Need to generate or download a subtitle? Try SubSink:");
    println!("      {}", style("subsink").green().bold());
    println!("    Run subsink, place the subtitle next to the video, and come back later.");
    println!();

    let options = vec![
        "⏳  Keep in queue (come back after getting subtitle with subsink)",
        "⏭️  Skip this timestamp (remove from queue)",
    ];

    let choice = Select::new("What would you like to do with this timestamp?", options).prompt()?;

    if choice.starts_with("⏳") {
        Ok(QueueNoSubChoice::Keep)
    } else {
        Ok(QueueNoSubChoice::Skip)
    }
}

pub fn select_target_word(
    sentence_text: &str,
    words: &[(String, String, bool)], // (target_word, reading, is_unknown)
) -> Result<Option<String>> {
    println!();
    println!(" 📝 Sentence: {}", style(sentence_text).cyan().bold());

    let mut options = Vec::new();
    for (w, r, is_unknown) in words {
        let tag = if *is_unknown {
            style("❓ [Unknown]").yellow()
        } else {
            style("✔ [Known]").dim()
        };
        options.push(format!("{} {:<14} ({})", tag, w, r));
    }
    options.push("⏭️  Skip this sentence".to_string());

    let choice = Select::new(
        "Select the word you don't know to create a card for:",
        options,
    )
    .prompt()?;

    if choice.contains("Skip this sentence") {
        return Ok(None);
    }

    for (w, r, _) in words {
        if choice.contains(w) && (r.is_empty() || choice.contains(r)) {
            return Ok(Some(w.clone()));
        }
    }

    Ok(None)
}
