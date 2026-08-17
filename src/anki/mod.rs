use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use std::path::Path;

use crate::config::AppConfig;
use crate::db::Database;
use crate::dict;
use crate::nlp::JapaneseTokenizer;

pub async fn anki_connected(url: &str) -> bool {
    let body = serde_json::json!({"action": "version", "version": 6});
    reqwest::Client::new()
        .post(url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
        .is_ok()
}

pub async fn anki_request(
    client: &reqwest::Client,
    url: &str,
    action: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let response = match client
        .post(url)
        .json(&serde_json::json!({"action": action, "version": 6, "params": params}))
        .send()
        .await
    {
        Ok(res) => res,
        Err(e) if e.is_connect() => {
            anyhow::bail!(
                "Anki is not connected. Please open Anki and make sure AnkiConnect is installed."
            );
        }
        Err(e) => return Err(e.into()),
    };

    let response = response.error_for_status()?;
    let body: serde_json::Value = response.json().await?;
    if let Some(error) = body.get("error").and_then(|value| value.as_str()) {
        anyhow::bail!("AnkiConnect error: {error}");
    }
    Ok(body
        .get("result")
        .cloned()
        .unwrap_or(serde_json::Value::Null))
}

pub async fn upload_anki_media(
    client: &reqwest::Client,
    url: &str,
    card_id: i64,
    path: &str,
    extension: &str,
) -> Result<Option<String>> {
    let path = Path::new(path);
    if !path.is_file() {
        return Ok(None);
    }
    let filename = format!("kotonoha-{card_id}.{extension}");
    let data = BASE64.encode(std::fs::read(path)?);
    anki_request(
        client,
        url,
        "storeMediaFile",
        serde_json::json!({"filename": filename, "data": data}),
    )
    .await?;
    Ok(Some(filename))
}

pub fn anki_search_text(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

pub async fn find_existing_anki_note(
    client: &reqwest::Client,
    url: &str,
    model_name: &str,
    sentence: &str,
) -> Result<Option<i64>> {
    // Anki determines duplicates from the first field. SentKanji is the
    // first field in the Kotonoha note type, so use it as the stable key.
    let note_ids = anki_request(
        client,
        url,
        "findNotes",
        serde_json::json!({"query": format!("SentKanji:\"{}\"", anki_search_text(sentence))}),
    )
    .await?;
    let Some(note_ids) = note_ids.as_array() else {
        return Ok(None);
    };
    if note_ids.is_empty() {
        return Ok(None);
    }

    let note_ids: Vec<i64> = note_ids
        .iter()
        .filter_map(serde_json::Value::as_i64)
        .collect();
    if note_ids.is_empty() {
        return Ok(None);
    }
    let notes = anki_request(
        client,
        url,
        "notesInfo",
        serde_json::json!({"notes": note_ids}),
    )
    .await?;
    let Some(notes) = notes.as_array() else {
        return Ok(None);
    };

    Ok(notes.iter().find_map(|note| {
        if note.get("modelName").and_then(serde_json::Value::as_str) != Some(model_name) {
            return None;
        }
        let value = note
            .get("fields")
            .and_then(|fields| fields.get("SentKanji"))
            .and_then(|field| field.get("value"))
            .and_then(serde_json::Value::as_str);
        (value == Some(sentence))
            .then(|| note.get("noteId").and_then(serde_json::Value::as_i64))
            .flatten()
    }))
}

pub fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn format_definition_for_anki(def: &str) -> String {
    // Take the first non-empty sense line only
    let first_line = def
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            trimmed.strip_prefix('│').unwrap_or(trimmed).trim()
        })
        .find(|line| !line.is_empty())
        .unwrap_or(def.trim());

    // Strip leading "N. " numbering
    let after_num = first_line
        .find(". ")
        .map(|i| first_line[i + 2..].trim())
        .unwrap_or(first_line);

    // Strip leading "[Grammar Tag] " part
    if after_num.starts_with('[') {
        if let Some(close) = after_num.find(']') {
            return after_num[close + 1..].trim().to_string();
        }
    }

    after_num.to_string()
}

pub fn sentence_with_furigana(
    tokenizer: &JapaneseTokenizer,
    sentence: &str,
    target_word: &str,
) -> String {
    tokenizer
        .tokenize(sentence)
        .map(|tokens| {
            tokens
                .into_iter()
                .map(|token| {
                    let surface = escape_html(&token.surface);
                    let is_target =
                        token.surface == target_word || token.dictionary_form == target_word;
                    let display = if token
                        .surface
                        .chars()
                        .any(|c| matches!(c, '\u{4E00}'..='\u{9FFF}'))
                        && !token.reading.is_empty()
                    {
                        format!(
                            "<ruby>{surface}<rt>{}</rt></ruby>",
                            escape_html(&token.reading)
                        )
                    } else {
                        surface
                    };
                    if is_target {
                        format!("<b>{display}</b>")
                    } else {
                        display
                    }
                })
                .collect()
        })
        .unwrap_or_else(|_| escape_html(sentence))
}

pub fn to_katakana(text: &str) -> String {
    text.chars()
        .map(|character| match character {
            '\u{3041}'..='\u{3096}' => {
                std::char::from_u32(character as u32 + 0x60).unwrap_or(character)
            }
            _ => character,
        })
        .collect()
}

pub fn pitch_number(pitch_accent: &str, mora_count: usize) -> usize {
    if let Ok(number) = pitch_accent.trim().parse::<usize>() {
        return number.min(mora_count);
    }
    let pattern = pitch_accent.trim().to_ascii_uppercase();
    if pattern.starts_with('H') {
        1
    } else {
        pattern
            .chars()
            .position(|level| level == 'L')
            .filter(|drop| *drop > 1)
            .unwrap_or_default()
    }
}

pub fn pitch_pattern(reading: &str, pitch_accent: &str) -> (String, String) {
    let morae = dict::split_morae(&to_katakana(reading));
    let pitch = pitch_number(pitch_accent, morae.len());
    let levels: Vec<bool> = (0..morae.len())
        .map(|index| {
            if pitch == 1 {
                index == 0
            } else if pitch == 0 {
                index > 0
            } else {
                index > 0 && index < pitch
            }
        })
        .collect();
    let pattern = morae
        .iter()
        .enumerate()
        .map(|(index, mora)| {
            let current = levels[index];
            let previous = index
                .checked_sub(1)
                .and_then(|previous| levels.get(previous))
                .copied();
            let next = levels.get(index + 1).copied();
            let shadow = match (previous, current, next) {
                (_, false, Some(true)) => "inset -2px -2px 0 0 #3366CC",
                (Some(true), true, Some(false)) => "inset -2px 2px 0 0 #3366CC",
                (_, true, _) => "inset 0 2px 0 0 #3366CC",
                _ => "inset 0 -2px 0 0 #3366CC",
            };
            format!("<span style=\"box-shadow: {shadow};\">{mora}</span>")
        })
        .collect::<String>();
    let notation: String = levels
        .iter()
        .map(|is_high| if *is_high { 'H' } else { 'L' })
        .collect();
    (
        format!(
            "{pattern} <span class=\"pitch_number\">{pitch}</span> <span class=\"pitch_pattern_text\">[{notation}]</span>"
        ),
        pitch.to_string(),
    )
}

pub async fn sync_to_anki(cfg: &AppConfig, db: &Database) -> Result<()> {
    if !anki_connected(&cfg.anki.connect_url).await {
        anyhow::bail!(
            "Anki is not connected. Please open Anki and make sure AnkiConnect is installed."
        );
    }

    let all_cards = db.get_all_mined_cards()?;
    if all_cards.is_empty() {
        println!(" ✔ No locally mined cards in database.");
        return Ok(());
    }

    let client = reqwest::Client::new();
    let tokenizer = JapaneseTokenizer::new()?;
    anki_request(
        &client,
        &cfg.anki.connect_url,
        "version",
        serde_json::json!({}),
    )
    .await?;
    anki_request(
        &client,
        &cfg.anki.connect_url,
        "createDeck",
        serde_json::json!({"deck": cfg.anki.deck_name}),
    )
    .await?;
    let fields = anki_request(
        &client,
        &cfg.anki.connect_url,
        "modelFieldNames",
        serde_json::json!({"modelName": cfg.anki.model_name}),
    )
    .await?;
    let required_fields = [
        "SentKanji",
        "SentFurigana",
        "SentEng",
        "SentAudio",
        "VocabKanji",
        "VocabFurigana",
        "VocabPitchPattern",
        "VocabPitchNum",
        "VocabDef",
        "VocabAudio",
        "Image",
        "Notes",
        "MakeProductionCard",
        "Focus",
    ];
    let available_fields = fields.as_array().ok_or_else(|| {
        anyhow::anyhow!(
            "AnkiConnect returned invalid fields for note type: {}",
            cfg.anki.model_name
        )
    })?;
    if required_fields.iter().any(|field| {
        !available_fields
            .iter()
            .any(|value| value.as_str() == Some(field))
    }) {
        anyhow::bail!(
            "Anki note type '{}' does not have the required Japanese sentences+ fields",
            cfg.anki.model_name
        );
    }

    let mut synced_new = 0;

    for (card, existing_anki_id) in all_cards {
        let _note_id = if let Some(id) = existing_anki_id {
            id
        } else {
            let existing_note_id = find_existing_anki_note(
                &client,
                &cfg.anki.connect_url,
                &cfg.anki.model_name,
                &card.sentence,
            )
            .await?;

            if let Some(note_id) = existing_note_id {
                db.mark_mined_card_synced(card.id, note_id)?;
                note_id
            } else {
                let sentence_furigana =
                    sentence_with_furigana(&tokenizer, &card.sentence, &card.target_word);
                let (pitch_pattern, pitch_number) =
                    pitch_pattern(&card.reading, &card.pitch_accent);
                let audio = match card.audio_path.as_deref() {
                    Some(path) => {
                        upload_anki_media(&client, &cfg.anki.connect_url, card.id, path, "opus")
                            .await?
                    }
                    None => None,
                };
                let image = match card.image_path.as_deref() {
                    Some(path) => {
                        upload_anki_media(&client, &cfg.anki.connect_url, card.id, path, "jpg")
                            .await?
                    }
                    None => None,
                };
                let sentence_audio = audio
                    .map(|filename| format!("[sound:{filename}]"))
                    .unwrap_or_default();
                let image = image
                    .map(|filename| format!("<img src=\"{filename}\">"))
                    .unwrap_or_default();

                let note_id = match anki_request(
                    &client,
                    &cfg.anki.connect_url,
                    "addNote",
                    serde_json::json!({
                        "note": {
                            "deckName": cfg.anki.deck_name,
                            "modelName": cfg.anki.model_name,
                            "fields": {
                                "SentKanji": card.sentence,
                                "SentFurigana": sentence_furigana,
                                "SentEng": "",
                                "SentAudio": sentence_audio,
                                "VocabKanji": card.target_word,
                                "VocabFurigana": card.reading,
                                "VocabPitchPattern": pitch_pattern,
                                "VocabPitchNum": pitch_number,
                                "VocabDef": format_definition_for_anki(&card.definition),
                                "VocabAudio": "",
                                "Image": image,
                                "Notes": "",
                                "MakeProductionCard": "",
                                "Focus": ""
                            },
                            "tags": ["jp1k", "kotonoha", "mined"]
                        }
                    }),
                )
                .await
                {
                    Ok(res) => res
                        .as_i64()
                        .ok_or_else(|| anyhow::anyhow!("AnkiConnect did not return a note ID"))?,
                    Err(error) if error.to_string().to_ascii_lowercase().contains("duplicate") => {
                        find_existing_anki_note(
                            &client,
                            &cfg.anki.connect_url,
                            &cfg.anki.model_name,
                            &card.sentence,
                        )
                        .await?
                        .ok_or(error)?
                    }
                    Err(error) => return Err(error),
                };

                db.mark_mined_card_synced(card.id, note_id)?;
                synced_new += 1;
                note_id
            }
        };
    }

    if synced_new == 0 {
        println!(" ✔ Anki Sync Complete! (All cards already up to date)");
    } else {
        println!(
            " ✔ Anki Sync Complete! ({} new card{} synced)",
            synced_new,
            if synced_new == 1 { "" } else { "s" }
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
