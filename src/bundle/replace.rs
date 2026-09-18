use anyhow::{Context, Result};
use console::style;
use std::fs::File;
use std::path::{Path, PathBuf};

use super::archive::package_bundle_archive;
use super::fingerprint::compute_subtitle_fingerprint;
use super::{get_bundles_cache_dir, BundleManifest};
use crate::srt::parse_subtitle;

struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn unpack_bundle_to_dir(bundle_path: &Path, dest_dir: &Path) -> Result<()> {
    let file = File::open(bundle_path)
        .with_context(|| format!("Failed to open bundle: {}", bundle_path.display()))?;
    let decoder = zstd::Decoder::new(file)
        .with_context(|| "Failed to initialize zstd decoder for bundle archive")?;
    let mut tar_archive = tar::Archive::new(decoder);
    tar_archive
        .unpack(dest_dir)
        .with_context(|| "Failed to unpack existing bundle files")?;
    Ok(())
}

fn update_manifest(
    manifest_path: &Path,
    new_sub_path: &Path,
    sentence_count: usize,
    new_fp: &str,
) -> Result<String> {
    let manifest_data = std::fs::read_to_string(manifest_path)
        .with_context(|| format!("Failed to read manifest at {}", manifest_path.display()))?;
    let mut manifest: BundleManifest = serde_json::from_str(&manifest_data)
        .with_context(|| "Failed to parse manifest.json inside bundle")?;

    let sub_filename = manifest.subtitle_file.clone();
    manifest.sentence_count = sentence_count;
    manifest.subtitle_fingerprint = Some(new_fp.to_string());
    if let Some(name) = new_sub_path.file_name().and_then(|s| s.to_str()) {
        manifest.source_subtitle = name.to_string();
    }

    let updated_json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(manifest_path, updated_json)?;
    Ok(sub_filename)
}

/// Replace the subtitle file inside an existing .koto bundle archive.
pub async fn replace_bundle_subtitle(bundle_path: &Path, new_sub_path: &Path) -> Result<()> {
    if !bundle_path.exists() {
        anyhow::bail!("Bundle file not found: {}", bundle_path.display());
    }
    if !new_sub_path.exists() {
        anyhow::bail!("New subtitle file not found: {}", new_sub_path.display());
    }

    // 1. Verify and parse new subtitle
    let sentences = parse_subtitle(new_sub_path)
        .with_context(|| format!("Failed to parse new subtitle: {}", new_sub_path.display()))?;
    if sentences.is_empty() {
        anyhow::bail!("New subtitle file contains 0 valid sentences.");
    }
    let sentence_count = sentences.len();
    let new_fp = compute_subtitle_fingerprint(new_sub_path)?;

    // Check for duplicate subtitle against other bundles
    let current_manifest = super::unpack::read_bundle_manifest(bundle_path).ok();
    let target_video_name = current_manifest
        .as_ref()
        .map(|m| m.source_video.as_str())
        .unwrap_or("target video");
    let video_fp = current_manifest
        .as_ref()
        .and_then(|m| m.video_fingerprint.as_deref())
        .unwrap_or("");

    let mut search_dirs = vec![get_bundles_cache_dir()];
    if let Some(parent) = bundle_path.parent() {
        if !search_dirs.contains(&parent.to_path_buf()) {
            search_dirs.push(parent.to_path_buf());
        }
    }

    if let Some(dup) = super::duplicate_guard::check_duplicate_subtitle(
        target_video_name,
        video_fp,
        &new_fp,
        None,
        &search_dirs,
    )
    .await
    {
        let canonical_bundle = bundle_path
            .canonicalize()
            .unwrap_or_else(|_| bundle_path.to_path_buf());
        let canonical_dup = Path::new(&dup.existing_bundle)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(&dup.existing_bundle));

        if canonical_bundle != canonical_dup {
            let proceed = super::duplicate_guard::prompt_duplicate_subtitle_warning(
                &dup,
                target_video_name,
                &new_fp,
                false,
            )?;
            if !proceed {
                anyhow::bail!("Subtitle replacement canceled due to duplicate subtitle warning.");
            }
        }
    }

    // 2. Prepare isolated temporary workspace
    let temp_root = get_bundles_cache_dir().join(format!(".replace_{}", std::process::id()));
    if temp_root.exists() {
        let _ = std::fs::remove_dir_all(&temp_root);
    }
    std::fs::create_dir_all(&temp_root)?;
    let _guard = TempDirGuard(temp_root.clone());

    // 3. Unpack existing bundle
    unpack_bundle_to_dir(bundle_path, &temp_root)?;

    // 4. Update manifest and locate target subtitle destination
    let manifest_path = temp_root.join("manifest.json");
    if !manifest_path.exists() {
        anyhow::bail!(
            "Invalid bundle archive: manifest.json not found in {}",
            bundle_path.display()
        );
    }
    let sub_dest_name = update_manifest(&manifest_path, new_sub_path, sentence_count, &new_fp)?;

    // 5. Overwrite internal subtitle file
    let target_sub_path = temp_root.join(sub_dest_name);
    std::fs::copy(new_sub_path, &target_sub_path)
        .with_context(|| "Failed to copy new subtitle into staging directory")?;

    // 6. Repackage into a temporary archive alongside target
    let temp_koto_path = bundle_path.with_extension("koto.tmp");
    package_bundle_archive(&temp_root, &temp_koto_path)?;

    // 7. Atomically replace original bundle
    std::fs::rename(&temp_koto_path, bundle_path)
        .with_context(|| "Failed to atomically overwrite original .koto archive")?;

    // 8. Invalidate unpacked cache folder
    if let Some(stem) = bundle_path.file_stem().and_then(|s| s.to_str()) {
        let cached_dir = get_bundles_cache_dir().join(stem);
        if cached_dir.exists() {
            let _ = std::fs::remove_dir_all(&cached_dir);
        }
    }

    let bundle_display = bundle_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("bundle.koto");
    let sub_display = new_sub_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("subtitles.srt");

    println!(
        "\n {} Subtitle replaced in [{}]:",
        style("✨").green().bold(),
        style(bundle_display).cyan().bold()
    );
    println!(
        "   📄 New Subtitle:   {}",
        style(sub_display).yellow().bold()
    );
    println!(
        "   🔢 Sentence Count: {} lines",
        style(sentence_count).green().bold()
    );
    println!(
        "   🏷️  Fingerprint:    {}\n",
        style(&new_fp[..8.min(new_fp.len())]).dim()
    );

    Ok(())
}
