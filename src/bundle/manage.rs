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
        let bundle_name = self
            .bundle_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown Bundle");

        let mut parts = Vec::new();
        if self.video_exists {
            let vid_name = self
                .source_video
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            parts.push(format!("video: {} ({})", vid_name, format_size(self.video_size)));
        }
        if self.subtitle_exists {
            let sub_name = self
                .source_subtitle
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            parts.push(format!("sub: {} ({})", sub_name, format_size(self.subtitle_size)));
        }

        format!("📦 {} [{}]", bundle_name, parts.join(", "))
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

        let source_video = PathBuf::from(&record.source_video);
        let source_subtitle = PathBuf::from(&record.source_subtitle);

        let video_exists = source_video.exists();
        let subtitle_exists = source_subtitle.exists();

        // If neither source exists on disk, nothing to clean
        if !video_exists && !subtitle_exists {
            continue;
        }

        let video_size = if video_exists {
            std::fs::metadata(&source_video).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        let subtitle_size = if subtitle_exists {
            std::fs::metadata(&source_subtitle).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        items.push(BundledSourceCleanupItem {
            bundle_path,
            source_video,
            source_subtitle,
            video_size,
            subtitle_size,
            video_exists,
            subtitle_exists,
        });
    }

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
