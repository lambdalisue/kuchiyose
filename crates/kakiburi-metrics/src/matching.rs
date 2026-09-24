//! 照合の系統の値を作る。文字だけで測れる 3 つ。
//!
//! 文字 bigram・文字種・読点の打ち方は形態素解析を要らない。機能語と品詞 bigram は
//! [語](crate::word)の側にある。
//!
//! 系統から部分ベクトルの数え上げを引く道はここ 1 つである（[`parts`]）。使う側が
//! 系統ごとに分岐を書けば、系統を足したときに直し忘れた場所が古いまま残る。

use std::collections::BTreeMap;

use kakiburi_doc::prose::Segment;
use kakiburi_doc::text;

use crate::morph::Analyzed;
use crate::system::System;
use crate::word::Counts;

/// 文字 bigram の次元。暫定値であり、指紋に含める。
///
/// 柳・金 2022 は 826〜2,489 を使っている。語彙を固定する以上、小さい側から始める。
pub const CHAR_BIGRAM_DIMS: usize = 500;

/// 読点の直前・直後の次元。暫定値であり、指紋に含める。
pub const COMMA_NEIGHBOR_DIMS: usize = 50;

/// 機能語の次元。暫定値であり、指紋に含める。
///
/// 語彙が開いているので上限が要る。 多く取れば取るほど良いわけではない——
/// まれな語は標準偏差が小さく、z 得点にすると雑音が暴れる。
pub const FUNCTION_WORD_DIMS: usize = 300;

/// 読点の間隔の上限。これ以上は 1 つにまとめる。
pub const COMMA_GAP_MAX: usize = 21;

/// 読点が分布と呼べる形になる下限。
pub const COMMA_FLOOR: usize = 10;

/// 文字種を測る下限。既定より緩い——10 次元しかないので短くても形になる。
pub const CHAR_TYPE_FLOOR: usize = 200;

/// 文末を表す番兵。
///
/// 置かなければ、文の最後の `、` が直後の分布からだけ静かに消える。
pub const SENTINEL: &str = "文末";

/// 文字 bigram。node を跨がない。
///
/// 跨げば、見出しの末尾と次の段落の先頭の組ができる——その隣接は書き手が選んだ
/// ものではない。
#[must_use]
pub fn char_bigrams(prose: &[Segment]) -> Counts {
    let mut counts: Counts = BTreeMap::new();
    for seg in prose {
        let cs: Vec<char> = seg.text.chars().collect();
        for w in cs.windows(2) {
            *counts.entry(w.iter().collect::<String>()).or_default() += 1;
        }
    }
    counts
}

/// 文字種の 10 区分。どれか 1 つが必ず当たるように閉じる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CharType {
    /// ひらがな。
    Hiragana,
    /// カタカナ。半角カタカナと `ー` `々` を含む。
    Katakana,
    /// 漢字。
    Kanji,
    /// 半角英字。
    LatinHalf,
    /// 全角英字。
    LatinFull,
    /// 半角数字。
    DigitHalf,
    /// 全角数字。
    DigitFull,
    /// 約物。
    Punctuation,
    /// 空白。
    Space,
    /// 上のどれでもない文字すべて。 記号にかぎらない——`é` `α` キリル文字もここ。
    Other,
}

impl CharType {
    /// 区分の名前。次元の識別子になる。
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            CharType::Hiragana => "ひらがな",
            CharType::Katakana => "カタカナ",
            CharType::Kanji => "漢字",
            CharType::LatinHalf => "半角英字",
            CharType::LatinFull => "全角英字",
            CharType::DigitHalf => "半角数字",
            CharType::DigitFull => "全角数字",
            CharType::Punctuation => "約物",
            CharType::Space => "空白",
            CharType::Other => "その他",
        }
    }

    /// 全部。次元の並びはこの順である。
    pub const ALL: [CharType; 10] = [
        CharType::Hiragana,
        CharType::Katakana,
        CharType::Kanji,
        CharType::LatinHalf,
        CharType::LatinFull,
        CharType::DigitHalf,
        CharType::DigitFull,
        CharType::Punctuation,
        CharType::Space,
        CharType::Other,
    ];

    /// 1 文字を振り分ける。
    ///
    /// 最後の 1 つを真の受け皿にする。 記号だけを受けると、非日本語の文字が
    /// どこにも属さず、割合の和が 1 にならないまま静かに狂う。
    #[must_use]
    pub fn of(c: char) -> Self {
        if text::is_hiragana(c) {
            return CharType::Hiragana;
        }
        if text::is_katakana(c) {
            return CharType::Katakana;
        }
        if text::is_kanji(c) {
            return CharType::Kanji;
        }
        match c {
            'A'..='Z' | 'a'..='z' => CharType::LatinHalf,
            '\u{FF21}'..='\u{FF3A}' | '\u{FF41}'..='\u{FF5A}' => CharType::LatinFull,
            '0'..='9' => CharType::DigitHalf,
            '\u{FF10}'..='\u{FF19}' => CharType::DigitFull,
            _ if is_punctuation(c) => CharType::Punctuation,
            _ if c.is_whitespace() => CharType::Space,
            _ => CharType::Other,
        }
    }
}

/// 約物。定義の表に挙げたものだけである。
///
/// 一般の記号を約物に流し込まない——`é` `α` を「その他」に落とすのが定義の要である。
fn is_punctuation(c: char) -> bool {
    matches!(
        c,
        '。' | '、'
            | '！'
            | '？'
            | '!'
            | '?'
            | '「'
            | '」'
            | '『'
            | '』'
            | '（'
            | '）'
            | '('
            | ')'
            | '・'
            | '…'
            | '〜'
            | '～'
    )
}

/// 文字種。地の文の全文字を 10 に振り分ける。
///
/// [日本語の文字](kakiburi_doc::text::is_japanese)に限らない——分母は全文字である。
#[must_use]
pub fn char_types(prose: &[Segment]) -> Counts {
    let mut counts: Counts = BTreeMap::new();
    // 0 の区分も次元として立てる。 立てなければ、書き手ごとに次元の数が変わる。
    for t in CharType::ALL {
        counts.insert(t.name().to_owned(), 0);
    }
    for seg in prose {
        for c in seg.text.chars() {
            *counts.entry(CharType::of(c).name().to_owned()).or_default() += 1;
        }
    }
    counts
}

/// 読点の打ち方。3 つを別の部分ベクトルとして返す。
///
/// 直前の文字 / 直後の文字 / 間隔。連結する前に、それぞれの中で相対頻度に直す——
/// 1 つにまとめて割れば、間隔の分布が文字の分布の分母に混ざる。
///
/// 間隔は「`、` から次の `、` または文末まで」である。 文頭から最初の読点までは
/// 数えない——そこは読点が作った間隔ではない。逆に、最後の読点から文末までは
/// 数える。読点の数と間隔の数が一致する。
///
/// 読点の直後がすぐ文末なら間隔は 0 字になるが、次元は 1 字から始まるので
/// 1 字に寄せる。捨てない——捨てると読点の数と間隔の数がずれる。
#[must_use]
pub fn comma_position(prose: &[Segment]) -> Vec<Counts> {
    let mut before: Counts = BTreeMap::new();
    let mut after: Counts = BTreeMap::new();
    let mut gaps: Counts = BTreeMap::new();
    for t in 1..=COMMA_GAP_MAX {
        gaps.insert(gap_name(t), 0);
    }
    for seg in prose {
        let cs: Vec<char> = seg.text.chars().collect();
        // 間隔は「その読点の直後」から「次の読点または文末」まで数える。
        // 開いたままの読点の位置。`Some` なら、そこから数えている最中である。
        let mut open: Option<usize> = None;
        let close = |open: &mut Option<usize>, n: usize, gaps: &mut Counts| {
            if open.take().is_some() {
                *gaps.entry(gap_name(n.max(1))).or_default() += 1;
            }
        };
        let mut since = 0usize;
        for (i, &c) in cs.iter().enumerate() {
            if c == '、' {
                close(&mut open, since, &mut gaps);
                let b = if i == 0 {
                    SENTINEL.to_owned()
                } else {
                    cs[i - 1].to_string()
                };
                *before.entry(b).or_default() += 1;
                let a = cs
                    .get(i + 1)
                    .filter(|n| !is_sentence_end(**n))
                    .map_or_else(|| SENTINEL.to_owned(), ToString::to_string);
                *after.entry(a).or_default() += 1;
                open = Some(i);
                since = 0;
                continue;
            }
            if is_sentence_end(c) {
                close(&mut open, since, &mut gaps);
                since = 0;
                continue;
            }
            if text::is_japanese(c) {
                since += 1;
            }
        }
        // node の末尾も文の終わりである。開いたままの読点をここで閉じる。
        close(&mut open, since, &mut gaps);
    }
    vec![before, after, gaps]
}

/// 間隔の次元の名前。
fn gap_name(n: usize) -> String {
    if n >= COMMA_GAP_MAX {
        format!("{COMMA_GAP_MAX}字以上")
    } else {
        format!("{n}字")
    }
}

/// 読点の間隔と、その読点のまわり。
///
/// 「21 字以上を増やせ」だけでは直せない。 どの読点がどの間隔を作っているかを
/// 言わなければ、受け取った側は自分で数えることになる——実際にそうなった。
///
/// 数え方は[読点の打ち方](comma_position)と同じである。日本語の文字だけを数える
/// ので、英数字を挟む文は見た目より短く出る。
#[must_use]
pub fn comma_gaps(prose: &[Segment]) -> Vec<(String, String)> {
    const AROUND: usize = 8;
    let mut out = Vec::new();
    for seg in prose {
        let cs: Vec<char> = seg.text.chars().collect();
        let mut open: Option<usize> = None;
        let mut since = 0usize;
        let close = |open: &mut Option<usize>, n: usize, out: &mut Vec<(String, String)>| {
            if let Some(at) = open.take() {
                let lo = at.saturating_sub(AROUND);
                let hi = (at + AROUND + 1).min(cs.len());
                out.push((gap_name(n.max(1)), cs[lo..hi].iter().collect()));
            }
        };
        for (i, &c) in cs.iter().enumerate() {
            if c == '、' {
                close(&mut open, since, &mut out);
                open = Some(i);
                since = 0;
                continue;
            }
            if is_sentence_end(c) {
                close(&mut open, since, &mut out);
                since = 0;
                continue;
            }
            if text::is_japanese(c) {
                since += 1;
            }
        }
        close(&mut open, since, &mut out);
    }
    out
}

/// 文の終わりの記号か。間隔はここで切る。
fn is_sentence_end(c: char) -> bool {
    matches!(c, '。' | '！' | '？' | '!' | '?')
}

/// 読点の数。除外の判定に使う。
#[must_use]
pub fn comma_count(prose: &[Segment]) -> usize {
    prose
        .iter()
        .map(|s| s.text.chars().filter(|&c| c == '、').count())
        .sum()
}

/// 系統ごとの部分ベクトルの数え上げ。
///
/// 使う側は系統の一覧を持たない。 ここが唯一の分岐である。
///
/// 形態素解析を要る系統は `analyzed` が `None` なら `None` を返す——
/// 0 を返さない。 0 は「測って 0 だった」という値である。
#[must_use]
pub fn parts(
    system: System,
    prose: &[Segment],
    analyzed: Option<&Analyzed>,
) -> Option<Vec<Counts>> {
    if !measurable(system, prose) {
        return None;
    }
    match system {
        System::CharBigram => enough(vec![char_bigrams(prose)], crate::floor::BIGRAMS),
        System::CharType => Some(vec![char_types(prose)]),
        System::Comma => Some(comma_position(prose)),
        // 語の側は延べ語数の下限を別に持つ。 字数で足りていても語で足りないことがある。
        System::FunctionWord => analyzed.filter(|a| a.enough_tokens()).and_then(|a| {
            enough(
                vec![crate::word::function_words(a)],
                crate::floor::FUNCTION_WORDS,
            )
        }),
        System::PosBigram => analyzed
            .filter(|a| a.enough_tokens())
            .and_then(|a| enough(vec![crate::word::pos_bigrams(a)], crate::floor::BIGRAMS)),
        // 判定に使わない系統。ここから値を出さない。
        System::BunsetsuPattern
        | System::Embedding
        | System::SentenceEnding
        | System::WordStyle
        | System::Orthography
        | System::Structure
        | System::Length => None,
    }
}

/// 系統ごとの部分ベクトルの次元の上限。並びは[`parts`]と同じである。
///
/// `None` は絞らないことを表す（文字種・品詞 bigram のように次元が固定の系統）。
#[must_use]
pub fn limits(system: System) -> Vec<Option<usize>> {
    match system {
        System::CharBigram => vec![Some(CHAR_BIGRAM_DIMS)],
        System::CharType => vec![None],
        System::Comma => vec![
            Some(COMMA_NEIGHBOR_DIMS),
            Some(COMMA_NEIGHBOR_DIMS),
            None, // 間隔は 1〜20 と 21 以上で固定である
        ],
        // 機能語は語彙が開いている。暫定値であり、指紋に含める。
        System::FunctionWord => vec![Some(FUNCTION_WORD_DIMS)],
        System::PosBigram => vec![None],
        _ => vec![],
    }
}

/// 実際に割る分母が下限に届いているか。届かなければ 測れない。
///
/// 見るのは素材の量ではなく、その系統が実際に数えた事象の数である。0 のベクトルを
/// 返してはいけない——返せば距離が計算でき、値が出て、判定が回る。そして
/// [0 と測れないの区別](../../../docs/spec/100-metrics.md#除外の既定)がそこで崩れる。
fn enough(parts: Vec<Counts>, floor: usize) -> Option<Vec<Counts>> {
    let total: usize = parts.iter().flat_map(BTreeMap::values).sum();
    (total >= floor).then_some(parts)
}

/// この単位でこの系統を測れるか。除外はここで 1 度だけ決める。
#[must_use]
pub fn measurable(system: System, prose: &[Segment]) -> bool {
    let ja = kakiburi_doc::prose::japanese_chars(prose);
    match system {
        System::CharType => ja >= CHAR_TYPE_FLOOR,
        System::Comma => ja >= crate::floor::JAPANESE_CHARS && comma_count(prose) >= COMMA_FLOOR,
        _ => ja >= crate::floor::JAPANESE_CHARS,
    }
}

/// 判定に使う系統。この 5 つが揃わなければ照合値を出さない。
pub const FOR_VERDICT: [System; 5] = [
    System::CharBigram,
    System::FunctionWord,
    System::PosBigram,
    System::Comma,
    System::CharType,
];

#[cfg(test)]
mod tests {
    use super::*;
    use kakiburi_doc::node::Kind;

    fn seg(t: &str) -> Segment {
        Segment {
            kind: Kind::Paragraph,
            text: t.to_owned(),
        }
    }

    fn long(t: &str) -> Vec<Segment> {
        // 除外の下限を越えるだけの地の文を作る。
        let mut v = vec![seg(t)];
        v.push(seg(&"あいうえお".repeat(400)));
        v
    }

    #[test]
    fn 文字_bigram_は_node_を跨がない() {
        // 跨げば、見出しの末尾と次の段落の先頭の組ができる。
        let c = char_bigrams(&[seg("あい"), seg("うえ")]);
        assert_eq!(c.get("あい"), Some(&1));
        assert_eq!(c.get("うえ"), Some(&1));
        assert_eq!(c.get("いう"), None, "node を跨いだ組を作ってはいけない");
    }

    #[test]
    fn 文字種は_10_区分を必ず立てる() {
        // 立てなければ、書き手ごとに次元の数が変わる。
        let c = char_types(&[seg("あ")]);
        assert_eq!(c.len(), 10);
        assert_eq!(c.get("ひらがな"), Some(&1));
        assert_eq!(c.get("漢字"), Some(&0));
    }

    #[test]
    fn 非日本語の文字はその他に入る() {
        // 記号だけを受けると、割合の和が 1 にならないまま静かに狂う。
        for c in ['é', 'α', 'д'] {
            assert_eq!(CharType::of(c), CharType::Other, "{c}");
        }
    }

    #[test]
    fn 全角と半角を分ける() {
        assert_eq!(CharType::of('A'), CharType::LatinHalf);
        assert_eq!(CharType::of('Ａ'), CharType::LatinFull);
        assert_eq!(CharType::of('1'), CharType::DigitHalf);
        assert_eq!(CharType::of('１'), CharType::DigitFull);
    }

    #[test]
    fn 長音符と繰り返し記号はカタカナに入れる() {
        // 日本語の文字がそれらを分けていないからである。
        assert_eq!(CharType::of('ー'), CharType::Katakana);
        assert_eq!(CharType::of('々'), CharType::Katakana);
        assert_eq!(CharType::of('ｱ'), CharType::Katakana);
    }

    #[test]
    fn 中黒は約物である() {
        assert_eq!(CharType::of('・'), CharType::Punctuation);
        assert_eq!(CharType::of('…'), CharType::Punctuation);
    }

    #[test]
    fn 割合の和は_1_になる() {
        let c = char_types(&[seg("あアA1Ａ１。 é、")]);
        let total: usize = c.values().sum();
        assert_eq!(total, "あアA1Ａ１。 é、".chars().count());
    }

    #[test]
    fn 読点は_3_つの部分ベクトルになる() {
        let p = comma_position(&[seg("これは、そうだ。")]);
        assert_eq!(p.len(), 3);
        assert_eq!(p[0].get("は"), Some(&1), "直前の文字");
        assert_eq!(p[1].get("そ"), Some(&1), "直後の文字");
    }

    #[test]
    fn 文末で終わる読点は番兵に入る() {
        // 置かなければ、その読点が直後の分布からだけ静かに消える。
        let p = comma_position(&[seg("これは、")]);
        assert_eq!(p[1].get(SENTINEL), Some(&1));
        let p = comma_position(&[seg("これは、。")]);
        assert_eq!(p[1].get(SENTINEL), Some(&1), "句点の前も文末である");
    }

    #[test]
    fn 間隔は読点から次の読点または文末まで数える() {
        // 「そうだ」の 3 字。文頭から読点までの「これは」は数えない——
        // そこは読点が作った間隔ではない。
        let p = comma_position(&[seg("これは、そうだ。")]);
        assert_eq!(p[2].get("3字"), Some(&1), "{:?}", p[2]);
        assert_eq!(p[2].values().sum::<usize>(), 1, "読点 1 つに間隔 1 つ");
    }

    #[test]
    fn 間隔は次の読点で切る() {
        // 「あい」「うえお」——読点 2 つに間隔 2 つ。
        let p = comma_position(&[seg("かき、あい、うえお。")]);
        assert_eq!(p[2].get("2字"), Some(&1), "{:?}", p[2]);
        assert_eq!(p[2].get("3字"), Some(&1), "{:?}", p[2]);
        assert_eq!(p[2].values().sum::<usize>(), 2);
    }

    #[test]
    fn 最後の読点から文末までも数える() {
        // 文頭から数えていたときは、ここが落ちていた。
        let p = comma_position(&[seg("あいうえお。かき、くけ。")]);
        assert_eq!(p[2].get("2字"), Some(&1), "{:?}", p[2]);
        assert_eq!(p[2].values().sum::<usize>(), 1);
    }

    #[test]
    fn 読点の数と間隔の数は一致する() {
        let p = comma_position(&[seg("あ、い、う。え、お")]);
        assert_eq!(p[2].values().sum::<usize>(), 3);
    }

    #[test]
    fn 文末で終わる読点の間隔は_1_字に寄せる() {
        // 0 字は次元に無い。捨てると読点の数と間隔の数がずれる。
        let p = comma_position(&[seg("これは、。")]);
        assert_eq!(p[2].get("1字"), Some(&1), "{:?}", p[2]);
    }

    #[test]
    fn 間隔は_21_字以上をまとめる() {
        let p = comma_position(&[seg(&format!("、{}", "あ".repeat(30)))]);
        assert_eq!(p[2].get("21字以上"), Some(&1));
        assert_eq!(p[2].len(), COMMA_GAP_MAX, "次元は固定である");
    }

    #[test]
    fn node_が_1_文字ずつなら文字_bigram_は測れない() {
        // 素材は足りているのに bigram が 1 つも作られない。0 のベクトルを
        // 返せば距離が計算でき、値が出て、判定が回る。
        let prose: Vec<Segment> = (0..1200).map(|_| seg("あ")).collect();
        assert!(
            kakiburi_doc::prose::japanese_chars(&prose) >= crate::floor::JAPANESE_CHARS,
            "素材は足りている"
        );
        assert_eq!(parts(System::CharBigram, &prose, None), None);
    }

    #[test]
    fn 文字だけの_3_系統は解析器を要らない() {
        // 読点は 10 個要る——文字だけで測れることと、除外は別の話である。
        let prose = long(&"これは、そうだ。".repeat(10));
        for s in [System::CharBigram, System::CharType, System::Comma] {
            assert!(parts(s, &prose, None).is_some(), "{s:?} は文字だけで測れる");
        }
    }

    #[test]
    fn 解析器が無ければ語の系統は測っていないとする() {
        // 0 を返さない。0 は「測って 0 だった」という値である。
        let prose = long("これは、そうだ。");
        for s in [System::FunctionWord, System::PosBigram] {
            assert_eq!(parts(s, &prose, None), None, "{s:?}");
        }
    }

    #[test]
    fn 判定に使わない系統からは値を出さない() {
        let prose = long("これは、そうだ。");
        for s in [System::Structure, System::Length, System::BunsetsuPattern] {
            assert_eq!(parts(s, &prose, None), None, "{s:?}");
        }
    }

    #[test]
    fn 上限の並びは部分ベクトルの並びと同じ() {
        // 食い違えば、絞る先を 1 つずれて当てる。
        // 全系統の除外を越える素材——字数・読点の数・延べ語数。
        let prose: Vec<Segment> = (0..200).map(|_| seg("これ は 、 そう だ 。")).collect();
        let analyzed =
            crate::morph::Analyzed::of(&prose, &crate::morph::stub::Stub::unidic()).unwrap();
        for s in FOR_VERDICT {
            let got =
                parts(s, &prose, Some(&analyzed)).unwrap_or_else(|| panic!("{s:?} が測れていない"));
            assert_eq!(got.len(), limits(s).len(), "{s:?}");
        }
    }

    #[test]
    fn 読点が_10_未満なら測らない() {
        // 分布と呼べる形にならない。
        let prose = long("これは、そうだ。");
        assert!(!measurable(System::Comma, &prose));
        let many = long(&"あいうえお、".repeat(10));
        assert!(measurable(System::Comma, &many));
    }

    #[test]
    fn 文字種は短くても測る() {
        // 10 次元しかないので、既定より緩い下限を持つ。
        let short = vec![seg(&"あいうえお".repeat(50))];
        assert!(measurable(System::CharType, &short));
        assert!(
            !measurable(System::CharBigram, &short),
            "文字 bigram は既定の下限である"
        );
    }

    #[test]
    fn 短すぎれば測らない() {
        let prose = vec![seg("短い。")];
        for s in FOR_VERDICT {
            assert_eq!(parts(s, &prose, None), None, "{s:?}");
        }
    }
}
