//! 記号と表記の指標。
//!
//! どれも[地の文](kuchiyose_doc::prose)だけを見る。地の文はコードもコードだけの
//! セルも含まないので、コードの多い記事ほど記号の率が上がることはない。

use kuchiyose_doc::prose::Segment;
use kuchiyose_doc::text;

use crate::directive::Counted;

/// 地の文の文字を、node の順に走る。node を跨ぐ隣接は作らない。
fn chars_per_node(prose: &[Segment]) -> impl Iterator<Item = Vec<char>> + '_ {
    prose.iter().map(|s| s.text.chars().collect())
}

/// 地の文の日本語の文字数。率の分母。
fn japanese(prose: &[Segment]) -> usize {
    prose.iter().map(|s| text::count_japanese(&s.text)).sum()
}

/// 1 文字を数えて、日本語 1,000 字あたりに直す。
fn per_1000(prose: &[Segment], hit: impl Fn(char) -> bool) -> Counted {
    let ja = japanese(prose);
    let n: usize = prose
        .iter()
        .map(|s| s.text.chars().filter(|&c| hit(c)).count())
        .sum();
    Counted::density(n, ja)
}

/// 全角括弧。`（` の数。
#[must_use]
pub fn full_width_paren(prose: &[Segment]) -> Counted {
    per_1000(prose, |c| c == '（')
}

/// 半角括弧。`(` の数。
#[must_use]
pub fn half_width_paren(prose: &[Segment]) -> Counted {
    per_1000(prose, |c| c == '(')
}

/// 鉤括弧。`「` の数。
#[must_use]
pub fn corner_bracket(prose: &[Segment]) -> Counted {
    per_1000(prose, |c| c == '「')
}

/// 感嘆符。字幅を合算する。
///
/// 合算してよいのは、これが頻度の指標だからである。どちらを使うかの選択は
/// [`exclamation_width`]が別に見る。
#[must_use]
pub fn exclamation(prose: &[Segment]) -> Counted {
    per_1000(prose, |c| c == '!' || c == '！')
}

/// 疑問符。字幅を合算する。
#[must_use]
pub fn question(prose: &[Segment]) -> Counted {
    per_1000(prose, |c| c == '?' || c == '？')
}

/// em dash。`—` の数。
#[must_use]
pub fn em_dash(prose: &[Segment]) -> Counted {
    per_1000(prose, |c| c == '\u{2014}')
}

/// 絵文字。数える単位は書記素クラスタである。
///
/// 見た目が 1 つだからである——ゼロ幅接合子で繋いだ列、国旗（地域指標符号の対）、
/// 肌の色の指定、異体字セレクタ付きのものは、どれも 1 と数える。
///
/// 符号位置で数えてはいけない。 国旗は 2、家族の ZWJ 列は 5 以上になり、
/// 絵文字を 1 つ置いた書き手が 5 つ置いたことになる。
#[must_use]
pub fn emoji(prose: &[Segment]) -> Counted {
    let ja = japanese(prose);
    let n: usize = chars_per_node(prose).map(|cs| clusters(&cs)).sum();
    Counted::density(n, ja)
}

/// 絵文字の列を、書記素クラスタの数として数える。最長一致で取る。
fn clusters(chars: &[char]) -> usize {
    let mut n = 0usize;
    let mut i = 0;
    while i < chars.len() {
        let Some(end) = cluster_end(chars, i) else {
            i += 1;
            continue;
        };
        n += 1;
        i = end;
    }
    n
}

/// `i` から始まる絵文字クラスタの終わり。絵文字で始まらなければ `None`。
fn cluster_end(chars: &[char], i: usize) -> Option<usize> {
    // 国旗。地域指標符号は 2 つで 1 つである。
    if is_regional(chars[i]) {
        return Some(if chars.get(i + 1).is_some_and(|&c| is_regional(c)) {
            i + 2
        } else {
            i + 1
        });
    }
    // keycap。`1️⃣` は数字・異体字セレクタ・囲みの 3 つで 1 つである。
    if matches!(chars[i], '0'..='9' | '#' | '*')
        && chars.get(i + 1) == Some(&'\u{FE0F}')
        && chars.get(i + 2) == Some(&'\u{20E3}')
    {
        return Some(i + 3);
    }
    if !is_emoji_presentation(chars[i]) {
        return None;
    }
    let mut j = i + 1;
    loop {
        j = tail(chars, j);
        // ゼロ幅接合子で繋がっていれば、その先も同じ 1 つである。
        if chars.get(j) == Some(&'\u{200D}') && chars.get(j + 1).is_some() {
            j += 2;
            continue;
        }
        return Some(j);
    }
}

/// 異体字セレクタ・肌の色・タグ列を読み飛ばす。
fn tail(chars: &[char], mut j: usize) -> usize {
    while let Some(&c) = chars.get(j) {
        let follows = c == '\u{FE0F}'
            || ('\u{1F3FB}'..='\u{1F3FF}').contains(&c)
            || ('\u{E0020}'..='\u{E007F}').contains(&c);
        if !follows {
            break;
        }
        j += 1;
    }
    j
}

fn is_regional(c: char) -> bool {
    ('\u{1F1E6}'..='\u{1F1FF}').contains(&c)
}

/// 絵文字として数える文字の範囲。暫定である。
///
/// 本来は Unicode の `Emoji_Presentation` と推奨列の表で決める
/// （[定義](../../../docs/spec/metrics/絵文字.md#数え方)）。その表をまだ持っていないので、
/// 実務上使われる区画を挙げている。
///
/// 暫定であることを[指紋](EMOJI_RANGES_VERSION)に出す。 表を入れたら値が変わる。
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

/// 絵文字の範囲表の版。指紋に出す。
///
/// 表を [`is_emoji_presentation`] から Unicode の正規の表へ替えたら上げる——
/// 上げなければ、範囲が変わったのに古い値が使い回される。
pub const EMOJI_RANGES_VERSION: &str = "暫定の区画表 1";

/// 中黒。`・` の数。
///
/// カタカナ語の区切りに使われた `・` を除く。 前後がともにカタカナである `・` は
/// 複合語の区切り（「アプリケーション・サーバ」）であって、並列の選択ではない。
#[must_use]
pub fn middle_dot(prose: &[Segment]) -> Counted {
    let ja = japanese(prose);
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
    Counted::density(n, ja)
}

/// 三点リーダの箇所。
///
/// 1 箇所とは、連続する `…` の並び全体、または連続する半角ピリオド 3 つ以上の
/// 並び全体を指す。`……` は 1 箇所、`...` も 1 箇所、`......` も 1 箇所である。
#[must_use]
pub fn ellipsis(prose: &[Segment]) -> Counted {
    let ja = japanese(prose);
    Counted::density(count_ellipsis(prose).0, ja)
}

/// 三点リーダの字数。箇所のうち重ねた箇所の割合。
#[must_use]
pub fn ellipsis_doubled(prose: &[Segment]) -> Counted {
    let (total, doubled) = count_ellipsis(prose);
    Counted::share(doubled, total, 5)
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
pub fn exclamation_width(prose: &[Segment]) -> Counted {
    width_ratio(prose, '！', '!', 5)
}

/// 疑問符の字幅。疑問符のうち全角 `？` の割合。
#[must_use]
pub fn question_width(prose: &[Segment]) -> Counted {
    width_ratio(prose, '？', '?', 5)
}

/// 数字の字幅。数字のうち全角の割合。
#[must_use]
pub fn digit_width(prose: &[Segment]) -> Counted {
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
pub fn wave_dash(prose: &[Segment]) -> Counted {
    width_ratio(prose, '\u{301C}', '\u{FF5E}', 3)
}

fn width_ratio(prose: &[Segment], picked: char, other: char, min: usize) -> Counted {
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

/// 分母が小さいと割合が跳ねる。下限に届かなければ 0 ではなく下限未満になる。
fn ratio(hit: usize, total: usize, min: usize) -> Counted {
    Counted::share(hit, total, min)
}

/// 和欧間スペース欠落。
///
/// 日本語の文字と英数字が隣接する箇所を数え、そのうち空白を挟んでいないものの割合。
///
/// 分母は日本語の文字数ではない。 文字数で割ると、英数字を多く使う題材ほど値が
/// 動く。約物を挟む場合は、機会にも欠落にも数えない——`Rust、` や `（Rust` は、
/// スペースを入れる習慣のある人でも入れない。
#[must_use]
pub fn missing_space(prose: &[Segment]) -> Counted {
    let (mut chance, mut missing) = (0usize, 0usize);
    for chars in chars_per_node(prose) {
        let mut i = 0;
        while i < chars.len() {
            let a = chars[i];
            if !is_side(a) {
                i += 1;
                continue;
            }
            // 空白列を読み飛ばす。0 個でも機会である——「隣接」だけを機会に
            // すると、空白を挟んだ側が分母から消えて値が必ず 1.0 になる。
            let mut j = i + 1;
            while chars.get(j).is_some_and(|&c| is_gap(c)) {
                j += 1;
            }
            let Some(&b) = chars.get(j) else { break };
            if is_cross(a, b) {
                chance += 1;
                if j == i + 1 {
                    missing += 1;
                }
            }
            i = j;
        }
    }
    Counted::share(missing, chance, 20)
}

/// 和文間スペース。
///
/// 日本語の文字どうしが隣り合う箇所を数え、そのうち空白を挟んでいるものの割合。
///
/// ふつうは 0 である。 日本語は分かち書きをしないので、`複数の リポジトリ を`
/// とは書かない。
///
/// [和欧間スペース欠落](missing_space)の裏返しではない。 あちらは入れるかどうかの
/// 習慣で、入れる人も入れない人もいる。こちらは入れてはいけないところに入っているか
/// を見る。
///
/// 書き手を分けるために置いたのではない。 実測で本人の記事 8 本すべてが 0 箇所
/// だったのに対し、和欧間の空白を機械的に入れて作った草稿が 103 箇所になり、
/// それでも判定が通った。 通してはいけないものが通る穴を塞ぐために置いている。
#[must_use]
pub fn wabun_space(prose: &[Segment]) -> Counted {
    let (mut chance, mut spaced) = (0usize, 0usize);
    for chars in chars_per_node(prose) {
        let mut i = 0;
        while i < chars.len() {
            let a = chars[i];
            if !text::is_wabun(a) {
                i += 1;
                continue;
            }
            let mut j = i + 1;
            while chars.get(j).is_some_and(|&c| is_gap(c)) {
                j += 1;
            }
            let Some(&b) = chars.get(j) else { break };
            if text::is_wabun(b) {
                chance += 1;
                if j > i + 1 {
                    spaced += 1;
                }
            }
            i = j;
        }
    }
    Counted::share(spaced, chance, 20)
}

/// 和欧の境目になりうる文字か。
fn is_side(c: char) -> bool {
    text::is_japanese(c) || is_alnum(c)
}

/// 英数字。半角だけである。
///
/// 全角の `Ｒ` や `１` は和文と同じ字幅で組まれるので、`日本語Ａ` はベタ組みが
/// 普通である——入れれば、全角を選んだというだけで欠落率が上がる。そして
/// 字幅は[数字の字幅](../../../docs/spec/metrics/数字の字幅.md)が別に測っている。
fn is_alnum(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

/// 和欧のあいだに置かれた空白と認めるか。
///
/// 半角スペースと全角スペースだけ。タブと改行は認めない——どちらも node の中の
/// 折り返しとして現れるもので、桁揃えや自動折り返しで入る。機会に数えれば、
/// 改行位置の癖がこの指標に化ける。
fn is_gap(c: char) -> bool {
    c == ' ' || c == '\u{3000}'
}

/// 日本語の文字と英数字の境目か。約物を挟むものは含めない。
fn is_cross(a: char, b: char) -> bool {
    (text::is_japanese(a) && is_alnum(b)) || (is_alnum(a) && text::is_japanese(b))
}

/// 笑い。差し込んだ箇所の数。
///
/// 連続は 1 と数える。`www` は 3 ではなく 1。`w` 1 つも 1 と数える——回数ではなく、
/// 笑いを差し込んだ箇所の数を見たい。
#[must_use]
pub fn laughter(prose: &[Segment]) -> Counted {
    let ja = japanese(prose);
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
            // 直後が日本語の文字でない `草`。「雑草」を拾わない。
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
    Counted::density(n, ja)
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
    use crate::Measured;
    use kuchiyose_doc::node::Kind;

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

    fn value(m: impl Into<Measured>) -> f64 {
        m.into().value().expect("測れているべき")
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
    fn 和文間スペースはふつう_0_である() {
        // 日本語は分かち書きをしないので、字の間に空白は入らない。
        let p = prose_with("");
        assert_eq!(wabun_space(&p), Measured::Value(0.0));
    }

    #[test]
    fn 和文間スペースは入った箇所を数える() {
        // 通してはいけないものが通る穴を塞ぐ。
        let p = prose_with(&"複数の リポジトリ を扱う。".repeat(6));
        let Measured::Value(v) = wabun_space(&p).measured() else {
            panic!("測れる");
        };
        assert!(v > 0.0, "{v}");
    }

    #[test]
    fn 和文間スペースは機会が少なければ測らない() {
        // 割合が跳ねるので、機会が下限に届かない文書では測らない。
        let p = vec![Segment {
            kind: Kind::Paragraph,
            text: "短い文である。".to_owned(),
        }];
        assert_eq!(wabun_space(&p), Measured::BelowFloor);
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

    /// 絵文字の数だけを取る。分母は同じなので、比べられる。
    fn emoji_count(extra: &str) -> f64 {
        value(emoji(&prose_with(extra)))
    }

    #[test]
    fn 国旗は_1_つと数える() {
        // 地域指標符号の対である。符号位置で数えると 2 になる。
        assert!((emoji_count("🇯🇵") - emoji_count("😀")).abs() < 1e-9);
    }

    #[test]
    fn zwj_で繋いだ列は_1_つと数える() {
        // 家族の絵文字は符号位置で数えると 5 以上になる。
        let family = "👨\u{200D}👩\u{200D}👧\u{200D}👦";
        assert!((emoji_count(family) - emoji_count("😀")).abs() < 1e-9);
    }

    #[test]
    fn 肌の色の指定は_1_つと数える() {
        assert!((emoji_count("👍\u{1F3FB}") - emoji_count("😀")).abs() < 1e-9);
    }

    #[test]
    fn keycap_は_1_つと数える() {
        // 数字・異体字セレクタ・囲みの 3 つで 1 つである。
        assert!((emoji_count("1\u{FE0F}\u{20E3}") - emoji_count("😀")).abs() < 1e-9);
    }

    #[test]
    fn 並んだ絵文字は別々に数える() {
        // 畳みすぎれば、3 つ置いた書き手が 1 つ置いたことになる。
        let one = emoji_count("😀");
        assert!((emoji_count("😀😀😀") - one * 3.0).abs() < 1e-9);
    }

    #[test]
    fn 絵文字でない文字は数えない() {
        assert!(emoji_count("あいうえお").abs() < 1e-9);
    }

    #[test]
    fn 全角スペースも空白として数える() {
        let mut text = String::new();
        for _ in 0..20 {
            text.push_str("あ　a。");
        }
        let p = prose_with(&text);
        assert!(
            value(missing_space(&p)).abs() < 1e-9,
            "全角スペースを空白と認めなければ 1.0 になる: {}",
            value(missing_space(&p))
        );
    }

    #[test]
    fn 空白が複数でも機会は_1_つである() {
        // 1 文字ずつ見ていた頃は、2 つ並ぶと機会から消えて値が上がった。
        let mut text = String::new();
        for _ in 0..20 {
            text.push_str("あ  a。");
        }
        let p = prose_with(&text);
        assert!(
            value(missing_space(&p)).abs() < 1e-9,
            "{}",
            value(missing_space(&p))
        );
    }

    #[test]
    fn 全角英数字は機会にしない() {
        // 和文と同じ字幅で組まれるのでベタ組みが普通である。数えれば、
        // 全角を選んだというだけで欠落率が上がる。
        let mut text = String::new();
        for _ in 0..20 {
            text.push_str("あＡ。");
        }
        let p = prose_with(&text);
        assert_eq!(missing_space(&p), Measured::BelowFloor);
    }

    #[test]
    fn タブは空白と認めない() {
        // node の中の折り返しであって、書き手が和欧の間に置いた空白ではない。
        let mut text = String::new();
        for _ in 0..20 {
            text.push_str("あ\ta。");
        }
        let p = prose_with(&text);
        assert_eq!(missing_space(&p), Measured::BelowFloor, "機会にしない");
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
