use console::{Style};
use crossterm::terminal;
use unicode_segmentation::UnicodeSegmentation;

pub type SentenceTranslations<'a> = (
    &'a Option<String>,
    &'a Option<String>,
    &'a Option<String>,
    &'a Option<String>,
);

pub struct CardRenderParams<'a> {
    pub rank: usize,
    pub sentence: &'a str,
    pub target_word: &'a str,
    pub reading: &'a str,
    pub pitch: &'a str,
    pub jpdb_rank: Option<u32>,
    pub definition: &'a str,
    pub known_context: &'a [String],
    pub unknown_context: &'a [String],
    pub ai_warning: Option<&'a str>,
    pub translations: Option<SentenceTranslations<'a>>,
}

pub fn visual_width(s: &str) -> usize {
    use unicode_width::UnicodeWidthStr;
    let plain = console::strip_ansi_codes(s);
    plain
        .as_ref()
        .graphemes(true)
        .map(|g| UnicodeWidthStr::width(g).clamp(1, 2))
        .sum()
}

pub fn box_width() -> usize {
    terminal::size().map(|(w, _)| w as usize).unwrap_or(80).clamp(60, 110)
}

pub fn box_row(content: &str, inner_w: usize) -> String {
    let vis = visual_width(content);
    let pad = inner_w.saturating_sub(vis);
    format!("│ {}{} │", content, " ".repeat(pad))
}

pub fn box_empty(inner_w: usize) -> String {
    format!("│ {} │", " ".repeat(inner_w))
}

pub fn render_progress(
    current: usize,
    total: usize,
    mined: usize,
    known: usize,
    skipped: usize,
    ignored: usize,
) {
    let magenta = Style::new().magenta().bold();
    let yellow = Style::new().yellow().bold();
    let cyan = Style::new().cyan().bold();
    let dim = Style::new().dim();

    println!(
        "🌸 Progress [{}/{}]  ───  ✨ {} Mined  •  🧠 {} Known  •  ⏭️ {} Skipped  •  🙈 {}",
        magenta.apply_to(current),
        total,
        yellow.apply_to(mined),
        cyan.apply_to(known),
        skipped,
        dim.apply_to(format!("{} Ignored", ignored)),
    );
}

pub fn render_card(p: CardRenderParams<'_>) {
    let CardRenderParams {
        rank,
        sentence,
        target_word,
        reading,
        pitch,
        jpdb_rank,
        definition,
        known_context,
        unknown_context,
        ai_warning,
        translations,
    } = p;
    let cyan = Style::new().cyan().bold();
    let yellow = Style::new().yellow().bold();
    let green = Style::new().green().bold();
    let red = Style::new().red().bold();

    let bw = box_width();
    let iw = bw - 4;
    const LABEL: usize = 15;

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

    let bottom = format!("└{}┘", "─".repeat(bw - 2));

    let lrow = |label: &str, value: &str| -> String {
        let label_pad = LABEL.saturating_sub(visual_width(label));
        let content = format!("{}{}{}", label, " ".repeat(label_pad), value);
        box_row(&content, iw)
    };

    let max_val_w = iw.saturating_sub(LABEL);

    let format_val = |val: &str| -> String {
        let vis = visual_width(val);
        if vis > max_val_w && max_val_w > 3 {
            let mut truncated = String::new();
            let mut current_w = 0;
            use unicode_width::UnicodeWidthStr;
            for g in val.graphemes(true) {
                let gw = UnicodeWidthStr::width(g).clamp(1, 2);
                if current_w + gw + 3 > max_val_w {
                    truncated.push_str("...");
                    break;
                }
                truncated.push_str(g);
                current_w += gw;
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

    println!(
        "{}",
        lrow(
            "Target Word:",
            &format!(
                "{} ({} [Pitch: {}])",
                green.apply_to(target_word),
                yellow.apply_to(&pitch_overbar),
                cyan.apply_to(&pitch_tag)
            )
        )
    );
    if let Some(r) = jpdb_rank {
        println!("{}", lrow("JPDB Rank:", &format!("#{}", r)));
    }

    if let Some(warn) = ai_warning {
        let warn_styled = format!("⚠️  AI Parsing Warning: {}", warn);
        println!("{}", lrow("AI Notice:", &red.apply_to(&format_val(&warn_styled)).to_string()));
    }

    if let Some(trans) = translations {
        if let Some(ref eng_nat) = trans.0 {
            println!("{}", lrow("English (Nat):", &cyan.apply_to(&format_val(eng_nat)).to_string()));
        }
        if let Some(ref eng_lit) = trans.1 {
            println!("{}", lrow("English (Lit):", &Style::new().dim().apply_to(&format_val(eng_lit)).to_string()));
        }
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
