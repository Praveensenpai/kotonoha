use anyhow::{Context, Result};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

use super::{get_bundles_cache_dir, BundleManifest, UnpackedBundle};

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
const ZIP_MAGIC: [u8; 2] = [0x50, 0x4b];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchiveFormat {
    TarZstd,
    Zip,
}

fn detect_archive_format(path: &Path) -> Result<ArchiveFormat> {
    let mut f =
        File::open(path).with_context(|| format!("Failed to open bundle: {}", path.display()))?;
    let mut header = [0u8; 4];
    let n = f.read(&mut header)?;
    if n >= 4 && header == ZSTD_MAGIC {
        Ok(ArchiveFormat::TarZstd)
    } else if n >= 2 && header[..2] == ZIP_MAGIC {
        Ok(ArchiveFormat::Zip)
    } else {
        anyhow::bail!("Unrecognized bundle archive format for {}", path.display());
    }
}

fn unpack_tar_zstd(koto_path: &Path, cache_dir: &Path) -> Result<()> {
    let file = File::open(koto_path)?;
    let decoder = zstd::Decoder::new(file)?;
    let mut tar_archive = tar::Archive::new(decoder);
    tar_archive.unpack(cache_dir)?;
    Ok(())
}

fn unpack_zip(koto_path: &Path, cache_dir: &Path) -> Result<()> {
    let file = File::open(koto_path)?;
    let mut archive = ZipArchive::new(file)?;
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
    Ok(())
}

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

        match detect_archive_format(koto_path)? {
            ArchiveFormat::TarZstd => unpack_tar_zstd(koto_path, &cache_dir)?,
            ArchiveFormat::Zip => unpack_zip(koto_path, &cache_dir)?,
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
    match detect_archive_format(koto_path)? {
        ArchiveFormat::TarZstd => {
            let file = File::open(koto_path)?;
            let decoder = zstd::Decoder::new(file)?;
            let mut tar_archive = tar::Archive::new(decoder);
            for entry in tar_archive.entries()? {
                let mut entry = entry?;
                if entry.path()?.ends_with("manifest.json") {
                    let mut contents = String::new();
                    entry.read_to_string(&mut contents)?;
                    let manifest: BundleManifest = serde_json::from_str(&contents)?;
                    return Ok(manifest);
                }
            }
            anyhow::bail!(
                "manifest.json not found in bundle archive: {}",
                koto_path.display()
            )
        }
        ArchiveFormat::Zip => {
            let file = File::open(koto_path)?;
            let mut archive = ZipArchive::new(file)?;
            let mut manifest_file = archive
                .by_name("manifest.json")
                .with_context(|| "manifest.json not found in bundle archive")?;
            let mut contents = String::new();
            manifest_file.read_to_string(&mut contents)?;
            let manifest: BundleManifest = serde_json::from_str(&contents)?;
            Ok(manifest)
        }
    }
}
