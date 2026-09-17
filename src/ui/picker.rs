use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use inquire::Text;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub mod render;
pub mod selector;
pub mod state;

pub use selector::run_file_selector;
pub use state::{SelectableFile, SubtitleStatus};

use super::helpers::natural_cmp;

pub fn is_hidden_or_ignored_entry(entry: &walkdir::DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    // Skip hidden dot-files and dot-directories except .koto bundle folders
    if entry.depth() > 0 && name.starts_with('.') && name != ".koto" {
        return false;
    }
    // Skip common large non-media build/cache directories
    if matches!(
        name.as_ref(),
        "node_modules"
            | "target"
            | "venv"
            | ".venv"
            | "env"
            | "collection.media"
            | "__pycache__"
            | "vendor"
    ) {
        return false;
    }
    true
}

fn resolve_search_dirs(home: &Path) -> Vec<PathBuf> {
    let mut search_dirs = vec![PathBuf::from(".")];
    for sub in &["Videos", "Downloads", "Anime"] {
        let dir = home.join(sub);
        if dir.exists() && !search_dirs.contains(&dir) {
            search_dirs.push(dir);
        }
    }
    search_dirs
}

fn discover_central_bundles(allowed_exts: &[&str]) -> Vec<PathBuf> {
    if !allowed_exts.contains(&"koto") {
        return Vec::new();
    }
    let cfg = crate::config::AppConfig::load().unwrap_or_default();
    if !cfg.bundles_dir.exists() {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(&cfg.bundles_dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && crate::bundle::is_bundle_file(p))
        .collect()
}

pub fn discover_media_files(allowed_exts: &[&str], spinner_msg: &str) -> Result<Vec<PathBuf>> {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template(" {spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message(spinner_msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let is_cwd_home = std::env::current_dir()
        .map(|cwd| cwd == home)
        .unwrap_or(false);
    let search_dirs = resolve_search_dirs(&home);
    let mut files = Vec::new();

    for dir in search_dirs {
        if !dir.exists() {
            continue;
        }

        let max_depth = if dir == Path::new(".") && is_cwd_home {
            2
        } else {
            6
        };

        for entry in WalkDir::new(&dir)
            .max_depth(max_depth)
            .into_iter()
            .filter_entry(is_hidden_or_ignored_entry)
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.is_file() {
                if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                    let ext = ext.to_lowercase();
                    if allowed_exts.contains(&ext.as_str()) {
                        files.push(p.to_path_buf());
                    }
                }
            }
        }
    }

    files.extend(discover_central_bundles(allowed_exts));
    pb.finish_and_clear();

    files.sort_by(|left, right| natural_cmp(&left.to_string_lossy(), &right.to_string_lossy()));
    files.dedup();

    Ok(files)
}

pub fn select_media_file() -> Result<PathBuf> {
    let files = discover_media_files(
        &["srt", "ass", "vtt", "mkv", "mp4", "webm", "koto"],
        "Scanning for media and subtitle files...",
    )?;

    if files.is_empty() {
        let input = Text::new("No media files auto-discovered. Enter file path:").prompt()?;
        return Ok(PathBuf::from(input));
    }

    let items: Vec<SelectableFile> = files.into_iter().filter_map(classify_media_file).collect();

    let mut selected = run_file_selector("Select Subtitle or Anime Video File", items, false)?;
    if let Some(path) = selected.pop() {
        Ok(path)
    } else {
        anyhow::bail!("No file selected.");
    }
}

pub fn select_media_files() -> Result<Vec<PathBuf>> {
    let files = discover_media_files(
        &["srt", "ass", "vtt", "mkv", "mp4", "webm", "koto"],
        "Scanning for media and subtitle files...",
    )?;

    if files.is_empty() {
        let input = Text::new("No media files auto-discovered. Enter file path:").prompt()?;
        return Ok(vec![PathBuf::from(input)]);
    }

    let items: Vec<SelectableFile> = files.into_iter().filter_map(classify_media_file).collect();

    run_file_selector("Select Subtitle, Video, or Bundle File(s)", items, true)
}

pub fn select_bundle_source_files() -> Result<Vec<PathBuf>> {
    let files = discover_media_files(
        &["srt", "ass", "vtt", "mkv", "mp4", "webm", "avi"],
        "Scanning for unbundled video and subtitle files...",
    )?;

    if files.is_empty() {
        let input = Text::new("No unbundled media files discovered. Enter file path:").prompt()?;
        return Ok(vec![PathBuf::from(input)]);
    }

    let items: Vec<SelectableFile> = files.into_iter().map(classify_bundle_source_file).collect();

    run_file_selector(
        "📦 Select Video or Subtitle File(s) to Bundle into .koto",
        items,
        true,
    )
}

fn classify_media_file(path: PathBuf) -> Option<SelectableFile> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "koto" {
        return Some(SelectableFile {
            path,
            status: SubtitleStatus::Bundle,
        });
    }

    if matches!(ext.as_str(), "mkv" | "mp4" | "webm") {
        let status = if crate::commands::find_paired_subtitle_for_video(&path).is_some() {
            SubtitleStatus::HasSub
        } else {
            SubtitleStatus::NoSub
        };
        return Some(SelectableFile { path, status });
    }

    if matches!(ext.as_str(), "srt" | "ass" | "vtt") {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let clean_stem = stem
            .trim_end_matches(".ja")
            .trim_end_matches(".jp")
            .trim_end_matches(".ja-JP")
            .trim_end_matches(".japanese")
            .trim_end_matches(".en");

        let has_video = std::fs::read_dir(parent).ok().is_some_and(|entries| {
            entries.flatten().any(|e| {
                let p = e.path();
                let p_ext = p
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if matches!(p_ext.as_str(), "mkv" | "mp4" | "webm") {
                    let p_stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    p_stem == stem
                        || p_stem == clean_stem
                        || p_stem.starts_with(clean_stem)
                        || clean_stem.starts_with(p_stem)
                } else {
                    false
                }
            })
        });

        if !has_video {
            return Some(SelectableFile {
                path,
                status: SubtitleStatus::HasSub,
            });
        }
    }

    None
}

fn classify_bundle_source_file(path: PathBuf) -> SelectableFile {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    if matches!(ext.as_str(), "mkv" | "mp4" | "webm" | "avi") {
        let status = if crate::commands::find_paired_subtitle_for_video(&path).is_some() {
            SubtitleStatus::HasSub
        } else {
            SubtitleStatus::NoSub
        };
        SelectableFile { path, status }
    } else {
        SelectableFile {
            path,
            status: SubtitleStatus::HasSub,
        }
    }
}
