//! 書き手が打っていない空白を、地の文に作らない。
//!
//! 正規化は空白を増やす。 取り込み元が折り返した改行、落とした行内コードの跡
//! ——どちらも書き手の打鍵ではないのに、そのままだと地の文の空白になる。
//!
//! 残すと 2 つ壊れる。
//!
//! | | |
//! | --- | --- |
//! | 取り込み元が値を動かす | 同じ文が、書き出し側の折り返し方だけで別の値になる |
//! | 題材が値を動かす | 行内コードの多い記事ほど、跡の空白が増える |
//!
//! [題材で判定が覆ってはいけない](../../../docs/spec/100-metrics.md#題材で判定が覆ってはいけない)。
//!
//! 手で打った空白には触れない。 印の付いていない空白は
//! [表記](../../../docs/spec/030-normalize.md#表記を潰さない)である。

use kakiburi_doc::text;

/// 書き手が打っていない空白に立てる印。組んでいるあいだだけ立ち、[`collapse`]で消える。
///
/// 印が要るのは、表示だけでは見分けられないからである。折り返しも、落とした
/// 行内コードの跡も、手で打った空白も、表示では同じ 1 個の空白になる。
pub const WRAP: char = '\u{1}';

/// 印を文字列にしたもの。[`str::replace`]に渡す。
pub const WRAP_STR: &str = "\u{1}";

/// 印の付いた空白を始末する。
///
/// | | どうするか |
/// | --- | --- |
/// | 和文どうしのあいだ | 消す。 日本語は分かち書きをしないので、そこに空白は表示されない |
/// | それ以外 | 空白 1 個にする。 表示ではそうなる |
///
/// 区分の端と改行に隣り合うぶんも消える——どちらも表示されない。
///
/// 実測で、はてなの書き出し 27 本に和文を割る折り返しが 935 箇所あった。始末する前は
/// [和文間スペース](../../kakiburi-metrics/src/symbol.rs)が本人の Markdown で 0.0015、
/// 同じ本人の HTML で 0.0225 と出ていた——差は書きぶりではなく書き出し側である。
#[must_use]
pub fn collapse(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let run = chars[i..]
            .iter()
            .take_while(|&&c| c == WRAP || c == ' ' || c == '\t')
            .count();
        if run == 0 || !chars[i..i + run].contains(&WRAP) {
            // 印が無いなら、書き手が打った空白である。
            out.push(chars[i]);
            i += 1;
            continue;
        }
        if !(eaten(out.chars().next_back()) && eaten(chars.get(i + run).copied())) {
            out.push(' ');
        }
        i += run;
    }
    out
}

/// この文字に隣り合う空白は、表示されないか。
fn eaten(c: Option<char>) -> bool {
    c.is_none_or(|c| c == '\n' || text::is_wabun(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(s: &str) -> String {
        collapse(&s.replace('|', WRAP_STR))
    }

    #[test]
    fn 和文どうしのあいだの印は消える() {
        assert_eq!(
            c("これは日本語の|文章である。"),
            "これは日本語の文章である。"
        );
    }

    #[test]
    fn 約物どうしのあいだの印も消える() {
        assert_eq!(
            c("そう書いた。|だから直した。"),
            "そう書いた。だから直した。"
        );
    }

    #[test]
    fn 和欧のあいだの印は空白になる() {
        assert_eq!(
            c("これは Vim|plugin である。"),
            "これは Vim plugin である。"
        );
    }

    #[test]
    fn 印の付いていない空白は残る() {
        assert_eq!(c("これは 日本語 である。"), "これは 日本語 である。");
    }

    #[test]
    fn 印に隣り合う空白もまとめて始末する() {
        // 落とした行内コードの跡は、両側の空白ごと 1 つの並びになる。
        assert_eq!(c("設定は | である。"), "設定はである。");
        assert_eq!(c("set the | flag"), "set the flag");
    }

    #[test]
    fn 端と改行に隣り合う印は消える() {
        assert_eq!(c("|あ|"), "あ");
        assert_eq!(c("前\n|後"), "前\n後");
    }
}
