use super::*;
use std::collections::HashSet;
use std::fs::File;

#[test]
fn test_clean_old_media_removes_oldest_excess_cards() {
    let temp_dir = std::env::temp_dir().join(format!("kotonoha_test_clean_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create 4 card pairs (card1, card2, card3, card4) with distinct mtime delays
    for i in 1..=4 {
        let opus_path = temp_dir.join(format!("word_{}.opus", i));
        let jpg_path = temp_dir.join(format!("word_{}.jpg", i));
        File::create(&opus_path).unwrap();
        File::create(&jpg_path).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    let protected = HashSet::new();
    // Keep max 2 cards -> should delete card1 and card2 (4 files total)
    let deleted = MediaExtractor::clean_old_media(&temp_dir, 2, &protected).unwrap();
    assert_eq!(deleted, 4);

    assert!(!temp_dir.join("word_1.opus").exists());
    assert!(!temp_dir.join("word_1.jpg").exists());
    assert!(!temp_dir.join("word_2.opus").exists());
    assert!(!temp_dir.join("word_2.jpg").exists());
    assert!(temp_dir.join("word_3.opus").exists());
    assert!(temp_dir.join("word_3.jpg").exists());
    assert!(temp_dir.join("word_4.opus").exists());
    assert!(temp_dir.join("word_4.jpg").exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_clean_old_media_preserves_protected_unsynced_files() {
    let temp_dir = std::env::temp_dir().join(format!("kotonoha_test_prot_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create 3 card pairs
    for i in 1..=3 {
        let opus_path = temp_dir.join(format!("word_{}.opus", i));
        let jpg_path = temp_dir.join(format!("word_{}.jpg", i));
        File::create(&opus_path).unwrap();
        File::create(&jpg_path).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // Protect oldest card (word_1)
    let mut protected = HashSet::new();
    protected.insert(temp_dir.join("word_1.opus"));

    // max_cards = 1. Cleanable cards are word_2 and word_3.
    // word_2 should be deleted, word_3 kept, word_1 protected and kept.
    let deleted = MediaExtractor::clean_old_media(&temp_dir, 1, &protected).unwrap();
    assert_eq!(deleted, 2);

    assert!(temp_dir.join("word_1.opus").exists());
    assert!(temp_dir.join("word_1.jpg").exists());
    assert!(!temp_dir.join("word_2.opus").exists());
    assert!(!temp_dir.join("word_2.jpg").exists());
    assert!(temp_dir.join("word_3.opus").exists());
    assert!(temp_dir.join("word_3.jpg").exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_clean_old_media_noop_when_under_limit_or_zero() {
    let temp_dir = std::env::temp_dir().join(format!("kotonoha_test_noop_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let opus = temp_dir.join("word_1.opus");
    File::create(&opus).unwrap();

    let protected = HashSet::new();
    // max_cards = 0 -> no-op
    let deleted = MediaExtractor::clean_old_media(&temp_dir, 0, &protected).unwrap();
    assert_eq!(deleted, 0);
    assert!(opus.exists());

    // max_cards = 5 -> under limit, no-op
    let deleted = MediaExtractor::clean_old_media(&temp_dir, 5, &protected).unwrap();
    assert_eq!(deleted, 0);
    assert!(opus.exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_extract_clean_show_context() {
    use std::path::PathBuf;

    let p1 = PathBuf::from("/media/[SubsPlease] Sousou no Frieren - 08 (1080p) [B7A49C12].mkv");
    assert_eq!(
        extract_clean_show_context(&p1),
        "Sousou no Frieren - Episode 08"
    );

    let p2 = PathBuf::from("/anime/[Erai-raws] Bocchi the Rock! - 04 [1080p].mkv");
    assert_eq!(
        extract_clean_show_context(&p2),
        "Bocchi the Rock! - Episode 04"
    );

    let p3 = PathBuf::from("/tv/Spy.x.Family.S01E03.1080p.mp4");
    assert_eq!(extract_clean_show_context(&p3), "Spy x Family - Episode 03");

    let p4 = PathBuf::from("/anime/Dungeon_Meshi_EP12.mkv");
    assert_eq!(
        extract_clean_show_context(&p4),
        "Dungeon Meshi - Episode 12"
    );

    let p5 = PathBuf::from("/subtitles/yuru_camp.srt");
    assert_eq!(extract_clean_show_context(&p5), "yuru camp");
}
