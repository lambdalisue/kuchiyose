//! 地の文。
//!
//! <strong>1 本の文字列ではない。</strong> node ごとに切れた、順序のある文字列の並びである。
//!
//! 連結しない。つなげば、見出しの末尾と次の段落の先頭が隣り合う。その隣接は
//! 書き手が選んだものではなく、構造が作った産物である。

use crate::node::{Kind, Node};
use crate::text;

/// 地の文の 1 本。node ひとつぶんの文字列。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// どの種類の node から来たか。
    pub kind: Kind,
    /// 文字列。入れ子の node の中身を、現れる順に 1 度だけ含む。
    pub text: String,
}

/// 識別子を伏せる。<strong>題材が書きぶりの値に入り込むのを止める。</strong>
///
/// `denops.vim` `main.ts` `Promise` のような半角英字の連なりは、<strong>書き手が選んだ
/// 書きぶりではなく題材が決めるもの</strong>である。それが繰り返し・語彙・文字 bigram に
/// そのまま入ると、<strong>同じ人が別の題材で書いた文章を「その人らしくない」と言う</strong>。
///
/// <strong>実測では、題材語だけを入れ替えて 5gram の最多率が 86% 動いた。</strong> 伏せると
/// 0.9% に落ちる。
///
/// <strong>消さずに 1 つの札に畳む。</strong> 消せば語数と位置が変わり、長さと連動する指標が
/// すべてずれる。
#[must_use]
pub fn mask_identifiers(prose: &[Segment]) -> Vec<Segment> {
    prose
        .iter()
        .map(|s| Segment {
            kind: s.kind,
            text: masked(&s.text),
        })
        .collect()
}

/// 伏せ字。
///
/// <strong>ひらがなにする。</strong> 半角英字のままだと字種の値が変わらない。
///
/// <strong>ふつうの語を使わない。</strong> 「それ」のような実在の語を伏せ字にすると、
/// <strong>伏せ字そのものが「本人が繰り返している言い回し」として指摘に出てくる</strong>
/// ——受け取った側に「それそれそれそれ」と書けと言うことになる。現代の文章に
/// 現れない仮名を使えば、実在の語と混ざらない。
pub const SENTINEL: &str = "ゐゑ";

fn masked(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = false;
    for c in text.chars() {
        // <strong>先頭は英字だけ。</strong> 数字から始まる並びは数量であって識別子ではない。
        let body =
            c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '/' | '#' | '@' | '-');
        if run && body {
            continue;
        }
        if !run && c.is_ascii_alphabetic() {
            out.push_str(SENTINEL);
            run = true;
            continue;
        }
        run = false;
        out.push(c);
    }
    out
}

#[cfg(test)]
mod mask_tests {
    use super::*;
    use crate::node::Kind;

    fn seg(t: &str) -> Segment {
        Segment {
            kind: Kind::Paragraph,
            text: t.to_owned(),
        }
    }

    #[test]
    fn 識別子は_1_つの札に畳まれる() {
        // <strong>題材が書きぶりの値に入り込むのを止める。</strong>
        let got = mask_identifiers(&[seg("denops.vim と main.ts を書く")]);
        assert_eq!(got[0].text, "ゐゑ と ゐゑ を書く");
    }

    #[test]
    fn 数字から始まる並びは畳まない() {
        // 数量は題材ではない。
        let got = mask_identifiers(&[seg("1,000 字の 2 割")]);
        assert_eq!(got[0].text, "1,000 字の 2 割");
    }

    #[test]
    fn 日本語は触らない() {
        let got = mask_identifiers(&[seg("これは日本語である。")]);
        assert_eq!(got[0].text, "これは日本語である。");
    }

    #[test]
    fn 消さずに畳む() {
        // <strong>消せば語数と位置が変わり、長さと連動する指標がすべてずれる。</strong>
        let got = mask_identifiers(&[seg("Vim を使う")]);
        assert!(!got[0].text.is_empty());
        assert!(got[0].text.contains("を使う"));
    }

    #[test]
    fn 種類は変えない() {
        let got = mask_identifiers(&[seg("Deno")]);
        assert_eq!(got[0].kind, Kind::Paragraph);
    }
}

/// node の並びから地の文を取り出す。
///
/// 入れ子の node は内側だけを数える。強調やリンクの中身は、それを含む段落の
/// 文字列の一部として 1 度だけ現れる。二重に数えない。
#[must_use]
pub fn prose(nodes: &[Node]) -> Vec<Segment> {
    let mut out = Vec::new();
    for n in nodes {
        collect(n, &mut out);
    }
    out
}

fn collect(n: &Node, out: &mut Vec<Segment>) {
    if n.kind.contributes_to_prose() {
        let text = flatten(n);
        // 日本語の無いセルは地の文ではない。比較表の `o` `-` `〇` `×` は
        // 日本語の散文ではなく、残せば表を作る選択が文字種に出てしまう。
        let keep = if n.kind == Kind::Cell {
            text::has_japanese_prose(&text)
        } else {
            !text.trim().is_empty()
        };
        if keep {
            out.push(Segment { kind: n.kind, text });
        }
        // テキストを持つ node の中の入れ子は flatten が畳んだ。だが構造を持つ
        // 子（項目・セルなど）は、それ自身が別の 1 本になる。
        for c in &n.children {
            if !is_inline(c.kind) {
                collect(c, out);
            }
        }
        return;
    }
    for c in &n.children {
        collect(c, out);
    }
}

/// 行の中に溶けこむ node か。段落の文字列の一部になる。
fn is_inline(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Emphasis | Kind::Link | Kind::InlineCode | Kind::Image
    )
}

/// この node の地の文。
///
/// <strong>行の中に溶けこむ子のテキストを足さない。</strong> 強調やリンクの中身は、それを含む
/// 段落の `text` に<strong>すでに 1 度だけ入っている</strong>——入れ子の node は数を数えるために
/// あり、文字列を足すためではない。足せば二重に数える。
///
/// <strong>そして文中の強調を表せるのはこの形だけである。</strong> `text` と子の並び順を持たない
/// 型で子のテキストを足すと、強調はつねに末尾に来てしまう。
fn flatten(n: &Node) -> String {
    n.text.clone()
}

/// 地の文の日本語の文字数。率の分母はこれである。
///
/// node を跨いで合計する。文字 bigram のように跨がないものと違い、分母は
/// 文書全体で 1 つである。
#[must_use]
pub fn japanese_chars(prose: &[Segment]) -> usize {
    prose.iter().map(|s| text::count_japanese(&s.text)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::Node;

    fn para(t: &str) -> Node {
        Node::leaf(Kind::Paragraph, t)
    }

    #[test]
    fn 地の文は連結しない() {
        let nodes = vec![Node::heading(1, "見出し"), para("本文である。")];
        let p = prose(&nodes);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].text, "見出し");
        assert_eq!(p[1].text, "本文である。");
    }

    #[test]
    fn コードブロックの中身は入らない() {
        let nodes = vec![
            para("説明。"),
            Node::leaf(Kind::CodeBlock, "let x = 1;"),
            para("続き。"),
        ];
        let p = prose(&nodes);
        assert_eq!(p.len(), 2);
        assert!(!p.iter().any(|s| s.text.contains("let")));
    }

    #[test]
    fn インラインコードの中身は入らない() {
        // 数えるための子は置くが、文字列は足さない。
        let mut n = para("設定は ");
        n.children.push(Node::leaf(Kind::InlineCode, "--force"));
        let p = prose(&[n]);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].text, "設定は ");
    }

    #[test]
    fn リンクは中身だけ入り参照先は入らない() {
        // 段落の text にすでに入っている。子は数えるためにある。
        let mut n = para("詳細は こちら");
        n.children.push(Node::leaf(Kind::Link, "こちら"));
        let p = prose(&[n]);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].text, "詳細は こちら");
    }

    #[test]
    fn 画像の代替文字は入らない() {
        let mut n = para("図。");
        n.children.push(Node::leaf(Kind::Image, "構成図"));
        let p = prose(&[n]);
        assert_eq!(p[0].text, "図。");
    }

    #[test]
    fn 強調は二重に数えない() {
        // 段落の text に「大事」が 1 度入っている。子はそれを繰り返さない。
        let mut n = Node::leaf(Kind::Paragraph, "ここが大事である");
        n.children.push(Node::leaf(Kind::Emphasis, "大事"));
        let p = prose(&[n]);
        assert_eq!(p.len(), 1, "強調が別の 1 本になってはいけない");
        assert_eq!(p[0].text, "ここが大事である", "二重に入れてはいけない");
    }

    #[test]
    fn セルは_1_つずつ別の_1_本になる() {
        // まとめれば桁揃えの空白が地の文に入り、隣のセルの先頭が前のセルの
        // 末尾と隣り合う。
        let table = Node::branch(
            Kind::Table,
            vec![
                Node::leaf(Kind::Cell, "機能"),
                Node::leaf(Kind::Cell, "あり"),
            ],
        );
        let p = prose(&[table]);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].text, "機能");
        assert_eq!(p[1].text, "あり");
    }

    #[test]
    fn 日本語の無いセルは地の文に入らない() {
        let table = Node::branch(
            Kind::Table,
            vec![
                Node::leaf(Kind::Cell, "リモート"),
                Node::leaf(Kind::Cell, "o"),
                Node::leaf(Kind::Cell, "-"),
                Node::leaf(Kind::Cell, "〇"),
                Node::leaf(Kind::Cell, "ー"),
            ],
        );
        let p = prose(&[table]);
        assert_eq!(p.len(), 1, "印だけのセルは落ちる");
        assert_eq!(p[0].text, "リモート");
    }

    #[test]
    fn 項目は箇条書きの下で_1_本ずつになる() {
        let list = Node::branch(
            Kind::Bullet,
            vec![
                Node::leaf(Kind::Item, "ひとつめ"),
                Node::leaf(Kind::Item, "ふたつめ"),
            ],
        );
        let p = prose(&[list]);
        assert_eq!(p.len(), 2);
    }

    #[test]
    fn 分母は約物を含まない() {
        let p = prose(&[para("これは、大事である。")]);
        // 「これは大事である」の 8 字。読点と句点は数えない。
        assert_eq!(japanese_chars(&p), 8);
    }

    #[test]
    fn 分母は_node_を跨いで合計する() {
        let p = prose(&[para("あいう"), para("えお")]);
        assert_eq!(japanese_chars(&p), 5);
    }
}
