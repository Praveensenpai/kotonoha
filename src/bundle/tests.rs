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
    assert!(unpacked.subtitle_path.exists());
    assert!(unpacked.audio_path.exists());
    assert!(unpacked.screenshots_dir.join("0.jpg").exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
}
