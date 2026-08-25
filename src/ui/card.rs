use console::Style;
use crossterm::terminal;
use std::sync::LazyLock;
use unicode_segmentation::UnicodeSegmentation;

static TOKENIZER: LazyLock<Option<crate::nlp::JapaneseTokenizer>> =
    LazyLock::new(|| crate::nlp::JapaneseTokenizer::new().ok());

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
    pub episode_freq: usize,
    pub density_tier: usize,
    pub definition: &'a str,
    pub known_context: &'a [String],
    pub unknown_context: &'a [String],
    pub ignored_context: &'a [String],
    pub ai_warning: Option<&'a str>,
    pub is_ai_selected: bool,
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
    terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80)
        .clamp(60, 110)
}

pub fn box_row(content: &str, inner_w: usize) -> String {
    let vis = visual_width(content);
    let pad = inner_w.saturating_sub(vis);
    format!("│ {}{} │", content, " ".repeat(pad))
}

pub fn box_empty(inner_w: usize) -> String {
    format!("│ {} │", " ".repeat(inner_w))
}

pub fn highlight_sentence_tokens(
    sentence: &str,
    target_word: &str,
    known_context: &[String],
    unknown_context: &[String],
    ignored_context: &[String],
) -> String {
    let green = Style::new().green().bold();
    let cyan = Style::new().cyan();
    let red = Style::new().red().bold();
    let dim = Style::new().dim();

    if let Some(ref tokenizer) = *TOKENIZER {
        if let Ok(tokens) = tokenizer.tokenize(sentence) {
            let mut out = String::new();
            for t in tokens {
                let surface = &t.surface;
                let dict = &t.dictionary_form;

                if surface == target_word || dict == target_word {
                    out.push_str(&green.apply_to(surface).to_string());
                } else if unknown_context.contains(dict) || unknown_context.contains(surface) {
                    out.push_str(&red.apply_to(surface).to_string());
                } else if t.is_proper_noun
                    || ignored_context
                        .iter()
                        .any(|ig| ig.starts_with(dict) || ig.starts_with(surface))
                {
                    out.push_str(&dim.apply_to(surface).to_string());
                } else if known_context.contains(dict)
                    || known_context.contains(surface)
                    || t.is_content_word
                {
                    out.push_str(&cyan.apply_to(surface).to_string());
                } else {
                    out.push_str(surface);
                }
            }
            return out;
        }
    }

    let mut result = sentence.to_string();
    if !target_word.is_empty() {
        result = result.replace(target_word, &green.apply_to(target_word).to_string());
    }
    for word in known_context {
        if word != target_word && !word.is_empty() {
            result = result.replace(word, &cyan.apply_to(word).to_string());
        }
    }
    for word in unknown_context {
        if word != target_word && !word.is_empty() {
            result = result.replace(word, &red.apply_to(word).to_string());
        }
    }
    for item in ignored_context {
        let raw_word = item.split_whitespace().next().unwrap_or(item.as_str());
        if !raw_word.is_empty() && raw_word != target_word {
            result = result.replace(raw_word, &dim.apply_to(raw_word).to_string());
        }
    }

    result
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
        episode_freq,
        density_tier,
        definition,
        known_context,
        unknown_context,
        ignored_context,
        ai_warning,
        is_ai_selected,
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

    let highlighted = highlight_sentence_tokens(
        sentence,
        target_word,
        known_context,
        unknown_context,
        ignored_context,
    );

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
    let pitch_num =
        pitch.parse::<usize>().unwrap_or_else(
            |_| {
                if pitch == "HLL" || pitch == "1" {
                    1
                } else {
                    0
                }
            },
        );
    let (reading_str, pitch_tag, _morae_cnt) = crate::dict::format_pitch_accent(reading, pitch_num);

    println!(
        "{}",
        lrow(
            "Target Word:",
            &format!(
                "{} ({} [Pitch: {}])",
                green.apply_to(target_word),
                yellow.apply_to(&reading_str),
                cyan.apply_to(&pitch_tag)
            )
        )
    );
    let tier_label = match density_tier {
        1 => "Tier 1 (1 Known + 1 Target)".to_string(),
        2 => "Tier 2 (2 Known + 1 Target)".to_string(),
        3 => "Tier 3 (3 Known + 1 Target)".to_string(),
        4 => "Tier 4 (Standalone Word)".to_string(),
        n => format!("Tier {} ({} Context Words)", n, n),
    };
    println!(
        "{}",
        lrow(
            "Mining Rank:",
            &format!("{}x in Ep | {}", episode_freq, tier_label)
        )
    );

    if let Some(warn) = ai_warning {
        let warn_styled = format!("⚠️  AI Parsing Warning: {}", warn);
        println!(
            "{}",
            lrow(
                "AI Notice:",
                &red.apply_to(&format_val(&warn_styled)).to_string()
            )
        );
    }

    if let Some(trans) = translations {
        if let Some(ref eng_nat) = trans.0 {
            println!(
                "{}",
                lrow(
                    "English (Nat):",
                    &cyan.apply_to(&format_val(eng_nat)).to_string()
                )
            );
        }
        if let Some(ref eng_lit) = trans.1 {
            println!(
                "{}",
                lrow(
                    "English (Lit):",
                    &Style::new()
                        .dim()
                        .apply_to(&format_val(eng_lit))
                        .to_string()
                )
            );
        }
    }

    let def_lines: Vec<&str> = definition.lines().collect();
    if def_lines.is_empty() {
        println!("{}", lrow("Definitions:", "No definition"));
    } else {
        for (idx, line) in def_lines.iter().enumerate() {
            let clean_line = line.trim();
            let clean_line = clean_line.strip_prefix('│').unwrap_or(clean_line).trim();
            if idx == 0 {
                let display_line = if is_ai_selected {
                    format!("✨ {}", clean_line)
                } else {
                    clean_line.to_string()
                };
                let truncated_line = format_val(&display_line);
                println!("{}", lrow("Definitions:", &truncated_line));
            } else {
                let truncated_line = format_val(clean_line);
                println!("{}", lrow("", &truncated_line));
            }
        }
    }

    println!(
        "{}",
        lrow(
            "Unknown Words:",
            &red.apply_to(&format_val(&unknown_str)).to_string()
        )
    );
    println!(
        "{}",
        lrow(
            "Known Words:",
            &cyan.apply_to(&format_val(&known_str)).to_string()
        )
    );
    if !ignored_context.is_empty() {
        let ignored_str = ignored_context.join(", ");
        println!(
            "{}",
            lrow(
                "Ignored/Names:",
                &Style::new()
                    .dim()
                    .apply_to(&format_val(&ignored_str))
                    .to_string()
            )
        );
    }
    println!("{}", box_empty(iw));
    println!("{}\n", bottom);
}
