use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::nlp::JapaneseTokenizer;

/// Extract word-reading pairs for words using the given tokenizer.
pub fn words_with_readings(
    tokenizer: &JapaneseTokenizer,
    words: Vec<String>,
) -> Vec<(String, String)> {
    words
        .into_iter()
        .map(|word| {
            let reading = tokenizer
                .tokenize(&word)
                .ok()
                .and_then(|tokens| {
                    tokens
                        .iter()
                        .find(|token| token.dictionary_form == word)
                        .or_else(|| (tokens.len() == 1).then(|| &tokens[0]))
                        .map(|token| token.reading.clone())
                })
                .unwrap_or_else(|| word.clone());
            (word, reading)
        })
        .collect()
}

/// Find paired subtitle and audio/video file for normal playback/mining.
pub fn find_paired_media(input_path: &Path) -> Result<(PathBuf, PathBuf)> {
    if crate::bundle::is_bundle_file(input_path) {
        let unpacked = crate::bundle::unpack_bundle(input_path)?;
        return Ok((unpacked.subtitle_path, unpacked.audio_path));
    }

    if crate::bundle::is_bundle_dir(input_path) {
        let manifest_data = std::fs::read_to_string(input_path.join("manifest.json"))?;
        let manifest: crate::bundle::BundleManifest = serde_json::from_str(&manifest_data)?;
        return Ok((
            input_path.join(manifest.subtitle_file),
            input_path.join(manifest.audio_file),
        ));
    }

    let parent = input_path.parent().unwrap_or_else(|| Path::new("."));
    let ext = input_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    let is_sub = matches!(ext.as_str(), "srt" | "ass" | "vtt");
    let is_vid = matches!(
        ext.as_str(),
        "mkv" | "mp4" | "webm" | "avi" | "opus" | "mp3" | "m4a"
    );

    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let clean_stem = stem
        .trim_end_matches(".ja")
        .trim_end_matches(".jp")
        .trim_end_matches(".ja-JP")
        .trim_end_matches(".japanese")
        .trim_end_matches(".en");

    if is_sub {
        let sub_path = input_path.to_path_buf();
        let mut candidate_dirs = vec![parent.to_path_buf()];
        let subfolder = parent.join(".koto");
        if subfolder.is_dir() {
            candidate_dirs.push(subfolder);
        }
        let cfg = crate::config::AppConfig::load().unwrap_or_default();
        if cfg.bundles_dir.is_dir() && !candidate_dirs.contains(&cfg.bundles_dir) {
            candidate_dirs.push(cfg.bundles_dir.clone());
        }

        for search_dir in candidate_dirs {
            if let Ok(entries) = std::fs::read_dir(&search_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if crate::bundle::is_bundle_file(&p) {
                        let p_stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                        if p_stem == stem
                            || p_stem == clean_stem
                            || stem.starts_with(p_stem)
                            || p_stem.starts_with(clean_stem)
                        {
                            let unpacked = crate::bundle::unpack_bundle(&p)?;
                            return Ok((sub_path, unpacked.audio_path));
                        }
                    }
                    let p_ext = p
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if matches!(
                        p_ext.as_str(),
                        "mkv" | "mp4" | "webm" | "avi" | "opus" | "mp3" | "m4a"
                    ) {
                        let p_stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                        if p_stem == stem
                            || p_stem == clean_stem
                            || stem.starts_with(p_stem)
                            || p_stem.starts_with(clean_stem)
                        {
                            return Ok((sub_path, p));
                        }
                    }
                }
            }
        }
        anyhow::bail!(
            "No matching video file (.mkv, .mp4, .koto) found for subtitle: {}\n   Place the video file in the same folder to mine cards.",
            input_path.display()
        );
    } else if is_vid {
        let vid_path = input_path.to_path_buf();
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                let p = entry.path();
                let p_ext = p
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if matches!(p_ext.as_str(), "srt" | "ass" | "vtt") {
                    let p_stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    let p_clean = p_stem
                        .trim_end_matches(".ja")
                        .trim_end_matches(".jp")
                        .trim_end_matches(".ja-JP")
                        .trim_end_matches(".japanese");
                    if p_stem == stem
                        || p_clean == stem
                        || p_stem.starts_with(stem)
                        || stem.starts_with(p_clean)
                    {
                        return Ok((p, vid_path));
                    }
                }
            }
        }
        anyhow::bail!(
            "No matching Japanese subtitle file (.srt, .ass) found for video: {}\n   Place the subtitle file in the same folder to mine cards.\n\n   Need to generate an .srt subtitle? Try SubSink:\n   https://github.com/Praveensenpai/subsink",
            input_path.display()
        );
    } else {
        anyhow::bail!("Unsupported file format: {}", input_path.display());
    }
}

/// Find paired media specifically for bundle creation (requires full video, not just audio).
pub fn find_paired_media_for_bundling(input_path: &Path) -> Result<(PathBuf, PathBuf)> {
    let parent = input_path.parent().unwrap_or_else(|| Path::new("."));
    let ext = input_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    let is_sub = matches!(ext.as_str(), "srt" | "ass" | "vtt");
    let is_vid = matches!(ext.as_str(), "mkv" | "mp4" | "webm" | "avi");

    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let clean_stem = stem
        .trim_end_matches(".ja")
        .trim_end_matches(".jp")
        .trim_end_matches(".ja-JP")
        .trim_end_matches(".japanese")
        .trim_end_matches(".en");

    if is_sub {
        let sub_path = input_path.to_path_buf();
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                let p = entry.path();
                let p_ext = p
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if matches!(p_ext.as_str(), "mkv" | "mp4" | "webm" | "avi") {
                    let p_stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    if p_stem == stem
                        || p_stem == clean_stem
                        || stem.starts_with(p_stem)
                        || p_stem.starts_with(clean_stem)
                    {
                        return Ok((sub_path, p));
                    }
                }
            }
        }
        anyhow::bail!(
            "No matching video file (.mkv, .mp4) found for subtitle: {}\n   A video file is required to pre-save audio and screenshots into a .koto bundle.",
            input_path.display()
        );
    } else if is_vid {
        let vid_path = input_path.to_path_buf();
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                let p = entry.path();
                let p_ext = p
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if matches!(p_ext.as_str(), "srt" | "ass" | "vtt") {
                    let p_stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    let p_clean = p_stem
                        .trim_end_matches(".ja")
                        .trim_end_matches(".jp")
                        .trim_end_matches(".ja-JP")
                        .trim_end_matches(".japanese");
                    if p_stem == stem
                        || p_clean == stem
                        || p_stem.starts_with(stem)
                        || stem.starts_with(p_clean)
                    {
                        return Ok((p, vid_path));
                    }
                }
            }
        }
        anyhow::bail!(
            "No matching Japanese subtitle file (.srt, .ass) found for video: {}\n   A subtitle file is required to pre-save into a .koto bundle.",
            input_path.display()
        );
    } else {
        anyhow::bail!(
            "Unsupported file format for bundling: {}",
            input_path.display()
        );
    }
}
