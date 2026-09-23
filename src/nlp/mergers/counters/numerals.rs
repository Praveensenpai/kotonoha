fn is_numeric_char(c: char) -> bool {
    matches!(
        c,
        '0'..='9'
            | '０'..='９'
            | '一'
            | '二'
            | '三'
            | '四'
            | '五'
            | '六'
            | '七'
            | '八'
            | '九'
            | '十'
            | '百'
            | '千'
            | '万'
            | '億'
            | '何'
            | '幾'
    )
}

pub fn is_numeric_token(surface: &str) -> bool {
    !surface.is_empty() && surface.chars().all(is_numeric_char)
}

fn int_to_kanji(n: usize) -> String {
    if n == 0 {
        return "〇".to_string();
    }
    const DIGITS: [&str; 10] = ["", "一", "二", "三", "四", "五", "六", "七", "八", "九"];
    if n < 10 {
        return DIGITS[n].to_string();
    }
    if n < 100 {
        let tens = n / 10;
        let rem = n % 10;
        let t_str = if tens == 1 {
            "十".to_string()
        } else {
            format!("{}十", DIGITS[tens])
        };
        let r_str = DIGITS[rem];
        return format!("{t_str}{r_str}");
    }
    let mut s = String::new();
    for c in n.to_string().chars() {
        match c {
            '0' => s.push('〇'),
            '1' => s.push('一'),
            '2' => s.push('二'),
            '3' => s.push('三'),
            '4' => s.push('四'),
            '5' => s.push('五'),
            '6' => s.push('六'),
            '7' => s.push('七'),
            '8' => s.push('八'),
            '9' => s.push('九'),
            _ => {}
        }
    }
    s
}

pub fn digits_to_kanji(s: &str) -> String {
    if let Ok(n) = s.parse::<usize>() {
        return int_to_kanji(n);
    }
    let mut ascii = String::new();
    for c in s.chars() {
        match c {
            '０'..='９' => ascii.push((c as u32 - '０' as u32 + '0' as u32) as u8 as char),
            _ => ascii.push(c),
        }
    }
    if let Ok(n) = ascii.parse::<usize>() {
        return int_to_kanji(n);
    }
    let mut result = String::new();
    for c in s.chars() {
        match c {
            '0' | '０' => result.push('〇'),
            '1' | '１' => result.push('一'),
            '2' | '２' => result.push('二'),
            '3' | '３' => result.push('三'),
            '4' | '４' => result.push('四'),
            '5' | '５' => result.push('五'),
            '6' | '６' => result.push('六'),
            '7' | '７' => result.push('七'),
            '8' | '８' => result.push('八'),
            '9' | '９' => result.push('九'),
            _ => result.push(c),
        }
    }
    result
}

pub fn is_lexical_counter_compound(kanji_num: &str, counter: &str) -> bool {
    if counter == "つ" {
        return true;
    }
    if counter == "人" {
        return matches!(kanji_num, "一" | "二" | "何");
    }
    if counter == "日" {
        return matches!(
            kanji_num,
            "一" | "二"
                | "三"
                | "四"
                | "五"
                | "六"
                | "七"
                | "八"
                | "九"
                | "十"
                | "十四"
                | "二十"
                | "二十四"
                | "何"
        );
    }
    if matches!(counter, "歳" | "才") && kanji_num == "二十" {
        return true;
    }
    kanji_num == "何"
}

fn derive_date_special_reading(kanji_num: &str) -> Option<&'static str> {
    match kanji_num {
        "一" => Some("ついたち"),
        "二" => Some("ふつか"),
        "三" => Some("みっか"),
        "四" => Some("よっか"),
        "五" => Some("いつか"),
        "六" => Some("むいか"),
        "七" => Some("なのか"),
        "八" => Some("ようか"),
        "九" => Some("ここのか"),
        "十" => Some("とおか"),
        "十四" => Some("じゅうよっか"),
        "二十" => Some("はつか"),
        "二十四" => Some("にじゅうよっか"),
        "何" => Some("なんにち"),
        _ => None,
    }
}

pub fn derive_special_reading(kanji_num: &str, counter: &str) -> Option<&'static str> {
    if counter == "人" {
        return match kanji_num {
            "一" => Some("ひとり"),
            "二" => Some("ふたり"),
            "何" => Some("なんにん"),
            _ => None,
        };
    }
    if counter == "つ" {
        return match kanji_num {
            "一" => Some("ひとつ"),
            "二" => Some("ふたつ"),
            "三" => Some("みっつ"),
            "四" => Some("よっつ"),
            "五" => Some("いつつ"),
            "六" => Some("むっつ"),
            "七" => Some("ななつ"),
            "八" => Some("やっつ"),
            "九" => Some("ここのつ"),
            "幾" => Some("いくつ"),
            _ => None,
        };
    }
    if counter == "日" {
        return derive_date_special_reading(kanji_num);
    }
    if matches!(counter, "歳" | "才") && kanji_num == "二十" {
        return Some("はたち");
    }
    if kanji_num == "何" {
        return match counter {
            "時" => Some("なんじ"),
            "分" => Some("なんぷん"),
            "回" => Some("なんかい"),
            "年" => Some("なんねん"),
            "月" => Some("なんがつ"),
            "歳" | "才" => Some("なんさい"),
            _ => None,
        };
    }
    None
}

pub fn derive_numeric_reading(surface: &str, raw_reading: &str) -> String {
    let has_non_digit = raw_reading
        .chars()
        .any(|c| !matches!(c, '0'..='9' | '０'..='９'));
    if has_non_digit {
        return raw_reading.to_string();
    }
    let mut derived = String::new();
    for c in surface.chars() {
        let kana = match c {
            '0' | '０' => "ぜろ",
            '1' | '１' => "いち",
            '2' | '２' => "に",
            '3' | '３' => "さん",
            '4' | '４' => "よん",
            '5' | '５' => "ご",
            '6' | '６' => "ろく",
            '7' | '７' => "なな",
            '8' | '８' => "はち",
            '9' | '９' => "きゅう",
            _ => "",
        };
        if !kana.is_empty() {
            derived.push_str(kana);
        } else {
            derived.push(c);
        }
    }
    if derived.is_empty() {
        raw_reading.to_string()
    } else {
        derived
    }
}
