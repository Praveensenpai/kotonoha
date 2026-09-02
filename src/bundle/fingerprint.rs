use anyhow::{Context, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Compute MD5 fingerprint of a subtitle file.
pub fn compute_subtitle_fingerprint(subtitle_path: &Path) -> Result<String> {
    let content = std::fs::read(subtitle_path)
        .with_context(|| format!("Failed to read subtitle for fingerprinting: {}", subtitle_path.display()))?;
    let digest = md5::compute(&content);
    Ok(format!("{:x}", digest))
}

/// Compute fast multi-chunk sampled fingerprint for large video files.
pub fn compute_video_fingerprint(video_path: &Path) -> Result<String> {
    let metadata = std::fs::metadata(video_path)
        .with_context(|| format!("Failed to stat video for fingerprinting: {}", video_path.display()))?;
    let file_len = metadata.len();

    let mut file = File::open(video_path)
        .with_context(|| format!("Failed to open video for fingerprinting: {}", video_path.display()))?;

    // Fast block sampling: 64KB from start, 64KB from middle, 64KB from end
    const CHUNK_SIZE: usize = 64 * 1024;
    let mut sample_bytes = Vec::with_capacity(CHUNK_SIZE * 3 + 16);
    sample_bytes.extend_from_slice(&file_len.to_le_bytes());

    let mut head = vec![0u8; CHUNK_SIZE.min(file_len as usize)];
    if let Ok(n) = file.read(&mut head) {
        sample_bytes.extend_from_slice(&head[..n]);
    }

    if file_len > (CHUNK_SIZE * 2) as u64 {
        let mid_offset = (file_len / 2).saturating_sub((CHUNK_SIZE / 2) as u64);
        if file.seek(SeekFrom::Start(mid_offset)).is_ok() {
            let mut mid = vec![0u8; CHUNK_SIZE];
            if let Ok(n) = file.read(&mut mid) {
                sample_bytes.extend_from_slice(&mid[..n]);
            }
        }

        let tail_offset = file_len.saturating_sub(CHUNK_SIZE as u64);
        if file.seek(SeekFrom::Start(tail_offset)).is_ok() {
            let mut tail = vec![0u8; CHUNK_SIZE];
            if let Ok(n) = file.read(&mut tail) {
                sample_bytes.extend_from_slice(&tail[..n]);
            }
        }
    }

    let digest = md5::compute(&sample_bytes);
    Ok(format!("{}_{:x}", file_len, digest))
}
