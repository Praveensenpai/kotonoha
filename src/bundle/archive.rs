use anyhow::{Context, Result};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use super::unpack::read_bundle_manifest;
use super::CreateBundleOptions;
use crate::ui::format_duration;

#[derive(Debug, Clone, Copy)]
pub struct BundleDurations {
    pub total: Duration,
    pub audio: Duration,
    pub screenshots: Duration,
    pub packaging: Duration,
}

pub async fn find_matching_bundle(
    final_output: &Path,
    video_fp: &str,
    sub_fp: &str,
    options: &CreateBundleOptions<'_>,
) -> Option<PathBuf> {
    if options.force {
        return None;
    }
    if final_output.exists() {
        if let Ok(existing) = read_bundle_manifest(final_output) {
            let m_vid = existing
                .video_fingerprint
                .as_deref()
                .map(|fp| fp == video_fp)
                .unwrap_or(false);
            let m_sub = existing
                .subtitle_fingerprint
                .as_deref()
                .map(|fp| fp == sub_fp)
                .unwrap_or(false);

            if m_vid && m_sub {
                return Some(final_output.to_path_buf());
            }
        }
    }
    if let Some(database) = options.db {
        if let Ok(Some(path)) = database.find_existing_bundle(video_fp, sub_fp).await {
            return Some(path);
        }
    }
    None
}

pub fn package_bundle_archive(temp_dir: &Path, final_output: &Path) -> Result<Duration> {
    let start = Instant::now();
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template(" {spinner:.green} [3/3] Packaging into standalone .koto archive...")
            .unwrap(),
    );
    pb.enable_steady_tick(Duration::from_millis(80));

    let file = File::create(final_output)
        .with_context(|| format!("Failed to create .koto file: {}", final_output.display()))?;
    let mut zip = ZipWriter::new(file);
    let zip_opts = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    for entry in WalkDir::new(temp_dir) {
        let entry = entry?;
        let path = entry.path();
        let rel_path = path.strip_prefix(temp_dir)?;

        if path.is_file() {
            zip.start_file(rel_path.to_string_lossy(), zip_opts)?;
            let mut f = File::open(path)?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
        } else if !rel_path.as_os_str().is_empty() {
            zip.add_directory(rel_path.to_string_lossy(), zip_opts)?;
        }
    }
    zip.finish()?;

    let dur = start.elapsed();
    pb.finish_and_clear();
    println!(
        " {} [3/3] Packaged .koto archive: {}",
        style("✔").green().bold(),
        style(format_duration(dur)).cyan().bold()
    );
    Ok(dur)
}

pub fn print_bundle_summary(final_output: &Path, video_path: &Path, durations: BundleDurations) {
    let orig_video_size = std::fs::metadata(video_path).map(|m| m.len()).unwrap_or(0);
    let koto_size = std::fs::metadata(final_output)
        .map(|m| m.len())
        .unwrap_or(0);

    let orig_mb = orig_video_size as f64 / (1024.0 * 1024.0);
    let koto_mb = koto_size as f64 / (1024.0 * 1024.0);
    let ratio = if orig_video_size > 0 {
        (1.0 - (koto_size as f64 / orig_video_size as f64)) * 100.0
    } else {
        0.0
    };

    println!(
        "\n {}",
        style("✨ Pre-saving completed successfully!")
            .green()
            .bold()
    );
    println!(
        " 📦 Saved:    {}",
        style(final_output.display()).cyan().bold()
    );
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
        " ⏱️  Duration: {} (Audio: {} | Screenshots: {} | Packaging: {})",
        style(format_duration(durations.total)).yellow().bold(),
        style(format_duration(durations.audio)).cyan(),
        style(format_duration(durations.screenshots)).cyan(),
        style(format_duration(durations.packaging)).cyan()
    );
    println!(
        " 💡 You can now run: {}",
        style(format!("kotonoha \"{}\"", final_output.display()))
            .yellow()
            .bold()
    );
}
