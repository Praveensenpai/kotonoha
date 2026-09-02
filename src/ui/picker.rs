use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use inquire::{MultiSelect, Select, Text};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use super::helpers::natural_cmp;

pub fn is_hidden_or_ignored_entry(entry: &walkdir::DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    // Skip hidden dot-files and dot-directories (e.g. .cache, .config, .cargo, .local, .git)
    if entry.depth() > 0 && name.starts_with('.') {
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
    let mut search_dirs = Vec::new();

    // 1. Current working directory
    search_dirs.push(PathBuf::from("."));

    // 2. Standard user media folders if they exist
    let videos = home.join("Videos");
    if videos.exists() && !search_dirs.contains(&videos) {
        search_dirs.push(videos);
    }
    let downloads = home.join("Downloads");
    if downloads.exists() && !search_dirs.contains(&downloads) {
        search_dirs.push(downloads);
    }
    let anime = home.join("Anime");
    if anime.exists() && !search_dirs.contains(&anime) {
        search_dirs.push(anime);
    }

    let is_cwd_home = std::env::current_dir().map(|cwd| cwd == home).unwrap_or(false);
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

    let items: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
    let selected = Select::new("Select Subtitle or Anime Video File:", items).prompt()?;
    Ok(PathBuf::from(selected))
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

    let items: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
    let selected = MultiSelect::new(
        "📦 Select Video or Subtitle File(s) to Bundle into .koto (Space to select, Enter to bundle):",
        items,
    )
    .with_page_size(15)
    .prompt()?;

    if selected.is_empty() {
        anyhow::bail!("No files selected for bundling.");
    }

    Ok(selected.into_iter().map(PathBuf::from).collect())
}
