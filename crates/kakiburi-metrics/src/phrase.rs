//! 手で選んだ語句で数える指標。
//!
//! **どれも一覧が定義の一部である。** 増減させたら、それは別の指標になる——
//! だから一覧をここに置き、[定義ファイル](../../../docs/spec/metrics/)と同じ順で並べる。
//!
//! **最長一致で取り、重ならない。** 一覧には短いものが長いものに含まれる組がある
//! （`ではなく` ⊂ `ではなくて`）。決めないと、1 か所が 1 とも 2 とも数えられる。
//!
//! 形態素解析を要らない。字句をそのまま照合する。

use kakiburi_doc::prose::Segment;
use kakiburi_doc::sentence;

use crate::directive::Counted;

/// 非断定の密度。**文末がこれで終わる数。**
///
/// 一覧はこの定義の一部である。`だろう` ⊂ `と言えるだろう` のように含む組があるので、
/// **1 つの文末から取るのは 1 つ**である。
pub const HEDGES: [&str; 12] = [
    "かもしれない",
    "かもしれません",
    "と思われる",
    "のではないか",
    "のではないでしょうか",
    "だろう",
    "でしょう",
    "ようだ",
    "ようです",
    "気がする",
    "はずだ",
    "と言えるだろう",
];

/// 対比構文。**地の文に現れる数。**
pub const CONTRASTS: [&str; 10] = [
    "ではなく",
    "ではなくて",
    "のではなく",
    "というより",
    "というよりも",
    "だけでなく",
    "のに対し",
    "に対して",
    "ではあるが",
    "とは違って",
];

/// 自己否定の密度。**文の冒頭がこれである数。**
///
/// 「しかし」「だが」は入れない。あれは話の向きを変える接続であって、自分の主張への
/// 留保とは限らない。**入れると出現数が跳ね上がり、普通の逆接を測ることになる。**
pub const SELF_NEGATIONS: [&str; 7] = [
    "とはいえ",
    "とはいうものの",
    "もっとも",
    "そうは言っても",
    "そうは言うものの",
    "とはいっても",
    "ただし",
];

/// 進行の実況。**文がこれを含む数。**
pub const NARRATIONS: [&str; 11] = [
    "本記事では",
    "この記事では",
    "本稿では",
    "ここからは",
    "ここまでで",
    "以下では",
    "順に見て",
    "前置きが長く",
    "まとめると",
    "話を戻すと",
    "本題に入る",
];

/// 脱線。**文がこれで始まる数。**
///
/// 「なお」は入れない。単独では補足一般に使われ、脱線に限らない。
pub const DIGRESSIONS: [&str; 10] = [
    "ちなみに",
    "余談だが",
    "余談ですが",
    "余談だけど",
    "そういえば",
    "話は逸れるが",
    "脇道に逸れるが",
    "蛇足だが",
    "どうでもいいが",
    "なお余談",
];

/// 非断定の密度。
#[must_use]
pub fn hedging(prose: &[Segment]) -> Counted {
    per_1000(prose, count_hedging)
}

/// 非断定の数。**文末で見る。**
///
/// 終端記号を外さないと、「かもしれない。」が一覧と一致せず、値が永久に 0 になる。
/// **1 つの文末から取るのは 1 つである**——`だろう` ⊂ `と言えるだろう`。
#[must_use]
pub fn count_hedging(prose: &[Segment]) -> usize {
    sentence::sentences_of(prose)
        .iter()
        .filter(|s| ends_with_any(sentence::ending(s), &HEDGES))
        .count()
}

/// 対比構文。
#[must_use]
pub fn contrast(prose: &[Segment]) -> Counted {
    per_1000(prose, count_contrast)
}

/// 対比の数。**地の文で数える。** 文ではないので、1 文に 2 つあれば 2 である。
#[must_use]
pub fn count_contrast(prose: &[Segment]) -> usize {
    prose
        .iter()
        .map(|s| count_longest(&s.text, &CONTRASTS))
        .sum()
}

/// 自己否定の密度。
#[must_use]
pub fn self_negation(prose: &[Segment]) -> Counted {
    per_1000(prose, count_self_negation)
}

/// 自己否定の数。**文の冒頭だけ見る。**
#[must_use]
pub fn count_self_negation(prose: &[Segment]) -> usize {
    sentence::sentences_of(prose)
        .iter()
        .filter(|s| starts_with_any(s.trim_start(), &SELF_NEGATIONS))
        .count()
}

/// 進行の実況。
#[must_use]
pub fn narration(prose: &[Segment]) -> Counted {
    per_1000(prose, count_narration)
}

/// 進行の実況の数。**1 つの文に 2 つ含まれても 1 と数える。**
#[must_use]
pub fn count_narration(prose: &[Segment]) -> usize {
    sentence::sentences_of(prose)
        .iter()
        .filter(|s| NARRATIONS.iter().any(|w| s.contains(w)))
        .count()
}

/// 脱線。
#[must_use]
pub fn digression(prose: &[Segment]) -> Counted {
    per_1000(prose, count_digression)
}

/// 脱線の数。**文の冒頭だけ見る。**
#[must_use]
pub fn count_digression(prose: &[Segment]) -> usize {
    sentence::sentences_of(prose)
        .iter()
        .filter(|s| starts_with_any(s.trim_start(), &DIGRESSIONS))
        .count()
}

/// 数えて、日本語 1,000 字あたりにする。
fn per_1000(prose: &[Segment], count: impl Fn(&[Segment]) -> usize) -> Counted {
    Counted::density(count(prose), japanese(prose))
}

fn japanese(prose: &[Segment]) -> usize {
    kakiburi_doc::prose::japanese_chars(prose)
}

fn ends_with_any(s: &str, list: &[&str]) -> bool {
    list.iter().any(|w| s.ends_with(w))
}

fn starts_with_any(s: &str, list: &[&str]) -> bool {
    list.iter().any(|w| s.starts_with(w))
}

/// 最長一致で、重ならないように数える。
///
/// 左から走査し、いちばん長く一致するものを 1 つ取って、その先から続ける。
/// **決めないと、1 か所が 1 とも 2 とも数えられる。**
fn count_longest(text: &str, list: &[&str]) -> usize {
    let mut n = 0usize;
    let mut at = 0usize;
    while at < text.len() {
        if !text.is_char_boundary(at) {
            at += 1;
            continue;
        }
        let rest = &text[at..];
        let hit = list
            .iter()
            .filter(|w| rest.starts_with(**w))
            .max_by_key(|w| w.len());
        match hit {
            Some(w) => {
                n += 1;
                at += w.len();
            }
            None => at += rest.chars().next().map_or(1, char::len_utf8),
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Measured;
    use kakiburi_doc::node::Kind;

    /// 下限を越える長さの地の文に、試す文字列を足す。
    fn prose_with(extra: &str) -> Vec<Segment> {
        vec![
            Segment {
                kind: Kind::Paragraph,
                text: "これは日本語の文章である。".repeat(100),
            },
            Segment {
                kind: Kind::Paragraph,
                text: extra.to_owned(),
            },
        ]
    }

    fn value(m: impl Into<Measured>) -> f64 {
        m.into().value().expect("測れている")
    }

    /// 数えるだけの地の文。**下限は掛けないので、数そのものを比べられる。**
    fn only(text: &str) -> Vec<Segment> {
        vec![Segment {
            kind: Kind::Paragraph,
            text: text.to_owned(),
        }]
    }

    #[test]
    fn 短ければ測らない() {
        // 0 を返せば「使わなかった」と読まれる。
        let short = vec![Segment {
            kind: Kind::Paragraph,
            text: "短い。".into(),
        }];
        assert_eq!(hedging(&short), Measured::BelowFloor);
        assert_eq!(contrast(&short), Measured::BelowFloor);
        assert_eq!(self_negation(&short), Measured::BelowFloor);
        assert_eq!(narration(&short), Measured::BelowFloor);
        assert_eq!(digression(&short), Measured::BelowFloor);
    }

    #[test]
    fn 非断定は文末で見る() {
        // 終端記号を外さないと「かもしれない。」が一致せず、値が永久に 0 になる。
        assert!(value(hedging(&prose_with("そうかもしれない。"))) > 0.0);
    }

    #[test]
    fn 非断定は_1_つの文末から_1_つだけ取る() {
        // `だろう` ⊂ `と言えるだろう`。2 と数えてはいけない。
        assert_eq!(count_hedging(&only("そう言えるだろう。")), 1);
        assert_eq!(count_hedging(&only("そうだろう。")), 1);
    }

    #[test]
    fn 非断定は文中に来ても数えない() {
        // 文末で終わるものを見る。
        assert!(value(hedging(&prose_with("だろうと思って書いた。"))).abs() < 1e-9);
    }

    #[test]
    fn 対比は最長一致で重ならない() {
        // `ではなく` ⊂ `ではなくて`。1 か所を 2 と数えない。
        assert_eq!(count_contrast(&only("それではなくて、これである。")), 1);
        assert_eq!(count_contrast(&only("それではなく、これである。")), 1);
    }

    #[test]
    fn 対比は地の文で数える() {
        // 文ではないので、1 文に 2 つあれば 2 と数える。
        assert_eq!(count_contrast(&only("Aではなく B である。")), 1);
        assert_eq!(
            count_contrast(&only("Aではなく B、Cではなく D である。")),
            2
        );
    }

    #[test]
    fn 自己否定は文の冒頭だけ見る() {
        assert!(value(self_negation(&prose_with("とはいえ、そうである。"))) > 0.0);
        assert!(value(self_negation(&prose_with("それはとはいえない。"))).abs() < 1e-9);
    }

    #[test]
    fn 自己否定に普通の逆接を入れない() {
        // 入れると出現数が跳ね上がり、この指標が普通の逆接を測ることになる。
        assert!(value(self_negation(&prose_with("しかし、そうである。"))).abs() < 1e-9);
        assert!(value(self_negation(&prose_with("だが、そうである。"))).abs() < 1e-9);
    }

    #[test]
    fn 進行の実況は文を数える() {
        // 1 つの文に 2 つ含まれても 1 である。
        assert_eq!(count_narration(&only("本記事では説明する。")), 1);
        assert_eq!(count_narration(&only("本記事では、以下では説明する。")), 1);
        assert_eq!(
            count_narration(&only("本記事では説明する。以下では例を出す。")),
            2,
            "別の文なら 2 である"
        );
    }

    #[test]
    fn 脱線は文の冒頭だけ見る() {
        assert!(value(digression(&prose_with("ちなみに、そうである。"))) > 0.0);
        assert!(value(digression(&prose_with("これはちなみに、である。"))).abs() < 1e-9);
    }

    #[test]
    fn 脱線に単独のなおを入れない() {
        // 単独では補足一般に使われ、脱線に限らない。
        assert!(value(digression(&prose_with("なお、そうである。"))).abs() < 1e-9);
        assert!(value(digression(&prose_with("なお余談だが、そうである。"))) > 0.0);
    }

    #[test]
    fn 一覧は定義と同じ数である() {
        // 増減させたら、それは別の指標になる。
        assert_eq!(HEDGES.len(), 12);
        assert_eq!(CONTRASTS.len(), 10);
        assert_eq!(SELF_NEGATIONS.len(), 7);
        assert_eq!(NARRATIONS.len(), 11);
        assert_eq!(DIGRESSIONS.len(), 10);
    }
}
