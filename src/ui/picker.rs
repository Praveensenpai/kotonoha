use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use inquire::{MultiSelect, Select, Text};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

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

    // 3. Central bundles directory if configured and exists
    let cfg = crate::config::AppConfig::load().unwrap_or_default();
    if cfg.bundles_dir.exists() && !search_dirs.contains(&cfg.bundles_dir) {
        search_dirs.push(cfg.bundles_dir);
    }

    let is_cwd_home = std::env::current_dir()
        .map(|cwd| cwd == home)
        .unwrap_or(false);
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

#[derive(Debug, Clone)]
struct MediaEntry {
    path: PathBuf,
    is_bundle: bool,
}

impl std::fmt::Display for MediaEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_bundle {
            write!(f, "📦 [BUNDLE] {}", self.path.display())
        } else {
            write!(f, "{}", self.path.display())
        }
    }
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

    let items: Vec<MediaEntry> = files
        .into_iter()
        .map(|p| {
            let is_bundle = crate::bundle::is_bundle_file(&p);
            MediaEntry { path: p, is_bundle }
        })
        .collect();
    let selected = Select::new("Select Subtitle or Anime Video File:", items).prompt()?;
    Ok(selected.path)
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
