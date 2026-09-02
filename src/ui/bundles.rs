use anyhow::Result;
use console::style;
use inquire::{Confirm, MultiSelect, Select};
use std::path::{Path, PathBuf};

use crate::bundle::{
    delete_bundle_archive, delete_source_media_files, format_size,
    get_bundled_items_with_existing_sources, read_bundle_manifest,
};
use crate::db::Database;
use super::TerminalUi;

impl TerminalUi {
    pub async fn manage_bundles_interactive(db: &Database) -> Result<()> {
        loop {
            println!("\n{}", style("📦 Kotonoha Bundle Manager").bold().magenta());
            println!("{}", style("═".repeat(50)).dim());

            let options = vec![
                "1. 📋 List All Recorded Bundles",
                "2. 🧹 Clean / Remove Original Source Files (Reclaim Disk Space)",
                "3. 🗑️ Delete .koto Bundles",
                "4. 🔄 Prune Missing Bundles from Database",
                "5. 🔙 Back / Exit",
            ];

            let choice = match Select::new("Choose an action:", options).prompt() {
                Ok(c) => c,
                Err(_) => return Ok(()),
            };

            match choice {
                c if c.starts_with("1.") => {
                    Self::list_bundles_view(db).await?;
                }
                c if c.starts_with("2.") => {
                    Self::clean_bundled_sources_interactive(db).await?;
                }
                c if c.starts_with("3.") => {
                    Self::delete_bundles_interactive(db).await?;
                }
                c if c.starts_with("4.") => {
                    Self::prune_bundles_interactive(db).await?;
                }
                _ => return Ok(()),
            }
        }
    }

    pub async fn list_bundles_view(db: &Database) -> Result<()> {
        let mut records = db.get_all_bundled_media().await?;
        if records.is_empty() {
            println!("\n{}", style("ℹ No bundles recorded in the database yet.").yellow());
            return Ok(());
        }

        records.sort_by(|a, b| super::natural_cmp(&a.bundle_path, &b.bundle_path));

        println!("\n{}", style(format!("📋 Recorded Bundles ({})", records.len())).bold().cyan());
        println!("{}", style("─".repeat(80)).dim());

        for (idx, r) in records.iter().enumerate() {
            let bundle_path = PathBuf::from(&r.bundle_path);
            let exists = bundle_path.exists();
            let size_str = if exists {
                std::fs::metadata(&bundle_path)
                    .map(|m| format_size(m.len()))
                    .unwrap_or_else(|_| "Unknown size".to_string())
            } else {
                style("MISSING FROM DISK").red().to_string()
            };

            let status_badge = if exists {
                style("✔ OK").green()
            } else {
                style("✖ NOT FOUND").red()
            };

            println!(
                "{}. {} [{}] ({})",
                idx + 1,
                style(&r.bundle_path).bold(),
                status_badge,
                size_str
            );
            let bundle_dir = bundle_path.parent().unwrap_or_else(|| Path::new("."));
            let vid_path = PathBuf::from(&r.source_video);
            let vid_exists = (vid_path.is_absolute() && vid_path.exists())
                || bundle_dir.join(&vid_path).exists()
                || vid_path.file_name().map(|f| bundle_dir.join(f).exists()).unwrap_or(false);

            let sub_path = PathBuf::from(&r.source_subtitle);
            let sub_exists = (sub_path.is_absolute() && sub_path.exists())
                || bundle_dir.join(&sub_path).exists()
                || sub_path.file_name().map(|f| bundle_dir.join(f).exists()).unwrap_or(false);

            println!(
                "   📹 Source Video: {} {}",
                style(&r.source_video).dim(),
                if vid_exists { style("(on disk)").yellow() } else { style("(cleaned/missing)").dim() }
            );
            println!(
                "   📝 Source Sub:   {} {}",
                style(&r.source_subtitle).dim(),
                if sub_exists { style("(on disk)").yellow() } else { style("(cleaned/missing)").dim() }
            );
            if let Ok(manifest) = read_bundle_manifest(&bundle_path) {
                println!(
                    "   📊 Sentences: {} | Screenshots: {}",
                    style(manifest.sentence_count).cyan(),
                    if manifest.has_screenshots {
                        style("Yes").green()
                    } else {
                        style("No").dim()
                    }
                );
            }
            println!("   🕒 Created At:   {}", style(&r.created_at).dim());
            println!("{}", style("─".repeat(80)).dim());
        }

        Ok(())
    }

    pub async fn clean_bundled_sources_interactive(db: &Database) -> Result<()> {
        let cleanup_items = get_bundled_items_with_existing_sources(db).await?;

        if cleanup_items.is_empty() {
            println!(
                "\n{}",
                style("✔ No original source files to clean (all source files are already removed or missing).")
                    .green()
            );
            return Ok(());
        }

        println!("\n{}", style("🧹 Clean Original Source Files (Reclaim Space)").bold().yellow());
        println!(
            "{}",
            style("The following source videos and subtitles are safely bundled into standalone .koto files.")
                .dim()
        );
        println!(
            "{}",
            style("You can safely delete the original video/sub files because .koto has all required audio/subs.")
                .dim()
        );
        println!();

        struct DisplayWrapper(crate::bundle::BundledSourceCleanupItem);

        impl std::fmt::Display for DisplayWrapper {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0.display_name())
            }
        }

        let display_items: Vec<DisplayWrapper> = cleanup_items
            .into_iter()
            .map(DisplayWrapper)
            .collect();

        let selected = match MultiSelect::new(
            "Select source files to delete (Space to tick/untick, Enter to proceed):",
            display_items,
        )
        .with_formatter(&|opts| format!("{} item(s) selected", opts.len()))
        .prompt() {
            Ok(s) => s,
            Err(_) => {
                println!("Deletion cancelled.");
                return Ok(());
            }
        };

        if selected.is_empty() {
            println!("No files selected for deletion.");
            return Ok(());
        }

        let chosen_items: Vec<crate::bundle::BundledSourceCleanupItem> = selected
            .into_iter()
            .map(|w| w.0)
            .collect();

        let total_size: u64 = chosen_items.iter().map(|item| item.total_source_size()).sum();
        let total_size_str = format_size(total_size);

        println!(
            "\n⚠️  {} items selected. You will free approx {}.",
            style(chosen_items.len()).bold().red(),
            style(&total_size_str).bold().green()
        );

        let confirm = Confirm::new("Are you sure you want to move these original source files to the trash bin?")
            .with_default(false)
            .prompt()?;

        if confirm {
            let freed = delete_source_media_files(&chosen_items)?;
            println!(
                "{}",
                style(format!("✔ Successfully moved source files to trash! Freed {}.", format_size(freed)))
                    .bold()
                    .green()
            );
        } else {
            println!("Operation aborted.");
        }

        Ok(())
    }

    pub async fn delete_bundles_interactive(db: &Database) -> Result<()> {
        let mut records = db.get_all_bundled_media().await?;
        if records.is_empty() {
            println!("\n{}", style("ℹ No bundles to delete.").yellow());
            return Ok(());
        }

        records.sort_by(|a, b| super::natural_cmp(&a.bundle_path, &b.bundle_path));

        struct BundleRecordWrapper(crate::db::entities::bundled_media::Model);

        impl std::fmt::Display for BundleRecordWrapper {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let p = Path::new(&self.0.bundle_path);
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or(&self.0.bundle_path);
                let exists = p.exists();
                let size = if exists {
                    std::fs::metadata(p).map(|m| format!(" [{}]", format_size(m.len()))).unwrap_or_default()
                } else {
                    " [MISSING]".to_string()
                };
                write!(f, "{}{}", name, size)
            }
        }

        let display_records: Vec<BundleRecordWrapper> = records
            .into_iter()
            .map(BundleRecordWrapper)
            .collect();

        let selected = match MultiSelect::new(
            "Select .koto bundles to move to trash (Space to tick, Enter to confirm):",
            display_records,
        )
        .with_formatter(&|opts| format!("{} bundle(s) selected", opts.len()))
        .prompt() {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };

        if selected.is_empty() {
            println!("No bundles selected.");
            return Ok(());
        }

        let confirm = Confirm::new(&format!(
            "Are you sure you want to move {} bundle(s) to the trash bin?",
            selected.len()
        ))
        .with_default(false)
        .prompt()?;

        if confirm {
            for wrapper in selected {
                let path = PathBuf::from(&wrapper.0.bundle_path);
                delete_bundle_archive(&path, Some(db)).await?;
                println!(" 🗑️ Moved to trash: {}", path.display());
            }
            println!("{}", style("✔ Selected bundles moved to trash.").green());
        }

        Ok(())
    }

    pub async fn prune_bundles_interactive(db: &Database) -> Result<()> {
        let pruned = db.prune_missing_bundles().await?;
        if pruned > 0 {
            println!(
                "{}",
                style(format!("✔ Pruned {} missing bundle records from the database.", pruned))
                    .bold()
                    .green()
            );
        } else {
            println!("{}", style("✔ All database bundle records are up to date with disk.").green());
        }
        Ok(())
    }
}
