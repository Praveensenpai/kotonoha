use anyhow::{Context, Result};
use indicatif::ProgressBar;
use rayon::prelude::*;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::media::MediaExtractor;
use crate::srt::SubtitleSentence;

/// Detects the video framerate using ffprobe, defaulting to 23.976 on failure.
pub fn detect_video_fps(video_path: &Path) -> f64 {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=r_frame_rate",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            &video_path.to_string_lossy(),
        ])
        .output();

    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if let Some((num, den)) = text.split_once('/') {
            if let (Ok(n), Ok(d)) = (num.parse::<f64>(), den.parse::<f64>()) {
                if d > 0.0 {
                    return n / d;
                }
            }
        } else if let Ok(fps) = text.parse::<f64>() {
            if fps > 0.0 {
                return fps;
            }
        }
    }
    23.976
}

/// Attempts hardware-accelerated VAAPI keyframe extraction.
fn dump_keyframes_vaapi(video_path: &Path, temp_kf_dir: &Path) -> Result<()> {
    if !Path::new("/dev/dri/renderD128").exists() {
        anyhow::bail!("VAAPI device /dev/dri/renderD128 not available");
    }

    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-nostdin",
            "-noautorotate",
            "-hwaccel",
            "vaapi",
            "-hwaccel_output_format",
            "vaapi",
            "-vaapi_device",
            "/dev/dri/renderD128",
            "-skip_frame",
            "nokey",
            "-i",
            &video_path.to_string_lossy(),
            "-an",
            "-sn",
            "-vf",
            "scale_vaapi=w=-1:h=360,hwdownload,format=nv12",
            "-fps_mode",
            "vfr",
            "-frame_pts",
            "1",
            "-q:v",
            "4",
            &temp_kf_dir.join("%d.jpg").to_string_lossy(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("Failed to spawn ffmpeg for VAAPI keyframe extraction")?;

    if !status.success() {
        anyhow::bail!("ffmpeg VAAPI keyframe extraction failed");
    }
    Ok(())
}

/// Software single-pass keyframe extraction.
fn dump_keyframes_cpu(video_path: &Path, temp_kf_dir: &Path) -> Result<()> {
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-nostdin",
            "-noautorotate",
            "-skip_frame",
            "nokey",
            "-i",
            &video_path.to_string_lossy(),
            "-an",
            "-sn",
            "-vf",
            "scale=-1:360:flags=fast_bilinear",
            "-fps_mode",
            "vfr",
            "-frame_pts",
            "1",
            "-q:v",
            "4",
            &temp_kf_dir.join("%d.jpg").to_string_lossy(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("Failed to spawn ffmpeg for CPU keyframe extraction")?;

    if !status.success() {
        anyhow::bail!("ffmpeg CPU keyframe extraction failed");
    }
    Ok(())
}

/// Finds the closest keyframe to a given target frame index.
pub fn find_best_keyframe(target_frame: u64, kf_frames: &[u64]) -> u64 {
    if kf_frames.is_empty() {
        return target_frame;
    }
    match kf_frames.binary_search(&target_frame) {
        Ok(idx) => kf_frames[idx],
        Err(0) => kf_frames[0],
        Err(idx) => {
            let prev = kf_frames[idx - 1];
            if idx < kf_frames.len() {
                let next = kf_frames[idx];
                if target_frame - prev <= next - target_frame {
                    prev
                } else {
                    next
                }
            } else {
                prev
            }
        }
    }
}

/// Maps extracted keyframes to subtitle sentences and links them to the destination directory.
fn map_keyframes_to_sentences(
    sentences: &[SubtitleSentence],
    kf_dir: &Path,
    dest_dir: &Path,
    fps: f64,
    pb: &ProgressBar,
) -> Result<()> {
    let mut kf_frames = Vec::new();
    for entry in std::fs::read_dir(kf_dir)?.flatten() {
        let p = entry.path();
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            if let Ok(frame_num) = stem.parse::<u64>() {
                kf_frames.push(frame_num);
            }
        }
    }
    kf_frames.sort_unstable();

    if kf_frames.is_empty() {
        anyhow::bail!("No keyframes found in temporary extraction directory");
    }

    for s in sentences {
        let mid_ms = s.start_ms + (s.end_ms.saturating_sub(s.start_ms)) / 2;
        let sec = mid_ms as f64 / 1000.0;
        let target_frame = (sec * fps).round() as u64;
        let best_kf = find_best_keyframe(target_frame, &kf_frames);

        let src = kf_dir.join(format!("{best_kf}.jpg"));
        let dest = dest_dir.join(format!("{}.jpg", s.index));
        if std::fs::hard_link(&src, &dest).is_err() {
            let _ = std::fs::copy(&src, &dest);
        }
        pb.inc(1);
    }

    Ok(())
}

/// Fallback legacy extraction: spawns an ffmpeg process per sentence.
fn extract_legacy(
    video_path: &Path,
    sentences: &[SubtitleSentence],
    dest_dir: &Path,
    pb: &ProgressBar,
) {
    sentences.par_iter().for_each(|s| {
        let shot_path = dest_dir.join(format!("{}.jpg", s.index));
        let mid_ms = s.start_ms + (s.end_ms.saturating_sub(s.start_ms)) / 2;
        let _ = MediaExtractor::extract_screenshot(video_path, mid_ms, &shot_path);
        pb.inc(1);
    });
}

/// Extracts 360p screenshots for all sentences using single-pass streaming with automatic fallbacks.
pub fn extract_bundle_screenshots(
    video_path: &Path,
    sentences: &[SubtitleSentence],
    dest_dir: &Path,
    pb: &ProgressBar,
) -> Result<()> {
    let temp_kf_dir = std::env::temp_dir().join(format!(
        "kotonoha_kf_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&temp_kf_dir)?;

    let fps = detect_video_fps(video_path);

    // 1. Try single-pass VAAPI
    let vaapi_res = dump_keyframes_vaapi(video_path, &temp_kf_dir);
    if vaapi_res.is_ok()
        && map_keyframes_to_sentences(sentences, &temp_kf_dir, dest_dir, fps, pb).is_ok()
    {
        let _ = std::fs::remove_dir_all(&temp_kf_dir);
        return Ok(());
    }

    // 2. Clean up and try single-pass CPU
    let _ = std::fs::remove_dir_all(&temp_kf_dir);
    std::fs::create_dir_all(&temp_kf_dir)?;
    let cpu_res = dump_keyframes_cpu(video_path, &temp_kf_dir);
    if cpu_res.is_ok()
        && map_keyframes_to_sentences(sentences, &temp_kf_dir, dest_dir, fps, pb).is_ok()
    {
        let _ = std::fs::remove_dir_all(&temp_kf_dir);
        return Ok(());
    }

    // 3. Fallback to legacy per-frame extraction
    let _ = std::fs::remove_dir_all(&temp_kf_dir);
    extract_legacy(video_path, sentences, dest_dir, pb);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_best_keyframe() {
        let kfs = vec![0, 100, 300, 600, 1000];
        assert_eq!(find_best_keyframe(0, &kfs), 0);
        assert_eq!(find_best_keyframe(40, &kfs), 0);
        assert_eq!(find_best_keyframe(60, &kfs), 100);
        assert_eq!(find_best_keyframe(250, &kfs), 300);
        assert_eq!(find_best_keyframe(1200, &kfs), 1000);
    }
}
