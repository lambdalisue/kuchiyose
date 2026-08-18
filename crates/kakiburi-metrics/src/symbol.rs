//! 記号と表記の指標。
//!
//! どれも[地の文](kakiburi_doc::prose)だけを見る。地の文はコードもコードだけの
//! セルも含まないので、コードの多い記事ほど記号の率が上がることはない。

use kakiburi_doc::prose::Segment;
use kakiburi_doc::text;

use crate::{floor, Measured};

/// 地の文の文字を、node の順に走る。<strong>node を跨ぐ隣接は作らない。</strong>
fn chars_per_node(prose: &[Segment]) -> impl Iterator<Item = Vec<char>> + '_ {
    prose.iter().map(|s| s.text.chars().collect())
}

/// 地の文の日本語の文字数。率の分母。
fn japanese(prose: &[Segment]) -> usize {
    prose.iter().map(|s| text::count_japanese(&s.text)).sum()
}

/// 1 文字を数えて、日本語 1,000 字あたりに直す。
fn per_1000(prose: &[Segment], hit: impl Fn(char) -> bool) -> Measured {
    let ja = japanese(prose);
    if ja < floor::JAPANESE_CHARS {
        return Measured::BelowFloor;
    }
    let n: usize = prose
        .iter()
        .map(|s| s.text.chars().filter(|&c| hit(c)).count())
        .sum();
    #[allow(clippy::cast_precision_loss)]
    Measured::Value(1000.0 * n as f64 / ja as f64)
}

/// 全角括弧。`（` の数。
#[must_use]
pub fn full_width_paren(prose: &[Segment]) -> Measured {
    per_1000(prose, |c| c == '（')
}

/// 半角括弧。`(` の数。
#[must_use]
pub fn half_width_paren(prose: &[Segment]) -> Measured {
    per_1000(prose, |c| c == '(')
}

/// 鉤括弧。`「` の数。
#[must_use]
pub fn corner_bracket(prose: &[Segment]) -> Measured {
    per_1000(prose, |c| c == '「')
}

/// 感嘆符。<strong>字幅を合算する。</strong>
///
/// 合算してよいのは、これが頻度の指標だからである。どちらを使うかの選択は
/// [`exclamation_width`]が別に見る。
#[must_use]
pub fn exclamation(prose: &[Segment]) -> Measured {
    per_1000(prose, |c| c == '!' || c == '！')
}

/// 疑問符。<strong>字幅を合算する。</strong>
#[must_use]
pub fn question(prose: &[Segment]) -> Measured {
    per_1000(prose, |c| c == '?' || c == '？')
}

/// em dash。`—` の数。
#[must_use]
pub fn em_dash(prose: &[Segment]) -> Measured {
    per_1000(prose, |c| c == '\u{2014}')
}

/// 絵文字。Unicode の Emoji_Presentation を持つ文字の数。
///
/// 表の全体を持たないので、実務上使われる範囲を挙げる。<strong>暫定である。</strong>
#[must_use]
pub fn emoji(prose: &[Segment]) -> Measured {
    per_1000(prose, is_emoji_presentation)
}

fn is_emoji_presentation(c: char) -> bool {
    matches!(c,
        '\u{1F300}'..='\u{1F5FF}'   // 記号と絵文字
        | '\u{1F600}'..='\u{1F64F}' // 顔
        | '\u{1F680}'..='\u{1F6FF}' // 乗り物と記号
        | '\u{1F900}'..='\u{1FAFF}' // 補助記号
        | '\u{1F1E6}'..='\u{1F1FF}' // 国旗
        | '\u{2600}'..='\u{27BF}'   // その他の記号（既定で絵文字表示のもの）
    )
}

/// 中黒。`・` の数。
///
/// <strong>カタカナ語の区切りに使われた `・` を除く。</strong> 前後がともにカタカナである `・` は
/// 複合語の区切り（「アプリケーション・サーバ」）であって、並列の選択ではない。
#[must_use]
pub fn middle_dot(prose: &[Segment]) -> Measured {
    let ja = japanese(prose);
    if ja < floor::JAPANESE_CHARS {
        return Measured::BelowFloor;
    }
    let mut n = 0usize;
    for chars in chars_per_node(prose) {
        for i in 0..chars.len() {
            if chars[i] != '・' {
                continue;
            }
            let prev = i.checked_sub(1).map(|j| chars[j]);
            let next = chars.get(i + 1).copied();
            let compound =
                prev.is_some_and(text::is_katakana) && next.is_some_and(text::is_katakana);
            if !compound {
                n += 1;
            }
        }
    }
    #[allow(clippy::cast_precision_loss)]
    Measured::Value(1000.0 * n as f64 / ja as f64)
}

/// 三点リーダの箇所。
///
/// <strong>1 箇所とは、連続する `…` の並び全体、または連続する半角ピリオド 3 つ以上の
/// 並び全体を指す。</strong>`……` は 1 箇所、`...` も 1 箇所、`......` も 1 箇所である。
#[must_use]
pub fn ellipsis(prose: &[Segment]) -> Measured {
    let ja = japanese(prose);
    if ja < floor::JAPANESE_CHARS {
        return Measured::BelowFloor;
    }
    #[allow(clippy::cast_precision_loss)]
    Measured::Value(1000.0 * count_ellipsis(prose).0 as f64 / ja as f64)
}

/// 三点リーダの字数。箇所のうち<strong>重ねた箇所</strong>の割合。
#[must_use]
pub fn ellipsis_doubled(prose: &[Segment]) -> Measured {
    let (total, doubled) = count_ellipsis(prose);
    if total < 5 {
        return Measured::BelowFloor;
    }
    #[allow(clippy::cast_precision_loss)]
    Measured::Value(doubled as f64 / total as f64)
}

/// 箇所の数と、そのうち重ねた箇所の数。
fn count_ellipsis(prose: &[Segment]) -> (usize, usize) {
    let (mut total, mut doubled) = (0usize, 0usize);
    for chars in chars_per_node(prose) {
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '…' {
                let start = i;
                while i < chars.len() && chars[i] == '…' {
                    i += 1;
                }
                total += 1;
                if i - start >= 2 {
                    doubled += 1;
                }
                continue;
            }
            if chars[i] == '.' {
                let start = i;
                while i < chars.len() && chars[i] == '.' {
                    i += 1;
                }
                // 半角は 3 つ以上でひとつの箇所。`.` 1 つや `..` は数えない。
                if i - start >= 3 {
                    total += 1;
                    if i - start >= 6 {
                        doubled += 1;
                    }
                }
                continue;
            }
            i += 1;
        }
    }
    (total, doubled)
}

/// 感嘆符の字幅。感嘆符のうち全角 `！` の割合。
#[must_use]
pub fn exclamation_width(prose: &[Segment]) -> Measured {
    width_ratio(prose, '！', '!', 5)
}

/// 疑問符の字幅。疑問符のうち全角 `？` の割合。
#[must_use]
pub fn question_width(prose: &[Segment]) -> Measured {
    width_ratio(prose, '？', '?', 5)
}

/// 数字の字幅。数字のうち全角の割合。
#[must_use]
pub fn digit_width(prose: &[Segment]) -> Measured {
    let (mut full, mut total) = (0usize, 0usize);
    for chars in chars_per_node(prose) {
        for c in chars {
            if c.is_ascii_digit() {
                total += 1;
            } else if ('\u{FF10}'..='\u{FF19}').contains(&c) {
                total += 1;
                full += 1;
            }
        }
    }
    ratio(full, total, 10)
}

/// 波ダッシュ。`〜` と `～` の合計のうち `〜`（U+301C）の割合。
#[must_use]
pub fn wave_dash(prose: &[Segment]) -> Measured {
    width_ratio(prose, '\u{301C}', '\u{FF5E}', 3)
}

fn width_ratio(prose: &[Segment], picked: char, other: char, min: usize) -> Measured {
    let (mut hit, mut total) = (0usize, 0usize);
    for chars in chars_per_node(prose) {
        for c in chars {
            if c == picked {
                hit += 1;
                total += 1;
            } else if c == other {
                total += 1;
            }
        }
    }
    ratio(hit, total, min)
}

#[allow(clippy::cast_precision_loss)]
fn ratio(hit: usize, total: usize, min: usize) -> Measured {
    if total < min {
        // 分母が小さいと割合が跳ねる。0 を返さない。
        return Measured::BelowFloor;
    }
    Measured::Value(hit as f64 / total as f64)
}

/// 和欧間スペース欠落。
///
/// 日本語の文字と英数字が隣接する箇所を数え、そのうち空白を挟んでいないものの割合。
///
/// <strong>分母は日本語の文字数ではない。</strong> 文字数で割ると、英数字を多く使う題材ほど値が
/// 動く。<strong>約物を挟む場合は、機会にも欠落にも数えない</strong>——`Rust、` や `（Rust` は、
/// スペースを入れる習慣のある人でも入れない。
#[must_use]
pub fn missing_space(prose: &[Segment]) -> Measured {
    let (mut chance, mut missing) = (0usize, 0usize);
    for chars in chars_per_node(prose) {
        for i in 0..chars.len() {
            let a = chars[i];
            // 空白を挟んだ隣接。機会に数える。
            if a == ' ' {
                let Some(prev) = i.checked_sub(1).map(|j| chars[j]) else {
                    continue;
                };
                let Some(next) = chars.get(i + 1).copied() else {
                    continue;
                };
                if is_cross(prev, next) {
                    chance += 1;
                }
                continue;
            }
            // 直に隣接。機会と欠落の両方に数える。
            let Some(next) = chars.get(i + 1).copied() else {
                continue;
            };
            if is_cross(a, next) {
                chance += 1;
                missing += 1;
            }
        }
    }
    if chance < 20 {
        return Measured::BelowFloor;
    }
    #[allow(clippy::cast_precision_loss)]
    Measured::Value(missing as f64 / chance as f64)
}

/// 日本語の文字と英数字の境目か。約物を挟むものは含めない。
fn is_cross(a: char, b: char) -> bool {
    let alnum = |c: char| c.is_ascii_alphanumeric();
    (text::is_japanese(a) && alnum(b)) || (alnum(a) && text::is_japanese(b))
}

/// 笑い。差し込んだ<strong>箇所</strong>の数。
///
/// 連続は 1 と数える。`www` は 3 ではなく 1。<strong>`w` 1 つも 1 と数える</strong>——回数ではなく、
/// 笑いを差し込んだ箇所の数を見たい。
#[must_use]
pub fn laughter(prose: &[Segment]) -> Measured {
    let ja = japanese(prose);
    if ja < floor::JAPANESE_CHARS {
        return Measured::BelowFloor;
    }
    let mut n = 0usize;
    for chars in chars_per_node(prose) {
        let mut i = 0;
        while i < chars.len() {
            // `（笑）` `(笑)`。最長一致で 1 つに畳む。
            if let Some(len) = paren_laugh(&chars, i) {
                n += 1;
                i += len;
                continue;
            }
            // `w` `ｗ` の連続。直前と直後が英字でないもの。
            if is_w(chars[i]) {
                let start = i;
                while i < chars.len() && is_w(chars[i]) {
                    i += 1;
                }
                let before_ok = start
                    .checked_sub(1)
                    .is_none_or(|j| !chars[j].is_ascii_alphabetic());
                let after_ok = chars.get(i).is_none_or(|c| !c.is_ascii_alphabetic());
                if before_ok && after_ok {
                    n += 1;
                }
                continue;
            }
            // <strong>直後が日本語の文字でない</strong> `草`。「雑草」を拾わない。
            if chars[i] == '草' {
                let next_ok = chars.get(i + 1).is_none_or(|&c| !text::is_japanese(c));
                if next_ok {
                    n += 1;
                }
                i += 1;
                continue;
            }
            i += 1;
        }
    }
    #[allow(clippy::cast_precision_loss)]
    Measured::Value(1000.0 * n as f64 / ja as f64)
}

fn is_w(c: char) -> bool {
    c == 'w' || c == '\u{FF57}'
}

/// `（笑）` `(笑)` とその字幅混在。当たれば長さを返す。
fn paren_laugh(chars: &[char], i: usize) -> Option<usize> {
    let open = *chars.get(i)?;
    if open != '（' && open != '(' {
        return None;
    }
    if *chars.get(i + 1)? != '笑' {
        return None;
    }
    let close = *chars.get(i + 2)?;
    if close == '）' || close == ')' {
        Some(3)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kakiburi_doc::node::Kind;

    /// 日本語 1,000 字を超える地の文を作る。除外に掛からせないため。
    fn prose_with(extra: &str) -> Vec<Segment> {
        let filler = "これは日本語の文章である。".repeat(100);
        vec![
            Segment {
                kind: Kind::Paragraph,
                text: filler,
            },
            Segment {
                kind: Kind::Paragraph,
                text: extra.to_owned(),
            },
        ]
    }

    fn value(m: Measured) -> f64 {
        m.value().expect("測れているべき")
    }

    #[test]
    fn 日本語が_1000_字に届かなければ測らない() {
        let p = vec![Segment {
            kind: Kind::Paragraph,
            text: "短い文である。（かっこ）".into(),
        }];
        // 0 を返せば「使わなかった」と読まれる。
        assert_eq!(full_width_paren(&p), Measured::BelowFloor);
    }

    #[test]
    fn 全角と半角の括弧は別に数える() {
        let p = prose_with("（全角）と (半角) を混ぜる。");
        assert!(value(full_width_paren(&p)) > 0.0);
        assert!(value(half_width_paren(&p)) > 0.0);
    }

    #[test]
    fn 感嘆符は字幅を合算する() {
        let p = prose_with("すごい! すごい！");
        let ja = japanese(&p);
        #[allow(clippy::cast_precision_loss)]
        let want = 1000.0 * 2.0 / ja as f64;
        assert!((value(exclamation(&p)) - want).abs() < 1e-9);
    }

    #[test]
    fn 字幅は別の指標が見る() {
        let p = prose_with("! ! ! ！ ！");
        // 5 個のうち全角 2 個。
        assert!((value(exclamation_width(&p)) - 0.4).abs() < 1e-9);
    }

    #[test]
    fn 字幅の分母が小さければ測らない() {
        let p = prose_with("すごい！");
        assert_eq!(exclamation_width(&p), Measured::BelowFloor);
    }

    #[test]
    fn カタカナ語の区切りの中黒は数えない() {
        // 「アプリケーション・サーバ」は複合語の区切りであって並列の選択ではない。
        let p = prose_with("アプリケーション・サーバを使う。");
        assert!((value(middle_dot(&p)) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn 並列の中黒は数える() {
        let p = prose_with("赤・青を選ぶ。");
        assert!(value(middle_dot(&p)) > 0.0);
    }

    #[test]
    fn 三点リーダは箇所で数える() {
        // `……` は 1 箇所、`...` も 1 箇所、`......` も 1 箇所。
        let p = prose_with("ああ…… いい... うう......");
        let ja = japanese(&p);
        #[allow(clippy::cast_precision_loss)]
        let want = 1000.0 * 3.0 / ja as f64;
        assert!((value(ellipsis(&p)) - want).abs() < 1e-9);
    }

    #[test]
    fn 半角ピリオドは_3_つ以上で_1_箇所() {
        let p = prose_with("文の終わり. 次の文。");
        assert!(
            (value(ellipsis(&p)) - 0.0).abs() < 1e-9,
            "ピリオド 1 つは箇所ではない"
        );
    }

    #[test]
    fn 重ねた箇所の割合を別に見る() {
        let p = prose_with("あ… い… う… え…… お……");
        // 5 箇所のうち重ねたのは 2。
        assert!((value(ellipsis_doubled(&p)) - 0.4).abs() < 1e-9);
    }

    #[test]
    fn 三点リーダが少なければ字数を測らない() {
        let p = prose_with("あ… い…");
        assert_eq!(ellipsis_doubled(&p), Measured::BelowFloor);
    }

    #[test]
    fn 波ダッシュは字幅の割合である() {
        let p = prose_with("1〜2、3〜4、5～6");
        // 3 個のうち U+301C が 2 個。
        assert!((value(wave_dash(&p)) - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn 数字の字幅は全角の割合である() {
        let p = prose_with("１２３ と 1234567");
        // 10 個のうち全角 3 個。
        assert!((value(digit_width(&p)) - 0.3).abs() < 1e-9);
    }

    #[test]
    fn 和欧間スペース欠落は隣接の機会で割る() {
        // 「あa。」は欠落 1・機会 1。「あ a。」は機会 1 だけ。
        // 句点で切るので、次の「あ」との隣接は生まれない。
        let mut text = String::new();
        for _ in 0..15 {
            text.push_str("あa。");
        }
        for _ in 0..15 {
            text.push_str("あ a。");
        }
        let p = prose_with(&text);
        // 機会 30 のうち欠落 15。
        assert!(
            (value(missing_space(&p)) - 0.5).abs() < 1e-9,
            "{}",
            value(missing_space(&p))
        );
    }

    #[test]
    fn 空白を入れる書き手は_0_に近づく() {
        let mut text = String::new();
        for _ in 0..30 {
            text.push_str("あ a。");
        }
        let p = prose_with(&text);
        assert!((value(missing_space(&p)) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn 空白を入れない書き手は_1_に近づく() {
        let mut text = String::new();
        for _ in 0..30 {
            text.push_str("あa。");
        }
        let p = prose_with(&text);
        assert!((value(missing_space(&p)) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn 約物を挟む隣接は機会に数えない() {
        let mut text = String::new();
        for _ in 0..30 {
            text.push_str("あ、Rust。");
        }
        let p = prose_with(&text);
        // 機会が 20 に届かない。
        assert_eq!(missing_space(&p), Measured::BelowFloor);
    }

    #[test]
    fn 笑いは箇所で数える() {
        // `www` は 3 ではなく 1。`w` 1 つも 1。
        let p = prose_with("そうだねwww そうかw （笑）これは草");
        let ja = japanese(&p);
        #[allow(clippy::cast_precision_loss)]
        let want = 1000.0 * 4.0 / ja as f64;
        assert!(
            (value(laughter(&p)) - want).abs() < 1e-9,
            "{}",
            value(laughter(&p))
        );
    }

    #[test]
    fn 英字の中の_w_は笑いではない() {
        let p = prose_with("window と write を使う。");
        assert!((value(laughter(&p)) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn 雑草の草は笑いではない() {
        // 直後が日本語の文字なら拾わない。
        let p = prose_with("雑草を抜く。草木も眠る。");
        assert!((value(laughter(&p)) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn 記号は_node_を跨がない() {
        // 「あa」の隣接が node を跨いで生まれてはいけない。
        let filler = "これは日本語の文章である。".repeat(100);
        let p = vec![
            Segment {
                kind: Kind::Paragraph,
                text: format!("{filler}あ"),
            },
            Segment {
                kind: Kind::Paragraph,
                text: "abc".into(),
            },
        ];
        // 跨げば 1 つ機会が生まれる。跨がなければ 0。
        assert_eq!(missing_space(&p), Measured::BelowFloor);
    }
}
