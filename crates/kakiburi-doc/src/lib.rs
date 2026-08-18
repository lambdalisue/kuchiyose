//! 文書の形。node の型と、数える単位。
//!
//! 地の文・日本語の文字・文・段落・節・項目・見出しの深さを、<strong>このクレートだけが
//! 定義する。</strong> 指標ごとに決め直さないことを、型で守る。
//!
//! 依存を持たない。形態素解析器も入出力も要らない。

pub mod node;
pub mod prose;
pub mod sentence;
pub mod text;

use node::{Kind, Node};
use prose::Segment;

/// 正規形の文書。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// 最上位の node。現れる順に並ぶ。
    pub nodes: Vec<Node>,
}

/// 節。見出しから、次の同じ深さ以上の見出しまで。見出し自身を含む。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section<'a> {
    /// 見出しの相対的な深さ。見出しより前の本文だけの節は `None`。
    pub depth: Option<u8>,
    /// この節に属する node。
    pub nodes: &'a [Node],
}

impl Document {
    /// node の並びから作る。
    #[must_use]
    pub fn new(nodes: Vec<Node>) -> Self {
        Self { nodes }
    }

    /// 地の文。node ごとに切れた文字列の並び。
    #[must_use]
    pub fn prose(&self) -> Vec<Segment> {
        prose::prose(&self.nodes)
    }

    /// 日本語の文字数。率の分母。
    #[must_use]
    pub fn japanese_chars(&self) -> usize {
        prose::japanese_chars(&self.prose())
    }

    /// 文。node を跨がない。
    #[must_use]
    pub fn sentences(&self) -> Vec<String> {
        sentence::sentences_of(&self.prose())
    }

    /// 段落。段落の node ひとつが 1 段落。
    #[must_use]
    pub fn paragraphs(&self) -> Vec<&Node> {
        let mut out = Vec::new();
        walk(&self.nodes, &mut |n| {
            if n.kind.is_paragraph() {
                out.push(n);
            }
        });
        out
    }

    /// 項目。<strong>最も外側のリストの直接の子ひとつ。</strong>
    ///
    /// 入れ子の子は数えない。数えると、深く入れ子にする書き手ほど項目が多いこと
    /// になり、入れ子の癖と項目数の癖が混ざる。
    ///
    /// <strong>「入れ子」は文書全体で見る。</strong> ほかのリストの中に入っているリストは、
    /// それ自体が数えられないだけでなく、その子も数えない。「各リストの直接の
    /// 子」と読むと入れ子の子が結局数に入り、上の理由がそのまま戻る。
    ///
    /// 箇条書きと番号リストで単位を分けない。どちらを選ぶかは別の指標が測る。
    #[must_use]
    pub fn items(&self) -> Vec<&Node> {
        let mut out = Vec::new();
        for n in &self.nodes {
            collect_items(n, false, &mut out);
        }
        out
    }

    /// 定型を落とした写しを返す。<strong>原本は変えない。</strong>
    ///
    /// 指定した文字列を <strong>どこかに含む[文](sentence)</strong> を、丸ごと落とす。文字列だけ
    /// 抜くと残りが繋がって存在しない文ができ、文の数も文末も狂う。node ごと
    /// 落とすと構造の指標が変わる——<strong>落とすのは中身であって node ではない。</strong>
    ///
    /// <strong>判定は部分一致である。</strong>「お世話になっております」は挨拶の一部として文の頭に
    /// 付くので、文全体との一致を求めれば 1 つも落ちない。<strong>照合そのものは 1 文字ずつの
    /// 完全一致で、字種も字幅も揃えない</strong>——半角で書いた挨拶と全角で書いた挨拶は、
    /// 書き手にとって別の選択である。
    ///
    /// <strong>空になった node は残す。</strong> 落とせば構造の指標が動く。
    #[must_use]
    pub fn without_boilerplate(&self, patterns: &[String]) -> Self {
        if patterns.is_empty() {
            return self.clone();
        }
        Self {
            nodes: self.nodes.iter().map(|n| strip_node(n, patterns)).collect(),
        }
    }

    /// 最も浅い見出しを深さ 1 とした相対の深さ。
    ///
    /// 同じ構成なら、取り込み元が違っても同じ深さになる。
    #[must_use]
    pub fn heading_depth(&self, node: &Node) -> Option<u8> {
        let raw = node.raw_depth?;
        let shallowest = self.shallowest_heading()?;
        Some(raw.saturating_sub(shallowest) + 1)
    }

    fn shallowest_heading(&self) -> Option<u8> {
        let mut min = None;
        walk(&self.nodes, &mut |n| {
            if let Some(d) = n.raw_depth {
                min = Some(min.map_or(d, |m: u8| m.min(d)));
            }
        });
        min
    }

    /// 節。
    ///
    /// 冒頭に見出しより前の本文があれば、それも 1 つの節として数える。見出しが
    /// 1 つも無い文書は、全体で 1 節である。
    #[must_use]
    pub fn sections(&self) -> Vec<Section<'_>> {
        let heads: Vec<(usize, u8)> = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| n.raw_depth.map(|d| (i, d)))
            .collect();

        if heads.is_empty() {
            return if self.nodes.is_empty() {
                Vec::new()
            } else {
                vec![Section {
                    depth: None,
                    nodes: &self.nodes,
                }]
            };
        }

        let mut out = Vec::new();
        if heads[0].0 > 0 {
            out.push(Section {
                depth: None,
                nodes: &self.nodes[..heads[0].0],
            });
        }
        for (k, &(start, depth)) in heads.iter().enumerate() {
            // 次の同じ深さ以上（数値が同じかそれより小さい）の見出しまで。
            let end = heads[k + 1..]
                .iter()
                .find(|&&(_, d)| d <= depth)
                .map_or(self.nodes.len(), |&(i, _)| i);
            out.push(Section {
                depth: self.heading_depth(&self.nodes[start]),
                nodes: &self.nodes[start..end],
            });
        }
        out
    }
}

/// node 1 つから定型を落とす。子にも掛ける。
fn strip_node(n: &Node, patterns: &[String]) -> Node {
    let mut out = n.clone();
    out.text = sentence::sentences(&n.text)
        .into_iter()
        .filter(|s| !patterns.iter().any(|p| s.contains(p.as_str())))
        .collect::<Vec<_>>()
        .concat();
    out.children = n.children.iter().map(|c| strip_node(c, patterns)).collect();
    out
}

/// 最も外側のリストの直接の子だけを集める。
///
/// `inside` は「すでにリストの中にいるか」。中に入ったら、そこから下のリストは
/// 入れ子なので数えない。<strong>リスト以外の node を挟んでも入れ子のままである</strong>——
/// 引用の中の箇条書きも、外側のリストの下にあるなら入れ子である。
fn collect_items<'a>(n: &'a Node, inside: bool, out: &mut Vec<&'a Node>) {
    let is_list = matches!(n.kind, Kind::Bullet | Kind::Ordered);
    if is_list && !inside {
        out.extend(n.children.iter().filter(|c| c.kind == Kind::Item));
    }
    let inside = inside || is_list;
    for c in &n.children {
        collect_items(c, inside, out);
    }
}

fn walk<'a>(nodes: &'a [Node], f: &mut impl FnMut(&'a Node)) {
    for n in nodes {
        f(n);
        walk(&n.children, f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn para(t: &str) -> Node {
        Node::leaf(Kind::Paragraph, t)
    }

    #[test]
    fn 段落は箇条書きや表を数えない() {
        let doc = Document::new(vec![
            para("本文である。"),
            Node::branch(Kind::Bullet, vec![Node::leaf(Kind::Item, "項目")]),
            Node::branch(Kind::Table, vec![Node::leaf(Kind::Cell, "セル")]),
            Node::leaf(Kind::Quote, "引用である。"),
            Node::heading(1, "見出し"),
            Node::leaf(Kind::CodeBlock, "code"),
        ]);
        assert_eq!(doc.paragraphs().len(), 1);
    }

    #[test]
    fn 項目は入れ子の子を数えない() {
        let inner = Node::branch(
            Kind::Bullet,
            vec![
                Node::leaf(Kind::Item, "内側 1"),
                Node::leaf(Kind::Item, "内側 2"),
            ],
        );
        let mut outer_item = Node::leaf(Kind::Item, "外側 1");
        outer_item.children.push(inner);
        let doc = Document::new(vec![Node::branch(
            Kind::Bullet,
            vec![outer_item, Node::leaf(Kind::Item, "外側 2")],
        )]);
        // 最も外側のリストの直接の子だけ——「外側 1」「外側 2」の 2 つ。
        // 内側のリストは入れ子なので、それ自身も子も数えない。
        let items = doc.items();
        assert_eq!(
            items.len(),
            2,
            "{:?}",
            items.iter().map(|i| &i.text).collect::<Vec<_>>()
        );
    }

    #[test]
    fn 入れ子は_node_を挟んでも入れ子である() {
        // 引用を挟んでも、外側のリストの下にあるなら入れ子である。
        let inner = Node::branch(Kind::Bullet, vec![Node::leaf(Kind::Item, "内側")]);
        let mut quote = Node::leaf(Kind::Quote, "引用");
        quote.children.push(inner);
        let mut outer_item = Node::leaf(Kind::Item, "外側");
        outer_item.children.push(quote);
        let doc = Document::new(vec![Node::branch(Kind::Bullet, vec![outer_item])]);
        assert_eq!(doc.items().len(), 1);
    }

    #[test]
    fn 並んだリストはどちらも最も外側である() {
        let doc = Document::new(vec![
            Node::branch(Kind::Bullet, vec![Node::leaf(Kind::Item, "a")]),
            Node::branch(
                Kind::Ordered,
                vec![Node::leaf(Kind::Item, "b"), Node::leaf(Kind::Item, "c")],
            ),
        ]);
        assert_eq!(doc.items().len(), 3);
    }

    #[test]
    fn 見出しの深さは最も浅いものからの相対() {
        // h2 から始まる文書でも、最も浅い h2 が深さ 1 になる。
        let doc = Document::new(vec![
            Node::heading(2, "章"),
            Node::heading(3, "節"),
            Node::heading(4, "項"),
        ]);
        assert_eq!(doc.heading_depth(&doc.nodes[0]), Some(1));
        assert_eq!(doc.heading_depth(&doc.nodes[1]), Some(2));
        assert_eq!(doc.heading_depth(&doc.nodes[2]), Some(3));
    }

    #[test]
    fn 取り込み元が違っても同じ深さになる() {
        let a = Document::new(vec![Node::heading(1, "章"), Node::heading(2, "節")]);
        let b = Document::new(vec![Node::heading(2, "章"), Node::heading(3, "節")]);
        assert_eq!(a.heading_depth(&a.nodes[1]), b.heading_depth(&b.nodes[1]));
    }

    #[test]
    fn 見出しが無ければ全体で_1_節() {
        let doc = Document::new(vec![para("本文。"), para("続き。")]);
        let s = doc.sections();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].depth, None);
    }

    #[test]
    fn 見出しより前の本文も_1_節() {
        let doc = Document::new(vec![
            para("前書き。"),
            Node::heading(1, "本題"),
            para("本文。"),
        ]);
        let s = doc.sections();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].depth, None);
        assert_eq!(s[0].nodes.len(), 1);
        assert_eq!(s[1].depth, Some(1));
        assert_eq!(s[1].nodes.len(), 2);
    }

    #[test]
    fn 節は次の同じ深さ以上の見出しまで() {
        let doc = Document::new(vec![
            Node::heading(1, "章 A"),
            para("A の本文。"),
            Node::heading(2, "節 A1"),
            para("A1 の本文。"),
            Node::heading(1, "章 B"),
            para("B の本文。"),
        ]);
        let s = doc.sections();
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].nodes.len(), 4, "章 A は節 A1 を含む");
        assert_eq!(s[1].nodes.len(), 2, "節 A1 は自身と本文");
        assert_eq!(s[2].nodes.len(), 2);
    }

    #[test]
    fn 文は_node_を跨がない() {
        // 繋げば「見出し本文である」という 1 文になってしまう。
        let doc = Document::new(vec![Node::heading(1, "見出し"), para("本文である。")]);
        let s = doc.sentences();
        assert_eq!(s.len(), 2, "{s:?}");
    }

    #[test]
    fn 分母は文書全体で_1_つ() {
        let doc = Document::new(vec![para("あいう"), Node::heading(1, "えお")]);
        assert_eq!(doc.japanese_chars(), 5);
    }

    #[test]
    fn 定型は文ごと落とす() {
        // 文字列だけ抜くと、残りが繋がって存在しない文ができる。
        let doc = Document::new(vec![para("お世話になっております。本題である。")]);
        let s = doc.without_boilerplate(&["お世話になっており".to_owned()]);
        assert_eq!(s.nodes[0].text, "本題である。");
    }

    #[test]
    fn 定型を落としても_node_は残す() {
        // 落とせば構造の指標が動く。
        let doc = Document::new(vec![para("挨拶である。"), para("本題である。")]);
        let s = doc.without_boilerplate(&["挨拶".to_owned()]);
        assert_eq!(s.nodes.len(), 2);
        assert_eq!(s.nodes[0].text, "");
        assert_eq!(s.paragraphs().len(), 2);
    }

    #[test]
    fn 定型の照合は字幅を揃えない() {
        // 半角で書いた挨拶と全角で書いた挨拶は、書き手にとって別の選択である。
        let doc = Document::new(vec![para("これは Rust である。")]);
        let s = doc.without_boilerplate(&["Ｒｕｓｔ".to_owned()]);
        assert_eq!(s.nodes[0].text, "これは Rust である。");
    }

    #[test]
    fn 定型は子にも掛ける() {
        let inner = Node::leaf(Kind::Item, "挨拶である。");
        let doc = Document::new(vec![Node::branch(Kind::Bullet, vec![inner])]);
        let s = doc.without_boilerplate(&["挨拶".to_owned()]);
        assert_eq!(s.nodes[0].children[0].text, "");
    }

    #[test]
    fn 指定が無ければそのままである() {
        let doc = Document::new(vec![para("本題である。")]);
        assert_eq!(doc.without_boilerplate(&[]), doc);
    }
}
