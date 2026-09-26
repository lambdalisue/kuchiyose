//! 構造と長さの指標。
//!
//! どれも node の数え方に依る。単位の意味は[文書の形](kakiburi_doc)が決める——
//! ここで決め直さない。

use kakiburi_doc::node::{Kind, Node};
use kakiburi_doc::{text, Document};

use crate::directive::Counted;
use crate::morph::{Analyzed, Morpheme};
use crate::{floor, Unmeasured};

/// node の数を、日本語 1,000 字あたりに直す。
fn nodes_per_1000(doc: &Document, kind: Kind) -> Counted {
    Counted::density(count(doc, kind), doc.japanese_chars())
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
pub fn headings(doc: &Document) -> Counted {
    nodes_per_1000(doc, Kind::Heading)
}

/// 箇条書きの数。順序のない列挙。
#[must_use]
pub fn bullets(doc: &Document) -> Counted {
    nodes_per_1000(doc, Kind::Bullet)
}

/// 番号リストの数。順序のある列挙。
#[must_use]
pub fn ordered_lists(doc: &Document) -> Counted {
    nodes_per_1000(doc, Kind::Ordered)
}

/// 表の数。
#[must_use]
pub fn tables(doc: &Document) -> Counted {
    nodes_per_1000(doc, Kind::Table)
}

/// 引用の数。
#[must_use]
pub fn quotes(doc: &Document) -> Counted {
    nodes_per_1000(doc, Kind::Quote)
}

/// 補足の数。記法は問わない。
#[must_use]
pub fn notes(doc: &Document) -> Counted {
    nodes_per_1000(doc, Kind::Note)
}

/// 警告の数。記法は問わない。
#[must_use]
pub fn warnings(doc: &Document) -> Counted {
    nodes_per_1000(doc, Kind::Warning)
}

/// 折りたたみの数。
#[must_use]
pub fn details(doc: &Document) -> Counted {
    nodes_per_1000(doc, Kind::Details)
}

/// 強調の数。
#[must_use]
pub fn emphasis(doc: &Document) -> Counted {
    nodes_per_1000(doc, Kind::Emphasis)
}

/// コードブロックの数。
///
/// 分母の文字数にコードブロックの中身を含めない。 含めると、長いコードを 1 つ
/// 貼っただけで率が下がる。[地の文](kakiburi_doc::prose)がコードを含まないので、
/// ここは分母をそのまま使えばよい。
#[must_use]
pub fn code_blocks(doc: &Document) -> Counted {
    nodes_per_1000(doc, Kind::CodeBlock)
}

/// 深い見出しの数。深さ 3 以上。
#[must_use]
pub fn deep_headings(doc: &Document) -> Counted {
    let deep = doc
        .nodes
        .iter()
        .filter(|n| doc.heading_depth(n).is_some_and(|d| d >= 3))
        .count();
    Counted::density(deep, doc.japanese_chars())
}

/// 1 文だけの段落の割合。
#[must_use]
pub fn single_sentence_paragraphs(doc: &Document) -> Counted {
    let paras = doc.paragraphs();
    let one = paras
        .iter()
        .filter(|p| kakiburi_doc::sentence::sentences(&p.text).len() == 1)
        .count();
    Counted::share(one, paras.len(), 10)
}

/// 敬体で終わる文の割合。node の種類ごとに数える。
///
/// 同じ書き手が、場所によって文体を変える。 実測で、ある書き手は段落の 93% を
/// 敬体で書き、箇条書きの 85%・見出しの 99% を常体で書いていた。
///
/// 地の文を 1 つの袋にすると、この使い分けが混ざって消える
/// （[文体は node の種類ごとに違う](../../../docs/spec/100-metrics.md#文体は-node-の種類ごとに違う)）。
/// 「敬体 7 割の人」としか言えず、箇条書きを敬体で書いた草稿を見分けられない。
///
/// 分母は敬体か常体で終わった文だけである。 体言止めと疑問符で終わる文はどちらでも
/// ないので数えない——入れると、体言止めの多い書き手ほど敬体率が下がる。
fn polite_rate(texts: &[String], floor: usize) -> Counted {
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
    Counted::share(polite, polite + plain, floor)
}

/// その文が敬体か。どちらでもなければ `None`。
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

/// 体言止めの割合。[敬体率](polite_rate)が見られないものを、こちらが見る。
///
/// 敬体率は体言止めを分母から落とす。 どちらでもないものを混ぜれば、
/// 体言止めの多い書き手ほど敬体率が下がるからである。だが落とした結果、
/// 項目を体言止めで書く人の項目は、1 つも数えられない——実測で、ある書き手の
/// 50 単位のうち箇条書きの敬体率が出たのは 4 単位だけだった。
///
/// その人がいちばんしていることが、いちばん見えない。 だからここで別に数える。
///
/// [文末表現](../../../docs/spec/metrics/文末表現.md)の系統は「取れなかった文も 1 つの
/// 次元として数える」と決めている。これはその次元を node の種類ごとに切り出したものである。
///
/// 解析器が要る。 体言止めは「文の最後の自立語が名詞で終わる」ことなので、
/// 語尾の文字列では決まらない——`できる` のように、語尾の一覧に載っていない
/// 動詞の活用形と見分けが付かない。
fn taigen_rate<'a>(segments: impl Iterator<Item = &'a Vec<Morpheme>>, floor: usize) -> Counted {
    let (mut taigen, mut total) = (0usize, 0usize);
    for seg in segments {
        for sentence in split_sentences(seg) {
            let Some(last) = sentence.iter().rev().find(|m| !is_punctuation(m)) else {
                continue;
            };
            total += 1;
            if last.pos1.starts_with("名詞") {
                taigen += 1;
            }
        }
    }
    Counted::share(taigen, total, floor)
}

/// 形態素列を文に割る。句点・感嘆符・疑問符で切る。
fn split_sentences(seg: &[Morpheme]) -> Vec<&[Morpheme]> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for (i, m) in seg.iter().enumerate() {
        if matches!(m.surface.as_str(), "。" | "！" | "？" | "." | "!" | "?") {
            out.push(&seg[start..=i]);
            start = i + 1;
        }
    }
    if start < seg.len() {
        out.push(&seg[start..]);
    }
    out
}

/// 記号か。文の終わりを決めるのは、その手前の語である。
fn is_punctuation(m: &Morpheme) -> bool {
    m.pos1.starts_with("補助記号") || m.pos1.starts_with("記号")
}

/// 文末の軸を、node の種類ごとに数えるときの下限。暫定値である。
const REGISTER_FLOOR: usize = 5;

/// 文末の軸の名前。1 つの定義が node の種類の数だけ軸を作る。
///
/// 種類ごとに書き足さない。 「段落の敬体率」「箇条書きの敬体率」と手で並べると、
/// 指標を足すたびに種類の数だけ定義が要り、足し忘れた指標だけが混ぜたままになる。
/// [地の文に入る種類](kakiburi_doc::node::Kind::PROSE)を回して名前を作る。
#[must_use]
pub fn register_names() -> Vec<String> {
    let mut out = Vec::with_capacity(Kind::PROSE.len() * 2);
    for kind in Kind::PROSE {
        out.push(format!("敬体率・{}", kind.name()));
        out.push(format!("体言止め率・{}", kind.name()));
    }
    out
}

/// 文末の軸を、node の種類ごとに測る。
///
/// まとめて測らない。 同じ書き手が段落では敬体、項目では体言止めで書くので、
/// 地の文を 1 つの袋にすると使い分けが袋の中で消える
/// （[文体は node の種類ごとに違う](../../../docs/spec/100-metrics.md#文体は-node-の種類ごとに違う)）。
///
/// どれが効くかは選ばない。 種類ごとに軸を出しておき、
/// [効くかの判定](../../kakiburi-scale/src/effective.rs)に選ばせる——書き手によって
/// 使い分ける場所が違うので、ここで決め打つと当たらない書き手が出る。
///
/// 敬体率は解析器を要らない。 体言止め率だけが要る——名詞で終わるかは
/// 語尾の文字列では決まらないためである。
#[must_use]
pub fn register_rates(
    prose: &[kakiburi_doc::prose::Segment],
    analyzed: Option<&Analyzed>,
) -> Vec<(String, Counted)> {
    let mut out = Vec::with_capacity(Kind::PROSE.len() * 2);
    for kind in Kind::PROSE {
        let texts: Vec<String> = prose
            .iter()
            .filter(|s| s.kind == kind)
            .map(|s| s.text.clone())
            .collect();
        out.push((
            format!("敬体率・{}", kind.name()),
            polite_rate(&texts, REGISTER_FLOOR),
        ));
        out.push((
            format!("体言止め率・{}", kind.name()),
            match analyzed {
                Some(a) => taigen_rate(a.segments_of(kind), REGISTER_FLOOR),
                None => Counted::unmeasured(Unmeasured::ToolMissing),
            },
        ));
    }
    out
}

/// 段落あたりの文数。段落に含まれる文を、段落の数で割る。
///
/// 見出しや項目やセルの文を分子に入れない。
#[must_use]
pub fn sentences_per_paragraph(doc: &Document) -> Counted {
    let paras = doc.paragraphs();
    let n: usize = paras
        .iter()
        .map(|p| kakiburi_doc::sentence::sentences(&p.text).len())
        .sum();
    Counted::share(n, paras.len(), floor::PARAGRAPHS)
}

/// 太字始まりの項目の割合。分母は項目数。
#[must_use]
pub fn bold_leading_items(doc: &Document) -> Counted {
    let items = doc.items();
    let bold = items
        .iter()
        .filter(|i| i.children.first().is_some_and(|c| c.kind == Kind::Emphasis))
        .count();
    Counted::share(bold, items.len(), 10)
}

/// 段落長の変動係数。段落ごとの日本語文字数の標準偏差 ÷ 平均。
#[must_use]
pub fn paragraph_length_cv(doc: &Document) -> Counted {
    let paras = doc.paragraphs();
    Counted::spread(
        paras
            .iter()
            .map(|p| text::count_japanese(&p.text))
            .collect(),
        10,
    )
}

/// 箇条書き項目長の変動係数。
///
/// 文書内の全ての項目を 1 つの分布として見る。箇条書きごとに分けない。
#[must_use]
pub fn item_length_cv(doc: &Document) -> Counted {
    let items = doc.items();
    Counted::spread(
        items
            .iter()
            .map(|i| text::count_japanese(&i.text))
            .collect(),
        10,
    )
}

/// 節の長さの変動係数。
#[must_use]
pub fn section_length_cv(doc: &Document) -> Counted {
    let sections = doc.sections();
    let lens: Vec<usize> = sections
        .iter()
        .map(|s| kakiburi_doc::prose::japanese_chars(&kakiburi_doc::prose::prose(s.nodes)))
        .collect();
    Counted::spread(lens, 5)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Measured;

    fn filler() -> Node {
        Node::leaf(Kind::Paragraph, "これは日本語の文章である。".repeat(100))
    }

    fn doc_with(mut nodes: Vec<Node>) -> Document {
        nodes.insert(0, filler());
        Document::new(nodes)
    }

    fn value(m: impl Into<Measured>) -> f64 {
        m.into().value().expect("測れているべき")
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
