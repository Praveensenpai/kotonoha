use anyhow::{Context, Result};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

use super::{get_bundles_cache_dir, BundleManifest, UnpackedBundle};

/// Unpack a .koto archive into the user cache directory if needed.
pub fn unpack_bundle(koto_path: &Path) -> Result<UnpackedBundle> {
    if !koto_path.exists() {
        anyhow::bail!("Bundle file not found: {}", koto_path.display());
    }

    let stem = koto_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("bundle");

    let koto_mtime = std::fs::metadata(koto_path)
        .and_then(|m| m.modified())
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

    let cache_dir = get_bundles_cache_dir().join(stem);
    let manifest_path = cache_dir.join("manifest.json");

    let needs_unpack = if manifest_path.exists() {
        let manifest_mtime = std::fs::metadata(&manifest_path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        koto_mtime > manifest_mtime
    } else {
        true
    };

    if needs_unpack {
        if cache_dir.exists() {
            let _ = std::fs::remove_dir_all(&cache_dir);
        }
        std::fs::create_dir_all(&cache_dir)?;

        let file = File::open(koto_path)
            .with_context(|| format!("Failed to open .koto bundle: {}", koto_path.display()))?;
        let mut archive = ZipArchive::new(file).with_context(|| {
            format!("Failed to read .koto zip archive: {}", koto_path.display())
        })?;

        for i in 0..archive.len() {
            let mut zip_file = archive.by_index(i)?;
            let outpath = match zip_file.enclosed_name() {
                Some(path) => cache_dir.join(path),
                None => continue,
            };

            if zip_file.is_dir() {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(p)?;
                    }
                }
                let mut outfile = File::create(&outpath)?;
                std::io::copy(&mut zip_file, &mut outfile)?;
            }
        }
    }

    let manifest_data = std::fs::read_to_string(&manifest_path).with_context(|| {
        format!(
            "Failed to read manifest.json in bundle cache: {}",
            manifest_path.display()
        )
    })?;
    let manifest: BundleManifest = serde_json::from_str(&manifest_data)
        .with_context(|| "Failed to parse bundle manifest.json")?;

    let subtitle_path = cache_dir.join(&manifest.subtitle_file);
    let audio_path = cache_dir.join(&manifest.audio_file);

    Ok(UnpackedBundle {
        subtitle_path,
        audio_path,
    })
}

/// Read manifest.json directly from a .koto archive without unpacking everything.
pub fn read_bundle_manifest(koto_path: &Path) -> Result<BundleManifest> {
    let file = File::open(koto_path)
        .with_context(|| format!("Failed to open bundle: {}", koto_path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("Failed to read bundle zip: {}", koto_path.display()))?;
    let mut manifest_file = archive
        .by_name("manifest.json")
        .with_context(|| "manifest.json not found in bundle archive")?;
    let mut contents = String::new();
    manifest_file.read_to_string(&mut contents)?;
    let manifest: BundleManifest = serde_json::from_str(&contents)?;
    Ok(manifest)
}
