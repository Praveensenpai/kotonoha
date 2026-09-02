use super::helpers::natural_cmp;
use super::picker::is_hidden_or_ignored_entry;
use walkdir::WalkDir;

#[test]
fn sorts_episode_numbers_naturally() {
    let mut episodes = [
        "episode-10.mkv",
        "episode-08.mkv",
        "episode-01.mkv",
        "episode-2.mkv",
    ];
    episodes.sort_by(|left, right| natural_cmp(left, right));
    assert_eq!(
        episodes,
        [
            "episode-01.mkv",
            "episode-2.mkv",
            "episode-08.mkv",
            "episode-10.mkv"
        ]
    );
}

#[test]
fn filters_hidden_and_system_directories() {
    let temp = std::env::temp_dir().join(format!("koto_filter_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(temp.join(".cache/nested")).unwrap();
    std::fs::create_dir_all(temp.join(".cargo/registry")).unwrap();
    std::fs::create_dir_all(temp.join("node_modules/pkg")).unwrap();
    std::fs::create_dir_all(temp.join("target/debug")).unwrap();
    std::fs::create_dir_all(temp.join("Videos/Anime")).unwrap();

    std::fs::write(temp.join(".cache/bad.srt"), "dummy").unwrap();
    std::fs::write(temp.join(".cargo/bad.srt"), "dummy").unwrap();
    std::fs::write(temp.join("node_modules/bad.mp4"), "dummy").unwrap();
    std::fs::write(temp.join("target/bad.mkv"), "dummy").unwrap();
    std::fs::write(temp.join("Videos/Anime/good.mkv"), "dummy").unwrap();

    let mut found_files = Vec::new();
    for entry in WalkDir::new(&temp)
        .into_iter()
        .filter_entry(is_hidden_or_ignored_entry)
        .filter_map(|e| e.ok())
    {
        if entry.path().is_file() {
            found_files.push(entry.file_name().to_string_lossy().to_string());
        }
    }

    assert_eq!(found_files, vec!["good.mkv"]);

    let _ = std::fs::remove_dir_all(&temp);
}

