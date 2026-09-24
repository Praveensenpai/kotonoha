use crate::srt::SubtitleSentence;

pub const DEFAULT_MAX_DIALOGUE_GAP_MS: u64 = 10_000;
pub const DEFAULT_MAX_CONTEXT_LINES: usize = 2;

fn format_sentence(s: &SubtitleSentence) -> String {
    match &s.actor {
        Some(actor) if !actor.is_empty() => format!("{}: {}", actor, s.text),
        _ => s.text.clone(),
    }
}

/// Extracts up to `max_lines_before` and `max_lines_after` surrounding dialogue lines
/// for the sentence at `target_idx`, respecting a maximum temporal gap (`max_gap_ms`).
///
/// Any dialogue gap exceeding `max_gap_ms` halts expansion in that direction (treating
/// pauses as scene transitions, music themes, or topic changes).
pub fn extract_dialogue_context(
    sentences: &[SubtitleSentence],
    target_idx: usize,
    max_lines_before: usize,
    max_lines_after: usize,
    max_gap_ms: u64,
) -> (Vec<String>, Vec<String>) {
    if target_idx >= sentences.len() {
        return (Vec::new(), Vec::new());
    }

    let target = &sentences[target_idx];

    let mut before = Vec::new();
    let mut prev_start = target.start_ms;
    for s in sentences[..target_idx].iter().rev() {
        if before.len() >= max_lines_before {
            break;
        }
        if prev_start < s.end_ms || (prev_start - s.end_ms) > max_gap_ms {
            break;
        }
        prev_start = s.start_ms;
        before.push(format_sentence(s));
    }
    before.reverse();

    let mut after = Vec::new();
    let mut next_end = target.end_ms;
    for s in sentences[target_idx + 1..].iter() {
        if after.len() >= max_lines_after {
            break;
        }
        if s.start_ms < next_end || (s.start_ms - next_end) > max_gap_ms {
            break;
        }
        next_end = s.end_ms;
        after.push(format_sentence(s));
    }

    (before, after)
}

/// Helper that locates `target` within `sentences` and extracts dialogue context.
pub fn extract_dialogue_context_for_sentence(
    sentences: &[SubtitleSentence],
    target: &SubtitleSentence,
    max_lines_before: usize,
    max_lines_after: usize,
    max_gap_ms: u64,
) -> (Vec<String>, Vec<String>) {
    let pos = sentences.iter().position(|s| {
        s.index == target.index && s.start_ms == target.start_ms && s.text == target.text
    });

    match pos {
        Some(idx) => extract_dialogue_context(
            sentences,
            idx,
            max_lines_before,
            max_lines_after,
            max_gap_ms,
        ),
        None => (Vec::new(), Vec::new()),
    }
}
