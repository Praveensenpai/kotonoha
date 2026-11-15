use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueEntry {
    pub video_path: PathBuf,
    pub timestamp_ms: u64,
}

pub fn queue_file_path() -> PathBuf {
    crate::config::default_data_dir().join("queue.jsonl")
}

pub fn read_queue(path: &Path) -> Result<Vec<QueueEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)?;
    let mut entries = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<QueueEntry>(trimmed) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

#[cfg(test)]
pub fn append_entry(path: &Path, entry: &QueueEntry) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let line = serde_json::to_string(entry)?;
    let mut current = if path.exists() {
        std::fs::read_to_string(path)?
    } else {
        String::new()
    };
    if !current.is_empty() && !current.ends_with('\n') {
        current.push('\n');
    }
    current.push_str(&line);
    current.push('\n');
    std::fs::write(path, current)?;
    Ok(())
}

pub const MPV_LUA_SCRIPT: &str = include_str!("../scripts/kotonoha-queue.lua");

pub fn ensure_mpv_companion_script_installed() -> Result<()> {
    let script_path = dirs::config_dir()
        .map(|p| p.join("mpv/scripts/kotonoha-queue.lua"))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config/mpv/scripts/kotonoha-queue.lua")
        });

    if !script_path.exists() {
        if let Some(parent) = script_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&script_path, MPV_LUA_SCRIPT);
    }
    Ok(())
}

pub fn remove_entries(path: &Path, indices_to_remove: &[usize]) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let entries = read_queue(path)?;
    let remaining: Vec<&QueueEntry> = entries
        .iter()
        .enumerate()
        .filter(|(idx, _)| !indices_to_remove.contains(idx))
        .map(|(_, e)| e)
        .collect();

    if remaining.is_empty() {
        std::fs::write(path, "")?;
        return Ok(());
    }

    let mut out = String::new();
    for entry in remaining {
        let line = serde_json::to_string(entry)?;
        out.push_str(&line);
        out.push('\n');
    }
    std::fs::write(path, out)?;
    Ok(())
}
