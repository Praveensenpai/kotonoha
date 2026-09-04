pub mod archive;
pub mod create;
pub mod destination;
pub mod fingerprint;
pub mod manage;
pub mod screenshots;
pub mod unpack;

#[cfg(test)]
mod tests;

pub use create::create_bundle;
pub use manage::*;
pub use unpack::{read_bundle_manifest, unpack_bundle};

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::db::Database;

/// Options passed to `create_bundle`.
#[derive(Debug, Clone)]
pub struct CreateBundleOptions<'a> {
    pub output_path: Option<&'a Path>,
    pub force: bool,
    pub db: Option<&'a Database>,
    pub storage_strategy: crate::config::BundleStorageStrategy,
    pub bundles_dir: &'a Path,
}

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
    #[serde(default)]
    pub video_fingerprint: Option<String>,
    #[serde(default)]
    pub subtitle_fingerprint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UnpackedBundle {
    pub subtitle_path: PathBuf,
    pub audio_path: PathBuf,
}

pub fn get_bundles_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".cache")
        })
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
