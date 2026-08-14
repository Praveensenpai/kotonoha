use super::natural_cmp;

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
