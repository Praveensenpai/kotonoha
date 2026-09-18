use anyhow::{Context, Result};
use console::style;
use inquire::Confirm;
use std::path::PathBuf;

use super::unpack::read_bundle_manifest;
use crate::db::Database;

#[derive(Debug, Clone)]
pub struct DuplicateSubtitleMatch {
    pub existing_bundle: String,
    pub existing_video: String,
    pub existing_sub: String,
}

/// Checks if a subtitle fingerprint was already bundled for a different video/episode.
pub async fn check_duplicate_subtitle(
    video_name: &str,
    video_fp: &str,
    sub_fp: &str,
    db: Option<&Database>,
    search_dirs: &[PathBuf],
) -> Option<DuplicateSubtitleMatch> {
    if sub_fp.is_empty() {
        return None;
    }

    // 1. Check SQLite database
    if let Some(database) = db {
        if let Ok(Some(record)) = database.find_bundle_by_subtitle_fingerprint(sub_fp).await {
            let is_diff_video = (!video_fp.is_empty() && record.video_fingerprint != video_fp)
                || (!video_name.is_empty() && record.source_video != video_name);
            if is_diff_video {
                return Some(DuplicateSubtitleMatch {
                    existing_bundle: record.bundle_path,
                    existing_video: record.source_video,
                    existing_sub: record.source_subtitle,
                });
            }
        }
    }

    // 2. Scan directories for existing .koto files
    for dir in search_dirs {
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if crate::bundle::is_bundle_file(&p) {
                if let Ok(manifest) = read_bundle_manifest(&p) {
                    if manifest.subtitle_fingerprint.as_deref() == Some(sub_fp) {
                        let is_diff_video = (!video_fp.is_empty()
                            && manifest.video_fingerprint.as_deref() != Some(video_fp))
                            || (!video_name.is_empty() && manifest.source_video != video_name);
                        if is_diff_video {
                            return Some(DuplicateSubtitleMatch {
                                existing_bundle: p.to_string_lossy().to_string(),
                                existing_video: manifest.source_video,
                                existing_sub: manifest.source_subtitle,
                            });
                        }
                    }
                }
            }
        }
    }

    None
}

/// Warn the user about a duplicate subtitle and prompt for confirmation unless forced.
pub fn prompt_duplicate_subtitle_warning(
    dup: &DuplicateSubtitleMatch,
    current_video_name: &str,
    sub_fp: &str,
    force: bool,
) -> Result<bool> {
    let hash_short = if sub_fp.len() >= 8 {
        &sub_fp[..8]
    } else {
        sub_fp
    };

    println!(
        "\n {} {}",
        style("⚠ Warning: Duplicate Subtitle Detected!")
            .yellow()
            .bold(),
        style(format!("(Hash: {})", hash_short)).dim()
    );
    println!(
        "   This subtitle file has identical content to one previously bundled for a different episode:"
    );
    println!(
        "     • Previously Used In: {}",
        style(&dup.existing_bundle).cyan().bold()
    );
    println!(
        "     • Previous Video:     {}",
        style(&dup.existing_video).dim()
    );
    if !dup.existing_sub.is_empty() {
        println!(
            "     • Previous Subtitle:  {}",
            style(&dup.existing_sub).dim()
        );
    }
    println!(
        "     • Current Target:     {}",
        style(current_video_name).yellow().bold()
    );
    println!(
        "   {}",
        style("Bundling the same subtitle to a different episode usually causes severe audio/subtitle desync!").red()
    );

    if force {
        println!(
            "   {} Continuing anyway (--force active).\n",
            style("→").yellow()
        );
        return Ok(true);
    }

    let proceed = Confirm::new("Do you want to proceed with this duplicate subtitle anyway?")
        .with_default(false)
        .prompt()
        .with_context(|| "Failed to read confirmation prompt")?;

    Ok(proceed)
}
