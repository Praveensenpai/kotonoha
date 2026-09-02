use anyhow::{Context, Result};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::media::MediaExtractor;
use crate::srt::parse_subtitle;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleManifest {
    pub version: u32,
    pub source_video: String,
    pub source_subtitle: String,
    pub created_at: String,
    pub audio_file: String,
    pub subtitle_file: String,
    pub sentence_count: usize,
    pub has_screenshots: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct UnpackedBundle {
    pub root_dir: PathBuf,
    pub subtitle_path: PathBuf,
    pub audio_path: PathBuf,
    pub screenshots_dir: PathBuf,
    pub manifest: BundleManifest,
}

pub fn get_bundles_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".cache"))
        .join("kotonoha")
        .join("bundles")
}

pub fn is_bundle_file(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("koto"))
        .unwrap_or(false)
}

pub fn is_bundle_dir(path: &Path) -> bool {
    path.is_dir() && path.join("manifest.json").exists()
}

pub fn unpack_bundle(koto_path: &Path) -> Result<UnpackedBundle> {
    if !koto_path.exists() {
        anyhow::bail!("Bundle file not found: {}", koto_path.display());
    }

    let stem = koto_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("bundle");

    let koto_mtime = std::fs::metadata(koto_path)
        .and_then(|m| m.modified())
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

    let cache_dir = get_bundles_cache_dir().join(stem);
    let manifest_path = cache_dir.join("manifest.json");

    let needs_unpack = if manifest_path.exists() {
        let manifest_mtime = std::fs::metadata(&manifest_path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        koto_mtime > manifest_mtime
    } else {
        true
    };

    if needs_unpack {
        if cache_dir.exists() {
            let _ = std::fs::remove_dir_all(&cache_dir);
        }
        std::fs::create_dir_all(&cache_dir)?;

        let file = File::open(koto_path)
            .with_context(|| format!("Failed to open .koto bundle: {}", koto_path.display()))?;
        let mut archive = ZipArchive::new(file)
            .with_context(|| format!("Failed to read .koto zip archive: {}", koto_path.display()))?;

        for i in 0..archive.len() {
            let mut zip_file = archive.by_index(i)?;
            let outpath = match zip_file.enclosed_name() {
                Some(path) => cache_dir.join(path),
                None => continue,
            };

            if zip_file.is_dir() {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(p)?;
                    }
                }
                let mut outfile = File::create(&outpath)?;
                std::io::copy(&mut zip_file, &mut outfile)?;
            }
        }
    }

    let manifest_data = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read manifest.json in bundle cache: {}", manifest_path.display()))?;
    let manifest: BundleManifest = serde_json::from_str(&manifest_data)
        .with_context(|| "Failed to parse bundle manifest.json")?;

    let subtitle_path = cache_dir.join(&manifest.subtitle_file);
    let audio_path = cache_dir.join(&manifest.audio_file);
    let screenshots_dir = cache_dir.join("screenshots");

    Ok(UnpackedBundle {
        root_dir: cache_dir,
        subtitle_path,
        audio_path,
        screenshots_dir,
        manifest,
    })
}

pub fn create_bundle(
    video_path: &Path,
    subtitle_path: &Path,
    output_path: Option<&Path>,
) -> Result<PathBuf> {
    let sub_stem = subtitle_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("presaved");
    let clean_stem = sub_stem
        .trim_end_matches(".ja")
        .trim_end_matches(".jp")
        .trim_end_matches(".ja-JP")
        .trim_end_matches(".japanese");

    let final_output = match output_path {
        Some(p) => p.to_path_buf(),
        None => {
            let parent = subtitle_path.parent().unwrap_or_else(|| Path::new("."));
            parent.join(format!("{}.koto", clean_stem))
        }
    };

    let temp_dir = std::env::temp_dir().join(format!("kotonoha_bundle_{}_{}", clean_stem, std::process::id()));
    if temp_dir.exists() {
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    std::fs::create_dir_all(&temp_dir)?;

    let sentences = parse_subtitle(subtitle_path)
        .with_context(|| format!("Failed to parse subtitle for bundling: {}", subtitle_path.display()))?;

    println!(
        "\n 📦 {} {}",
        style("Pre-saving Kotonoha Bundle:").cyan().bold(),
        style(final_output.file_name().unwrap_or_default().to_string_lossy()).bold()
    );
    println!(" ℹ Subtitle Lines: {}", style(sentences.len()).yellow().bold());

    // Step 1: Compress full audio to Opus 64k
    let audio_dest = temp_dir.join("audio.opus");
    let pb_audio = ProgressBar::new_spinner();
    pb_audio.set_style(
        ProgressStyle::default_spinner()
            .template(" {spinner:.green} [1/3] Compressing audio track (.opus @ 64kbps)... {msg}")
            .unwrap(),
    );
    pb_audio.enable_steady_tick(std::time::Duration::from_millis(80));

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
        let _ = std::fs::remove_dir_all(&temp_dir);
        anyhow::bail!("ffmpeg failed to compress full audio track to Opus 64kbps");
    }
    pb_audio.finish_with_message("Done ✔");

    // Step 2: Extract 360p screenshots for each subtitle sentence
    let screenshots_dir = temp_dir.join("screenshots");
    std::fs::create_dir_all(&screenshots_dir)?;

    let pb_shots = ProgressBar::new(sentences.len() as u64);
    pb_shots.set_style(
        ProgressStyle::default_bar()
            .template(" ℹ [2/3] Extracting 360p screenshots [{bar:35.yellow/blue}] {pos}/{len} ({percent}%)")
            .unwrap()
            .progress_chars("█▓▒░"),
    );

    sentences.par_iter().for_each(|s| {
        let shot_path = screenshots_dir.join(format!("{}.jpg", s.index));
        let _ = MediaExtractor::extract_screenshot(video_path, s.start_ms, &shot_path);
        pb_shots.inc(1);
    });
    pb_shots.finish();

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
        source_video: video_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
        source_subtitle: subtitle_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        audio_file: "audio.opus".to_string(),
        subtitle_file: sub_filename,
        sentence_count: sentences.len(),
        has_screenshots: true,
    };

    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(temp_dir.join("manifest.json"), manifest_json)?;

    // Step 4: Package into .koto zip file
    println!(" ℹ [3/3] Packaging into standalone .koto archive...");
    let file = File::create(&final_output)
        .with_context(|| format!("Failed to create .koto file: {}", final_output.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    for entry in WalkDir::new(&temp_dir) {
        let entry = entry?;
        let path = entry.path();
        let rel_path = path.strip_prefix(&temp_dir)?;

        if path.is_file() {
            zip.start_file(rel_path.to_string_lossy(), options)?;
            let mut f = File::open(path)?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
        } else if !rel_path.as_os_str().is_empty() {
            zip.add_directory(rel_path.to_string_lossy(), options)?;
        }
    }
    zip.finish()?;

    // Clean up temporary files
    let _ = std::fs::remove_dir_all(&temp_dir);

    // Calculate compression stats
    let orig_video_size = std::fs::metadata(video_path).map(|m| m.len()).unwrap_or(0);
    let koto_size = std::fs::metadata(&final_output).map(|m| m.len()).unwrap_or(0);

    let orig_mb = orig_video_size as f64 / (1024.0 * 1024.0);
    let koto_mb = koto_size as f64 / (1024.0 * 1024.0);
    let ratio = if orig_video_size > 0 {
        (1.0 - (koto_size as f64 / orig_video_size as f64)) * 100.0
    } else {
        0.0
    };

    println!("\n {}", style("✨ Pre-saving completed successfully!").green().bold());
    println!(" 📦 Saved:    {}", style(final_output.display()).cyan().bold());
    println!(
        " 📊 Size:     {} (Original Video: {})",
        style(format!("{:.1} MB", koto_mb)).green().bold(),
        style(format!("{:.1} MB", orig_mb)).dim()
    );
    if ratio > 0.0 {
        println!(
            " 🚀 Savings:  {} space saved!",
            style(format!("{:.1}%", ratio)).green().bold()
        );
    }
    println!(
        " 💡 You can now run: {}",
        style(format!("kotonoha \"{}\"", final_output.display())).yellow().bold()
    );

    Ok(final_output)
}

#[cfg(test)]
mod tests;
