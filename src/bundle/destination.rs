use anyhow::{Context, Result};
use console::style;
use std::path::{Path, PathBuf};

use crate::config::BundleStorageStrategy;

/// Checks whether a given directory exists and is writable.
pub fn is_directory_writable(dir: &Path) -> bool {
    if !dir.exists() {
        if let Some(parent) = dir.parent() {
            return is_directory_writable(parent);
        }
        return false;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let probe_path = dir.join(format!(".koto_probe_{}_{}", std::process::id(), nanos));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe_path)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(probe_path);
            true
        }
        Err(_) => false,
    }
}

/// Resolves the destination `.koto` file path based on storage strategy.
/// If the target directory is not writable, gracefully falls back to `central_dir`.
pub fn resolve_bundle_destination(
    source_path: &Path,
    clean_stem: &str,
    strategy: BundleStorageStrategy,
    central_dir: &Path,
) -> Result<PathBuf> {
    let parent = source_path.parent().unwrap_or_else(|| Path::new("."));

    match strategy {
        BundleStorageStrategy::Colocated => {
            if is_directory_writable(parent) {
                Ok(parent.join(format!("{clean_stem}.koto")))
            } else {
                println!(
                    " ⚠️  {} ({})\n    {} {}",
                    style(
                        "Source directory is read-only; falling back to central bundles directory:"
                    )
                    .yellow()
                    .bold(),
                    style(parent.display()).dim(),
                    style("Saving to:").cyan(),
                    style(central_dir.display()).cyan().bold()
                );
                std::fs::create_dir_all(central_dir).with_context(|| {
                    format!(
                        "Failed to create bundles directory: {}",
                        central_dir.display()
                    )
                })?;
                Ok(central_dir.join(format!("{clean_stem}.koto")))
            }
        }
        BundleStorageStrategy::Subfolder => {
            let sub_dir = parent.join(".koto");
            if is_directory_writable(parent) {
                std::fs::create_dir_all(&sub_dir).with_context(|| {
                    format!("Failed to create .koto subfolder: {}", sub_dir.display())
                })?;
                Ok(sub_dir.join(format!("{clean_stem}.koto")))
            } else {
                println!(
                    " ⚠️  {} ({})\n    {} {}",
                    style(
                        "Source directory is read-only; falling back to central bundles directory:"
                    )
                    .yellow()
                    .bold(),
                    style(parent.display()).dim(),
                    style("Saving to:").cyan(),
                    style(central_dir.display()).cyan().bold()
                );
                std::fs::create_dir_all(central_dir).with_context(|| {
                    format!(
                        "Failed to create bundles directory: {}",
                        central_dir.display()
                    )
                })?;
                Ok(central_dir.join(format!("{clean_stem}.koto")))
            }
        }
        BundleStorageStrategy::Central => {
            std::fs::create_dir_all(central_dir).with_context(|| {
                format!(
                    "Failed to create central bundles directory: {}",
                    central_dir.display()
                )
            })?;
            Ok(central_dir.join(format!("{clean_stem}.koto")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_bundle_destination_colocated() {
        let temp_dir = std::env::temp_dir().join(format!("koto_dest_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let source = temp_dir.join("Episode 01.mkv");
        let central = temp_dir.join("central_bundles");

        let dest = resolve_bundle_destination(
            &source,
            "Episode 01",
            BundleStorageStrategy::Colocated,
            &central,
        )
        .unwrap();

        assert_eq!(dest, temp_dir.join("Episode 01.koto"));
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_resolve_bundle_destination_central() {
        let temp_dir =
            std::env::temp_dir().join(format!("koto_dest_central_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let source = temp_dir.join("Episode 01.mkv");
        let central = temp_dir.join("central_bundles");

        let dest = resolve_bundle_destination(
            &source,
            "Episode 01",
            BundleStorageStrategy::Central,
            &central,
        )
        .unwrap();

        assert_eq!(dest, central.join("Episode 01.koto"));
        assert!(central.exists());
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_resolve_bundle_destination_subfolder() {
        let temp_dir = std::env::temp_dir().join(format!("koto_dest_sub_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let source = temp_dir.join("Episode 01.mkv");
        let central = temp_dir.join("central_bundles");

        let dest = resolve_bundle_destination(
            &source,
            "Episode 01",
            BundleStorageStrategy::Subfolder,
            &central,
        )
        .unwrap();

        assert_eq!(dest, temp_dir.join(".koto").join("Episode 01.koto"));
        assert!(temp_dir.join(".koto").exists());
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
