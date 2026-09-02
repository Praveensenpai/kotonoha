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
                let truncated_glosses: Vec<&str> = glosses.into_iter().take(max_glosses).collect();
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
