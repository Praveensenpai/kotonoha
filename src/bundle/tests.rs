use super::fingerprint::*;
use super::*;
use std::fs::File;
use std::io::Write;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

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
        zip.write_all(serde_json::to_string(&manifest).unwrap().as_bytes())
            .unwrap();

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

    let manifest = read_bundle_manifest(&koto_path).expect("read manifest");
    assert_eq!(manifest.sentence_count, 2);
    assert_eq!(manifest.video_fingerprint.as_deref(), Some("1234_abcd"));
    assert_eq!(manifest.subtitle_fingerprint.as_deref(), Some("sub_5678"));

    let unpacked = unpack_bundle(&koto_path).expect("unpack bundle");
    assert!(unpacked.subtitle_path.exists());
    assert!(unpacked.audio_path.exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_bundle_tar_zstd_packaging_and_unpacking() {
    use super::archive::package_bundle_archive;

    let temp_dir = std::env::temp_dir().join(format!("koto_zstd_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");

    let bundle_content_dir = temp_dir.join("content");
    std::fs::create_dir_all(bundle_content_dir.join("screenshots")).unwrap();

    let manifest = BundleManifest {
        version: 1,
        source_video: "sample_zstd.mkv".to_string(),
        source_subtitle: "sample_zstd.srt".to_string(),
        created_at: "2026-09-05T00:00:00Z".to_string(),
        audio_file: "audio.opus".to_string(),
        subtitle_file: "subtitles.srt".to_string(),
        sentence_count: 5,
        has_screenshots: true,
        video_fingerprint: Some("vid_fp_zstd".to_string()),
        subtitle_fingerprint: Some("sub_fp_zstd".to_string()),
    };
    std::fs::write(
        bundle_content_dir.join("manifest.json"),
        serde_json::to_string(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(
        bundle_content_dir.join("subtitles.srt"),
        "1\n00:00:01,000 --> 00:00:02,000\nテスト\n",
    )
    .unwrap();
    std::fs::write(bundle_content_dir.join("audio.opus"), b"DUMMY_OPUS_AUDIO").unwrap();
    std::fs::write(
        bundle_content_dir.join("screenshots").join("1.jpg"),
        b"DUMMY_SHOT_1",
    )
    .unwrap();

    let koto_path = temp_dir.join("output.koto");
    package_bundle_archive(&bundle_content_dir, &koto_path).expect("package tar.zst bundle");
    assert!(koto_path.exists());

    let read_manifest = read_bundle_manifest(&koto_path).expect("read manifest from tar.zst");
    assert_eq!(read_manifest.source_video, "sample_zstd.mkv");
    assert_eq!(read_manifest.sentence_count, 5);

    let unpacked = unpack_bundle(&koto_path).expect("unpack tar.zst bundle");
    assert!(unpacked.subtitle_path.exists());
    assert!(unpacked.audio_path.exists());

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

#[tokio::test]
async fn test_replace_bundle_subtitle() {
    let temp_dir = std::env::temp_dir().join(format!("koto_replace_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");

    let staging_dir = temp_dir.join("staging");
    std::fs::create_dir_all(&staging_dir).unwrap();

    let manifest = BundleManifest {
        version: 1,
        source_video: "test.mkv".to_string(),
        source_subtitle: "test.srt".to_string(),
        created_at: "2026-09-02T16:00:00Z".to_string(),
        audio_file: "audio.opus".to_string(),
        subtitle_file: "subtitles.srt".to_string(),
        sentence_count: 1,
        has_screenshots: false,
        video_fingerprint: Some("vid_fp".to_string()),
        subtitle_fingerprint: Some("sub_fp".to_string()),
    };
    std::fs::write(
        staging_dir.join("manifest.json"),
        serde_json::to_string(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(
        staging_dir.join("subtitles.srt"),
        "1\n00:00:01,000 --> 00:00:02,000\nOld subtitle\n",
    )
    .unwrap();
    std::fs::write(staging_dir.join("audio.opus"), b"OPUS_AUDIO").unwrap();

    let bundle_file = temp_dir.join("test_replace.koto");
    archive::package_bundle_archive(&staging_dir, &bundle_file).expect("package test bundle");

    let new_sub_file = temp_dir.join("new_subtitle.ja.srt");
    std::fs::write(
        &new_sub_file,
        "1\n00:00:01,000 --> 00:00:02,000\nNew subtitle 1\n\n2\n00:00:03,000 --> 00:00:04,000\nNew subtitle 2\n",
    )
    .unwrap();

    replace_bundle_subtitle(&bundle_file, &new_sub_file)
        .await
        .expect("replace subtitle");

    let updated_manifest = read_bundle_manifest(&bundle_file).expect("read manifest");
    assert_eq!(updated_manifest.sentence_count, 2);
    assert_eq!(updated_manifest.source_subtitle, "new_subtitle.ja.srt");

    let unpacked = unpack_bundle(&bundle_file).expect("unpack bundle");
    let content = std::fs::read_to_string(&unpacked.subtitle_path).expect("read unpacked sub");
    assert!(content.contains("New subtitle 2"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_duplicate_subtitle_guard() {
    let temp_dir = std::env::temp_dir().join(format!("koto_dup_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");

    let sub_fp = "shared_sub_fp_12345";
    let manifest1 = BundleManifest {
        version: 1,
        source_video: "Episode 01.mkv".to_string(),
        source_subtitle: "Episode 01.srt".to_string(),
        created_at: "2026-09-02T16:00:00Z".to_string(),
        audio_file: "audio.opus".to_string(),
        subtitle_file: "subtitles.srt".to_string(),
        sentence_count: 100,
        has_screenshots: false,
        video_fingerprint: Some("vid_fp_ep01".to_string()),
        subtitle_fingerprint: Some(sub_fp.to_string()),
    };

    let staging_dir = temp_dir.join("staging1");
    std::fs::create_dir_all(&staging_dir).unwrap();
    std::fs::write(
        staging_dir.join("manifest.json"),
        serde_json::to_string(&manifest1).unwrap(),
    )
    .unwrap();
    std::fs::write(staging_dir.join("subtitles.srt"), "dummy").unwrap();
    std::fs::write(staging_dir.join("audio.opus"), "dummy").unwrap();

    let bundle1 = temp_dir.join("Episode 01.koto");
    archive::package_bundle_archive(&staging_dir, &bundle1).unwrap();

    // Now check if bundling Episode 02 with the same sub_fp detects the duplicate!
    let dup = duplicate_guard::check_duplicate_subtitle(
        "Episode 02.mkv",
        "vid_fp_ep02",
        sub_fp,
        None,
        std::slice::from_ref(&temp_dir),
    )
    .await;

    assert!(dup.is_some());
    let match_dup = dup.unwrap();
    assert_eq!(match_dup.existing_video, "Episode 01.mkv");

    // But bundling Episode 01 again with the SAME video does NOT trigger duplicate
    let no_dup = duplicate_guard::check_duplicate_subtitle(
        "Episode 01.mkv",
        "vid_fp_ep01",
        sub_fp,
        None,
        std::slice::from_ref(&temp_dir),
    )
    .await;
    assert!(no_dup.is_none());

    let _ = std::fs::remove_dir_all(&temp_dir);
}
