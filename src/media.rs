use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};

pub struct MediaExtractor;

impl MediaExtractor {
    pub fn extract_preview_audio(
        video_path: &Path,
        start_ms: u64,
        end_ms: u64,
        output_path: &Path,
    ) -> Result<()> {
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let start_sec = (start_ms as f64 / 1000.0 - 0.25).max(0.0);
        let duration_sec = ((end_ms - start_ms) as f64 / 1000.0 + 0.5).max(0.5);

        let is_opus = video_path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("opus"))
            .unwrap_or(false);

        let mut cmd = Command::new("ffmpeg");
        cmd.args([
            "-y",
            "-ss",
            &format!("{:.3}", start_sec),
            "-i",
            &video_path.to_string_lossy(),
            "-t",
            &format!("{:.3}", duration_sec),
            "-vn",
        ]);

        if is_opus {
            cmd.args(["-c:a", "copy"]);
        } else {
            cmd.args(["-c:a", "libopus", "-b:a", "64k", "-ar", "48000"]);
        }

        cmd.arg(output_path);
        cmd.stdout(Stdio::null()).stderr(Stdio::null());

        let status = cmd
            .status()
            .context("Failed to spawn ffmpeg for audio extraction")?;

        if !status.success() {
            // If copy failed on opus, retry with re-encode
            if is_opus {
                let status_retry = Command::new("ffmpeg")
                    .args([
                        "-y",
                        "-ss",
                        &format!("{:.3}", start_sec),
                        "-i",
                        &video_path.to_string_lossy(),
                        "-t",
                        &format!("{:.3}", duration_sec),
                        "-vn",
                        "-c:a",
                        "libopus",
                        "-b:a",
                        "64k",
                        "-ar",
                        "48000",
                        &output_path.to_string_lossy(),
                    ])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .context("Failed to spawn ffmpeg for audio extraction retry")?;
                if !status_retry.success() {
                    anyhow::bail!("ffmpeg failed to extract preview audio from opus source");
                }
            } else {
                anyhow::bail!("ffmpeg failed to extract preview audio");
            }
        }

        Ok(())
    }

    pub fn extract_screenshot_with_index(
        video_path: &Path,
        timestamp_ms: u64,
        sentence_index: Option<usize>,
        output_path: &Path,
    ) -> Result<()> {
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Check if pre-extracted screenshot exists in bundle
        if let Some(idx) = sentence_index {
            let direct_shot = video_path.join(format!("screenshots/{}.jpg", idx));
            if direct_shot.exists() && std::fs::copy(&direct_shot, output_path).is_ok() {
                return Ok(());
            }
            if let Some(parent) = video_path.parent() {
                let adj_shot = parent.join(format!("screenshots/{}.jpg", idx));
                if adj_shot.exists() && std::fs::copy(&adj_shot, output_path).is_ok() {
                    return Ok(());
                }
            }
        }

        let sec = timestamp_ms as f64 / 1000.0;

        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-nostdin",
                "-noautorotate",
                "-skip_frame",
                "nokey",
                "-ss",
                &format!("{:.3}", sec),
                "-an",
                "-sn",
                "-i",
                &video_path.to_string_lossy(),
                "-vf",
                "scale=-1:360:flags=fast_bilinear",
                "-vframes",
                "1",
                "-q:v",
                "4",
                "-threads",
                "1",
                &output_path.to_string_lossy(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("Failed to spawn ffmpeg for screenshot extraction")?;

        if !status.success() {
            anyhow::bail!("ffmpeg failed to extract screenshot");
        }

        Ok(())
    }

    pub fn extract_screenshot(
        video_path: &Path,
        timestamp_ms: u64,
        output_path: &Path,
    ) -> Result<()> {
        Self::extract_screenshot_with_index(video_path, timestamp_ms, None, output_path)
    }

    pub fn play_preview_audio(audio_path: &Path) -> Option<std::process::Child> {
        let player = if which_exists("mpv") {
            vec![
                "mpv",
                "--no-config",
                "--no-video",
                "--really-quiet",
                "--no-terminal",
            ]
        } else if which_exists("pw-play") {
            vec!["pw-play"]
        } else if which_exists("paplay") {
            vec!["paplay"]
        } else {
            vec!["ffplay", "-nodisp", "-autoexit", "-loglevel", "quiet"]
        };

        let bin = player[0];
        let mut cmd = Command::new(bin);
        for arg in &player[1..] {
            cmd.arg(arg);
        }
        cmd.arg(audio_path);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());

        cmd.spawn().ok()
    }

    /// Play just one subtitle interval directly from the source video.  This avoids
    /// creating a cache file for the lightweight `--inspect` workflow.
    pub fn play_subtitle_segment(
        video_path: &Path,
        start_ms: u64,
        end_ms: u64,
    ) -> Option<std::process::Child> {
        let start_sec = (start_ms as f64 / 1000.0 - 0.25).max(0.0);
        let duration_sec = ((end_ms.saturating_sub(start_ms)) as f64 / 1000.0 + 0.5).max(0.5);

        let mut cmd = if which_exists("mpv") {
            let mut cmd = Command::new("mpv");
            cmd.args([
                "--no-config",
                "--no-video",
                "--really-quiet",
                "--no-terminal",
                &format!("--start={start_sec:.3}"),
                &format!("--length={duration_sec:.3}"),
            ]);
            cmd
        } else {
            let mut cmd = Command::new("ffplay");
            cmd.args([
                "-nodisp",
                "-autoexit",
                "-loglevel",
                "quiet",
                "-ss",
                &format!("{start_sec:.3}"),
                "-t",
                &format!("{duration_sec:.3}"),
            ]);
            cmd
        };

        cmd.arg(video_path);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
        cmd.spawn().ok()
    }

    /// Prune old cached media files from `media_dir` down to `max_cards`.
    /// Preserves any files present in `protected_paths` (e.g. unsynced mined cards).
    /// Returns the number of files successfully deleted.
    pub fn clean_old_media(
        media_dir: &Path,
        max_cards: usize,
        protected_paths: &std::collections::HashSet<std::path::PathBuf>,
    ) -> Result<usize> {
        if max_cards == 0 || !media_dir.exists() || !media_dir.is_dir() {
            return Ok(0);
        }

        let media_extensions = ["opus", "jpg", "jpeg", "png", "mp3", "wav", "webm"];

        let entries = match std::fs::read_dir(media_dir) {
            Ok(e) => e,
            Err(_) => return Ok(0),
        };

        // Group files by card stem (e.g., "word_1" for "word_1.opus" and "word_1.jpg")
        let mut card_files: std::collections::HashMap<
            String,
            Vec<(std::path::PathBuf, std::time::SystemTime)>,
        > = std::collections::HashMap::new();

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !media_extensions.contains(&ext.as_str()) {
                continue;
            }

            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            card_files.entry(stem).or_default().push((path, mtime));
        }

        // Filter out cards that contain any protected file path
        let mut cleanable_cards: Vec<(String, std::time::SystemTime, Vec<std::path::PathBuf>)> =
            Vec::new();

        for (stem, files) in card_files {
            let is_protected = files.iter().any(|(p, _)| {
                protected_paths.contains(p)
                    || protected_paths
                        .iter()
                        .any(|prot| prot.file_name() == p.file_name())
            });

            if !is_protected {
                let newest_mtime = files
                    .iter()
                    .map(|(_, m)| *m)
                    .max()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                let paths = files.into_iter().map(|(p, _)| p).collect();
                cleanable_cards.push((stem, newest_mtime, paths));
            }
        }

        if cleanable_cards.len() <= max_cards {
            return Ok(0);
        }

        // Sort ascending by modification time (oldest first)
        cleanable_cards.sort_by_key(|(_, mtime, _)| *mtime);

        let excess_count = cleanable_cards.len() - max_cards;
        let mut deleted_files = 0;

        for (_, _, files) in cleanable_cards.into_iter().take(excess_count) {
            for file_path in files {
                if std::fs::remove_file(&file_path).is_ok() {
                    deleted_files += 1;
                }
            }
        }

        Ok(deleted_files)
    }
}

fn which_exists(bin: &str) -> bool {
    if let Some(paths) = std::env::var_os("PATH") {
        for path in std::env::split_paths(&paths) {
            let full_path = path.join(bin);
            if full_path.is_file() {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests;
