use anyhow::Result;
use console::style;
use std::collections::HashSet;
use std::path::Path;

use crate::commands;
use crate::config::AppConfig;
use crate::db::Database;
use crate::miner::{context, BuildCandidateParams, CandidateSentence, MiningEngine};
use crate::nlp::JapaneseTokenizer;
use crate::queue::{self, QueueEntry};
use crate::srt::{self, SubtitleSentence};
use crate::ui::queue::QueueNoSubChoice;
use crate::ui::TerminalUi;

type WordEntry = (String, String, bool);
type ParsedTokensAndWords = (Vec<crate::nlp::TokenInfo>, Vec<WordEntry>);

#[cfg(test)]
mod tests;

pub async fn run_queue_command(
    cfg: &AppConfig,
    db: &mut Database,
    http_client: reqwest::Client,
) -> Result<()> {
    let queue_path = queue::queue_file_path();
    let entries = queue::read_queue(&queue_path)?;

    if entries.is_empty() {
        println!(" {}  Queue is empty.", style("ℹ").cyan().bold());
        print_install_hint();
        return Ok(());
    }

    println!(
        " 📥  Found {} queued moment{} from mpv.",
        style(entries.len()).cyan().bold(),
        if entries.len() == 1 { "" } else { "s" }
    );

    let selected_indices = TerminalUi::select_queue_entries(&entries)?;
    if selected_indices.is_empty() {
        println!(" ℹ  No timestamps selected.");
        return Ok(());
    }

    let tokenizer = JapaneseTokenizer::new()?;
    let known_words = db.get_known_words().await?;
    let ignored_words = db.get_ignored_words().await?;

    let (candidates, to_remove) = collect_queue_candidates(
        &entries,
        &selected_indices,
        &tokenizer,
        &known_words,
        &ignored_words,
    )?;

    if !candidates.is_empty() {
        crate::session::mining::run_mining_loop(candidates, cfg, db, http_client).await?;
    }

    cleanup_queue_entries(&queue_path, &to_remove)?;
    Ok(())
}

fn collect_queue_candidates(
    entries: &[QueueEntry],
    selected_indices: &[usize],
    tokenizer: &JapaneseTokenizer,
    known: &HashSet<String>,
    ignored: &HashSet<String>,
) -> Result<(Vec<CandidateSentence>, Vec<usize>)> {
    let mut candidates = Vec::new();
    let mut to_remove = Vec::new();

    for &idx in selected_indices {
        let entry = &entries[idx];
        match process_queue_entry(entry, tokenizer, known, ignored)? {
            QueueStepResult::Candidate(cand) => {
                candidates.push(*cand);
                to_remove.push(idx);
            }
            QueueStepResult::DiscardFromQueue => {
                to_remove.push(idx);
            }
            QueueStepResult::KeepInQueue => {}
        }
    }

    Ok((candidates, to_remove))
}

fn cleanup_queue_entries(queue_path: &Path, to_remove: &[usize]) -> Result<()> {
    if to_remove.is_empty() {
        return Ok(());
    }
    queue::remove_entries(queue_path, to_remove)?;
    println!(
        " ✔  Removed {} processed timestamp{} from queue.",
        style(to_remove.len()).green().bold(),
        if to_remove.len() == 1 { "" } else { "s" }
    );
    Ok(())
}

enum QueueStepResult {
    Candidate(Box<CandidateSentence>),
    KeepInQueue,
    DiscardFromQueue,
}

fn process_queue_entry(
    entry: &QueueEntry,
    tokenizer: &JapaneseTokenizer,
    known: &HashSet<String>,
    ignored: &HashSet<String>,
) -> Result<QueueStepResult> {
    let sub_path = match commands::find_paired_subtitle_for_video(&entry.video_path) {
        Some(p) => p,
        None => {
            return match TerminalUi::ask_no_subtitle_action(&entry.video_path, entry.timestamp_ms)?
            {
                QueueNoSubChoice::Keep => Ok(QueueStepResult::KeepInQueue),
                QueueNoSubChoice::Skip => Ok(QueueStepResult::DiscardFromQueue),
            };
        }
    };

    let subs = srt::parse_subtitle(&sub_path)?;
    let nearest = match find_nearest_sentence(&subs, entry.timestamp_ms) {
        Some(s) => s,
        None => {
            println!(
                " ⚠  No subtitle lines found near timestamp {}",
                entry.timestamp_ms
            );
            return Ok(QueueStepResult::KeepInQueue);
        }
    };

    let mut sentence = nearest.clone();
    sentence.video_path = Some(entry.video_path.clone());

    let (tokens, words) = collect_candidate_words(&sentence, tokenizer, known, ignored)?;
    if words.is_empty() {
        println!(
            " ℹ  No content words found in matched sentence: {}",
            sentence.text
        );
        return Ok(QueueStepResult::KeepInQueue);
    }

    let target_word = if words.len() == 1 {
        words[0].0.clone()
    } else {
        match TerminalUi::select_target_word(&sentence.text, &words)? {
            Some(w) => w,
            None => return Ok(QueueStepResult::KeepInQueue),
        }
    };

    let cand = build_single_candidate(&sentence, &subs, &target_word, &tokens, known, ignored);
    Ok(QueueStepResult::Candidate(Box::new(cand)))
}

fn find_nearest_sentence(subs: &[SubtitleSentence], ts_ms: u64) -> Option<&SubtitleSentence> {
    subs.iter().min_by_key(|s| {
        let mid = (s.start_ms + s.end_ms) / 2;
        mid.abs_diff(ts_ms)
    })
}

fn collect_candidate_words(
    sentence: &SubtitleSentence,
    tokenizer: &JapaneseTokenizer,
    known: &HashSet<String>,
    ignored: &HashSet<String>,
) -> Result<ParsedTokensAndWords> {
    let tokens = tokenizer.tokenize(&sentence.text)?;
    let mut words = Vec::new();
    let mut seen = HashSet::new();

    for t in &tokens {
        if t.is_content_word
            && !ignored.contains(&t.dictionary_form)
            && seen.insert(t.dictionary_form.clone())
        {
            let is_unknown = !known.contains(&t.dictionary_form);
            words.push((t.dictionary_form.clone(), t.reading.clone(), is_unknown));
        }
    }

    words.sort_by_key(|(_, _, is_unk)| !*is_unk);
    Ok((tokens, words))
}

fn build_single_candidate(
    sentence: &SubtitleSentence,
    all_subs: &[SubtitleSentence],
    target_word: &str,
    tokens: &[crate::nlp::TokenInfo],
    known: &HashSet<String>,
    ignored: &HashSet<String>,
) -> CandidateSentence {
    let (before_context, after_context) = context::extract_dialogue_context_for_sentence(
        all_subs,
        sentence,
        context::DEFAULT_MAX_CONTEXT_LINES,
        context::DEFAULT_MAX_CONTEXT_LINES,
        context::DEFAULT_MAX_DIALOGUE_GAP_MS,
    );
    let series_title = sentence
        .video_path
        .as_deref()
        .map(crate::media::extract_clean_show_context);

    MiningEngine::build_candidate(BuildCandidateParams {
        sentence,
        target_word,
        tokens,
        known_words: known,
        ignored_words: ignored,
        before_context,
        after_context,
        series_title,
    })
}

fn print_install_hint() {
    println!();
    println!(" To queue timestamps directly while watching in mpv:");
    println!("   1. Copy the companion script to your mpv scripts directory:");
    println!("      {}", style("mkdir -p ~/.config/mpv/scripts").dim());
    println!(
        "      {}",
        style("cp scripts/kotonoha-queue.lua ~/.config/mpv/scripts/")
            .green()
            .bold()
    );
    println!(
        "   2. Press {} while watching any anime video in mpv.",
        style("Shift+Q").yellow().bold()
    );
    println!(
        "   3. Run {} to mine cards from your queued moments.",
        style("kotonoha --queue").cyan().bold()
    );
    println!();
}
