//! HTML を正規形に落とす。
//!
//! 対応表は[`crate::markup::html_kind`]が持つ。 ここは木を組むだけである。
//!
//! HTML に警告の記法は無い——`<aside>` は補足に落ちる。だから警告は
//! 「書けない」升目になり、0 ではなく「測れない」を返す。

use kakiburi_doc::node::{Kind, Node};
use kakiburi_doc::Document;

use crate::markup;
use crate::refuse::Refusal;
use crate::space::{self, WRAP, WRAP_STR};

/// 中身を持たない要素。閉じ札を待たない。
const VOID: [&str; 8] = ["br", "hr", "img", "meta", "link", "input", "area", "col"];

/// 本文に入らない要素。中身ごと落とす。
const SKIP: [&str; 5] = ["script", "style", "head", "title", "template"];

/// HTML を読む。
///
/// 対応表に無い要素を見つけたら断る。推測して段落に落とさない。
pub fn parse(input: impl AsRef<str>) -> Result<Document, Refusal> {
    let tokens = tokenize(input.as_ref())?;
    let mut p = Builder { tokens, at: 0 };
    let built = p.children(None, false)?;
    let nodes = built.nodes;
    Ok(Document::new(nodes))
}

/// 字句。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    /// 開き札。
    Open {
        /// 要素名。小文字。
        name: String,
        /// 中身を持たないか。
        void: bool,
    },
    /// 閉じ札。
    Close {
        /// 要素名。小文字。
        name: String,
    },
    /// 文字。
    Text(String),
}

/// 字句に切る。
fn tokenize(s: &str) -> Result<Vec<Token>, Refusal> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '<' {
            text.push(chars[i]);
            i += 1;
            continue;
        }
        // コメント。中身ごと落とす。
        if chars[i..].starts_with(&['<', '!', '-', '-']) {
            let Some(end) = find(&chars, i + 4, &['-', '-', '>']) else {
                return Err(Refusal::Broken {
                    detail: "コメントが閉じていない".into(),
                });
            };
            i = end + 3;
            continue;
        }
        // 宣言。`<!doctype html>` など。
        if chars.get(i + 1) == Some(&'!') {
            let Some(end) = find(&chars, i + 2, &['>']) else {
                return Err(Refusal::Broken {
                    detail: "宣言が閉じていない".into(),
                });
            };
            i = end + 1;
            continue;
        }
        // 札になれない `<` は地の文である。`a < b` のような。
        // 直後が名前を始められる文字でなければ、札ではない。
        if !chars
            .get(i + 1)
            .is_some_and(|c| c.is_ascii_alphabetic() || *c == '/')
        {
            text.push('<');
            i += 1;
            continue;
        }
        let Some(end) = find(&chars, i + 1, &['>']) else {
            // 閉じていない `<` は地の文である。
            text.push('<');
            i += 1;
            continue;
        };
        let raw: String = chars[i + 1..end].iter().collect();
        i = end + 1;
        if !text.is_empty() {
            out.push(Token::Text(std::mem::take(&mut text)));
        }
        let trimmed = raw.trim();
        if let Some(rest) = trimmed.strip_prefix('/') {
            out.push(Token::Close {
                name: element_name(rest),
            });
            continue;
        }
        let name = element_name(trimmed);
        // 自分で閉じる札も、中身を持たない要素も、閉じ札を待たない。
        let void = trimmed.ends_with('/') || VOID.contains(&name.as_str());
        out.push(Token::Open { name, void });
    }
    if !text.is_empty() {
        out.push(Token::Text(text));
    }
    Ok(out)
}

/// 部分列を探す。
fn find(chars: &[char], from: usize, needle: &[char]) -> Option<usize> {
    (from..chars.len().saturating_sub(needle.len() - 1))
        .find(|&j| &chars[j..j + needle.len()] == needle)
}

/// 実体参照を解く。
///
/// 解かなければ `&amp;` が地の文に 5 文字として入る。 記号の率が上がり、
/// [書きぶりではなく記法を測る](../../../docs/spec/030-normalize.md#意味で認識する)。
///
/// 解いた結果は表記である。`&#x3042;` も `&#12354;` も `あ` になるが、
/// それは記法の違いであって書きぶりではない。
fn decode_entities(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '&' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let Some(end) = find(&chars, i + 1, &[';']) else {
            // 閉じていない `&` は地の文である。
            out.push('&');
            i += 1;
            continue;
        };
        let name: String = chars[i + 1..end].iter().collect();
        // 長すぎるものは実体参照ではない。`A & B; C` のような。
        if name.is_empty() || name.len() > 32 {
            out.push('&');
            i += 1;
            continue;
        }
        if let Some(c) = entity(&name) {
            out.push(c);
            i = end + 1;
            continue;
        }
        // 対応表に無い実体参照。そのまま残す——推測して当てない。
        out.push('&');
        i += 1;
    }
    out
}

/// 実体参照の名前から文字を引く。
fn entity(name: &str) -> Option<char> {
    match name {
        "lt" => Some('<'),
        "gt" => Some('>'),
        "amp" => Some('&'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        // 不改行空白は空白である。 潰さずに空白として持つ。
        "nbsp" => Some('\u{00A0}'),
        "hellip" => Some('…'),
        "mdash" => Some('\u{2014}'),
        "ndash" => Some('\u{2013}'),
        _ => {
            // 数値参照。`&#12354;` と `&#x3042;`。
            let rest = name.strip_prefix('#')?;
            let code = if let Some(hex) = rest.strip_prefix(['x', 'X']) {
                u32::from_str_radix(hex, 16).ok()?
            } else {
                rest.parse::<u32>().ok()?
            };
            char::from_u32(code)
        }
    }
}

/// 札の中身から要素名を取る。属性を落とす。
fn element_name(raw: &str) -> String {
    raw.trim()
        .trim_end_matches('/')
        .split([' ', '\t', '\n', '\r'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

struct Builder {
    tokens: Vec<Token>,
    at: usize,
}

/// 組んだ結果。node と、その場の文字を別に持つ。
///
/// 文字を node として持つと、親の `text` にも子にも同じ文字が入り、地の文で
/// 二重に数える。
struct Built {
    /// 子の node。
    nodes: Vec<Node>,
    /// その場の文字。溶けこむ子の中身を含む。
    text: String,
}

impl Builder {
    /// `until` の閉じ札まで、子を組む。
    ///
    /// `raw` は `<pre>` の中。そこの空白は表示されるので潰さない。
    fn children(&mut self, until: Option<&str>, raw: bool) -> Result<Built, Refusal> {
        let mut nodes: Vec<Node> = Vec::new();
        let mut text = String::new();
        while self.at < self.tokens.len() {
            match self.tokens[self.at].clone() {
                Token::Close { name } => {
                    if until == Some(name.as_str()) {
                        self.at += 1;
                        return Ok(Built {
                            nodes,
                            text: if raw { text } else { space::collapse(&text) },
                        });
                    }
                    // 対応しない閉じ札。推測して直さない。
                    return Err(Refusal::Broken {
                        detail: format!("</{name}> に対応する開き札が無い"),
                    });
                }
                Token::Text(t) => {
                    self.at += 1;
                    let t = decode_entities(&t);
                    // 地の文の改行は、書き手の改行ではなく書き出し側の折り返しである。
                    // 印を付けて持ち回り、区分の端まで見える[始末](space::collapse)に任せる。
                    text.push_str(&if raw {
                        t
                    } else {
                        t.replace(WRAP, "").replace(['\r', '\n'], WRAP_STR)
                    });
                }
                Token::Open { name, void } => {
                    self.at += 1;
                    if SKIP.contains(&name.as_str()) {
                        if !void {
                            self.skip_to_close(&name);
                        }
                        continue;
                    }
                    // `<br>` は改行である。 区切り線ではない——
                    // [改行の位置は潰さない](../../../docs/spec/030-normalize.md#表記を潰さない)。
                    if name == "br" {
                        text.push('\n');
                        continue;
                    }
                    // node を作らず、中身を親へ透かす。 行と区分は node ではない。
                    // `<pre>` の直下の ``` も同じ——そこはコードブロックの一部で
                    // あって、インラインコードではない。
                    if markup::is_transparent(&name) || (name == "code" && until == Some("pre")) {
                        let inner = self.children(Some(&name), raw)?;
                        text.push_str(&inner.text);
                        nodes.extend(inner.nodes);
                        continue;
                    }
                    let Some(kind) = markup::html_kind(&name) else {
                        return Err(Refusal::UnknownMarkup {
                            markup: format!("<{name}>"),
                        });
                    };
                    if void {
                        // `<img>` も `<hr>` も中身を持たない。
                        nodes.push(Node::leaf(kind, ""));
                        continue;
                    }
                    let inner = self.children(Some(&name), raw || name == "pre")?;
                    if is_inline(kind) {
                        // 行に溶けこむ。 文字を親に畳み、数えるための子を残す。
                        if kind == Kind::InlineCode || kind == Kind::Image {
                            // 中身は地の文に入らない。**跡に空白を残さない**——
                            // 残すと行内コードの多い記事ほど空白が増える。
                            text.push(WRAP);
                        } else {
                            text.push_str(&inner.text);
                        }
                        let mut c = Node::leaf(kind, inner.text.clone());
                        c.children = inner.nodes;
                        nodes.push(c);
                        continue;
                    }
                    nodes.push(build(kind, &name, inner));
                }
            }
        }
        if let Some(name) = until {
            return Err(Refusal::Broken {
                detail: format!("<{name}> が閉じていない"),
            });
        }
        Ok(Built {
            nodes,
            text: if raw { text } else { space::collapse(&text) },
        })
    }

    /// この要素の閉じ札まで飛ばす。
    fn skip_to_close(&mut self, name: &str) {
        let mut depth = 1usize;
        while self.at < self.tokens.len() {
            match &self.tokens[self.at] {
                Token::Open { name: n, void } if n == name && !void => depth += 1,
                Token::Close { name: n } if n == name => {
                    depth -= 1;
                    self.at += 1;
                    if depth == 0 {
                        return;
                    }
                    continue;
                }
                _ => {}
            }
            self.at += 1;
        }
    }
}

/// 行に溶けこむ node か。
fn is_inline(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Emphasis | Kind::Link | Kind::InlineCode | Kind::Image
    )
}

/// node を組む。
///
/// 区分の端の空白は表示されない。 `<p>` の直後と `</p>` の直前の折り返しが
/// そのまま地の文に入ると、書き出し側の字下げが空白として数えられる。
/// コードブロックだけは端も表示されるので、そのまま置く。
fn build(kind: Kind, name: &str, inner: Built) -> Node {
    let text = if kind == Kind::CodeBlock {
        inner.text
    } else {
        inner.text.trim_matches(' ').to_string()
    };
    let mut n = Node::leaf(kind, text);
    if kind == Kind::Heading {
        n.raw_depth = heading_depth(name);
    }
    n.children = inner.nodes;
    n
}

fn heading_depth(name: &str) -> Option<u8> {
    match name {
        "h1" => Some(1),
        "h2" => Some(2),
        "h3" => Some(3),
        "h4" => Some(4),
        "h5" => Some(5),
        "h6" => Some(6),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(doc: &Document) -> Vec<Kind> {
        doc.nodes.iter().map(|n| n.kind).collect()
    }

    #[test]
    fn 段落を読む() {
        let d = parse("<p>本文である。</p>").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Paragraph]);
        assert_eq!(d.nodes[0].text, "本文である。");
    }

    #[test]
    fn aside_は補足になる() {
        // HTML に警告の記法は無い。aside は補足に落ちる。
        let d = parse("<aside>補足である。</aside>").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Note]);
    }

    #[test]
    fn 対応表に無い要素は断る() {
        let e = parse("<blink>点滅</blink>").unwrap_err();
        assert!(matches!(e, Refusal::UnknownMarkup { .. }), "{e:?}");
    }

    /// 木を平らにして種類だけ並べる。入れ子まで見る。
    fn all_kinds(doc: &Document) -> Vec<Kind> {
        fn walk(ns: &[Node], out: &mut Vec<Kind>) {
            for n in ns {
                out.push(n.kind);
                walk(&n.children, out);
            }
        }
        let mut out = Vec::new();
        walk(&doc.nodes, &mut out);
        out
    }

    #[test]
    fn 表は_1_つの表_node_になる() {
        // 行や区分まで表に数えると、ふつうの表 1 つで密度が何倍にもなる。
        let d = parse("<table><thead><tr><th>見出し</th></tr></thead><tbody><tr><td>本文</td></tr></tbody></table>")
            .unwrap();
        let ks = all_kinds(&d);
        assert_eq!(
            ks.iter().filter(|&&k| k == Kind::Table).count(),
            1,
            "{ks:?}"
        );
        assert_eq!(ks.iter().filter(|&&k| k == Kind::Cell).count(), 2, "{ks:?}");
    }

    #[test]
    fn pre_だけがコードブロックである() {
        let d = parse("<pre><code>let x = 1;</code></pre>").unwrap();
        let ks = all_kinds(&d);
        assert_eq!(ks, vec![Kind::CodeBlock], "{ks:?}");
        assert_eq!(d.nodes[0].text, "let x = 1;", "中身は畳んで持つ");
    }

    #[test]
    fn 段落の中の_code_はインラインコードである() {
        // コードブロックにすると、インラインコードが永久に 0 になる。
        let d = parse("<p>設定は <code>--force</code> である。</p>").unwrap();
        let ks = all_kinds(&d);
        assert_eq!(ks, vec![Kind::Paragraph, Kind::InlineCode], "{ks:?}");
    }

    #[test]
    fn インラインコードの中身は地の文に入らない() {
        let d = parse("<p>設定は <code>--force</code> である。</p>").unwrap();
        let p = d.prose();
        assert_eq!(p.len(), 1);
        assert!(!p[0].text.contains("--force"), "{:?}", p[0].text);
    }

    #[test]
    fn 閉じていない要素は断る() {
        let e = parse("<p>本文である。").unwrap_err();
        assert!(matches!(e, Refusal::Broken { .. }), "{e:?}");
    }

    #[test]
    fn 対応しない閉じ札は断る() {
        let e = parse("<p>本文</p></div>").unwrap_err();
        assert!(matches!(e, Refusal::Broken { .. }), "{e:?}");
    }

    #[test]
    fn 見出しの深さを札から取る() {
        let d = parse("<h2>章</h2><h3>節</h3>").unwrap();
        assert_eq!(d.heading_depth(&d.nodes[0]), Some(1), "最も浅いものが 1");
        assert_eq!(d.heading_depth(&d.nodes[1]), Some(2));
    }

    #[test]
    fn 表はセルごとに_1_本になる() {
        let html = "<table><tr><td>機能</td><td>あり</td></tr></table>";
        let d = parse(html).unwrap();
        let texts: Vec<String> = d.prose().into_iter().map(|s| s.text).collect();
        assert_eq!(
            texts,
            vec!["機能".to_owned(), "あり".to_owned()],
            "{texts:?}"
        );
    }

    #[test]
    fn 日本語の無いセルは地の文に入らない() {
        let html = "<table><tr><td>リモート</td><td>o</td></tr></table>";
        let d = parse(html).unwrap();
        assert_eq!(d.prose().len(), 1);
    }

    #[test]
    fn コードの中身は地の文に入らない() {
        let d = parse("<p>設定は <code>--force</code> である。</p>").unwrap();
        // 跡に空白を残さない。**両側の空白ごと落ちる。**
        assert_eq!(d.nodes[0].text, "設定はである。");
        // 和欧のあいだなら、表示されるぶんの空白 1 個が残る。
        let d = parse("<p>run the <code>--force</code> flag</p>").unwrap();
        assert_eq!(d.nodes[0].text, "run the flag");
    }

    #[test]
    fn リンクは中身だけ入る() {
        let d = parse("<p>詳細は <a href=\"http://x\">こちら</a>。</p>").unwrap();
        assert_eq!(d.nodes[0].text, "詳細は こちら。");
    }

    #[test]
    fn 画像は中身を持たない() {
        let d = parse("<p>図。<img src=\"a.png\" alt=\"構成図\"></p>").unwrap();
        assert_eq!(d.nodes[0].text, "図。");
    }

    #[test]
    fn 強調は数えるための子になる() {
        let d = parse("<p>ここが <em>大事</em> である。</p>").unwrap();
        assert_eq!(d.nodes[0].text, "ここが 大事 である。");
        let n = d.nodes[0]
            .children
            .iter()
            .filter(|c| c.kind == Kind::Emphasis)
            .count();
        assert_eq!(n, 1);
    }

    #[test]
    fn 箇条書きは項目を持つ() {
        let d = parse("<ul><li>ひとつめ</li><li>ふたつめ</li></ul>").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Bullet]);
        assert_eq!(d.items().len(), 2);
    }

    #[test]
    fn 番号リストは箇条書きと分ける() {
        let d = parse("<ol><li>ひとつめ</li></ol>").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Ordered]);
    }

    #[test]
    fn 中身を持たない要素は閉じ札を待たない() {
        let d = parse("<p>前<br>後</p><hr>").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Paragraph, Kind::Divider]);
    }

    #[test]
    fn 自分で閉じる札も待たない() {
        let d = parse("<p>図。<img src=\"a.png\" /></p>").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Paragraph]);
    }

    #[test]
    fn script_と_style_は中身ごと落ちる() {
        // 本文ではない。断らずに落とす——対応表に無いのではなく、本文に入らない。
        let d = parse("<style>p{color:red}</style><p>本文。</p><script>alert(1)</script>").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Paragraph]);
        assert_eq!(d.nodes[0].text, "本文。");
    }

    #[test]
    fn コメントは落ちる() {
        let d = parse("<!-- 覚え書き --><p>本文。</p>").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Paragraph]);
    }

    #[test]
    fn 宣言は落ちる() {
        let d = parse("<!doctype html><p>本文。</p>").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Paragraph]);
    }

    #[test]
    fn 閉じていない不等号は地の文である() {
        let d = parse("<p>条件は a < b である。</p>").unwrap();
        assert_eq!(d.nodes[0].text, "条件は a < b である。");
    }

    #[test]
    fn 実体参照を解く() {
        // 解かなければ `&amp;` が地の文に 5 文字として入り、記号の率が上がる。
        let d = parse("<p>A &amp; B、a &lt; b、&quot;引用&quot;。</p>").unwrap();
        assert_eq!(d.nodes[0].text, "A & B、a < b、\"引用\"。");
    }

    #[test]
    fn 数値参照も解く() {
        let d = parse("<p>&#12354;と&#x3044;。</p>").unwrap();
        assert_eq!(d.nodes[0].text, "あとい。");
    }

    #[test]
    fn 三点リーダの実体参照は三点リーダになる() {
        let d = parse("<p>そうかも&hellip;</p>").unwrap();
        assert_eq!(d.nodes[0].text, "そうかも…");
    }

    #[test]
    fn 対応表に無い実体参照はそのまま残る() {
        // 推測して当てない。
        let d = parse("<p>&unknownentity;である。</p>").unwrap();
        assert!(d.nodes[0].text.starts_with('&'), "{}", d.nodes[0].text);
    }

    #[test]
    fn 閉じていない実体参照は地の文である() {
        let d = parse("<p>A & B である。</p>").unwrap();
        assert_eq!(d.nodes[0].text, "A & B である。");
    }

    #[test]
    fn br_は改行であって区切り線ではない() {
        // 改行の位置は潰さない。
        let d = parse("<p>前<br>後</p>").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Paragraph]);
        assert_eq!(d.nodes[0].text, "前\n後");
    }

    #[test]
    fn 表記は潰さない() {
        let d = parse("<p>全角（かっこ）と半角(paren)、〜 と ～。</p>").unwrap();
        assert_eq!(d.nodes[0].text, "全角（かっこ）と半角(paren)、〜 と ～。");
    }

    #[test]
    fn 和文を割る折り返しは空白にならない() {
        // 書き出し側が折り返しただけで、書き手は空白を打っていない。
        let d = parse("<p>これは日本語の\n文章である。</p>").unwrap();
        assert_eq!(d.nodes[0].text, "これは日本語の文章である。");
    }

    #[test]
    fn 約物を割る折り返しも空白にならない() {
        // 実測では、折り返しの 8 割が約物に隣り合っていた。
        let d = parse("<p>そう書いた。\nだから直した。</p>").unwrap();
        assert_eq!(d.nodes[0].text, "そう書いた。だから直した。");
    }

    #[test]
    fn 和欧を割る折り返しは空白になる() {
        // **こちらは表示される。** 消すと語が繋がってしまう。
        let d = parse("<p>これは Vim\nplugin である。</p>").unwrap();
        assert_eq!(d.nodes[0].text, "これは Vim plugin である。");
    }

    #[test]
    fn 手で打った空白は残る() {
        // **折り返しだけを潰す。** 打たれた空白は表記である。
        let d = parse("<p>これは 日本語 である。</p>").unwrap();
        assert_eq!(d.nodes[0].text, "これは 日本語 である。");
    }

    #[test]
    fn 札をまたぐ折り返しも潰す() {
        // 印は区分の端まで持ち回るので、行内の札で切れても始末できる。
        let d = parse("<p>詳細は<a href=\"http://x\">こちら</a>\nを見よ。</p>").unwrap();
        assert_eq!(d.nodes[0].text, "詳細はこちらを見よ。");
    }

    #[test]
    fn コードブロックの中の改行は潰さない() {
        let d = parse("<pre><code>let a = 1;\nlet b = 2;\n</code></pre>").unwrap();
        assert_eq!(d.nodes[0].text, "let a = 1;\nlet b = 2;\n");
    }

    #[test]
    fn 決定的である() {
        let html = "<h1>章</h1><p>本文である。</p><aside>補足。</aside>";
        assert_eq!(parse(html).unwrap(), parse(html).unwrap());
    }

    #[test]
    fn 折りたたみを読む() {
        let d = parse("<details><p>中身である。</p></details>").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Details]);
    }

    #[test]
    fn 引用を読む() {
        let d = parse("<blockquote>引用である。</blockquote>").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Quote]);
    }
}
