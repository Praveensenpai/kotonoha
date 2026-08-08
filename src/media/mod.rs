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

        let status = Command::new("ffmpeg")
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
            .context("Failed to spawn ffmpeg for audio extraction")?;

        if !status.success() {
            anyhow::bail!("ffmpeg failed to extract preview audio");
        }

        Ok(())
    }

    pub fn extract_screenshot(
        video_path: &Path,
        timestamp_ms: u64,
        output_path: &Path,
    ) -> Result<()> {
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let sec = timestamp_ms as f64 / 1000.0;

        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-ss",
                &format!("{:.3}", sec),
                "-i",
                &video_path.to_string_lossy(),
                "-vf",
                "scale=-1:360",
                "-vframes",
                "1",
                "-q:v",
                "4",
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

    pub fn play_preview_audio(audio_path: &Path) -> Option<std::process::Child> {
        let player = if which_exists("mpv") {
            vec!["mpv", "--no-video", "--really-quiet", "--no-terminal"]
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
}

fn which_exists(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
