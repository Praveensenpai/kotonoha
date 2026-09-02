mod pairing;
pub use pairing::*;

use anyhow::Result;
use std::path::PathBuf;

use crate::anki;
use crate::config::AppConfig;
use crate::db::Database;
use crate::nlp::JapaneseTokenizer;
use crate::srt::parse_subtitle;
use crate::ui::TerminalUi;

/// Dispatches command-line flags and returns Ok(true) if handled, or Ok(false) to continue.
pub async fn handle_cli_flag(arg: &str) -> Result<bool> {
    if arg == "--version" || arg == "-v" || arg == "-V" {
        println!("kotonoha {}", env!("CARGO_PKG_VERSION"));
        return Ok(true);
    }
    if arg == "--help" || arg == "-h" || arg == "--h" {
        println!(
            "🌸 kotonoha {} — Japanese i+1 Sentence Miner",
            env!("CARGO_PKG_VERSION")
        );
        println!("\nUSAGE:");
        println!("  kotonoha                           Launch interactive TUI file picker");
        println!("  kotonoha <MEDIA_FILE>              Parse specific subtitle/video/koto file");
        println!("  kotonoha --bundle         | -b     Pre-save video into lightweight .koto package (~18MB)");
        println!("  kotonoha --bundles        | -B     View, inspect and manage saved .koto bundles");
        println!("  kotonoha --clean-bundled  | -C     Select & remove original source files of bundled media");
        println!("  kotonoha --config         | -c     Interactive TUI configuration manager");
        println!("  kotonoha --show-config    | -S     Display active configuration settings");
        println!("  kotonoha --inspect [FILE] | -i     Inspect sentences (Space plays selected audio; ★=i+1)");
        println!("  kotonoha --manage-known   | -k     View & remove words from the known database");
        println!("  kotonoha --manage-mined   | -m     View & remove words from the mined list");
        println!("  kotonoha --manage-ignored | -I     View & remove words from the ignore list");
        println!("  kotonoha --sync           | -s     Push locally mined cards to Anki");
        println!("  kotonoha --version        | -v     Print version information");
        println!("  kotonoha --help           | -h     Show help information");
        println!("  Flags:   --force          | -f     Force re-bundling or overwriting");
        return Ok(true);
    }
    if arg == "--bundle" || arg == "-b" || arg == "bundle" || arg == "--presave" {
        let cfg = AppConfig::load().unwrap_or_default();
        let db = Database::open(&cfg.db_path).await.ok();

        let raw_args: Vec<String> = std::env::args().skip(2).collect();
        let force = raw_args.iter().any(|a| a == "--force" || a == "-f");
        let non_flag_args: Vec<PathBuf> = raw_args
            .into_iter()
            .filter(|a| a != "--force" && a != "-f")
            .map(PathBuf::from)
            .collect();

        let direct_arg = non_flag_args.first().cloned();
        let custom_out = non_flag_args.get(1).cloned();

        if let Some(input_path) = direct_arg {
            let (sub_path, vid_path) = find_paired_media_for_bundling(&input_path)?;
            crate::bundle::create_bundle(
                &vid_path,
                &sub_path,
                custom_out.as_deref(),
                force,
                db.as_ref(),
            )
            .await?;
        } else {
            let selected_files = TerminalUi::select_bundle_source_files()?;
            let total = selected_files.len();
            for (idx, input_path) in selected_files.iter().enumerate() {
                if total > 1 {
                    println!(
                        "\n--- [Batch Item {}/{}] Processing: {} ---",
                        idx + 1,
                        total,
                        input_path.display()
                    );
                }
                match find_paired_media_for_bundling(input_path) {
                    Ok((sub_path, vid_path)) => {
                        let _ = crate::bundle::create_bundle(
                            &vid_path,
                            &sub_path,
                            None,
                            force,
                            db.as_ref(),
                        )
                        .await?;
                    }
                    Err(e) => {
                        eprintln!(" ✖ Skipping {}: {}", input_path.display(), e);
                    }
                }
            }
        }
        return Ok(true);
    }
    if arg == "--bundles" || arg == "-B" || arg == "--manage-bundles" || arg == "bundles" {
        let cfg = AppConfig::load().unwrap_or_default();
        let db = Database::open(&cfg.db_path).await?;
        TerminalUi::manage_bundles_interactive(&db).await?;
        return Ok(true);
    }
    if arg == "--clean-bundled" || arg == "-C" || arg == "--clean-sources" || arg == "--clean" {
        let cfg = AppConfig::load().unwrap_or_default();
        let db = Database::open(&cfg.db_path).await?;
        TerminalUi::clean_bundled_sources_interactive(&db).await?;
        return Ok(true);
    }
    if arg == "--config" || arg == "-c" {
        let mut cfg = AppConfig::load()?;
        TerminalUi::configure_interactive(&mut cfg)?;
        return Ok(true);
    }
    if arg == "--show-config" || arg == "-S" {
        let cfg = AppConfig::load()?;
        TerminalUi::show_config(&cfg);
        return Ok(true);
    }
    if arg == "--sync" || arg == "-s" {
        let cfg = AppConfig::load()?;
        let db = Database::open(&cfg.db_path).await?;
        anki::sync_to_anki(&cfg, &db).await?;
        return Ok(true);
    }
    if arg == "--inspect" || arg == "-i" {
        let cfg = AppConfig::load()?;
        let db = Database::open(&cfg.db_path).await?;
        let input_path = match std::env::args().nth(2) {
            Some(p) => PathBuf::from(p),
            None => TerminalUi::select_media_file()?,
        };
        let (subtitle_path, video_path) = match find_paired_media(&input_path) {
            Ok((sub, vid)) => (sub, Some(vid)),
            Err(_) => {
                let ext = input_path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let sub = if ext == "srt" || ext == "ass" {
                    input_path
                } else {
                    let srt = input_path.with_extension("ja.srt");
                    if srt.exists() {
                        srt
                    } else {
                        input_path
                    }
                };
                (sub, None)
            }
        };
        let sentences = parse_subtitle(&subtitle_path)?;
        let tokenizer = JapaneseTokenizer::new()?;
        let known_words = db.get_known_words().await?;
        let ignored_words = db.get_ignored_words().await?;
        TerminalUi::inspect_sentences(crate::ui::InspectSentencesParams {
            sentences: &sentences,
            tokenizer: &tokenizer,
            known_words: &known_words,
            ignored_words: &ignored_words,
            video_path: video_path.as_deref(),
        })?;
        return Ok(true);
    }
    if arg == "--manage-ignored" || arg == "-I" || arg == "--ignored" {
        let cfg = AppConfig::load()?;
        let db = Database::open(&cfg.db_path).await?;
        let tokenizer = JapaneseTokenizer::new()?;
        let words = words_with_readings(&tokenizer, db.get_ignored_words_sorted().await?);
        let to_remove = TerminalUi::manage_ignored_words(&words)?;
        if !to_remove.is_empty() {
            let count = db.remove_ignored_words(&to_remove).await?;
            println!(" ✔ Removed {} word(s) from the ignore list.", count);
        } else {
            println!(" ℹ No changes made.");
        }
        return Ok(true);
    }
    if arg == "--manage-known" || arg == "-k" || arg == "--known" {
        let cfg = AppConfig::load()?;
        let db = Database::open(&cfg.db_path).await?;
        let tokenizer = JapaneseTokenizer::new()?;
        let words = words_with_readings(&tokenizer, db.get_known_words_sorted_by_source("known").await?);
        let to_remove = TerminalUi::manage_known_words(&words)?;
        if !to_remove.is_empty() {
            let count = db.remove_known_words(&to_remove).await?;
            println!(" ✔ Removed {} word(s) from your known list.", count);
        } else {
            println!(" ℹ No changes made.");
        }
        return Ok(true);
    }
    if arg == "--manage-mined" || arg == "-m" || arg == "--mined" {
        let cfg = AppConfig::load()?;
        let db = Database::open(&cfg.db_path).await?;
        let tokenizer = JapaneseTokenizer::new()?;
        let words = words_with_readings(&tokenizer, db.get_known_words_sorted_by_source("mined").await?);
        let to_remove = TerminalUi::manage_mined_words(&words)?;
        if !to_remove.is_empty() {
            let count = db.remove_known_words(&to_remove).await?;
            println!(" ✔ Removed {} word(s) from your mined list.", count);
        } else {
            println!(" ℹ No changes made.");
        }
        return Ok(true);
    }

    Ok(false)
}
