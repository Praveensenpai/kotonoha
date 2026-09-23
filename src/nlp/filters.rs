pub struct MorphemeMeta<'a> {
    pub pos_category: &'a str,
    pub pos_sub: &'a str,
    pub dictionary_form: &'a str,
    pub surface: &'a str,
    pub is_subsidiary_verb: bool,
}

pub fn is_formal_noun(dict_form: &str) -> bool {
    matches!(
        dict_form,
        "こと" | "もの" | "やつ" | "ため" | "ところ" | "わけ" | "はず" | "つもり"
    )
}

pub fn is_conjunction_particle(dict_form: &str) -> bool {
    matches!(
        dict_form,
        "だって"
            | "だけど"
            | "だから"
            | "なのに"
            | "けれど"
            | "けれども"
            | "でも"
            | "しかし"
            | "ただし"
            | "なお"
            | "ちなみに"
            | "および"
            | "ならびに"
    )
}

pub fn is_audio_grunt(dict_form: &str, surface: &str) -> bool {
    let is_laughter = matches!(
        dict_form,
        "ヒヒ"
            | "狒々"
            | "ハハ"
            | "イヒヒ"
            | "ニヒヒ"
            | "ヘヘ"
            | "ホホ"
            | "ウフフ"
            | "アハハ"
            | "エヘヘ"
            | "ひひ"
            | "はは"
            | "いひひ"
            | "にひひ"
            | "へへ"
            | "ほほ"
    ) || matches!(
        surface,
        "ヒヒ"
            | "狒々"
            | "ハハ"
            | "イヒヒ"
            | "ニヒヒ"
            | "ヘヘ"
            | "ホホ"
            | "ウフフ"
            | "アハハ"
            | "エヘヘ"
            | "ひひ"
            | "はは"
            | "いひひ"
            | "にひひ"
            | "へへ"
            | "ほほ"
            | "そっ"
            | "じゃっ"
            | "ジャッ"
    );

    is_laughter
        || matches!(
            dict_form,
            "おっ"
                | "あっ"
                | "えっ"
                | "うっ"
                | "はっ"
                | "ふっ"
                | "んっ"
                | "くっ"
                | "ちっ"
                | "つっ"
                | "そっ"
                | "じゃっ"
                | "ジャッ"
                | "オッ"
                | "アッ"
                | "エッ"
                | "ウッ"
                | "ハッ"
                | "フッ"
                | "ンッ"
                | "クッ"
                | "チッ"
        )
}

pub fn is_symbol_or_junk(meta: &MorphemeMeta<'_>) -> bool {
    let is_grunt = is_audio_grunt(meta.dictionary_form, meta.surface);
    let is_junk_pos = matches!(
        meta.pos_category,
        "記号" | "補助記号" | "感動詞" | "助詞" | "助動詞" | "数詞" | "空白"
    );
    let is_junk_sub = matches!(meta.pos_sub, "数詞" | "接尾");
    let is_common_junk_lemma = matches!(
        meta.dictionary_form,
        "…" | "？"
            | "！"
            | "♪"
            | "―"
            | "ー"
            | "、"
            | "。"
            | "～"
            | "する"
            | "いる"
            | "ある"
            | "なる"
            | "の"
            | "ん"
            | "よう"
            | "あ"
            | "え"
            | "お"
            | "う"
            | "い"
            | "そっ"
            | "じゃっ"
            | "ジャッ"
            | "ヒヒ"
            | "狒々"
    );

    (is_grunt || is_junk_pos || is_junk_sub || meta.is_subsidiary_verb || is_common_junk_lemma)
        && !is_formal_noun(meta.dictionary_form)
        && !is_conjunction_particle(meta.dictionary_form)
}

pub fn is_predicate_suffix(is_content_word: bool, surface: &str) -> bool {
    if is_content_word {
        return false;
    }
    let is_case_or_conj = matches!(
        surface,
        "は" | "が"
            | "を"
            | "に"
            | "と"
            | "も"
            | "へ"
            | "から"
            | "まで"
            | "より"
            | "ので"
            | "のに"
            | "けど"
            | "けれど"
            | "ば"
            | "なら"
    );
    let is_sentence_final = matches!(
        surface,
        "よ" | "ね" | "わ" | "ぞ" | "ぜ" | "さ" | "か" | "な"
    );
    let is_punct = matches!(
        surface,
        "。" | "、"
            | "！"
            | "？"
            | "…"
            | "―"
            | "ー"
            | "「"
            | "」"
            | "（"
            | "）"
            | " "
            | "　"
            | "・"
    );

    !is_case_or_conj && !is_sentence_final && !is_punct
}

pub fn is_predicate_lemma(dict_form: &str) -> bool {
    let last = dict_form.chars().last();
    matches!(
        last,
        Some('る')
            | Some('う')
            | Some('く')
            | Some('ぐ')
            | Some('す')
            | Some('つ')
            | Some('ぬ')
            | Some('ぶ')
            | Some('む')
            | Some('い')
    )
}
