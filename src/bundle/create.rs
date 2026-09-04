use anyhow::{Context, Result};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::srt::{parse_subtitle, SubtitleSentence};
use crate::ui::format_duration;

use super::archive::{
    find_matching_bundle, package_bundle_archive, print_bundle_summary, BundleDurations,
};
use super::destination::resolve_bundle_destination;
use super::fingerprint::{compute_subtitle_fingerprint, compute_video_fingerprint};
use super::{BundleManifest, CreateBundleOptions};

fn compress_bundle_audio(video_path: &Path, audio_dest: &Path) -> Result<Duration> {
    let start = Instant::now();
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template(" {spinner:.green} [1/3] Compressing audio track (.opus @ 64kbps)...")
            .unwrap(),
    );
    pb.enable_steady_tick(Duration::from_millis(80));

    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            &video_path.to_string_lossy(),
            "-vn",
            "-c:a",
            "libopus",
            "-b:a",
            "64k",
            "-ar",
            "48000",
            "-ac",
            "2",
            &audio_dest.to_string_lossy(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("Failed to spawn ffmpeg for full audio extraction")?;

    if !status.success() {
        anyhow::bail!("ffmpeg failed to compress full audio track to Opus 64kbps");
    }
    let dur = start.elapsed();
    pb.finish_and_clear();
    println!(
        " {} [1/3] Audio compressed (.opus @ 64kbps): {}",
        style("✔").green().bold(),
        style(format_duration(dur)).cyan().bold()
    );
    Ok(dur)
}

fn extract_bundle_screenshots_step(
    video_path: &Path,
    sentences: &[SubtitleSentence],
    dest_dir: &Path,
) -> Result<Duration> {
    let start = Instant::now();
    let pb = ProgressBar::new(sentences.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(" ℹ [2/3] Extracting 360p screenshots [{bar:35.yellow/blue}] {pos}/{len} ({percent}%)")
            .unwrap()
            .progress_chars("█▓▒░"),
    );

    super::screenshots::extract_bundle_screenshots(video_path, sentences, dest_dir, &pb)?;
    let dur = start.elapsed();
    pb.finish_and_clear();
    println!(
        " {} [2/3] Extracted {} screenshots: {}",
        style("✔").green().bold(),
        sentences.len(),
        style(format_duration(dur)).cyan().bold()
    );
    Ok(dur)
}

/// Compress video audio, extract screenshots, and create a standalone .koto archive.
pub async fn create_bundle(
    video_path: &Path,
    subtitle_path: &Path,
    options: CreateBundleOptions<'_>,
) -> Result<PathBuf> {
    let total_start = Instant::now();
    let sub_stem = subtitle_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("presaved");
    let clean_stem = sub_stem
        .trim_end_matches(".ja")
        .trim_end_matches(".jp")
        .trim_end_matches(".ja-JP")
        .trim_end_matches(".japanese");

    let final_output = match options.output_path {
        Some(p) => p.to_path_buf(),
        None => resolve_bundle_destination(
            subtitle_path,
            clean_stem,
            options.storage_strategy,
            options.bundles_dir,
        )?,
    };

    let video_fp = compute_video_fingerprint(video_path).unwrap_or_default();
    let sub_fp = compute_subtitle_fingerprint(subtitle_path).unwrap_or_default();

    if let Some(existing) = find_matching_bundle(&final_output, &video_fp, &sub_fp, &options).await
    {
        println!(
            "\n ℹ {} {}",
            style("Bundle already exists and is up to date:")
                .green()
                .bold(),
            style(existing.display()).cyan().bold()
        );
        println!(
            "   {} (Use {} to force rebuild)",
            style("Skipping redundant re-encoding.").dim(),
            style("--force").yellow().bold()
        );
        return Ok(existing);
    }

    let temp_dir = std::env::temp_dir().join(format!(
        "kotonoha_bundle_{}_{}",
        clean_stem,
        std::process::id()
    ));
    if temp_dir.exists() {
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    std::fs::create_dir_all(&temp_dir)?;

    let sentences = parse_subtitle(subtitle_path).with_context(|| {
        format!(
            "Failed to parse subtitle for bundling: {}",
            subtitle_path.display()
        )
    })?;

    println!(
        "\n 📦 {} {}",
        style("Pre-saving Kotonoha Bundle:").cyan().bold(),
        style(
            final_output
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        )
        .bold()
    );
    println!(
        " ℹ Subtitle Lines: {}",
        style(sentences.len()).yellow().bold()
    );

    // Step 1: Compress audio
    let audio_dest = temp_dir.join("audio.opus");
    let audio_dur = match compress_bundle_audio(video_path, &audio_dest) {
        Ok(d) => d,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(e);
        }
    };

    // Step 2: Extract screenshots
    let screenshots_dir = temp_dir.join("screenshots");
    std::fs::create_dir_all(&screenshots_dir)?;
    let shots_dur = match extract_bundle_screenshots_step(video_path, &sentences, &screenshots_dir)
    {
        Ok(d) => d,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(e);
        }
    };

    // Step 3: Copy subtitle file & write manifest
    let sub_ext = subtitle_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("srt");
    let sub_filename = format!("subtitles.{}", sub_ext);
    let sub_dest = temp_dir.join(&sub_filename);
    std::fs::copy(subtitle_path, &sub_dest)
        .with_context(|| "Failed to copy subtitle into bundle")?;

    let manifest = BundleManifest {
        version: 1,
        source_video: video_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        source_subtitle: subtitle_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        audio_file: "audio.opus".to_string(),
        subtitle_file: sub_filename,
        sentence_count: sentences.len(),
        has_screenshots: true,
        video_fingerprint: Some(video_fp.clone()),
        subtitle_fingerprint: Some(sub_fp.clone()),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(temp_dir.join("manifest.json"), manifest_json)?;

    // Step 4: Package into .koto zip file
    let zip_dur = match package_bundle_archive(&temp_dir, &final_output) {
        Ok(d) => d,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(e);
        }
    };

    let _ = std::fs::remove_dir_all(&temp_dir);

    // Save to database
    if let Some(database) = options.db {
        let abs_video = video_path
            .canonicalize()
            .unwrap_or_else(|_| video_path.to_path_buf());
        let abs_sub = subtitle_path
            .canonicalize()
            .unwrap_or_else(|_| subtitle_path.to_path_buf());
        let abs_output = final_output
            .canonicalize()
            .unwrap_or_else(|_| final_output.clone());

        let _ = database
            .record_bundle(
                &abs_output,
                &abs_video.to_string_lossy(),
                &abs_sub.to_string_lossy(),
                &video_fp,
                &sub_fp,
            )
            .await;
    }

    let total_dur = total_start.elapsed();
    print_bundle_summary(
        &final_output,
        video_path,
        BundleDurations {
            total: total_dur,
            audio: audio_dur,
            screenshots: shots_dur,
            packaging: zip_dur,
        },
    );

    Ok(final_output)
}
