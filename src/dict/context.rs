#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextHint {
    AsStated,
}

const AS_STATED_PATTERNS: &[&str] = &[
    "言うとおり",
    "言う通り",
    "言ったとおり",
    "言った通り",
    "いうとおり",
    "いう通り",
    "いったとおり",
    "いった通り",
    "思うとおり",
    "思う通り",
    "思ったとおり",
    "思った通り",
    "おもうとおり",
    "おもう通り",
    "おもったとおり",
    "おもった通り",
    "見るとおり",
    "見る通り",
    "見たとおり",
    "見た通り",
    "そのとおり",
    "その通り",
    "予定どおり",
    "予定通り",
    "説明どおり",
    "説明通り",
];

pub fn context_hint(sentence: &str, target_word: &str) -> Option<ContextHint> {
    if !matches!(target_word, "とおり" | "通り" | "どおり") {
        return None;
    }
    AS_STATED_PATTERNS
        .iter()
        .any(|pattern| sentence.contains(pattern))
        .then_some(ContextHint::AsStated)
}

pub fn sense_line_score(line: &str, hint: ContextHint) -> i32 {
    let line = line.to_ascii_lowercase();
    match hint {
        ContextHint::AsStated => {
            let positive = [
                "according to",
                "in accordance",
                "just as",
                "exactly as",
                "as ",
                "following",
                "manner",
            ];
            let negative = [
                "street",
                "road",
                "avenue",
                "thoroughfare",
                "traffic",
                "flow of",
            ];
            positive
                .iter()
                .map(|term| if line.contains(term) { 100 } else { 0 })
                .sum::<i32>()
                - negative
                    .iter()
                    .map(|term| if line.contains(term) { 25 } else { 0 })
                    .sum::<i32>()
        }
    }
}

pub fn format_contextual_definition(
    definition: &str,
    hint: Option<ContextHint>,
    max_senses: usize,
    max_glosses: usize,
) -> String {
    let Some(hint) = hint else {
        return truncate_definition(definition, max_senses, max_glosses);
    };

    let mut lines: Vec<&str> = definition
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.is_empty() {
        return definition.to_string();
    }

    if lines.iter().any(|line| sense_line_score(line, hint) > 0) {
        lines.sort_by_key(|line| -sense_line_score(line, hint));
    }
    truncate_definition(&lines.join("\n"), max_senses, max_glosses)
}

pub fn has_contextual_sense(definition: &str, hint: ContextHint) -> bool {
    definition
        .lines()
        .any(|line| sense_line_score(line, hint) >= 100)
}

pub fn parse_senses(def: &str) -> Vec<String> {
    def.lines()
        .map(|line| {
            let t = line.trim();
            t.strip_prefix('│').unwrap_or(t).trim().to_string()
        })
        .filter(|line| !line.is_empty())
        .collect()
}

pub fn is_placeholder_definition(definition: &str) -> bool {
    definition.trim() == "1. [def] vocabulary word"
}

pub fn truncate_definition(def: &str, max_senses: usize, max_glosses: usize) -> String {
    if def.is_empty() || is_placeholder_definition(def) || def == "No dictionary definition found" {
        return def.to_string();
    }

    let mut new_senses = Vec::new();
    let mut num = 1;

    for line in def.lines() {
        if num > max_senses {
            break;
        }

        let clean = line.trim();
        let clean = clean.strip_prefix('│').unwrap_or(clean).trim();

        if let Some(dot_idx) = clean.find(". [") {
            let rest = &clean[dot_idx + 2..];
            if let Some(close_bracket) = rest.find(']') {
                let pos_part = &rest[..close_bracket + 1];
                let glosses_part = rest[close_bracket + 1..].trim();
                let glosses: Vec<&str> = glosses_part.split(", ").collect();
                let clean_glosses: Vec<&str> = glosses
                    .into_iter()
                    .filter(|g| {
                        let t = g.trim();
                        !t.is_empty()
                            && !t.starts_with("see:")
                            && !t.starts_with("antonym:")
                            && t != "see:"
                            && t != "antonym:"
                            && !t.starts_with('（')
                            && !t.ends_with('）')
                    })
                    .collect();
                let truncated_glosses: Vec<&str> =
                    clean_glosses.into_iter().take(max_glosses).collect();
                if truncated_glosses.is_empty() {
                    continue;
                }
                new_senses.push(format!(
                    "{}. {} {}",
                    num,
                    pos_part,
                    truncated_glosses.join(", ")
                ));
                num += 1;
                continue;
            }
        }

        new_senses.push(clean.to_string());
    }

    if new_senses.is_empty() {
        def.to_string()
    } else {
        new_senses.join("\n│                 ")
    }
}

fn is_redirect_stub(def: &str) -> bool {
    let clean = def.trim();
    (clean.contains("see:,") || clean.contains("see: "))
        && (clean.starts_with("1. [1 pn] what, see:") || clean.len() < 40)
}

fn candidate_priority_score(
    c: &crate::dict::LookupResult,
    target_word: &str,
    target_reading: &str,
) -> i32 {
    let mut score = 0;

    if c.expression == target_word {
        score += 1000;
    }
    if !target_reading.is_empty() && c.reading == target_reading {
        score += 500;
    }

    if is_redirect_stub(&c.definition) {
        score -= 800;
    }

    match target_word {
        "よい" | "良い" | "いい" => {
            if c.definition.contains("adj-i")
                || c.definition.contains("good")
                || c.definition.contains("fine")
            {
                score += 400;
            }
            if c.definition.contains("evening")
                || c.definition.contains("what was said")
                || c.definition.contains("drunkenness")
            {
                score -= 600;
            }
        }
        "なん" => {
            if c.expression == "何" || c.definition.contains("what") {
                score += 600;
            }
            if c.expression == "難" || c.definition.contains("difficulty") {
                score -= 600;
            }
        }
        "そう" => {
            if c.definition.contains("in that way")
                || c.definition.contains("so,")
                || c.definition.contains("thus")
            {
                score += 300;
            }
            if c.definition.contains("aux adj-na") || c.definition.contains("appearing that") {
                score -= 300;
            }
        }
        _ => {}
    }

    score
}

pub fn sort_candidates_for_context(
    candidates: &mut [crate::dict::LookupResult],
    target_word: &str,
    target_reading: &str,
) {
    if candidates.is_empty() {
        return;
    }
    candidates.sort_by_key(|c| -candidate_priority_score(c, target_word, target_reading));
}
