use console::measure_text_width;
use std::cmp::Ordering;

use super::card::box_width;

/// Compare strings naturally, treating consecutive ASCII digits as a number.
pub fn natural_cmp(left: &str, right: &str) -> Ordering {
    let mut left_chars = left.chars().peekable();
    let mut right_chars = right.chars().peekable();

    loop {
        match (left_chars.peek(), right_chars.peek()) {
            (Some(left_char), Some(right_char))
                if left_char.is_ascii_digit() && right_char.is_ascii_digit() =>
            {
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

pub fn format_word_with_reading(word: &str, reading: &str) -> String {
    if reading.is_empty() || reading == word {
        word.to_string()
    } else {
        format!("{} ({})", word, reading)
    }
}

pub fn format_timestamp(start_ms: u64) -> String {
    let total_minutes = start_ms / 60_000;
    let seconds = (start_ms % 60_000) / 1_000;
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

/// Prints a full-width box with the app title centered inside.
pub fn print_banner() {
    let bw = box_width();
    let iw = bw - 4;

    let title = "🌸  K O T O N O H A  ──  Japanese $i+1$ Sentence Miner";
    let title_vis = measure_text_width(title);

    let total_pad = iw.saturating_sub(title_vis);
    let left_pad = total_pad / 2;
    let right_pad = total_pad - left_pad;

    let top = format!("┌{}┐", "─".repeat(bw - 2));
    let middle = format!(
        "│ {}{}{} │",
        " ".repeat(left_pad),
        title,
        " ".repeat(right_pad)
    );
    let bottom = format!("└{}┘", "─".repeat(bw - 2));

    println!("\n{}", top);
    println!("{}", middle);
    println!("{}\n", bottom);
}
