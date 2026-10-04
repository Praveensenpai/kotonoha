use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiAnalysisResult {
    pub card_index: usize,
    pub recommended_candidate_index: Option<usize>,
    pub recommended_sense_index: Option<usize>,
    pub recommended_reading: Option<String>,
    pub parsing_warning: Option<String>,
    pub custom_definition_suggestion: Option<String>,
    pub explanation: Option<String>,
    pub english_natural: Option<String>,
    pub english_literal: Option<String>,
    pub kannada_natural: Option<String>,
    pub kannada_literal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchAiAnalysisResponse {
    pub results: Vec<AiAnalysisResult>,
}

pub struct CardBatchInput<'a> {
    pub card_index: usize,
    pub sentence: &'a str,
    pub target_word: &'a str,
    pub target_reading: &'a str,
    pub candidates: &'a [crate::dict::LookupResult],
    pub series_title: Option<&'a str>,
    pub before_context: &'a [String],
    pub after_context: &'a [String],
}

pub struct GeminiAiService;

fn build_prompt(cards_summary: &str) -> String {
    format!(
        r#"You are an expert Japanese linguist and lexicographer.
Analyze ALL Japanese cards in this batch. Match each target word in its sentence and conversational dialogue context (using the show/episode context and surrounding preceding/following lines when available) against its dictionary candidates:

Cards Batch:
{cards_summary}

For EACH card index:
1. Check if the target word has any tokenizer/segmentation misparse in the sentence. If so, provide a short `parsing_warning`. Otherwise null.
2. Always provide a concise, custom English `custom_definition_suggestion` for the target word's meaning in THIS sentence context. Leverage the surrounding conversation and show theme to resolve ambiguity, figurative usage, and domain nuances. It must be a short gloss, not a sentence translation. For example, for `私のせいですか？`, return exactly `fault; blame; cause of a bad result`.
3. Set `recommended_candidate_index` and `recommended_sense_index` to null when the custom gloss is the best display definition. Use candidate/sense indexes only when the dictionary entry itself is already an ideal contextual definition and no custom gloss is needed.
4. Provide `recommended_reading` as the contextual hiragana reading for the target word in this sentence (e.g. for `先に出た`, return `さき`). If identical or unknown, return null.

Return ONLY a valid JSON object matching this exact schema:
{{
  "results": [
    {{
      "card_index": number,
      "recommended_candidate_index": number or null,
      "recommended_sense_index": number or null,
      "recommended_reading": string or null,
      "parsing_warning": string or null,
      "custom_definition_suggestion": string or null,
      "explanation": string or null
    }}
  ]
}}"#
    )
}

pub struct UnifiedAiService;

impl UnifiedAiService {
    pub async fn analyze_batch(
        client: &reqwest::Client,
        cfg: &crate::config::AppConfig,
        cards: &[CardBatchInput<'_>],
    ) -> Result<Vec<AiAnalysisResult>> {
        if cards.is_empty() {
            return Ok(Vec::new());
        }

        if cfg.ai.enable_deepseek {
            match DeepSeekAiService::analyze_batch(
                client,
                &cfg.ai.deepseek_url,
                &cfg.ai.deepseek_model,
                &cfg.ai.deepseek_api_key,
                cards,
            )
            .await
            {
                Ok(results) => return Ok(results),
                Err(err) => {
                    eprintln!("  ℹ DeepSeek batch analysis unavailable ({err}); falling back to Gemini API...");
                }
            }
        }

        if let Some(ref api_key) = cfg.ai.gemini_api_key {
            if cfg.ai.has_valid_gemini_key() {
                return GeminiAiService::analyze_batch(client, api_key, &cfg.ai.gemini_model, cards)
                    .await;
            }
        }

        anyhow::bail!("No AI provider available (DeepSeek failed and no valid Gemini API key configured)")
    }
}

pub struct DeepSeekAiService;

impl DeepSeekAiService {
    pub async fn analyze_batch(
        client: &reqwest::Client,
        url: &str,
        model: &str,
        api_key: &str,
        cards: &[CardBatchInput<'_>],
    ) -> Result<Vec<AiAnalysisResult>> {
        if cards.is_empty() {
            return Ok(Vec::new());
        }

        let cards_summary = format_cards_summary(cards);
        let prompt = build_prompt(&cards_summary);

        let payload = serde_json::json!({
            "model": model,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": 0.1
        });

        let resp = client
            .post(url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&payload)
            .timeout(std::time::Duration::from_secs(20))
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("DeepSeek API error ({status}): {err_text}");
        }

        let body: serde_json::Value = resp.json().await?;
        let text = body["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing message content in DeepSeek response"))?;

        let cleaned = clean_json_text(text);
        let value: serde_json::Value = serde_json::from_str(&cleaned)
            .map_err(|e| anyhow::anyhow!("Failed to parse DeepSeek JSON ({e}): {cleaned}"))?;

        let mut parsed: BatchAiAnalysisResponse = serde_json::from_value(value)?;
        for res in &mut parsed.results {
            if let Some(cand_idx) = res.recommended_candidate_index {
                if cand_idx > 0 {
                    res.recommended_candidate_index = Some(cand_idx - 1);
                }
            }
            if let Some(sense_idx) = res.recommended_sense_index {
                if sense_idx > 0 {
                    res.recommended_sense_index = Some(sense_idx - 1);
                }
            }
        }
        Ok(parsed.results)
    }
}

fn clean_json_text(raw: &str) -> String {
    let mut s = raw.trim();
    if s.starts_with("```json") {
        s = &s[7..];
    } else if s.starts_with("```") {
        s = &s[3..];
    }
    if s.ends_with("```") {
        s = &s[..s.len() - 3];
    }
    s.trim().to_string()
}

fn format_cards_summary(cards: &[CardBatchInput<'_>]) -> String {
    cards
        .iter()
        .map(|c| {
            let candidates_str = c
                .candidates
                .iter()
                .enumerate()
                .map(|(idx, cand)| {
                    format!(
                        "  Candidate #{}: Expression: {}, Reading: {}\n  Definitions:\n  {}",
                        idx + 1,
                        cand.expression,
                        cand.reading,
                        cand.definition.replace('\n', "\n  ")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");

            let reading_str = if c.target_reading.is_empty() {
                String::new()
            } else {
                format!("\nTarget Reading in Sentence: \"{}\"", c.target_reading)
            };

            let show_str = match c.series_title {
                Some(title) if !title.is_empty() => {
                    format!("Show / Episode: \"{}\"\n", title)
                }
                _ => String::new(),
            };

            let mut dialogue_lines = Vec::new();
            for (b_idx, prev) in c.before_context.iter().enumerate() {
                let rel_num = c.before_context.len() - b_idx;
                dialogue_lines.push(format!("    [Prev -{}]: {}", rel_num, prev));
            }
            dialogue_lines.push(format!("  >>> [TARGET SENTENCE]: {} <<<", c.sentence));
            for (a_idx, next) in c.after_context.iter().enumerate() {
                dialogue_lines.push(format!("    [Next +{}]: {}", a_idx + 1, next));
            }
            let dialogue_str = dialogue_lines.join("\n");

            format!(
                "Card Index #{}:\n{}Dialogue Context:\n{}\nTarget Word: \"{}\"{}\nCandidates:\n{}",
                c.card_index, show_str, dialogue_str, c.target_word, reading_str, candidates_str
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n--------------------\n\n")
}

impl GeminiAiService {
    pub async fn analyze_batch(
        client: &reqwest::Client,
        api_key: &str,
        model: &str,
        cards: &[CardBatchInput<'_>],
    ) -> Result<Vec<AiAnalysisResult>> {
        if cards.is_empty() {
            return Ok(Vec::new());
        }

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            urlencoding::encode(model),
            urlencoding::encode(api_key)
        );

        let cards_summary = format_cards_summary(cards);
        let prompt = build_prompt(&cards_summary);

        let payload = serde_json::json!({
            "contents": [{
                "parts": [{
                    "text": prompt
                }]
            }],
            "generationConfig": {
                "response_mime_type": "application/json",
                "temperature": 0.1
            }
        });

        let max_attempts = 3;
        let mut last_error = String::new();

        for attempt in 1..=max_attempts {
            let resp = client.post(&url).json(&payload).send().await;
            match resp {
                Ok(response) if response.status().is_success() => {
                    let body: serde_json::Value = response.json().await?;
                    let text = body["candidates"][0]["content"]["parts"][0]["text"]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("Invalid response structure from Gemini"))?;

                    let cleaned = clean_json_text(text);
                    let value: serde_json::Value = serde_json::from_str(&cleaned)?;
                    let mut parsed: BatchAiAnalysisResponse = serde_json::from_value(value)?;
                    for res in &mut parsed.results {
                        if let Some(cand_idx) = res.recommended_candidate_index {
                            if cand_idx > 0 {
                                res.recommended_candidate_index = Some(cand_idx - 1);
                            }
                        }
                        if let Some(sense_idx) = res.recommended_sense_index {
                            if sense_idx > 0 {
                                res.recommended_sense_index = Some(sense_idx - 1);
                            }
                        }
                    }
                    return Ok(parsed.results);
                }
                Ok(response) => {
                    let status = response.status();
                    let err_text = response.text().await.unwrap_or_default();
                    last_error = if status.as_u16() == 429 {
                        "Gemini API rate limit exceeded (429 Too Many Requests)".to_string()
                    } else {
                        format!("Gemini API error ({}): {}", status, err_text)
                    };
                }
                Err(e) => {
                    last_error = e.to_string();
                }
            }

            if attempt < max_attempts {
                let delay = attempt as u64;
                tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
            }
        }

        anyhow::bail!(last_error)
    }
}

#[cfg(test)]
mod tests;
