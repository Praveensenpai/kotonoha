use anyhow::{bail, Context, Result};
use std::fs::File;
use std::io::Cursor;
use std::path::Path;

const SUDACHI_CORE_DICT_URL: &str =
    "https://d2ej7fkh96fzlu.cloudfront.net/sudachidict/sudachi-dictionary-latest-core.zip";

static DICT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Ensures the Sudachi Japanese system dictionary is available at the given path, downloading it if missing.
pub fn ensure_system_dict(dict_path: &Path) -> Result<()> {
    if dict_path.exists() {
        return Ok(());
    }

    let _lock = DICT_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("Dictionary lock poisoned"))?;
    if dict_path.exists() {
        return Ok(());
    }

    let target = dict_path.to_path_buf();
    std::thread::spawn(move || download_and_extract_dict(&target))
        .join()
        .map_err(|_| anyhow::anyhow!("Dictionary download worker thread panicked"))?
}

fn download_and_extract_dict(dict_path: &Path) -> Result<()> {
    eprintln!(" 📥 Downloading Sudachi Japanese dictionary (~72 MB)...");
    let response = reqwest::blocking::get(SUDACHI_CORE_DICT_URL)
        .context("Failed to connect to Sudachi dictionary CDN")?;
    let bytes = response
        .bytes()
        .context("Failed to download Sudachi dictionary archive")?;

    let reader = Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(reader).context("Failed to read dictionary zip archive")?;

    let tmp_path = dict_path.with_extension(format!("tmp.{}", std::process::id()));
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        if name.ends_with(".dic") {
            let mut out = File::create(&tmp_path)?;
            std::io::copy(&mut file, &mut out)?;
            std::fs::rename(&tmp_path, dict_path)?;
            eprintln!(" ✔ Installed Sudachi system dictionary!");
            return Ok(());
        }
    }

    if tmp_path.exists() {
        let _ = std::fs::remove_file(&tmp_path);
    }
    bail!("No .dic file found inside downloaded Sudachi dictionary archive");
}
