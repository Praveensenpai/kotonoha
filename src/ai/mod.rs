use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiAnalysisResult {
    pub card_index: usize,
    pub recommended_candidate_index: Option<usize>,
    pub recommended_sense_index: Option<usize>,
    pub parsing_warning: Option<String>,
    pub custom_definition_suggestion: Option<String>,
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchAiAnalysisResponse {
    pub results: Vec<AiAnalysisResult>,
}

pub struct CardBatchInput<'a> {
    pub card_index: usize,
    pub sentence: &'a str,
    pub target_word: &'a str,
    pub candidates: &'a [crate::dict::LookupResult],
}

pub struct GeminiAiService;

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

        let cards_summary = cards
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

                format!(
                    "Card Index #{}:\nSentence: \"{}\"\nTarget Word: \"{}\"\nCandidates:\n{}",
                    c.card_index, c.sentence, c.target_word, candidates_str
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n--------------------\n\n");

        let prompt = format!(
            r#"You are an expert Japanese linguist and lexicographer.
Analyze ALL Japanese cards in this batch. Match each target word in its sentence context against its dictionary candidates.

Cards Batch:
{cards_summary}

For EACH card index:
1. Check if the target word has any tokenizer/segmentation misparse in the sentence. If so, provide a short `parsing_warning`. Otherwise null.
2. Select 0-based `recommended_candidate_index` and `recommended_sense_index` matching sentence context. If none fit, set `recommended_candidate_index` to null.
3. If no candidate fits or candidates are empty, provide a clean English `custom_definition_suggestion`. Otherwise null.

Return ONLY a valid JSON object matching this exact schema:
{{
  "results": [
    {{
      "card_index": number,
      "recommended_candidate_index": number or null,
      "recommended_sense_index": number or null,
      "parsing_warning": string or null,
      "custom_definition_suggestion": string or null,
      "explanation": string or null
    }}
  ]
}}"#
        );

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

        let max_attempts = 5;
        let mut last_error = String::new();

        for attempt in 1..=max_attempts {
            let resp = client.post(&url).json(&payload).send().await;
            match resp {
                Ok(response) if response.status().is_success() => {
                    let body: serde_json::Value = response.json().await?;
                    let text = body["candidates"][0]["content"]["parts"][0]["text"]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("Invalid response structure from Gemini"))?;

                    let parsed: BatchAiAnalysisResponse = serde_json::from_str(text)?;
                    return Ok(parsed.results);
                }
                Ok(response) => {
                    let err_text = response.text().await.unwrap_or_default();
                    last_error = format!("Gemini API error: {}", err_text);
                }
                Err(e) => {
                    last_error = e.to_string();
                }
            }

            if attempt < max_attempts {
                let delay = attempt as u64; // 1s, 2s, 3s, 4s, 5s
                eprintln!(
                    " ⚠️  Gemini API busy (attempt {}/{}). Retrying in {}s...",
                    attempt, max_attempts, delay
                );
                tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
            }
        }

        anyhow::bail!(last_error)
    }
}
