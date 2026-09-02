use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use std::path::Path;

use crate::config::AppConfig;
use crate::db::Database;
use crate::nlp::JapaneseTokenizer;

use super::formatter::{
    anki_search_text, format_definition_for_anki, pitch_pattern, sentence_with_furigana,
};

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

pub async fn find_existing_anki_note(
    client: &reqwest::Client,
    url: &str,
    model_name: &str,
    sentence: &str,
) -> Result<Option<i64>> {
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

pub async fn sync_to_anki(cfg: &AppConfig, db: &Database) -> Result<()> {
    if !anki_connected(&cfg.anki.connect_url).await {
        anyhow::bail!(
            "Anki is not connected. Please open Anki and make sure AnkiConnect is installed."
        );
    }

    let all_cards = db.get_all_mined_cards().await?;
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
                db.mark_mined_card_synced(card.id, note_id).await?;
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
                                "SentEng": card.english_natural.as_deref().unwrap_or_default(),
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

                db.mark_mined_card_synced(card.id, note_id).await?;
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
