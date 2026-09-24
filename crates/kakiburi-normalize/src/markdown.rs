//! Markdown を正規形に落とす。
//!
//! 記法は潰す。表記は潰さない——字種、字幅、空白の入れ方はそのまま残す。

use kakiburi_doc::node::{Kind, Node};
use kakiburi_doc::Document;

use crate::markup;
use crate::refuse::Refusal;
use crate::source::Source;

/// Markdown を読む。
///
/// 対応表に無い記法を見つけたら断る。推測して補足に落とさない。
pub fn parse(input: impl AsRef<str>, source: Source) -> Result<Document, Refusal> {
    let mut p = Parser {
        lines: input.as_ref().lines().collect(),
        at: 0,
        source,
    };
    p.skip_front_matter();
    let mut nodes = Vec::new();
    while p.at < p.lines.len() {
        let before = p.at;
        let node = p.block()?;
        // block は必ず入力を進める。 進まなければ無限に回り、node を積み続けて
        // メモリを食い潰す。エラーにならないので、走らせるまで気付けない。
        if p.at == before {
            return Err(Refusal::Broken {
                detail: format!("{} 行目を読み進められない: {}", before + 1, p.lines[before]),
            });
        }
        if let Some(n) = node {
            nodes.push(n);
        }
    }
    Ok(Document::new(nodes))
}

struct Parser<'a> {
    lines: Vec<&'a str>,
    at: usize,
    source: Source,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&'a str> {
        self.lines.get(self.at).copied()
    }

    /// 冒頭の front matter を読み飛ばす。本文ではない。
    fn skip_front_matter(&mut self) {
        if self.peek().map(str::trim_end) != Some("---") {
            return;
        }
        let mut i = self.at + 1;
        while i < self.lines.len() {
            if self.lines[i].trim_end() == "---" {
                self.at = i + 1;
                return;
            }
            i += 1;
        }
        // 閉じが無ければ front matter ではなかったことにする。
    }

    /// 中身を block として解釈し直す。
    ///
    /// 子を持つ node の中身を 1 本の文字列に畳まない。 畳めば内側の段落・リスト・
    /// 表・コードブロックがまるごと消え、コードブロックの中身が地の文に混ざる。
    /// [文書の形](../../../docs/spec/020-document.md#文書は-node-でできている)は
    /// 引用・補足・警告・折りたたみ・脚注が子を持つと定めている。
    fn blocks_of(&self, body: &[&'a str]) -> Result<Vec<Node>, Refusal> {
        let mut p = Parser {
            lines: body.to_vec(),
            at: 0,
            source: self.source,
        };
        let mut nodes = Vec::new();
        while p.at < p.lines.len() {
            let before = p.at;
            let node = p.block()?;
            if p.at == before {
                return Err(Refusal::Broken {
                    detail: format!(
                        "中身の {} 行目を読み進められない: {}",
                        before + 1,
                        p.lines[before]
                    ),
                });
            }
            if let Some(n) = node {
                nodes.push(n);
            }
        }
        Ok(nodes)
    }

    /// 中身を持つ node を組む。文字は子が持つ。
    fn container(&self, kind: Kind, body: &[&'a str]) -> Result<Node, Refusal> {
        Ok(Node::branch(kind, self.blocks_of(body)?))
    }

    fn block(&mut self) -> Result<Option<Node>, Refusal> {
        let Some(line) = self.peek() else {
            return Ok(None);
        };
        if line.trim().is_empty() {
            self.at += 1;
            return Ok(None);
        }
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            return self.fence().map(Some);
        }
        if is_divider(line) {
            self.at += 1;
            return Ok(Some(Node::leaf(Kind::Divider, "")));
        }
        if let Some((depth, text)) = heading(line) {
            self.at += 1;
            return Ok(Some(Node::heading(depth, inline_checked(text)?)));
        }
        if let Some(name) = footnote_definition(line) {
            return self.footnote(name).map(Some);
        }
        if line.trim_start().starts_with('>') {
            return self.quote_or_alert().map(Some);
        }
        if let Some(name) = directive_open(line) {
            if markup::has_directives(self.source) {
                return self.directive(name).map(Some);
            }
            // 対応表に無い記法である。断る。
            //
            // 地の文に流すと `:::` が記号として数えられ、しかも補足と警告に
            // 0 が並ぶ。実測では、Zenn の記事を github-markdown として読むと
            // 補足 18 箇所と警告 2 箇所が消え、段落の数まで変わった。
            // エラーにならないので、取り込み元の申告違いに気付けない。
            return Err(Refusal::UnknownMarkup {
                markup: format!(":::{name}"),
            });
        }
        if is_table_row(line) {
            return self.table().map(Some);
        }
        if list_marker(line).is_some() {
            return self.list().map(Some);
        }
        self.paragraph().map(Some)
    }

    fn fence(&mut self) -> Result<Node, Refusal> {
        let open = self.lines[self.at].trim_start();
        let tick = if open.starts_with("```") {
            "```"
        } else {
            "~~~"
        };
        self.at += 1;
        let mut body = Vec::new();
        while let Some(l) = self.peek() {
            self.at += 1;
            if l.trim_start().starts_with(tick) {
                return Ok(Node::leaf(Kind::CodeBlock, body.join("\n")));
            }
            body.push(l);
        }
        // 閉じが無い。壊れた記法である。
        Err(Refusal::Broken {
            detail: format!("コードブロックの {tick} が閉じていない"),
        })
    }

    /// 引用か Alert か。Alert を先に見る。
    ///
    /// 素朴に引用として解釈すると、引用の密度が実際より高く出て、補足の密度に
    /// 0 が並ぶ。エラーにならないので、ここを間違えても気付けない。
    fn quote_or_alert(&mut self) -> Result<Node, Refusal> {
        let body = self.take_quote_body();
        if markup::has_alerts(self.source) {
            if let Some(name) = alert_marker(body.first().copied().unwrap_or("")) {
                let Some(kind) = markup::alert_kind(name) else {
                    return Err(Refusal::UnknownMarkup {
                        markup: format!("[!{name}]"),
                    });
                };
                return self.container(kind, &body[1..]);
            }
        }
        self.container(Kind::Quote, &body)
    }

    /// 引用の中身を取る。行頭の `>` を外す。
    fn take_quote_body(&mut self) -> Vec<&'a str> {
        let mut body = Vec::new();
        while let Some(l) = self.peek() {
            let t = l.trim_start();
            if !t.starts_with('>') {
                break;
            }
            self.at += 1;
            let rest = t.strip_prefix('>').unwrap_or(t);
            body.push(rest.strip_prefix(' ').unwrap_or(rest));
        }
        body
    }

    fn directive(&mut self, name: &'a str) -> Result<Node, Refusal> {
        let Some(kind) = markup::directive_kind(name) else {
            return Err(Refusal::UnknownMarkup {
                markup: format!(":::{name}"),
            });
        };
        self.at += 1;
        let mut body = Vec::new();
        while let Some(l) = self.peek() {
            self.at += 1;
            // 開きと同じ規則で閉じる。 開きは字下げを許すので、閉じだけを
            // 行頭に縛ると字下げして開いた directive は決して閉じられない。
            //
            // 実素材で踏んだ。箇条書きの直後の `:::` が 2 字下がっていると、
            // そこで閉じずに次の `:::` まで飲み込み、飲み込んだ中の `:::message` が
            // 「閉じていない」として出てくる——本当の原因から遠い場所で断る。
            if l.trim() == ":::" {
                return self.container(kind, &body);
            }
            body.push(l);
        }
        Err(Refusal::Broken {
            detail: format!(":::{name} が閉じていない"),
        })
    }

    fn footnote(&mut self, name: &'a str) -> Result<Node, Refusal> {
        let line = self.lines[self.at];
        self.at += 1;
        let marker = format!("[^{name}]:");
        let body = line
            .trim_start()
            .strip_prefix(&marker)
            .unwrap_or("")
            .trim_start();
        Ok(Node::leaf(Kind::Footnote, inline_checked(body)?))
    }

    /// 表。セル 1 つが 1 つの node である。
    ///
    /// 行のままにすると桁揃えの空白が地の文に入り、隣のセルの先頭が前のセルの
    /// 末尾と隣り合う。その隣接は書き手が選んだものではない。
    fn table(&mut self) -> Result<Node, Refusal> {
        let mut cells = Vec::new();
        while let Some(l) = self.peek() {
            if !is_table_row(l) {
                break;
            }
            self.at += 1;
            if is_table_delimiter(l) {
                continue;
            }
            for c in split_row(l) {
                let t = c.trim();
                if !t.is_empty() {
                    cells.push(Node::leaf(Kind::Cell, inline_checked(t)?));
                }
            }
        }
        Ok(Node::branch(Kind::Table, cells))
    }

    /// 箇条書きまたは番号リスト。
    ///
    /// `-` と `*` と `+` を潰す。記法の違いであって書きぶりではない。
    /// 箇条書きと番号リストは潰さない——どちらを選ぶかは別の指標が測る。
    fn list(&mut self) -> Result<Node, Refusal> {
        let (kind, indent) = {
            let m = list_marker(self.lines[self.at]).expect("呼ぶ前に確かめている");
            (m.kind, m.indent)
        };
        let mut items: Vec<Node> = Vec::new();
        while let Some(l) = self.peek() {
            let Some(m) = list_marker(l) else { break };
            if m.indent < indent || m.kind != kind {
                break;
            }
            if m.indent > indent {
                // 入れ子。項目の子として持つ。項目としては数えない。
                let inner = self.list()?;
                if let Some(last) = items.last_mut() {
                    last.children.push(inner);
                } else {
                    items.push(Node::branch(Kind::Item, vec![inner]));
                }
                continue;
            }
            self.at += 1;
            {
                let (text, mut kids) = inline_with_children(m.text)?;
                // 強調で始まる項目は、強調を先頭の子に置く。
                // 畳んだ子は順序を持たないので、[太字始まりの項目](../../../docs/spec/metrics/太字始まりの項目.md)が
                // 先頭かどうかを読めなくなる。
                if starts_with_emphasis(m.text) {
                    if let Some(i) = kids.iter().position(|k| k.kind == Kind::Emphasis) {
                        let e = kids.remove(i);
                        kids.insert(0, e);
                    }
                }
                let mut n = Node::leaf(Kind::Item, text);
                n.children = kids;
                items.push(n);
            }
        }
        Ok(Node::branch(kind, items))
    }

    fn paragraph(&mut self) -> Result<Node, Refusal> {
        let mut body = Vec::new();
        while let Some(l) = self.peek() {
            // 切れ目は block の振り分けと同じでなければならない。 ずれると、
            // block が拾わない行で paragraph も切れ、何も消費せず無限に回る。
            if body.is_empty() {
                // 1 行目は必ず取る。block が「段落だ」と判断した行である。
                self.at += 1;
                body.push(l);
                continue;
            }
            if self.starts_block(l) {
                break;
            }
            self.at += 1;
            body.push(l);
        }
        let (text, kids) = inline_with_children(body.join("\n"))?;
        let mut n = Node::leaf(Kind::Paragraph, text);
        n.children = kids;
        Ok(n)
    }

    /// この行から別の block が始まるか。block の振り分けと 1 対 1 に対応する。
    fn starts_block(&self, line: &str) -> bool {
        let t = line.trim_start();
        t.is_empty()
            || t.starts_with('>')
            || t.starts_with("```")
            || t.starts_with("~~~")
            || heading(line).is_some()
            || is_divider(line)
            || is_table_row(line)
            || list_marker(line).is_some()
            || footnote_definition(line).is_some()
            || (markup::has_directives(self.source) && directive_open(line).is_some())
    }
}

/// 行の中の記法を落とし、数えるための子を作る。
///
/// インラインコードの中身とリンクの参照先は地の文に入らない。画像の代替文字も
/// 入らない。強調の `**` と `__` は記法なので潰す——中身は残る。
///
/// 子は数を数えるためにある。[強調](kakiburi_doc::node::Kind::Emphasis)は node なので、
/// 落とすだけだと[強調の指標](../../../docs/spec/metrics/強調.md)が永久に 0 になる。
/// エラーにならないので気付けない。子のテキストは地の文に足さない——
/// 親の文字列にすでに 1 度入っている。
///
/// 対応表に無い記法があれば断る。落として通せば、落ちた分だけ値が狂った文書が
/// 正常な顔でコーパスに入る。
fn inline_checked(s: impl AsRef<str>) -> Result<String, Refusal> {
    Ok(inline_with_children(s)?.0)
}

/// 地の文と、行の中に溶けこむ子。
fn inline_with_children(s: impl AsRef<str>) -> Result<(String, Vec<Node>), Refusal> {
    let s = s.as_ref();
    let mut out = String::with_capacity(s.len());
    let mut kids: Vec<Node> = Vec::new();
    let mut in_emphasis = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // Markdown の中のインライン HTML。対応表にある要素なら記法として
            // 潰し、中身を残す。無い要素は断る。
            '<' => {
                let mut raw = String::new();
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == '>' {
                        closed = true;
                        break;
                    }
                    raw.push(c);
                }
                if !closed {
                    // 記法ではない。`a < b` のような地の文である。
                    out.push('<');
                    out.push_str(&raw);
                    continue;
                }
                let name = html_tag_name(&raw);
                let Some(kind) = markup::html_kind(&name) else {
                    return Err(Refusal::UnknownMarkup {
                        markup: format!("<{name}>"),
                    });
                };
                // 開き札だけを数える。閉じ札で二重に数えない。
                if !raw.trim_start().starts_with('/') && kind == Kind::Emphasis {
                    kids.push(Node::leaf(Kind::Emphasis, ""));
                }
            }
            // インラインコードは中身ごと落とす。
            '`' => {
                let mut body = String::new();
                for c in chars.by_ref() {
                    if c == '`' {
                        break;
                    }
                    body.push(c);
                }
                // 中身は地の文に入らない。**跡に空白を残さない**——
                // 残すと行内コードの多い記事ほど空白が増える。
                out.push(crate::space::WRAP);
                kids.push(Node::leaf(Kind::InlineCode, body));
            }
            // 画像は代替文字ごと落とす。リンクは中身だけ残す。
            '!' if chars.peek() == Some(&'[') => {
                chars.next();
                let mut alt = String::new();
                let mut depth = 1;
                for c in chars.by_ref() {
                    match c {
                        '[' => {
                            depth += 1;
                            alt.push(c);
                        }
                        ']' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                            alt.push(c);
                        }
                        _ => alt.push(c),
                    }
                }
                skip_link_target(&mut chars);
                out.push(crate::space::WRAP);
                kids.push(Node::leaf(Kind::Image, alt));
            }
            '[' => {
                let mut label = String::new();
                let mut depth = 1;
                for c in chars.by_ref() {
                    match c {
                        '[' => {
                            depth += 1;
                            label.push(c);
                        }
                        ']' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                            label.push(c);
                        }
                        _ => label.push(c),
                    }
                }
                skip_link_target(&mut chars);
                let (text, inner) = inline_with_children(&label)?;
                out.push_str(&text);
                kids.push(Node::leaf(Kind::Link, text.clone()));
                kids.extend(inner);
            }
            // 強調の記法だけを落とす。開くときだけ数える。
            //
            // `**` は開きと閉じで 2 度通る。数を 2 で割って畳むと、1 度しか
            // 通らない HTML の `<em>` が消える。切り替えで数える。
            '*' | '_' => {
                if chars.peek() == Some(&c) {
                    chars.next();
                    if !in_emphasis {
                        kids.push(Node::leaf(Kind::Emphasis, ""));
                    }
                    in_emphasis = !in_emphasis;
                }
            }
            _ => out.push(c),
        }
    }
    Ok((crate::space::collapse(&out), kids))
}

/// 強調で始まる行か。`**` `__` と、対応表にある強調の HTML 札を見る。
fn starts_with_emphasis(line: &str) -> bool {
    let t = line.trim_start();
    if t.starts_with("**") || t.starts_with("__") {
        return true;
    }
    let Some(rest) = t.strip_prefix('<') else {
        return false;
    };
    let Some(end) = rest.find('>') else {
        return false;
    };
    markup::html_kind(html_tag_name(&rest[..end])) == Some(Kind::Emphasis)
}

/// `<` と `>` の間から要素名を取る。閉じ札と属性を落とす。
fn html_tag_name(raw: &str) -> String {
    let t = raw.trim().trim_start_matches('/').trim_end_matches('/');
    t.split([' ', '\t', '\n'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// リンクの参照先を読み飛ばす。地の文には入らない。
fn skip_link_target(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    match chars.peek() {
        Some('(') => {
            chars.next();
            let mut depth = 1;
            for c in chars.by_ref() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
        // 参照形式のリンク。`[語][鍵]` の `[鍵]` を落とす。
        Some('[') => {
            chars.next();
            for c in chars.by_ref() {
                if c == ']' {
                    break;
                }
            }
        }
        _ => {}
    }
}

fn heading(line: &str) -> Option<(u8, &str)> {
    let t = line.trim_start();
    let hashes = t.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &t[hashes..];
    // `#見出し` は見出しではない。空白が要る。
    let body = rest.strip_prefix(' ')?;
    Some((
        u8::try_from(hashes).ok()?,
        body.trim_end_matches([' ', '#']),
    ))
}

fn is_divider(line: &str) -> bool {
    let t = line.trim();
    if t.len() < 3 {
        return false;
    }
    ['-', '*', '_'].iter().any(|&m| {
        t.chars().all(|c| c == m || c == ' ') && t.chars().filter(|&c| c == m).count() >= 3
    })
}

fn alert_marker(line: &str) -> Option<&str> {
    let t = line.trim();
    t.strip_prefix("[!")?.strip_suffix(']')
}

/// `:::` に続く名前を、修飾を含めて返す。
///
/// `message alert` の `alert` を落とすと、警告が補足に化ける。
fn directive_open(line: &str) -> Option<&str> {
    let t = line.trim();
    let rest = t.strip_prefix(":::")?.trim();
    if rest.is_empty() {
        return None;
    }
    Some(rest)
}

fn footnote_definition(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let rest = t.strip_prefix("[^")?;
    let end = rest.find("]:")?;
    Some(&rest[..end])
}

fn is_table_row(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') && t.len() > 1
}

fn is_table_delimiter(line: &str) -> bool {
    split_row(line)
        .iter()
        .all(|c| !c.trim().is_empty() && c.trim().chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
}

fn split_row(line: &str) -> Vec<&str> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').collect()
}

struct Marker<'a> {
    kind: Kind,
    indent: usize,
    text: &'a str,
}

fn list_marker(line: &str) -> Option<Marker<'_>> {
    if is_divider(line) {
        return None;
    }
    let indent = line.len() - line.trim_start().len();
    let t = line.trim_start();
    // 箇条書き。`-` `*` `+` は記法の違いなので潰す。
    for m in ['-', '*', '+'] {
        if let Some(rest) = t.strip_prefix(m) {
            if let Some(text) = rest.strip_prefix(' ') {
                return Some(Marker {
                    kind: Kind::Bullet,
                    indent,
                    text: text.trim_start(),
                });
            }
        }
    }
    // 番号リスト。
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 {
        let rest = &t[digits..];
        if let Some(text) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")) {
            return Some(Marker {
                kind: Kind::Ordered,
                indent,
                text: text.trim_start(),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(doc: &Document) -> Vec<Kind> {
        doc.nodes.iter().map(|n| n.kind).collect()
    }

    #[test]
    fn alert_を引用より先に認識する() {
        // ここを間違えると引用の密度が高く出て、補足の密度に 0 が並ぶ。
        // エラーにならないので気付けない。
        let md = "> [!NOTE]\n> ここは補足である。\n";
        let d = parse(md, Source::GithubMarkdown).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Note]);
        // 中身は子が持つ。畳めば内側の構造が消える。
        assert_eq!(d.nodes[0].children.len(), 1);
        assert_eq!(d.nodes[0].children[0].kind, Kind::Paragraph);
        assert_eq!(d.nodes[0].children[0].text, "ここは補足である。");
    }

    #[test]
    fn 引用の中の構造は残る() {
        // 畳めば、内側のリストも表もコードブロックも消える。
        let md = "> 説明である。\n>\n> - ひとつ\n> - ふたつ\n";
        let d = parse(md, Source::GithubMarkdown).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Quote]);
        let inner: Vec<Kind> = d.nodes[0].children.iter().map(|n| n.kind).collect();
        assert_eq!(inner, vec![Kind::Paragraph, Kind::Bullet], "{inner:?}");
    }

    #[test]
    fn 引用の中のコードは地の文に混ざらない() {
        // 畳めば `let x = 1;` が地の文に入り、記号の率が題材で動く。
        let md = "> 例である。\n>\n> ```\n> let x = 1;\n> ```\n";
        let d = parse(md, Source::GithubMarkdown).unwrap();
        let joined: String = d.prose().iter().map(|s| s.text.clone()).collect();
        assert!(!joined.contains("let x"), "{joined:?}");
    }

    #[test]
    fn important_は警告に落ちる() {
        let d = parse("> [!IMPORTANT]\n> 読み飛ばすな。\n", Source::GithubMarkdown).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Warning]);
    }

    #[test]
    fn alert_記法を持たない取り込み元では引用のままである() {
        // 素の CommonMark に Alert は無い。引用として読むのが正しい。
        let md = "> [!NOTE]\n> ここは引用である。\n";
        let d = parse(md, Source::PlainMarkdown).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Quote]);
    }

    #[test]
    fn 対応表に無い_alert_は断る() {
        let md = "> [!HINT]\n> これは何か。\n";
        let e = parse(md, Source::GithubMarkdown).unwrap_err();
        assert!(matches!(e, Refusal::UnknownMarkup { .. }), "{e:?}");
    }

    #[test]
    fn 普通の引用は引用である() {
        let d = parse("> 引用である。\n", Source::GithubMarkdown).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Quote]);
    }

    #[test]
    fn directive_は補足と警告に分かれる() {
        let md = ":::note\n補足。\n:::\n\n:::warning\n警告。\n:::\n";
        let d = parse(md, Source::DirectiveMarkdown).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Note, Kind::Warning]);
    }

    #[test]
    fn zenn_の_message_は補足で_alert_つきは警告() {
        let md = ":::message\n補足。\n:::\n\n:::message alert\n警告。\n:::\n";
        let d = parse(md, Source::DirectiveMarkdown).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Note, Kind::Warning]);
    }

    #[test]
    fn block_が拾わない行で回り続けない() {
        // block の振り分けと paragraph の切れ目がずれると、何も消費せず無限に
        // 回り、node を積み続けてメモリを食い潰す。止まることを試す。
        //
        // 一度これで 5.7 GB まで膨らんだ。エラーにならないので走らせるまで
        // 気付けなかった。
        for md in [
            "本文である。\n",
            "  字下げした本文である。\n",
            "本文\n続き\n\nもう一段。\n",
            "= 見出しでない行 =\n",
            "1) 番号らしき行\n",
            "|\n",
            "::\n",
        ] {
            let r = parse(md, Source::GithubMarkdown);
            // 断るのはよい。回り続けるのが駄目である。
            let _ = r;
        }
    }

    #[test]
    fn 進まない行があれば断る() {
        // 不変条件そのものを試す。壊れたら止まらずに断る。
        let md = ":::\n";
        let d = parse(md, Source::DirectiveMarkdown);
        // `:::` だけの行は directive の開きではない。段落として消費される。
        assert!(d.is_ok(), "{d:?}");
    }

    #[test]
    fn 取り込み元を間違えたら断る() {
        // Zenn の記事を github-markdown として読むと、補足と警告に 0 が並び、
        // 段落の数まで変わる。エラーにならないので気付けない。
        let md = ":::message\n補足である。\n:::\n";
        let e = parse(md, Source::GithubMarkdown).unwrap_err();
        assert!(matches!(e, Refusal::UnknownMarkup { .. }), "{e:?}");
    }

    #[test]
    fn 字下げした閉じでも閉じる() {
        // 開きは字下げを許す。 閉じだけを行頭に縛れば、字下げして開いた
        // directive は決して閉じられない。整形器が箇条書きの直後の `:::` を
        // 下げることは実素材で普通に起きる。
        let md = ":::message\n- あ\n- い\n  :::\n";
        let doc = parse(md, Source::DirectiveMarkdown).expect("通る");
        assert_eq!(doc.nodes.len(), 1);
        assert_eq!(doc.nodes[0].kind, Kind::Note);
    }

    #[test]
    fn 字下げして開いた_directive_も閉じる() {
        let md = "  :::message\n  補足である。\n  :::\n";
        let doc = parse(md, Source::DirectiveMarkdown).expect("通る");
        assert_eq!(doc.nodes[0].kind, Kind::Note);
    }

    #[test]
    fn 閉じていない_directive_は断る() {
        let e = parse(":::note\n補足。\n", Source::DirectiveMarkdown).unwrap_err();
        assert!(matches!(e, Refusal::Broken { .. }), "{e:?}");
    }

    #[test]
    fn 閉じていないコードブロックは断る() {
        let e = parse("```rust\nlet x = 1;\n", Source::GithubMarkdown).unwrap_err();
        assert!(matches!(e, Refusal::Broken { .. }), "{e:?}");
    }

    #[test]
    fn コードブロックの中身は地の文に入らない() {
        let md = "説明。\n\n```rust\nlet x = 1;\n```\n\n続き。\n";
        let d = parse(md, Source::GithubMarkdown).unwrap();
        let p = d.prose();
        assert_eq!(p.len(), 2);
        assert!(!p.iter().any(|s| s.text.contains("let")));
    }

    #[test]
    fn 箇条書きの記法は潰す() {
        // `-` と `*` と `+` は記法の違いであって書きぶりではない。
        for m in ['-', '*', '+'] {
            let d = parse(
                format!("{m} ひとつめ\n{m} ふたつめ\n"),
                Source::GithubMarkdown,
            )
            .unwrap();
            assert_eq!(kinds(&d), vec![Kind::Bullet], "{m} が箇条書きにならない");
            assert_eq!(d.items().len(), 2);
        }
    }

    #[test]
    fn 番号リストは箇条書きと分けたままにする() {
        // どちらを選ぶかは書き手の選択で、別の指標が測る。
        let d = parse("1. ひとつめ\n2. ふたつめ\n", Source::GithubMarkdown).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Ordered]);
    }

    #[test]
    fn 表はセルごとに_1_本になる() {
        let md = "| 機能 | あり |\n| --- | --- |\n| リモート | o |\n";
        let d = parse(md, Source::GithubMarkdown).unwrap();
        let p = d.prose();
        // 区切り行は落ちる。`o` だけのセルは地の文に入らない。
        let texts: Vec<&str> = p.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, vec!["機能", "あり", "リモート"], "{texts:?}");
    }

    #[test]
    fn 見出しの深さは記法から取る() {
        let d = parse("# 章\n## 節\n### 項\n", Source::GithubMarkdown).unwrap();
        assert_eq!(d.heading_depth(&d.nodes[0]), Some(1));
        assert_eq!(d.heading_depth(&d.nodes[2]), Some(3));
    }

    #[test]
    fn 空白の無い井桁は見出しではない() {
        let d = parse("#タグではない\n", Source::GithubMarkdown).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Paragraph]);
    }

    #[test]
    fn インラインコードの中身は落ちる() {
        let d = parse("設定は `--force` である。\n", Source::GithubMarkdown).unwrap();
        // 跡に空白を残さない。**両側の空白ごと落ちる。**
        assert_eq!(d.nodes[0].text, "設定はである。");
        // 和欧のあいだなら、表示されるぶんの空白 1 個が残る。
        let d = parse("run the `--force` flag\n", Source::GithubMarkdown).unwrap();
        assert_eq!(d.nodes[0].text, "run the flag");
    }

    #[test]
    fn リンクは中身だけ残り参照先は落ちる() {
        let d = parse(
            "詳細は [こちら](https://example.com) を見る。\n",
            Source::GithubMarkdown,
        )
        .unwrap();
        assert_eq!(d.nodes[0].text, "詳細は こちら を見る。");
    }

    #[test]
    fn 参照形式のリンクも参照先が落ちる() {
        let d = parse(
            "[Netrw][] と [Fern][fern] を比べる。\n",
            Source::GithubMarkdown,
        )
        .unwrap();
        assert_eq!(d.nodes[0].text, "Netrw と Fern を比べる。");
    }

    #[test]
    fn 画像は代替文字ごと落ちる() {
        let d = parse("図。![構成図](a.png)\n", Source::GithubMarkdown).unwrap();
        assert_eq!(d.nodes[0].text, "図。");
    }

    #[test]
    fn 強調の記法は潰れて中身が残る() {
        let d = parse("ここが **大事** である。\n", Source::GithubMarkdown).unwrap();
        assert_eq!(d.nodes[0].text, "ここが 大事 である。");
        let d = parse("ここが __大事__ である。\n", Source::GithubMarkdown).unwrap();
        assert_eq!(d.nodes[0].text, "ここが 大事 である。");
    }

    #[test]
    fn 対応表にある_html_の札は潰れて中身が残る() {
        // Markdown の中にインライン HTML は来る。落とさなければ `<strong>` が
        // 地の文に入り、記号の率が上がる。
        let d = parse("ここが <em>大事</em> である。\n", Source::GithubMarkdown).unwrap();
        assert_eq!(d.nodes[0].text, "ここが 大事 である。");
    }

    #[test]
    fn 属性つきの札も潰れる() {
        let d = parse(
            "詳細は <a href=\"https://example.com\">こちら</a>。\n",
            Source::GithubMarkdown,
        )
        .unwrap();
        assert_eq!(d.nodes[0].text, "詳細は こちら。");
    }

    #[test]
    fn 対応表に無い_html_の札は断る() {
        let e = parse(
            "これは <blink>点滅</blink> する。\n",
            Source::GithubMarkdown,
        )
        .unwrap_err();
        assert!(matches!(e, Refusal::UnknownMarkup { .. }), "{e:?}");
    }

    #[test]
    fn 閉じていない不等号は地の文である() {
        // `a < b` は記法ではない。断ってはいけない。
        let d = parse("条件は a < b である。\n", Source::GithubMarkdown).unwrap();
        assert_eq!(d.nodes[0].text, "条件は a < b である。");
    }

    #[test]
    fn 強調は数えるための_node_になる() {
        // 記法を落とすだけだと、強調の指標が永久に 0 になる。
        // エラーにならないので、実際の記事に当てるまで気付けない。
        let stars = "\u{2A}\u{2A}";
        let md = format!("ここが {stars}大事{stars} である。\n");
        let d = parse(&md, Source::GithubMarkdown).unwrap();
        assert_eq!(d.nodes[0].text, "ここが 大事 である。");
        let n = d.nodes[0]
            .children
            .iter()
            .filter(|c| c.kind == Kind::Emphasis)
            .count();
        assert_eq!(n, 1, "強調が 1 つ立つ");
    }

    #[test]
    fn 強調の中身は二重に入らない() {
        let stars = "\u{2A}\u{2A}";
        let md = format!("{stars}大事{stars}である。\n");
        let d = parse(&md, Source::GithubMarkdown).unwrap();
        let p = d.prose();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].text, "大事である。", "地の文に 1 度だけ入る");
    }

    #[test]
    fn html_の強調も数える() {
        let d = parse("ここが <em>大事</em> である。\n", Source::GithubMarkdown).unwrap();
        let n = d.nodes[0]
            .children
            .iter()
            .filter(|c| c.kind == Kind::Emphasis)
            .count();
        assert_eq!(n, 1, "開き札だけを数える");
    }

    #[test]
    fn 太字始まりの項目は強調が先頭に来る() {
        let stars = "\u{2A}\u{2A}";
        let md = format!("- {stars}見出し{stars}: 説明である\n- 普通の項目\n");
        let d = parse(&md, Source::GithubMarkdown).unwrap();
        let items = d.items();
        assert_eq!(items.len(), 2);
        assert_eq!(
            items[0].children.first().map(|c| c.kind),
            Some(Kind::Emphasis),
            "強調で始まる項目は先頭が強調"
        );
        assert!(
            items[1]
                .children
                .first()
                .is_none_or(|c| c.kind != Kind::Emphasis),
            "普通の項目は先頭が強調ではない"
        );
    }

    #[test]
    fn インラインコードとリンクも_node_になる() {
        let d = parse(
            "設定は `--force` で、詳細は [こちら](https://example.com)。\n",
            Source::GithubMarkdown,
        )
        .unwrap();
        let kinds: Vec<Kind> = d.nodes[0].children.iter().map(|c| c.kind).collect();
        assert!(kinds.contains(&Kind::InlineCode), "{kinds:?}");
        assert!(kinds.contains(&Kind::Link), "{kinds:?}");
    }

    #[test]
    fn 表記は潰さない() {
        // 字種・字幅・空白の入れ方はそのまま残る。
        let md = "全角（かっこ）と半角(paren)、〜 と ～、… と ……。Rust と Ｒｕｓｔ。\n";
        let d = parse(md, Source::GithubMarkdown).unwrap();
        assert_eq!(
            d.nodes[0].text,
            "全角（かっこ）と半角(paren)、〜 と ～、… と ……。Rust と Ｒｕｓｔ。"
        );
    }

    #[test]
    fn front_matter_は本文ではない() {
        let md = "---\ntitle: 題\n---\n\n本文である。\n";
        let d = parse(md, Source::GithubMarkdown).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Paragraph]);
        assert_eq!(d.nodes[0].text, "本文である。");
    }

    #[test]
    fn 区切り線は箇条書きではない() {
        let d = parse("---\n", Source::GithubMarkdown).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Divider]);
    }

    #[test]
    fn 脚注の定義は脚注になる() {
        let d = parse("[^1]: これは脚注である。\n", Source::GithubMarkdown).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Footnote]);
        assert_eq!(d.nodes[0].text, "これは脚注である。");
    }

    #[test]
    fn 入れ子の項目は親の項目として数えない() {
        let md = "- 外側\n  - 内側\n";
        let d = parse(md, Source::GithubMarkdown).unwrap();
        // 最も外側のリストの直接の子だけ——「外側」の 1 つ。
        // 内側のリストは入れ子なので、それ自身も子も数えない。
        let items = d.items();
        assert_eq!(
            items.len(),
            1,
            "{:?}",
            items.iter().map(|i| &i.text).collect::<Vec<_>>()
        );
        assert_eq!(items[0].text, "外側");
    }

    #[test]
    fn 段落は空行で切れる() {
        let d = parse("ひとつめ。\n\nふたつめ。\n", Source::GithubMarkdown).unwrap();
        assert_eq!(d.paragraphs().len(), 2);
    }

    #[test]
    fn 段落は見出しや箇条書きで切れる() {
        let md = "本文。\n# 見出し\n- 項目\n| 表 |\n";
        let d = parse(md, Source::GithubMarkdown).unwrap();
        assert_eq!(
            kinds(&d),
            vec![Kind::Paragraph, Kind::Heading, Kind::Bullet, Kind::Table]
        );
    }

    #[test]
    fn 同じ内容は書式が違っても同じ正規形になる() {
        // 揃えなければ、測っているのは書き手ではなく取り込み元である。
        let a = parse("- ひとつ\n- ふたつ\n", Source::GithubMarkdown).unwrap();
        let b = parse("* ひとつ\n* ふたつ\n", Source::GithubMarkdown).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn 決定的である() {
        let md = "# 章\n\n本文である。\n\n> [!TIP]\n> 補足。\n";
        let a = parse(md, Source::GithubMarkdown).unwrap();
        let b = parse(md, Source::GithubMarkdown).unwrap();
        assert_eq!(a, b);
    }
}
