use super::*;
use std::fs;

#[test]
fn test_read_queue_nonexistent() {
    let tmp_dir = std::env::temp_dir().join("kotonoha_test_queue_nonexistent");
    let _ = fs::remove_dir_all(&tmp_dir);
    let queue_path = tmp_dir.join("queue.jsonl");

    let entries = read_queue(&queue_path).unwrap();
    assert!(entries.is_empty());
}

#[test]
fn test_append_and_read_queue() {
    let tmp_dir = std::env::temp_dir().join("kotonoha_test_queue_append");
    let _ = fs::remove_dir_all(&tmp_dir);
    let queue_path = tmp_dir.join("queue.jsonl");

    let entry1 = QueueEntry {
        video_path: PathBuf::from("/anime/ep01.mkv"),
        timestamp_ms: 12500,
    };
    let entry2 = QueueEntry {
        video_path: PathBuf::from("/anime/ep02.mkv"),
        timestamp_ms: 45000,
    };

    append_entry(&queue_path, &entry1).unwrap();
    append_entry(&queue_path, &entry2).unwrap();

    let entries = read_queue(&queue_path).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0], entry1);
    assert_eq!(entries[1], entry2);

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_remove_entries() {
    let tmp_dir = std::env::temp_dir().join("kotonoha_test_queue_remove");
    let _ = fs::remove_dir_all(&tmp_dir);
    let queue_path = tmp_dir.join("queue.jsonl");

    let entry1 = QueueEntry {
        video_path: PathBuf::from("/anime/ep01.mkv"),
        timestamp_ms: 10000,
    };
    let entry2 = QueueEntry {
        video_path: PathBuf::from("/anime/ep01.mkv"),
        timestamp_ms: 20000,
    };
    let entry3 = QueueEntry {
        video_path: PathBuf::from("/anime/ep01.mkv"),
        timestamp_ms: 30000,
    };

    append_entry(&queue_path, &entry1).unwrap();
    append_entry(&queue_path, &entry2).unwrap();
    append_entry(&queue_path, &entry3).unwrap();

    // Remove index 1 (entry2)
    remove_entries(&queue_path, &[1]).unwrap();

    let entries = read_queue(&queue_path).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0], entry1);
    assert_eq!(entries[1], entry3);

    // Remove all remaining
    remove_entries(&queue_path, &[0, 1]).unwrap();
    let entries_empty = read_queue(&queue_path).unwrap();
    assert!(entries_empty.is_empty());

    let _ = fs::remove_dir_all(&tmp_dir);
}
