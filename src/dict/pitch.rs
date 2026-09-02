pub fn split_morae(reading: &str) -> Vec<String> {
    let small_kana = [
        'ゃ', 'ゅ', 'ょ', 'ぁ', 'ぃ', 'ぅ', 'ぇ', 'ぉ', 'ャ', 'ュ', 'ョ', 'ァ', 'ィ', 'ゥ', 'ェ',
        'ォ',
    ];
    let mut morae = Vec::new();
    let chars: Vec<char> = reading.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && small_kana.contains(&chars[i + 1]) {
            morae.push(format!("{}{}", chars[i], chars[i + 1]));
            i += 2;
        } else {
            morae.push(chars[i].to_string());
            i += 1;
        }
    }
    morae
}

pub fn format_pitch_accent(reading: &str, pitch_num: usize) -> (String, String, usize) {
    let morae = split_morae(reading);
    let total_morae = morae.len();
    if total_morae == 0 {
        return (
            reading.to_string(),
            format!("[{}] H (0 morae)", pitch_num),
            0,
        );
    }

    let mut pattern = vec![0; total_morae];
    let k = pitch_num;

    if k == 1 {
        pattern[0] = 1;
    } else if k == 0 || k >= total_morae {
        for val in pattern.iter_mut().take(total_morae).skip(1) {
            *val = 1;
        }
    } else {
        for val in pattern.iter_mut().take(k.min(total_morae)).skip(1) {
            *val = 1;
        }
    }

    let mut hl_str = String::new();

    for (i, _) in morae.iter().enumerate() {
        let is_high = pattern[i] == 1;
        if is_high {
            hl_str.push('H');
        } else {
            hl_str.push('L');
        }
    }

    (
        reading.to_string(),
        format!("[{}] {} ({} morae)", k, hl_str, total_morae),
        total_morae,
    )
}
