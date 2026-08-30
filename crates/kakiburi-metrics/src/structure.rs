//! 構造と長さの指標。
//!
//! どれも node の数え方に依る。<strong>単位の意味は[文書の形](kakiburi_doc)が決める</strong>——
//! ここで決め直さない。

use kakiburi_doc::node::{Kind, Node};
use kakiburi_doc::{text, Document};

use crate::{floor, Measured};

/// node の数を、日本語 1,000 字あたりに直す。
fn nodes_per_1000(doc: &Document, kind: Kind) -> Measured {
    let ja = doc.japanese_chars();
    if ja < floor::JAPANESE_CHARS {
        return Measured::BelowFloor;
    }
    #[allow(clippy::cast_precision_loss)]
    Measured::Value(1000.0 * count(doc, kind) as f64 / ja as f64)
}

/// その種類の node の数。入れ子も数える。
fn count(doc: &Document, kind: Kind) -> usize {
    fn walk(nodes: &[Node], kind: Kind, n: &mut usize) {
        for x in nodes {
            if x.kind == kind {
                *n += 1;
            }
            walk(&x.children, kind, n);
        }
    }
    let mut n = 0;
    walk(&doc.nodes, kind, &mut n);
    n
}

/// 見出しの数。深さは問わない。
#[must_use]
pub fn headings(doc: &Document) -> Measured {
    nodes_per_1000(doc, Kind::Heading)
}

/// 箇条書きの数。順序のない列挙。
#[must_use]
pub fn bullets(doc: &Document) -> Measured {
    nodes_per_1000(doc, Kind::Bullet)
}

/// 番号リストの数。順序のある列挙。
#[must_use]
pub fn ordered_lists(doc: &Document) -> Measured {
    nodes_per_1000(doc, Kind::Ordered)
}

/// 表の数。
#[must_use]
pub fn tables(doc: &Document) -> Measured {
    nodes_per_1000(doc, Kind::Table)
}

/// 引用の数。
#[must_use]
pub fn quotes(doc: &Document) -> Measured {
    nodes_per_1000(doc, Kind::Quote)
}

/// 補足の数。<strong>記法は問わない。</strong>
#[must_use]
pub fn notes(doc: &Document) -> Measured {
    nodes_per_1000(doc, Kind::Note)
}

/// 警告の数。<strong>記法は問わない。</strong>
#[must_use]
pub fn warnings(doc: &Document) -> Measured {
    nodes_per_1000(doc, Kind::Warning)
}

/// 折りたたみの数。
#[must_use]
pub fn details(doc: &Document) -> Measured {
    nodes_per_1000(doc, Kind::Details)
}

/// 強調の数。
#[must_use]
pub fn emphasis(doc: &Document) -> Measured {
    nodes_per_1000(doc, Kind::Emphasis)
}

/// コードブロックの数。
///
/// <strong>分母の文字数にコードブロックの中身を含めない。</strong> 含めると、長いコードを 1 つ
/// 貼っただけで率が下がる。[地の文](kakiburi_doc::prose)がコードを含まないので、
/// ここは分母をそのまま使えばよい。
#[must_use]
pub fn code_blocks(doc: &Document) -> Measured {
    nodes_per_1000(doc, Kind::CodeBlock)
}

/// 深い見出しの数。深さ 3 以上。
#[must_use]
pub fn deep_headings(doc: &Document) -> Measured {
    let ja = doc.japanese_chars();
    if ja < floor::JAPANESE_CHARS {
        return Measured::BelowFloor;
    }
    let deep = doc
        .nodes
        .iter()
        .filter(|n| doc.heading_depth(n).is_some_and(|d| d >= 3))
        .count();
    #[allow(clippy::cast_precision_loss)]
    Measured::Value(1000.0 * deep as f64 / ja as f64)
}

/// 1 文だけの段落の割合。
#[must_use]
pub fn single_sentence_paragraphs(doc: &Document) -> Measured {
    let paras = doc.paragraphs();
    if paras.len() < 10 {
        return Measured::BelowFloor;
    }
    let one = paras
        .iter()
        .filter(|p| kakiburi_doc::sentence::sentences(&p.text).len() == 1)
        .count();
    #[allow(clippy::cast_precision_loss)]
    Measured::Value(one as f64 / paras.len() as f64)
}

/// 敬体で終わる文の割合。<strong>node の種類ごとに数える。</strong>
///
/// <strong>同じ書き手が、場所によって文体を変える。</strong> 実測で、ある書き手は段落の 93% を
/// 敬体で書き、箇条書きの 85%・見出しの 99% を常体で書いていた。
///
/// <strong>地の文を 1 つの袋にすると、この使い分けが混ざって消える</strong>
/// （[文体は node の種類ごとに違う](../../../docs/spec/100-metrics.md#文体は-node-の種類ごとに違う)）。
/// 「敬体 7 割の人」としか言えず、<strong>箇条書きを敬体で書いた草稿を見分けられない。</strong>
///
/// <strong>分母は敬体か常体で終わった文だけである。</strong> 体言止めと疑問符で終わる文はどちらでも
/// ないので数えない——入れると、体言止めの多い書き手ほど敬体率が下がる。
fn polite_rate(texts: &[String], floor: usize) -> Measured {
    let (mut polite, mut plain) = (0usize, 0usize);
    for t in texts {
        for s in kakiburi_doc::sentence::sentences(t) {
            match register_of(&s) {
                Some(true) => polite += 1,
                Some(false) => plain += 1,
                None => {}
            }
        }
    }
    if polite + plain < floor {
        return Measured::BelowFloor;
    }
    #[allow(clippy::cast_precision_loss)]
    Measured::Value(polite as f64 / (polite + plain) as f64)
}

/// その文が敬体か。<strong>どちらでもなければ `None`。</strong>
fn register_of(sentence: &str) -> Option<bool> {
    let t = sentence.trim_end_matches(['。', '！', '？', '.', '!', '?', ' ', '　']);
    const POLITE: [&str; 8] = [
        "です",
        "ます",
        "ました",
        "ません",
        "でしょう",
        "ましょう",
        "でした",
        "ください",
    ];
    const PLAIN: [&str; 10] = [
        "である",
        "だ",
        "した",
        "する",
        "ない",
        "いる",
        "なる",
        "った",
        "れる",
        "たい",
    ];
    if POLITE.iter().any(|w| t.ends_with(w)) {
        return Some(true);
    }
    if PLAIN.iter().any(|w| t.ends_with(w)) {
        return Some(false);
    }
    None
}

/// 段落の敬体率。
#[must_use]
pub fn polite_paragraphs(doc: &Document) -> Measured {
    let texts: Vec<String> = doc.paragraphs().iter().map(|p| p.text.clone()).collect();
    polite_rate(&texts, 10)
}

/// 箇条書きの項目の敬体率。
#[must_use]
pub fn polite_items(doc: &Document) -> Measured {
    let texts: Vec<String> = doc.items().iter().map(|p| p.text.clone()).collect();
    polite_rate(&texts, 5)
}

/// 段落あたりの文数。<strong>段落に含まれる</strong>文を、段落の数で割る。
///
/// 見出しや項目やセルの文を分子に入れない。
#[must_use]
pub fn sentences_per_paragraph(doc: &Document) -> Measured {
    let paras = doc.paragraphs();
    if paras.len() < floor::PARAGRAPHS {
        return Measured::BelowFloor;
    }
    let n: usize = paras
        .iter()
        .map(|p| kakiburi_doc::sentence::sentences(&p.text).len())
        .sum();
    #[allow(clippy::cast_precision_loss)]
    Measured::Value(n as f64 / paras.len() as f64)
}

/// 太字始まりの項目の割合。分母は項目数。
#[must_use]
pub fn bold_leading_items(doc: &Document) -> Measured {
    let items = doc.items();
    if items.len() < 10 {
        return Measured::BelowFloor;
    }
    let bold = items
        .iter()
        .filter(|i| i.children.first().is_some_and(|c| c.kind == Kind::Emphasis))
        .count();
    #[allow(clippy::cast_precision_loss)]
    Measured::Value(bold as f64 / items.len() as f64)
}

/// 段落長の変動係数。段落ごとの日本語文字数の標準偏差 ÷ 平均。
#[must_use]
pub fn paragraph_length_cv(doc: &Document) -> Measured {
    let paras = doc.paragraphs();
    if paras.len() < 10 {
        return Measured::BelowFloor;
    }
    cv(&paras
        .iter()
        .map(|p| text::count_japanese(&p.text))
        .collect::<Vec<_>>())
}

/// 箇条書き項目長の変動係数。
///
/// 文書内の全ての項目を <strong>1 つの分布</strong>として見る。箇条書きごとに分けない。
#[must_use]
pub fn item_length_cv(doc: &Document) -> Measured {
    let items = doc.items();
    if items.len() < 10 {
        return Measured::BelowFloor;
    }
    cv(&items
        .iter()
        .map(|i| text::count_japanese(&i.text))
        .collect::<Vec<_>>())
}

/// 節の長さの変動係数。
#[must_use]
pub fn section_length_cv(doc: &Document) -> Measured {
    let sections = doc.sections();
    if sections.len() < 5 {
        return Measured::BelowFloor;
    }
    let lens: Vec<usize> = sections
        .iter()
        .map(|s| kakiburi_doc::prose::japanese_chars(&kakiburi_doc::prose::prose(s.nodes)))
        .collect();
    cv(&lens)
}

/// 変動係数。標準偏差 ÷ 平均。
#[allow(clippy::cast_precision_loss)]
fn cv(values: &[usize]) -> Measured {
    if values.is_empty() {
        return Measured::BelowFloor;
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<usize>() as f64 / n;
    if mean <= 0.0 {
        // 平均が 0 では割れない。0 を返さない——測れていない。
        return Measured::BelowFloor;
    }
    let var = values
        .iter()
        .map(|&v| (v as f64 - mean).powi(2))
        .sum::<f64>()
        / n;
    Measured::Value(var.sqrt() / mean)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filler() -> Node {
        Node::leaf(Kind::Paragraph, "これは日本語の文章である。".repeat(100))
    }

    fn doc_with(mut nodes: Vec<Node>) -> Document {
        nodes.insert(0, filler());
        Document::new(nodes)
    }

    fn value(m: Measured) -> f64 {
        m.value().expect("測れているべき")
    }

    #[test]
    fn 補足と警告は記法を問わず数える() {
        let d = doc_with(vec![
            Node::leaf(Kind::Note, "補足である。"),
            Node::leaf(Kind::Warning, "警告である。"),
            Node::leaf(Kind::Note, "もう 1 つの補足。"),
        ]);
        assert!(value(notes(&d)) > 0.0);
        assert!(value(warnings(&d)) > 0.0);
        // 補足 2 つ、警告 1 つ。
        assert!(value(notes(&d)) > value(warnings(&d)));
    }

    #[test]
    fn 箇条書きと番号リストは別に数える() {
        let d = doc_with(vec![
            Node::branch(Kind::Bullet, vec![Node::leaf(Kind::Item, "あ")]),
            Node::branch(Kind::Ordered, vec![Node::leaf(Kind::Item, "い")]),
        ]);
        assert!(value(bullets(&d)) > 0.0);
        assert!(value(ordered_lists(&d)) > 0.0);
    }

    #[test]
    fn 深い見出しは深さ_3_以上である() {
        let d = Document::new(vec![
            filler(),
            Node::heading(1, "章"),
            Node::heading(2, "節"),
            Node::heading(3, "項"),
            Node::heading(4, "細目"),
        ]);
        let ja = d.japanese_chars();
        #[allow(clippy::cast_precision_loss)]
        let want = 1000.0 * 2.0 / ja as f64;
        assert!((value(deep_headings(&d)) - want).abs() < 1e-9);
    }

    #[test]
    fn 段落が_10_未満なら割合を測らない() {
        let d = doc_with(vec![Node::leaf(Kind::Paragraph, "1 文だけ。")]);
        assert_eq!(single_sentence_paragraphs(&d), Measured::BelowFloor);
    }

    #[test]
    fn 一文だけの段落の割合を数える() {
        let mut nodes = vec![filler()];
        for _ in 0..5 {
            nodes.push(Node::leaf(Kind::Paragraph, "1 文だけである。"));
        }
        for _ in 0..4 {
            nodes.push(Node::leaf(Kind::Paragraph, "2 文である。もう 1 文。"));
        }
        let d = Document::new(nodes);
        // filler も 1 段落だが 100 文ある。段落 10 個、うち 1 文だけが 5 個。
        assert!((value(single_sentence_paragraphs(&d)) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn 段落あたりの文数は段落の文だけ数える() {
        // 見出しや項目の文を分子に入れない。
        let mut nodes = vec![];
        for _ in 0..5 {
            nodes.push(Node::leaf(Kind::Paragraph, "1 文。"));
        }
        nodes.push(Node::heading(1, "見出しである"));
        nodes.push(Node::branch(
            Kind::Bullet,
            vec![Node::leaf(Kind::Item, "項目である")],
        ));
        let d = Document::new(nodes);
        assert!((value(sentences_per_paragraph(&d)) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn 変動係数は揺れないと_0_になる() {
        let mut nodes = vec![];
        for _ in 0..10 {
            nodes.push(Node::leaf(Kind::Paragraph, "あいうえお。"));
        }
        let d = Document::new(nodes);
        assert!((value(paragraph_length_cv(&d)) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn 変動係数は揺れると大きくなる() {
        let mut nodes = vec![];
        for _ in 0..5 {
            nodes.push(Node::leaf(Kind::Paragraph, "あ。"));
        }
        for _ in 0..5 {
            nodes.push(Node::leaf(Kind::Paragraph, "あ".repeat(100)));
        }
        let d = Document::new(nodes);
        assert!(value(paragraph_length_cv(&d)) > 0.5);
    }

    #[test]
    fn 項目は_1_つの分布として見る() {
        // 箇条書きごとに分けない。
        let mut lists = vec![];
        for _ in 0..2 {
            let items: Vec<Node> = (0..6)
                .map(|i| Node::leaf(Kind::Item, "あ".repeat(i + 1)))
                .collect();
            lists.push(Node::branch(Kind::Bullet, items));
        }
        let d = Document::new(lists);
        assert!(value(item_length_cv(&d)) > 0.0);
    }

    #[test]
    fn 太字始まりの項目を数える() {
        let mut items = vec![];
        for _ in 0..5 {
            let mut i = Node::leaf(Kind::Item, "");
            i.children.push(Node::leaf(Kind::Emphasis, "太字"));
            items.push(i);
        }
        for _ in 0..5 {
            items.push(Node::leaf(Kind::Item, "普通の項目"));
        }
        let d = Document::new(vec![Node::branch(Kind::Bullet, items)]);
        assert!((value(bold_leading_items(&d)) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn 節が_5_未満なら変動係数を測らない() {
        let d = Document::new(vec![filler(), Node::heading(1, "章")]);
        assert_eq!(section_length_cv(&d), Measured::BelowFloor);
    }

    #[test]
    fn 節の長さの変動係数を測る() {
        let mut nodes = vec![];
        for i in 0..6 {
            nodes.push(Node::heading(1, format!("章 {i}")));
            nodes.push(Node::leaf(Kind::Paragraph, "あ".repeat((i + 1) * 10)));
        }
        let d = Document::new(nodes);
        assert!(value(section_length_cv(&d)) > 0.0);
    }

    #[test]
    fn 日本語が足りなければ構造も測らない() {
        let d = Document::new(vec![Node::heading(1, "短い")]);
        assert_eq!(headings(&d), Measured::BelowFloor);
    }
}
