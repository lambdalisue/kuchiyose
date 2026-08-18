//! 文と文末。
//!
//! 半角も切る。字幅は書き手の選択であって潰さない。全角だけで切れば、半角を
//! 使う書き手の文が黙って連結され、文を数えるものすべてが狂う。

use crate::prose::Segment;
use crate::text;

/// 終端記号。半角を含む。
const TERMINATORS: [char; 5] = ['。', '！', '？', '!', '?'];

/// 終端記号の直後に続けば同じ文に含める閉じ括弧。
const CLOSERS: [char; 4] = ['」', '』', '）', '"'];

/// 開き括弧と、それに対応する閉じ括弧。
///
/// 括弧の内側の `。` では切らない。引用の中の文を、外側の文として数えない。
const BRACKETS: [(char, char); 5] = [
    ('「', '」'),
    ('『', '』'),
    ('（', '）'),
    ('(', ')'),
    ('\u{201C}', '\u{201D}'),
];

fn opener_of(c: char) -> Option<char> {
    BRACKETS.iter().find(|(o, _)| *o == c).map(|(_, cl)| *cl)
}

/// 1 本の地の文を文に切る。
///
/// node を跨がない。node の末尾は、終端記号が無くても文の終わりである——
/// 見出しや箇条書きの項目は `。` で終わらないことが多く、0 文として捨てると
/// 見出しの多い文書ほど文が少ないことになり、構造の癖が文の癖に化ける。
#[must_use]
pub fn sentences(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut stack: Vec<char> = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        cur.push(c);
        i += 1;

        if let Some(close) = opener_of(c) {
            stack.push(close);
            continue;
        }
        if stack.last() == Some(&c) {
            stack.pop();
            continue;
        }
        // 括弧の内側では切らない。
        if !stack.is_empty() || !TERMINATORS.contains(&c) {
            continue;
        }
        // 連続する終端記号は同じ文の末尾に付ける（`！？` など）。
        while i < chars.len() && TERMINATORS.contains(&chars[i]) {
            cur.push(chars[i]);
            i += 1;
        }
        // 直後の閉じ括弧までを同じ文に含める。
        while i < chars.len() && CLOSERS.contains(&chars[i]) {
            cur.push(chars[i]);
            i += 1;
        }
        out.push(std::mem::take(&mut cur));
    }

    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out.retain(|s| s.chars().any(text::is_japanese));
    out
}

/// 地の文全体の文。node を跨がない。
#[must_use]
pub fn sentences_of(prose: &[Segment]) -> Vec<String> {
    prose.iter().flat_map(|s| sentences(&s.text)).collect()
}

/// 文末。終端記号と、それに続く閉じ括弧を外した末尾。
///
/// 外さずに照合すれば、「かもしれない」で終わる文は「かもしれない。」なので
/// 一致せず、値が永久に 0 になる。エラーにはならない。
///
/// 外すのは末尾の終端記号と閉じ括弧だけである。文中の記号は残す。
#[must_use]
pub fn ending(sentence: &str) -> &str {
    sentence.trim_end_matches(|c| TERMINATORS.contains(&c) || CLOSERS.contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 半角の終端記号でも切る() {
        // 全角だけで切ると、半角を使う書き手の文が黙って連結される。
        let s = sentences("これは本当か? そうらしい! なるほど。");
        assert_eq!(s.len(), 3, "{s:?}");
    }

    #[test]
    fn 直後の閉じ括弧は同じ文に含める() {
        let s = sentences("彼は「そうだ。」と言った。");
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0], "彼は「そうだ。」と言った。");
    }

    #[test]
    fn 三点リーダでは切らない() {
        // 言いさしであって文の終わりではない。
        let s = sentences("そうかもしれない……ただ分からない。");
        assert_eq!(s.len(), 1, "{s:?}");
    }

    #[test]
    fn 括弧の内側の句点では切らない() {
        let s = sentences("引用（これは中の文である。）を示す。");
        assert_eq!(s.len(), 1, "{s:?}");
    }

    #[test]
    fn 半角括弧の内側でも切らない() {
        let s = sentences("注記(これは中である。)を置く。");
        assert_eq!(s.len(), 1, "{s:?}");
    }

    #[test]
    fn 終端記号が無い_node_も_1_文である() {
        // 見出しや項目は `。` で終わらない。0 文にすると構造の癖が文の癖に化ける。
        let s = sentences("実装に効く三つ");
        assert_eq!(s.len(), 1, "{s:?}");
        assert_eq!(s[0], "実装に効く三つ");
    }

    #[test]
    fn 連続する終端記号はまとめる() {
        let s = sentences("本当に？！ そうだ。");
        assert_eq!(s.len(), 2, "{s:?}");
        assert_eq!(s[0], "本当に？！");
    }

    #[test]
    fn 文末は終端記号を外す() {
        // 外さないと「かもしれない」と照合できず、値が永久に 0 になる。
        assert_eq!(ending("ここが違うかもしれない。"), "ここが違うかもしれない");
        assert_eq!(ending("そうだろうか？"), "そうだろうか");
        assert_eq!(ending("そうだ！"), "そうだ");
        assert_eq!(ending("本当に？！"), "本当に");
    }

    #[test]
    fn 文末は閉じ括弧も外す() {
        assert_eq!(ending("と言った「そうだ。」"), "と言った「そうだ");
    }

    #[test]
    fn 文末は文中の記号を残す() {
        assert_eq!(ending("これは（たぶん）そうだ。"), "これは（たぶん）そうだ");
    }

    #[test]
    fn 日本語を含まない断片は文に数えない() {
        let s = sentences("OK. Fine.");
        assert!(s.is_empty(), "{s:?}");
    }
}
