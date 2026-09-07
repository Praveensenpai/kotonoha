use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::Read;

fn locate_or_migrate_dict_dir() -> std::path::PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let data_dir = dirs::data_dir()
        .map(|p| p.join("kotonoha"))
        .unwrap_or_else(|| home.join(".local/share/kotonoha"));
    let dict_dir = data_dir.join("dicts");

    let legacy_config_dir = dirs::config_dir()
        .map(|p| p.join("kotonoha"))
        .unwrap_or_else(|| home.join(".config/kotonoha"));
    let legacy_dict_dir = legacy_config_dir.join("dicts");

    if legacy_dict_dir.exists() && !dict_dir.exists() {
        let _ = std::fs::create_dir_all(&data_dir);
        let _ = std::fs::rename(&legacy_dict_dir, &dict_dir);
    } else if legacy_dict_dir.exists() && dict_dir.exists() {
        for name in ["JMdict_english.zip", "kanjium_pitch_accents.zip"] {
            let old_file = legacy_dict_dir.join(name);
            let new_file = dict_dir.join(name);
            if old_file.exists() && !new_file.exists() {
                let _ = std::fs::rename(&old_file, &new_file);
            } else if old_file.exists() {
                let _ = std::fs::remove_file(&old_file);
            }
        }
        let _ = std::fs::remove_dir(&legacy_dict_dir);
    }

    dict_dir
}

pub async fn ensure_offline_dictionaries_ready(
    client: &reqwest::Client,
    db: &mut crate::db::Database,
) -> Result<()> {
    if db.is_offline_dict_indexed().await.unwrap_or(false) {
        return Ok(());
    }

    let dict_dir = locate_or_migrate_dict_dir();
    std::fs::create_dir_all(&dict_dir)?;

    let jmdict_path = dict_dir.join("JMdict_english.zip");
    let pitch_path = dict_dir.join("kanjium_pitch_accents.zip");

    let jmdict_url =
        "https://github.com/yomidevs/jmdict-yomitan/releases/latest/download/JMdict_english.zip";
    let pitch_url =
        "https://github.com/Ajatt-Tools/rikaitan/raw/dictionaries/kanjium_pitch_accents.zip";

    if !jmdict_path.exists() {
        println!(" 📥 Downloading offline bilingual dictionary (JMdict ~15 MB)...");
        if let Err(e) = download_file_with_progress(client, jmdict_url, &jmdict_path).await {
            eprintln!(" ⚠️ JMdict download warning: {}", e);
        }
    }

    if !pitch_path.exists() {
        println!(" 📥 Downloading pitch accent dictionary (Kanjium ~1 MB)...");
        if let Err(e) = download_file_with_progress(client, pitch_url, &pitch_path).await {
            eprintln!(" ⚠️ Pitch accent dictionary download warning: {}", e);
        }
    }

    println!(" ⚡ Indexing offline Yomitan dictionaries into local SQLite database...");
    let mut all_terms = Vec::new();
    let mut pitch_map: std::collections::HashMap<(String, String), String> =
        std::collections::HashMap::new();

    // Parse Pitch Accents
    if pitch_path.exists() {
        if let Ok(file) = std::fs::File::open(&pitch_path) {
            if let Ok(mut archive) = zip::ZipArchive::new(file) {
                for i in 0..archive.len() {
                    if let Ok(mut zip_file) = archive.by_index(i) {
                        let name = zip_file.name().to_string();
                        if name.starts_with("term_meta_bank_") && name.ends_with(".json") {
                            let mut contents = String::new();
                            if zip_file.read_to_string(&mut contents).is_ok() {
                                if let Ok(arr) =
                                    serde_json::from_str::<Vec<serde_json::Value>>(&contents)
                                {
                                    for entry in arr {
                                        if let Some(item_arr) = entry.as_array() {
                                            if item_arr.len() >= 3 {
                                                let expr =
                                                    item_arr[0].as_str().unwrap_or("").to_string();
                                                let tag = item_arr[1].as_str().unwrap_or("");
                                                if tag == "pitch" {
                                                    let pitch_data = &item_arr[2];
                                                    let reading = pitch_data["reading"]
                                                        .as_str()
                                                        .unwrap_or(&expr)
                                                        .to_string();
                                                    let pos = pitch_data["pitches"]
                                                        .as_array()
                                                        .and_then(|a| a.first())
                                                        .and_then(|p| p["position"].as_u64())
                                                        .unwrap_or(0)
                                                        as usize;
                                                    let label = match pos {
                                                        0 => "Heiban [0]".to_string(),
                                                        1 => "Atamadaka [1]".to_string(),
                                                        n => format!("Nakadaka [{}]", n),
                                                    };
                                                    pitch_map.insert((expr, reading), label);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Parse JMdict terms
    if jmdict_path.exists() {
        if let Ok(file) = std::fs::File::open(&jmdict_path) {
            if let Ok(mut archive) = zip::ZipArchive::new(file) {
                for i in 0..archive.len() {
                    if let Ok(mut zip_file) = archive.by_index(i) {
                        let name = zip_file.name().to_string();
                        if name.starts_with("term_bank_") && name.ends_with(".json") {
                            let mut contents = String::new();
                            if zip_file.read_to_string(&mut contents).is_ok() {
                                if let Ok(arr) =
                                    serde_json::from_str::<Vec<serde_json::Value>>(&contents)
                                {
                                    for entry in arr {
                                        if let Some(item_arr) = entry.as_array() {
                                            if item_arr.len() >= 6 {
                                                let expr =
                                                    item_arr[0].as_str().unwrap_or("").to_string();
                                                let reading =
                                                    item_arr[1].as_str().unwrap_or("").to_string();
                                                let pos_tag =
                                                    item_arr[2].as_str().unwrap_or("Vocab");
                                                if pos_tag == "forms" || pos_tag.contains("forms") {
                                                    continue;
                                                }
                                                let score = item_arr[4].as_i64().unwrap_or(0);

                                                let mut glosses = Vec::new();
                                                extract_text_from_yomitan_json(
                                                    &item_arr[5],
                                                    &mut glosses,
                                                );

                                                if !glosses.is_empty() {
                                                    let def = format!(
                                                        "1. [{}] {}",
                                                        pos_tag,
                                                        glosses.join(", ")
                                                    );
                                                    let pitch = pitch_map
                                                        .get(&(expr.clone(), reading.clone()))
                                                        .or_else(|| {
                                                            pitch_map
                                                                .get(&(expr.clone(), expr.clone()))
                                                        })
                                                        .cloned()
                                                        .unwrap_or_else(|| "LH".to_string());

                                                    all_terms.push((
                                                        expr,
                                                        reading,
                                                        def,
                                                        pitch,
                                                        "JMdict".to_string(),
                                                        score,
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if !all_terms.is_empty() {
        let inserted = db.insert_offline_terms_batch(&all_terms).await?;
        println!(" ✨ Successfully indexed {} offline vocabulary terms into local SQLite (< 1ms queries)!", inserted);
    }

    Ok(())
}

pub async fn download_file_with_progress(
    client: &reqwest::Client,
    url: &str,
    target_path: &std::path::Path,
) -> Result<()> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("Failed to download file from {}: {}", url, resp.status());
    }

    let total_size = resp.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
            .progress_chars("#>-"),
    );

    let bytes = resp.bytes().await?;
    pb.finish_with_message("Download complete");

    std::fs::write(target_path, bytes)?;
    Ok(())
}

pub fn extract_text_from_yomitan_json(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::String(s) => {
            let s_trim = s.trim();
            if !s_trim.is_empty()
                && !s_trim.starts_with("forms ")
                && !s_trim.starts_with("see ")
                && s_trim != "⟶"
            {
                out.push(s_trim.to_string());
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                extract_text_from_yomitan_json(item, out);
            }
        }
        serde_json::Value::Object(obj) => {
            if let Some(data) = obj.get("data") {
                if let Some(c) = data.get("content").and_then(|c| c.as_str()) {
                    if c == "forms" {
                        return;
                    }
                }
            }
            if let Some(t) = obj.get("type").and_then(|t| t.as_str()) {
                if t == "forms" {
                    return;
                }
            }
            if let Some(content) = obj.get("content") {
                extract_text_from_yomitan_json(content, out);
            }
        }
        _ => {}
    }
}
