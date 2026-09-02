use super::*;
use std::io::Write;

#[test]
fn test_bundle_manifest_serialization() {
    let manifest = BundleManifest {
        version: 1,
        source_video: "Test Episode 01.mkv".to_string(),
        source_subtitle: "Test Episode 01.ja.srt".to_string(),
        created_at: "2026-09-02T16:00:00Z".to_string(),
        audio_file: "audio.opus".to_string(),
        subtitle_file: "subtitles.srt".to_string(),
        sentence_count: 42,
        has_screenshots: true,
        video_fingerprint: Some("test_vid_fp".to_string()),
        subtitle_fingerprint: Some("test_sub_fp".to_string()),
    };

    let serialized = serde_json::to_string(&manifest).expect("serialize");
    let deserialized: BundleManifest = serde_json::from_str(&serialized).expect("deserialize");

    assert_eq!(deserialized.version, 1);
    assert_eq!(deserialized.source_video, "Test Episode 01.mkv");
    assert_eq!(deserialized.sentence_count, 42);
    assert!(deserialized.has_screenshots);
}

#[test]
fn test_is_bundle_file() {
    assert!(is_bundle_file(Path::new("episode_01.koto")));
    assert!(is_bundle_file(Path::new("/path/to/my_show.KOTO")));
    assert!(!is_bundle_file(Path::new("episode_01.mkv")));
    assert!(!is_bundle_file(Path::new("episode_01.srt")));
}

#[test]
fn test_bundle_archive_unpacking() {
    let temp_dir = std::env::temp_dir().join(format!("koto_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");

    let koto_path = temp_dir.join("test_sample.koto");
    {
        let file = File::create(&koto_path).expect("create koto");
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();

        // Write manifest.json
        zip.start_file("manifest.json", options).unwrap();
        let manifest = BundleManifest {
            version: 1,
            source_video: "sample.mkv".to_string(),
            source_subtitle: "sample.srt".to_string(),
            created_at: "2026-09-02T16:00:00Z".to_string(),
            audio_file: "audio.opus".to_string(),
            subtitle_file: "subtitles.srt".to_string(),
            sentence_count: 2,
            has_screenshots: true,
            video_fingerprint: Some("1234_abcd".to_string()),
            subtitle_fingerprint: Some("sub_5678".to_string()),
        };
        zip.write_all(serde_json::to_string(&manifest).unwrap().as_bytes()).unwrap();

        // Write subtitle
        zip.start_file("subtitles.srt", options).unwrap();
        zip.write_all(b"1\n00:00:01,000 --> 00:00:03,000\n\xE3\x81\x93\xE3\x82\x93\xE3\x81\xAB\xE3\x81\xA1\xE3\x81\xAF\n").unwrap();

        // Write audio dummy
        zip.start_file("audio.opus", options).unwrap();
        zip.write_all(b"DUMMY_OPUS_DATA").unwrap();

        // Write screenshots/0.jpg
        zip.start_file("screenshots/0.jpg", options).unwrap();
        zip.write_all(b"DUMMY_JPG").unwrap();

        zip.finish().unwrap();
    }

    let unpacked = unpack_bundle(&koto_path).expect("unpack bundle");
    assert_eq!(unpacked.manifest.sentence_count, 2);
    assert_eq!(unpacked.manifest.video_fingerprint.as_deref(), Some("1234_abcd"));
    assert_eq!(unpacked.manifest.subtitle_fingerprint.as_deref(), Some("sub_5678"));
    assert!(unpacked.root_dir.exists());
    assert!(unpacked.subtitle_path.exists());
    assert!(unpacked.audio_path.exists());
    assert!(unpacked.screenshots_dir.join("0.jpg").exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_fingerprint_generation() {
    let temp_dir = std::env::temp_dir().join(format!("koto_fp_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let sub_path = temp_dir.join("test.srt");
    std::fs::write(&sub_path, "1\n00:00:01,000 --> 00:00:02,000\nテスト\n").unwrap();

    let vid_path = temp_dir.join("test.mkv");
    std::fs::write(&vid_path, vec![0xAB; 100 * 1024]).unwrap();

    let sub_fp = compute_subtitle_fingerprint(&sub_path).expect("sub fp");
    let vid_fp = compute_video_fingerprint(&vid_path).expect("vid fp");

    assert!(!sub_fp.is_empty());
    assert!(vid_fp.starts_with("102400_"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_bundle_management_and_cleanup() {
    let temp_dir = std::env::temp_dir().join(format!("koto_manage_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let db_path = temp_dir.join("test.db");
    let db = crate::db::Database::open(&db_path).await.expect("open db");

    let bundle_file = temp_dir.join("Episode 01.koto");
    std::fs::write(&bundle_file, b"DUMMY_KOTO").unwrap();

    let vid_file = temp_dir.join("Episode 01.mkv");
    std::fs::write(&vid_file, vec![0x12; 50 * 1024]).unwrap();

    let sub_file = temp_dir.join("Episode 01.ja.srt");
    std::fs::write(&sub_file, b"DUMMY_SRT").unwrap();

    db.record_bundle(
        &bundle_file,
        &vid_file.to_string_lossy(),
        &sub_file.to_string_lossy(),
        "fp_vid",
        "fp_sub",
    )
    .await
    .expect("record bundle");

    let items = get_bundled_items_with_existing_sources(&db)
        .await
        .expect("get cleanup items");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].video_size, 50 * 1024);
    assert!(items[0].video_exists);
    assert!(items[0].subtitle_exists);

    // Delete source files
    let freed = delete_source_media_files(&items).expect("delete source files");
    assert!(freed >= 50 * 1024);
    assert!(!vid_file.exists());
    assert!(!sub_file.exists());
    assert!(bundle_file.exists());

    // After deletion, cleanup items list should be empty
    let items_after = get_bundled_items_with_existing_sources(&db)
        .await
        .expect("get cleanup items after");
    assert_eq!(items_after.len(), 0);

    // Prune test
    std::fs::remove_file(&bundle_file).unwrap();
    let pruned = db.prune_missing_bundles().await.expect("prune");
    assert_eq!(pruned, 1);

    let _ = std::fs::remove_dir_all(&temp_dir);
}
