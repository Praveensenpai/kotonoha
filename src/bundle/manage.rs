use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::db::Database;

#[derive(Debug, Clone)]
pub struct BundledSourceCleanupItem {
    pub bundle_path: PathBuf,
    pub source_video: PathBuf,
    pub source_subtitle: PathBuf,
    pub video_size: u64,
    pub subtitle_size: u64,
    pub video_exists: bool,
    pub subtitle_exists: bool,
}

impl BundledSourceCleanupItem {
    pub fn total_source_size(&self) -> u64 {
        let mut total = 0;
        if self.video_exists {
            total += self.video_size;
        }
        if self.subtitle_exists {
            total += self.subtitle_size;
        }
        total
    }

    pub fn display_name(&self) -> String {
        let stem = self
            .bundle_path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or_else(|| {
                self.bundle_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown")
            });

        let total = format_size(self.total_source_size());

        let types = match (self.video_exists, self.subtitle_exists) {
            (true, true) => {
                let vid_ext = self
                    .source_video
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("video");
                let sub_ext = if self.source_subtitle.to_string_lossy().ends_with(".ja.srt") {
                    "ja.srt"
                } else {
                    self.source_subtitle
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("sub")
                };
                format!(".{} + .{}", vid_ext, sub_ext)
            }
            (true, false) => {
                let vid_ext = self
                    .source_video
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("video");
                format!(".{}", vid_ext)
            }
            (false, true) => {
                let sub_ext = if self.source_subtitle.to_string_lossy().ends_with(".ja.srt") {
                    "ja.srt"
                } else {
                    self.source_subtitle
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("sub")
                };
                format!(".{}", sub_ext)
            }
            (false, false) => String::new(),
        };

        if types.is_empty() {
            format!("{}   [{}]", stem, total)
        } else {
            format!("{}   [{}] ({})", stem, total, types)
        }
    }
}

pub fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

pub async fn get_bundled_items_with_existing_sources(
    db: &Database,
) -> Result<Vec<BundledSourceCleanupItem>> {
    let records = db.get_all_bundled_media().await?;
    let mut items = Vec::new();

    for record in records {
        let bundle_path = PathBuf::from(&record.bundle_path);
        // Only consider valid bundles that exist on disk
        if !bundle_path.exists() {
            continue;
        }

        let bundle_dir = bundle_path.parent().unwrap_or_else(|| Path::new("."));

        // Resolve video path: 1) absolute/literal path, 2) bundle_dir.join, 3) bundle_path.with_file_name
        let raw_video = PathBuf::from(&record.source_video);
        let resolved_video = if raw_video.is_absolute() && raw_video.exists() {
            raw_video
        } else if bundle_dir.join(&raw_video).exists() {
            bundle_dir.join(&raw_video)
        } else if let Some(file_name) = raw_video.file_name() {
            let candidate = bundle_dir.join(file_name);
            if candidate.exists() {
                candidate
            } else {
                raw_video
            }
        } else {
            raw_video
        };

        // Resolve subtitle path: 1) absolute/literal path, 2) bundle_dir.join, 3) bundle_path.with_file_name
        let raw_sub = PathBuf::from(&record.source_subtitle);
        let resolved_sub = if raw_sub.is_absolute() && raw_sub.exists() {
            raw_sub
        } else if bundle_dir.join(&raw_sub).exists() {
            bundle_dir.join(&raw_sub)
        } else if let Some(file_name) = raw_sub.file_name() {
            let candidate = bundle_dir.join(file_name);
            if candidate.exists() {
                candidate
            } else {
                raw_sub
            }
        } else {
            raw_sub
        };

        let video_exists = resolved_video.exists();
        let subtitle_exists = resolved_sub.exists();

        // If neither source exists on disk, nothing to clean
        if !video_exists && !subtitle_exists {
            continue;
        }

        let video_size = if video_exists {
            std::fs::metadata(&resolved_video).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        let subtitle_size = if subtitle_exists {
            std::fs::metadata(&resolved_sub).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        items.push(BundledSourceCleanupItem {
            bundle_path,
            source_video: resolved_video,
            source_subtitle: resolved_sub,
            video_size,
            subtitle_size,
            video_exists,
            subtitle_exists,
        });
    }

    items.sort_by(|a, b| {
        crate::ui::natural_cmp(
            &a.bundle_path.to_string_lossy(),
            &b.bundle_path.to_string_lossy(),
        )
    });

    Ok(items)
}

pub fn delete_source_media_files(items: &[BundledSourceCleanupItem]) -> Result<u64> {
    let mut total_freed: u64 = 0;

    for item in items {
        if item.video_exists && item.source_video.exists() {
            if let Ok(meta) = std::fs::metadata(&item.source_video) {
                total_freed += meta.len();
            }
            let _ = std::fs::remove_file(&item.source_video);
        }
        if item.subtitle_exists && item.source_subtitle.exists() {
            if let Ok(meta) = std::fs::metadata(&item.source_subtitle) {
                total_freed += meta.len();
            }
            let _ = std::fs::remove_file(&item.source_subtitle);
        }
    }

    Ok(total_freed)
}

pub async fn delete_bundle_archive(bundle_path: &Path, db: Option<&Database>) -> Result<()> {
    if bundle_path.exists() {
        std::fs::remove_file(bundle_path)
            .with_context(|| format!("Failed to delete bundle file: {}", bundle_path.display()))?;
    }

    if let Some(database) = db {
        let _ = database
            .delete_bundled_media_by_path(&bundle_path.to_string_lossy())
            .await;
    }

    Ok(())
}
