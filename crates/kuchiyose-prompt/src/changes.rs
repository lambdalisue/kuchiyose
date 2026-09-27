//! 2 つの版のあいだで変えたところ（[仕様](../../../docs/spec/400-write.md#捨てた直しを伝える)）。
//!
//! 文を単位に比べ、変わった文の並びごとに、変える前と後の断片を 1 組にする。
//! 断片は、前後で同じ字を落とし、前後に少しだけ地の文を残して切る。 採らなかった直しを
//! 道具に伝えるための要約で、差分そのものではない。

/// 変えたところ 1 か所。どちらかが空なら、足したか消したかである。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// 変える前の断片。
    pub before: String,
    /// 変えた後の断片。
    pub after: String,
}

/// 変えたところを 1 か所ごとに見せるとき、変わった字の前後に残す字数。暫定値である。
///
/// 実測の直しは文末の言い換えが多く、その語幹が見える長さにした。
pub const CONTEXT: usize = 8;

/// 断片の長さの上限（字）。これを超えた断片は切って「…」を付ける。暫定値である。
pub const MAX_FRAGMENT: usize = 60;

/// 文の数の積がこれを超えれば、並びを揃えずに、前後で同じ文を落とした残りを 1 か所とする。
///
/// 揃えるのに要る表の大きさがこの積である。 版どうしの差は小さいので、前後で同じ文を
/// 落とした残りがここに届くことはまず無い。
const MAX_TABLE: usize = 4_000_000;

/// 文に割る。文末の約物か改行で切り、直後の閉じ括弧は前の文に付ける。
fn sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut ended = false;
    for (i, c) in text.char_indices() {
        if ended && !matches!(c, '」' | '』' | '）' | ')' | '"') {
            out.push(&text[start..i]);
            start = i;
            ended = false;
        }
        if matches!(c, '。' | '！' | '？' | '!' | '?' | '\n') {
            ended = true;
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// 変わった文の並び。`(前の範囲, 後の範囲)` を文書の順に返す。
fn hunks(a: &[&str], b: &[&str]) -> Vec<(std::ops::Range<usize>, std::ops::Range<usize>)> {
    let head = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let tail = a[head..]
        .iter()
        .rev()
        .zip(b[head..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (a_mid, b_mid) = (&a[head..a.len() - tail], &b[head..b.len() - tail]);
    if a_mid.is_empty() && b_mid.is_empty() {
        return Vec::new();
    }
    let (n, m) = (a_mid.len(), b_mid.len());
    if n.saturating_mul(m) > MAX_TABLE || n == 0 || m == 0 {
        return vec![(head..head + n, head..head + m)];
    }
    // 最長共通部分列。 lcs[i][j] は a_mid[i..] と b_mid[j..] の長さ。
    let w = m + 1;
    let mut lcs = vec![0u32; (n + 1) * w];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i * w + j] = if a_mid[i] == b_mid[j] {
                lcs[(i + 1) * w + j + 1] + 1
            } else {
                lcs[(i + 1) * w + j].max(lcs[i * w + j + 1])
            };
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    let (mut from_i, mut from_j) = (0, 0);
    let mut open = false;
    while i < n || j < m {
        if i < n && j < m && a_mid[i] == b_mid[j] {
            if open {
                out.push((head + from_i..head + i, head + from_j..head + j));
                open = false;
            }
            i += 1;
            j += 1;
            continue;
        }
        if !open {
            (from_i, from_j, open) = (i, j, true);
        }
        // 同じ長さなら消すほうを先に進める。 どちらを選んでも決まった答えになる。
        if j >= m || (i < n && lcs[(i + 1) * w + j] >= lcs[i * w + j + 1]) {
            i += 1;
        } else {
            j += 1;
        }
    }
    if open {
        out.push((head + from_i..head + n, head + from_j..head + m));
    }
    out
}

/// 断片を 1 行に収める。改行は空白にし、端の空白は落とす。
fn one_line(chars: &[char]) -> String {
    let s: String = chars
        .iter()
        .map(|c| if *c == '\n' { ' ' } else { *c })
        .collect();
    s.trim().to_owned()
}

/// 長すぎる断片を切る。
fn clip(s: String) -> String {
    if s.chars().count() <= MAX_FRAGMENT {
        return s;
    }
    let mut out: String = s.chars().take(MAX_FRAGMENT).collect();
    out.push('…');
    out
}

/// 変わった文の並びを 1 か所の断片にする。前後で同じ字を落とし、[`CONTEXT`] 字だけ残す。
fn fragment(before: &str, after: &str) -> Change {
    let (x, y): (Vec<char>, Vec<char>) = (before.chars().collect(), after.chars().collect());
    let prefix = x.iter().zip(&y).take_while(|(a, b)| a == b).count();
    let room = x.len().min(y.len()) - prefix;
    let suffix = x
        .iter()
        .rev()
        .zip(y.iter().rev())
        .take(room)
        .take_while(|(a, b)| a == b)
        .count();
    let from = prefix.saturating_sub(CONTEXT);
    let cut = |v: &[char]| {
        let to = (v.len() - suffix + CONTEXT).min(v.len());
        let mut s = String::new();
        if from > 0 {
            s.push('…');
        }
        s.push_str(&one_line(&v[from..to]));
        if to < v.len() && !one_line(&v[to..]).is_empty() {
            s.push('…');
        }
        s
    };
    // 片方が空なら、足したか消したかである。 前後の地の文は付けない。
    if x.iter().collect::<String>().trim().is_empty() {
        return Change {
            before: String::new(),
            after: clip(one_line(&y)),
        };
    }
    if y.iter().collect::<String>().trim().is_empty() {
        return Change {
            before: clip(one_line(&x)),
            after: String::new(),
        };
    }
    Change {
        before: clip(cut(&x)),
        after: clip(cut(&y)),
    }
}

/// `before` から `after` への変えたところを、文書の順に返す。
///
/// 空白と改行だけの違いは数えない。 同じ材料からは同じ答えが出る。
#[must_use]
pub fn changes(before: &str, after: &str) -> Vec<Change> {
    let (a, b) = (sentences(before), sentences(after));
    hunks(&a, &b)
        .into_iter()
        .filter_map(|(ra, rb)| {
            let (x, y) = (a[ra].concat(), b[rb].concat());
            let squash = |s: &str| s.split_whitespace().collect::<String>();
            (squash(&x) != squash(&y)).then(|| fragment(&x, &y))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(before: &str, after: &str) -> Change {
        Change {
            before: before.into(),
            after: after.into(),
        }
    }

    #[test]
    fn 同じ文章なら変えたところは無い() {
        let text = "一文目です。二文目です。\n";
        assert!(changes(text, text).is_empty());
    }

    #[test]
    fn 文末を言い換えた文は変わった字の前後だけを残す() {
        let before =
            "前置きの文です。このツールは日本語の技術記事の文章に特化しています。後の文です。";
        let after = "前置きの文です。このツールは日本語の技術記事の文章に特化したものになります。後の文です。";
        assert_eq!(
            changes(before, after),
            vec![change(
                "…事の文章に特化しています。",
                "…事の文章に特化したものになります。"
            )]
        );
    }

    #[test]
    fn 離れた文の変更は別々に文書の順で数える() {
        let before = "あれを決めます。間の文。これを数えます。";
        let after = "あれを決めています。間の文。これを数えています。";
        assert_eq!(
            changes(before, after),
            vec![
                change("あれを決めます。", "あれを決めています。"),
                change("これを数えます。", "これを数えています。"),
            ]
        );
    }

    #[test]
    fn 足した文と消した文は片方を空にする() {
        assert_eq!(
            changes("一。三。", "一。二。三。"),
            vec![change("", "二。")]
        );
        assert_eq!(
            changes("一。二。三。", "一。三。"),
            vec![change("二。", "")]
        );
    }

    #[test]
    fn 空白と改行だけの違いは数えない() {
        assert!(changes(
            "一文目です。\n二文目です。\n",
            "一文目です。\n\n二文目です。\n"
        )
        .is_empty());
    }

    #[test]
    fn 閉じ括弧は前の文に付ける() {
        assert_eq!(
            sentences("「そうです。」と言った。"),
            vec!["「そうです。」", "と言った。"]
        );
    }

    #[test]
    fn 長い断片は上限で切る() {
        let long = "あ".repeat(MAX_FRAGMENT * 2);
        let got = changes("短い。", &format!("短い。{long}。"));
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].after.chars().count(), MAX_FRAGMENT + 1);
        assert!(got[0].after.ends_with('…'));
    }

    #[test]
    fn 同じ材料からは同じ答えが出る() {
        let (a, b) = ("甲です。乙です。丙です。", "甲でした。乙です。丁です。");
        assert_eq!(changes(a, b), changes(a, b));
    }
}
