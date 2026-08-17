pub mod mining;

use anyhow::Result;
use console::style;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::config::AppConfig;
use crate::db::Database;
use crate::miner::{CandidateSentence, MiningEngine};
use crate::nlp::JapaneseTokenizer;
use crate::srt::SubtitleSentence;
use crate::ui::{SessionMode, TerminalUi};

pub struct ComprehensionStats {
    pub total_lines: usize,
    pub known_lines: usize,
    pub i1_lines: usize,
    pub i2_plus_lines: usize,
    pub comp_ratio: f64,
    pub already_known_count: usize,
    pub mined_count: usize,
    pub unknown_count: usize,
}

pub fn bootstrap_vocabulary(
    sentences: &[SubtitleSentence],
    tokenizer: &JapaneseTokenizer,
    db: &mut Database,
    known_words: &HashSet<String>,
    ignored_words: &HashSet<String>,
) -> Result<()> {
    let mut word_counts: HashMap<String, (usize, String, bool)> = HashMap::new();
    for sub in sentences {
        if let Ok(tokens) = tokenizer.tokenize(&sub.text) {
            for t in tokens {
                if t.is_content_word
                    && !known_words.contains(&t.dictionary_form)
                    && !ignored_words.contains(&t.dictionary_form)
                {
                    let entry = word_counts.entry(t.dictionary_form.clone()).or_insert((
                        0,
                        t.reading.clone(),
                        t.is_proper_noun,
                    ));
                    entry.0 += 1;
                    if t.is_proper_noun {
                        entry.2 = true;
                    }
                }
            }
        }
    }

    let mut top_vocab: Vec<(String, usize, String, bool)> = word_counts
        .into_iter()
        .map(|(word, (count, reading, is_pn))| (word, count, reading, is_pn))
        .filter(|(_, count, _, _)| *count >= 2)
        .collect();
    top_vocab.sort_by_key(|b| std::cmp::Reverse(b.1));

    let general_candidates: Vec<(String, usize, String)> = top_vocab
        .iter()
        .filter(|(_, _, _, is_pn)| !*is_pn)
        .map(|(w, c, r, _)| (w.clone(), *c, r.clone()))
        .take(100)
        .collect();

    let name_candidates: Vec<(String, usize, String)> = top_vocab
        .iter()
        .filter(|(_, _, _, is_pn)| *is_pn)
        .map(|(w, c, r, _)| (w.clone(), *c, r.clone()))
        .take(50)
        .collect();

    if !name_candidates.is_empty() {
        let newly_ignored = TerminalUi::bootstrap_ignored_names(&name_candidates)?;
        if !newly_ignored.is_empty() {
            for w in &newly_ignored {
                let _ = db.add_ignored_word(w);
            }
            println!(
                " 🚫 Marked {} character names/proper nouns as ignored!",
                newly_ignored.len()
            );
        }
    }

    if !general_candidates.is_empty() {
        let newly_known = TerminalUi::bootstrap_known_words(&general_candidates)?;
        if !newly_known.is_empty() {
            let count = db.add_known_words(&newly_known)?;
            println!(" ✔ Marked {} words as known!", count);
        }
    }

    Ok(())
}

pub fn calculate_comprehension_stats(
    sentences: &[SubtitleSentence],
    tokenizer: &JapaneseTokenizer,
    db: &Database,
    ignored_words: &HashSet<String>,
) -> Result<ComprehensionStats> {
    let already_known_set = db.get_known_words_by_source("known")?;
    let mined_set = db.get_known_words_by_source("mined")?;

    let mut file_already_known = HashSet::new();
    let mut file_mined = HashSet::new();
    let mut file_unknown = HashSet::new();

    let mut known_lines_count = 0usize;
    let mut i1_lines_count = 0usize;
    let mut i2_plus_lines_count = 0usize;

    for sub in sentences {
        let mut line_unknown_words = 0usize;
        if let Ok(tokens) = tokenizer.tokenize(&sub.text) {
            for t in tokens {
                if t.is_content_word && !ignored_words.contains(&t.dictionary_form) {
                    if already_known_set.contains(&t.dictionary_form) {
                        file_already_known.insert(t.dictionary_form.clone());
                    } else if mined_set.contains(&t.dictionary_form) {
                        file_mined.insert(t.dictionary_form.clone());
                    } else {
                        file_unknown.insert(t.dictionary_form.clone());
                        line_unknown_words += 1;
                    }
                }
            }
        }
        if line_unknown_words == 0 {
            known_lines_count += 1;
        } else if line_unknown_words == 1 {
            i1_lines_count += 1;
        } else {
            i2_plus_lines_count += 1;
        }
    }

    let total_lines = sentences.len();
    let comp_ratio = if total_lines > 0 {
        (known_lines_count as f64 / total_lines as f64) * 100.0
    } else {
        0.0
    };

    Ok(ComprehensionStats {
        total_lines,
        known_lines: known_lines_count,
        i1_lines: i1_lines_count,
        i2_plus_lines: i2_plus_lines_count,
        comp_ratio,
        already_known_count: file_already_known.len(),
        mined_count: file_mined.len(),
        unknown_count: file_unknown.len(),
    })
}

pub fn collect_review_known_candidates(
    sentences: &[SubtitleSentence],
    tokenizer: &JapaneseTokenizer,
    known_words: &HashSet<String>,
    ignored_words: &HashSet<String>,
) -> Vec<CandidateSentence> {
    let mut known_candidates = Vec::new();
    let mut seen_words = HashSet::new();

    for sub in sentences {
        if sub.text.chars().count() < 4 {
            continue;
        }
        if let Ok(tokens) = tokenizer.tokenize(&sub.text) {
            let mut unknown_words = Vec::new();
            let mut known_context = Vec::new();
            let mut ignored_context = Vec::new();
            let mut target = None;
            let mut reading = None;

            for t in &tokens {
                let dict_form = &t.dictionary_form;
                if ignored_words.contains(dict_form) {
                    let entry = format!("{} (Ignored)", dict_form);
                    if !ignored_context.contains(&entry) {
                        ignored_context.push(entry);
                    }
                    continue;
                }
                if t.is_proper_noun {
                    let entry = format!("{} (Name)", dict_form);
                    if !ignored_context.contains(&entry) {
                        ignored_context.push(entry);
                    }
                    continue;
                }
                if !t.is_content_word {
                    if matches!(
                        dict_form.as_str(),
                        "ちゃん" | "さん" | "君" | "様" | "殿" | "氏" | "たん" | "先輩"
                    ) {
                        let entry = format!("{} (Suffix)", dict_form);
                        if !ignored_context.contains(&entry) {
                            ignored_context.push(entry);
                        }
                    }
                    continue;
                }
                if known_words.contains(dict_form) {
                    if !known_context.contains(dict_form) {
                        known_context.push(dict_form.clone());
                    }
                    if target.is_none() && !seen_words.contains(dict_form) {
                        target = Some(dict_form.clone());
                        reading = Some(t.reading.clone());
                    }
                } else {
                    unknown_words.push(dict_form.clone());
                }
            }

            if unknown_words.is_empty() {
                if let Some(target_word) = target {
                    seen_words.insert(target_word.clone());
                    let target_reading = reading.unwrap_or_else(|| target_word.clone());
                    known_candidates.push(CandidateSentence {
                        sentence: sub.clone(),
                        target_word,
                        target_reading,
                        known_context_words: known_context,
                        unknown_context_words: Vec::new(),
                        ignored_context_words: ignored_context,
                        episode_freq: 1,
                        density_tier: 1,
                    });
                }
            }
        }
    }
    known_candidates
}

pub async fn run_session(
    sentences: Vec<SubtitleSentence>,
    video_path: &Path,
    cfg: &AppConfig,
    mut db: Database,
    http_client: reqwest::Client,
) -> Result<()> {
    let tokenizer = JapaneseTokenizer::new()?;
    let known_words = db.get_known_words()?;
    let ignored_words = db.get_ignored_words()?;

    bootstrap_vocabulary(
        &sentences,
        &tokenizer,
        &mut db,
        &known_words,
        &ignored_words,
    )?;

    let known_words = db.get_known_words()?;
    let ignored_words = db.get_ignored_words()?;

    let stats = calculate_comprehension_stats(&sentences, &tokenizer, &db, &ignored_words)?;
    let engine = MiningEngine::new(tokenizer);
    let candidates = engine.find_candidates(&sentences, &known_words, &ignored_words);

    println!(
        " 📊 Subtitle Line Comprehension Stats:\n   • Lines Known: {} / {} ({} comprehension ratio)\n   • i+1 Candidate Lines: {}\n   • Hard Lines (2+ Unknowns): {}\n   • Vocab Stats: {} | {} | {}\n",
        style(stats.known_lines).green().bold(),
        style(stats.total_lines).cyan().bold(),
        style(format!("{:.1}%", stats.comp_ratio)).yellow().bold(),
        style(format!("{} i+1 lines ({} eligible candidates)", stats.i1_lines, candidates.len())).green().bold(),
        style(format!("{} hard lines", stats.i2_plus_lines)).red().bold(),
        style(format!("{} Known Words", stats.already_known_count)).blue().bold(),
        style(format!("{} Mined Cards", stats.mined_count)).magenta().bold(),
        style(format!("{} Unknown Words", stats.unknown_count)).red().bold(),
    );

    let session_mode = TerminalUi::select_session_mode(candidates.len(), stats.known_lines)?;

    let all_candidates = match session_mode {
        SessionMode::Exit => {
            println!(" 🚪 Exiting kotonoha.");
            return Ok(());
        }
        SessionMode::MineI1Candidates => candidates,
        SessionMode::ReviewKnownLines => {
            let mode_tokenizer = JapaneseTokenizer::new()?;
            collect_review_known_candidates(
                &sentences,
                &mode_tokenizer,
                &known_words,
                &ignored_words,
            )
        }
    };

    if all_candidates.is_empty() {
        println!(" ℹ No candidate sentences available for this mode.");
        return Ok(());
    }

    mining::run_mining_loop(all_candidates, video_path, cfg, &mut db, http_client).await
}
