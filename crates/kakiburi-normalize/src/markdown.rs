//! Markdown を正規形に落とす。
//!
//! 記法は潰す。表記は潰さない——字種、字幅、空白の入れ方はそのまま残す。

use std::collections::HashSet;
use std::iter::Peekable;
use std::str::Chars;

use kakiburi_doc::node::{Kind, Node};
use kakiburi_doc::Document;

use crate::markup;
use crate::refuse::Refusal;
use crate::space::WRAP;

/// 解決できる参照の名前。[`ref_key`]で揃えてある。
type Refs = HashSet<String>;

/// Markdown を読む。
///
/// 対応表に無い記法を見つけたら断る。推測して補足に落とさない。
pub fn parse(input: impl AsRef<str>) -> Result<Document, Refusal> {
    let input = input.as_ref();
    // 参照は定義より前で使える。 読みながらでは解決できないので、一度読んで
    // 定義を集めてから読み直す。 定義の見分けを block の振り分けと別に書くと、
    // 行としては定義なのに名前は解決できない、というずれが生まれる。
    let none = Refs::new();
    let refs: Refs = Parser::new(input, &none).read()?.1.into_iter().collect();
    let (nodes, _) = Parser::new(input, &refs).read()?;
    Ok(Document::new(nodes))
}

struct Parser<'a> {
    lines: Vec<&'a str>,
    at: usize,
    /// 本文の 1 行目。front matter の直後である。
    first: usize,
    refs: &'a Refs,
    /// 読んだ参照定義の名前。
    defined: Vec<String>,
    /// 直前の block がリストか脚注か。 字下げした行はその続きであって、
    /// コードブロックではない。
    after_item: bool,
    /// 入れ子の中身を読んでいるか。読み進められないときの断り文句に出す。
    nested: bool,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str, refs: &'a Refs) -> Self {
        let mut p = Self::of(input.lines().collect(), refs, false);
        p.skip_front_matter();
        p.first = p.at;
        p
    }

    fn of(lines: Vec<&'a str>, refs: &'a Refs, nested: bool) -> Self {
        Self {
            lines,
            at: 0,
            first: 0,
            refs,
            defined: Vec::new(),
            after_item: false,
            nested,
        }
    }

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

    /// 終わりまで block を読む。読んだ参照定義の名前も返す。
    fn read(mut self) -> Result<(Vec<Node>, Vec<String>), Refusal> {
        let mut nodes = Vec::new();
        while self.at < self.lines.len() {
            let before = self.at;
            let node = self.block()?;
            // block は必ず入力を進める。 進まなければ無限に回り、node を積み続けて
            // メモリを食い潰す。エラーにならないので、走らせるまで気付けない。
            if self.at == before {
                let whose = if self.nested { "中身の " } else { "" };
                return Err(Refusal::Broken {
                    detail: format!(
                        "{whose}{} 行目を読み進められない: {}",
                        before + 1,
                        self.lines[before]
                    ),
                });
            }
            if let Some(n) = node {
                nodes.push(n);
            }
        }
        Ok((nodes, self.defined))
    }

    /// 中身を block として解釈し直す。
    ///
    /// 子を持つ node の中身を 1 本の文字列に畳まない。 畳めば内側の段落・リスト・
    /// 表・コードブロックがまるごと消え、コードブロックの中身が地の文に混ざる。
    /// [文書の形](../../../docs/spec/020-document.md#文書は-node-でできている)は
    /// 引用・補足・警告・折りたたみ・脚注が子を持つと定めている。
    fn blocks_of(&mut self, body: &[&'a str]) -> Result<Vec<Node>, Refusal> {
        let (nodes, mut defined) = Parser::of(body.to_vec(), self.refs, true).read()?;
        self.defined.append(&mut defined);
        Ok(nodes)
    }

    /// 中身を持つ node を組む。文字は子が持つ。
    fn container(&mut self, kind: Kind, body: &[&'a str]) -> Result<Node, Refusal> {
        Ok(Node::branch(kind, self.blocks_of(body)?))
    }

    /// 前の行が空か、本文の頭か。
    fn follows_blank(&self) -> bool {
        self.at == self.first || self.lines[self.at - 1].trim().is_empty()
    }

    fn block(&mut self) -> Result<Option<Node>, Refusal> {
        let Some(line) = self.peek() else {
            return Ok(None);
        };
        if line.trim().is_empty() {
            self.at += 1;
            return Ok(None);
        }
        let continues_item = std::mem::replace(&mut self.after_item, false);
        // 段落の続きは paragraph が先に取るので、ここに来る字下げは段落の続きではない。
        // 項目の続きは除く——コードにすれば、書き手の文が地の文から消える。
        if strip_code_indent(line).is_some() && self.follows_blank() && !continues_item {
            return Ok(Some(self.indented_code()));
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
            return Ok(Some(Node::heading(depth, inline_checked(text, self.refs)?)));
        }
        if let Some(name) = footnote_definition(line) {
            let n = self.footnote(name)?;
            self.after_item = true;
            return Ok(Some(n));
        }
        // 参照定義の行は node にしない。 書き手が本文として書いたものではない。
        if let Some(label) = reference_definition(line) {
            self.at += 1;
            self.defined.push(ref_key(label));
            return Ok(None);
        }
        if let Some(rest) = details_open(line) {
            return self.details(rest).map(Some);
        }
        if is_details_close(line) {
            return Err(Refusal::Broken {
                detail: "</details> に対応する <details> が無い".into(),
            });
        }
        if line.trim_start().starts_with('>') {
            return self.quote_or_alert().map(Some);
        }
        if let Some(name) = directive_open(line) {
            return self.directive(name).map(Some);
        }
        if is_table_row(line) {
            return self.table().map(Some);
        }
        if list_marker(line).is_some() {
            let n = self.list()?;
            self.after_item = true;
            return Ok(Some(n));
        }
        self.paragraph().map(Some)
    }

    /// 4 字下げのコードブロック。 字下げを外した中身を持つ。
    fn indented_code(&mut self) -> Node {
        let mut body = Vec::new();
        while let Some(l) = self.peek() {
            if l.trim().is_empty() {
                body.push("");
            } else if let Some(rest) = strip_code_indent(l) {
                body.push(rest);
            } else {
                break;
            }
            self.at += 1;
        }
        // 末尾の空行はコードではない。 ブロックの切れ目である。
        while body.last() == Some(&"") {
            body.pop();
        }
        Node::leaf(Kind::CodeBlock, body.join("\n"))
    }

    /// `<details>` の折りたたみ。 中身は Markdown として読み直す。
    ///
    /// `rest` は開き札の後ろの残り。 `<summary>` はそこか中身の頭に来る。
    fn details(&mut self, rest: &'a str) -> Result<Node, Refusal> {
        self.at += 1;
        let mut body: Vec<&'a str> = Vec::new();
        // 1 行で閉じる折りたたみ。
        let one_line = rest.trim_end();
        let close = "</details>";
        if one_line.len() >= close.len()
            && one_line[one_line.len() - close.len()..].eq_ignore_ascii_case(close)
        {
            body.push(&one_line[..one_line.len() - close.len()]);
            let body = strip_summary(&body)?;
            return self.container(Kind::Details, &body);
        }
        if !rest.trim().is_empty() {
            body.push(rest);
        }
        let mut depth = 1usize;
        let mut fence = None;
        loop {
            let Some(l) = self.peek() else {
                return Err(Refusal::Broken {
                    detail: "<details> が閉じていない".into(),
                });
            };
            self.at += 1;
            // コードの中の札で開け閉めしない。 札の書き方を説明する記事は普通にある。
            if let Some(open) = fence {
                if closes_fence(l, open) {
                    fence = None;
                }
            } else if let Some(open) = fence_open(l) {
                fence = Some(open);
            } else if details_open(l).is_some() {
                depth += 1;
            } else if is_details_close(l) {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            body.push(l);
        }
        let body = strip_summary(&body)?;
        self.container(Kind::Details, &body)
    }

    fn fence(&mut self) -> Result<Node, Refusal> {
        let open = fence_open(self.lines[self.at]).expect("呼ぶ前に確かめている");
        self.at += 1;
        let mut body = Vec::new();
        while let Some(l) = self.peek() {
            self.at += 1;
            if closes_fence(l, open) {
                return Ok(Node::leaf(Kind::CodeBlock, body.join("\n")));
            }
            body.push(l);
        }
        // 閉じが無い。壊れた記法である。
        let (mark, n) = open;
        Err(Refusal::Broken {
            detail: format!(
                "コードブロックの {} が閉じていない",
                mark.to_string().repeat(n)
            ),
        })
    }

    /// 引用か Alert か。Alert を先に見る。
    ///
    /// 素朴に引用として解釈すると、引用の密度が実際より高く出て、補足の密度に
    /// 0 が並ぶ。エラーにならないので、ここを間違えても気付けない。
    fn quote_or_alert(&mut self) -> Result<Node, Refusal> {
        let body = self.take_quote_body();
        if let Some(name) = alert_marker(body.first().copied().unwrap_or("")) {
            let Some(kind) = markup::alert_kind(name) else {
                return Err(Refusal::UnknownMarkup {
                    markup: format!("[!{name}]"),
                });
            };
            return self.container(kind, &body[1..]);
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
        Ok(Node::leaf(Kind::Footnote, inline_checked(body, self.refs)?))
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
                    cells.push(Node::leaf(Kind::Cell, inline_checked(t, self.refs)?));
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
            // 落ちる先の node が無い。 通せばリンクとして数えられる。
            if let Some(mark) = task_marker(m.text) {
                return Err(Refusal::UnknownMarkup {
                    markup: format!("{mark}（task list）"),
                });
            }
            self.at += 1;
            {
                let (text, mut kids) = inline_with_children(m.text, self.refs)?;
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
            // 下線の見出し。 区切り線より先に見る——`---` は段落の直後なら見出しの
            // 下線で、空行の後なら区切り線である。
            if let Some(depth) = setext_underline(l) {
                self.at += 1;
                return Ok(Node::heading(
                    depth,
                    inline_checked(body.join("\n"), self.refs)?,
                ));
            }
            if self.starts_block(l) {
                break;
            }
            self.at += 1;
            body.push(l);
        }
        let (text, kids) = inline_with_children(body.join("\n"), self.refs)?;
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
            || directive_open(line).is_some()
            || details_open(line).is_some()
            || is_details_close(line)
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
fn inline_checked(s: impl AsRef<str>, refs: &Refs) -> Result<String, Refusal> {
    Ok(inline_with_children(s, refs)?.0)
}

/// 地の文と、行の中に溶けこむ子。
fn inline_with_children(s: impl AsRef<str>, refs: &Refs) -> Result<(String, Vec<Node>), Refusal> {
    let s = s.as_ref();
    let mut out = String::with_capacity(s.len());
    let mut kids: Vec<Node> = Vec::new();
    let mut in_emphasis = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // 直後が名前を始められない `<` は札ではない。`a < b` や `<-` のような
            // 地の文である。
            '<' if !chars
                .peek()
                .is_some_and(|c| c.is_ascii_alphabetic() || *c == '/') =>
            {
                out.push('<');
            }
            // Markdown の中のインライン HTML。 強調・リンク・コードの札だけを記法として
            // 潰す。 それ以外は、ブロックの札でも断る——認めれば補足や表に 2 通りの
            // 書き方ができる。HTML で書くなら `.html` として入れる。
            '<' => {
                let mut look = chars.clone();
                let mut raw = String::new();
                let mut closed = false;
                for c in look.by_ref() {
                    if c == '>' {
                        closed = true;
                        break;
                    }
                    raw.push(c);
                }
                if !closed {
                    // 記法ではない。`a < b` のような地の文である。
                    chars = look;
                    out.push('<');
                    out.push_str(&raw);
                    continue;
                }
                // 自動リンク。 見えている URL は参照先そのものなので、地の文に入らない。
                if is_autolink(&raw) {
                    chars = look;
                    out.push(WRAP);
                    kids.push(Node::leaf(Kind::Link, ""));
                    continue;
                }
                let name = html_tag_name(&raw);
                // HTML の要素名でなければ札ではない。`Box<dyn Trait>` や `Vec<T>` のような
                // 地の文である。 `<` だけを出し、後ろは地の文として読み続ける。
                if !markup::is_html_element(&name) {
                    out.push('<');
                    continue;
                }
                chars = look;
                let Some(kind) = inline_html_kind(&name) else {
                    return Err(Refusal::UnknownMarkup {
                        markup: format!("<{name}>"),
                    });
                };
                // 開き札だけを数える。閉じ札で二重に数えない。
                if raw.trim_start().starts_with('/') {
                    continue;
                }
                match kind {
                    Kind::Emphasis => kids.push(Node::leaf(Kind::Emphasis, "")),
                    // バッククォートで書いたときと同じく、中身ごと落とす。
                    Kind::InlineCode => {
                        let body =
                            take_until(&mut chars, "</code>").ok_or_else(|| Refusal::Broken {
                                detail: "<code> が閉じていない".into(),
                            })?;
                        out.push(WRAP);
                        kids.push(Node::leaf(Kind::InlineCode, body));
                    }
                    _ => {}
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
                out.push(WRAP);
                kids.push(Node::leaf(Kind::InlineCode, body));
            }
            // 画像は代替文字ごと落とす。
            '!' if chars.peek() == Some(&'[') => {
                chars.next();
                let alt = take_bracket(&mut chars);
                if resolve_target(&mut chars, &alt, refs) {
                    out.push(WRAP);
                    kids.push(Node::leaf(Kind::Image, alt));
                } else {
                    // 画像にならない。表示のとおり文字として残す。
                    let (text, inner) = inline_with_children(&alt, refs)?;
                    out.push_str(&format!("![{text}]"));
                    kids.extend(inner);
                }
            }
            // 脚注の参照。参照を解決しない。
            '[' if chars.peek() == Some(&'^') => {
                let label = take_bracket(&mut chars);
                skip_link_target(&mut chars);
                let (text, inner) = inline_with_children(&label, refs)?;
                out.push_str(&text);
                kids.push(Node::leaf(Kind::Link, text.clone()));
                kids.extend(inner);
            }
            // リンクは中身だけ残す。
            '[' => {
                let label = take_bracket(&mut chars);
                let (text, inner) = inline_with_children(&label, refs)?;
                if resolve_target(&mut chars, &label, refs) {
                    out.push_str(&text);
                    kids.push(Node::leaf(Kind::Link, text.clone()));
                } else {
                    // 定義の無い参照はリンクではない。 表示でも角括弧のまま残る。
                    out.push_str(&format!("[{text}]"));
                }
                kids.extend(inner);
            }
            // 取り消し線。 落ちる先の node が無い。 対にならない `~~` は記法ではない。
            '~' if chars.peek() == Some(&'~') && closes_strikethrough(&chars) => {
                return Err(Refusal::UnknownMarkup {
                    markup: "~~".into(),
                });
            }
            // エスケープ。 CommonMark のとおり、記号そのものにして記法にしない。
            // `\` を残せば記号として数えられ、表示と違う文字列を測る。
            '\\' if chars.peek().is_some_and(char::is_ascii_punctuation) => {
                out.extend(chars.next());
            }
            // 文字参照。 数値のものは文字にする——CommonMark と同じく、記法にはしない。
            // 名前のものは断る。 名前から文字を引く表を持たず、残せば記号として数えられる。
            '&' if char_reference(&chars).is_some() => {
                let name = char_reference(&chars).unwrap_or_default();
                let Some(c) = numeric_reference(&name) else {
                    return Err(Refusal::UnknownMarkup {
                        markup: format!("&{name};"),
                    });
                };
                for _ in 0..=name.chars().count() {
                    chars.next();
                }
                out.push(c);
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

/// Markdown の中で記法として潰す HTML の札。 強調・リンク・コードだけである。
fn inline_html_kind(name: &str) -> Option<Kind> {
    markup::html_kind(name).filter(|k| matches!(k, Kind::Emphasis | Kind::Link | Kind::InlineCode))
}

/// `<` と `>` の間が自動リンクか。URI とメールアドレスの 2 通り。
fn is_autolink(raw: &str) -> bool {
    if raw.is_empty() || raw.contains(|c: char| c.is_whitespace() || c == '<') {
        return false;
    }
    if let Some((scheme, _)) = raw.split_once(':') {
        if (2..=32).contains(&scheme.len())
            && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
        {
            return true;
        }
    }
    let Some((local, domain)) = raw.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".!#$%&'*+/=?^_`{|}~-".contains(c))
        && domain
            .split('.')
            .all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
}

/// `end` まで読み、手前までを返す。 `end` が来なければ `None`。
fn take_until(chars: &mut Peekable<Chars<'_>>, end: &str) -> Option<String> {
    let mut body = String::new();
    for c in chars.by_ref() {
        body.push(c);
        if body.to_ascii_lowercase().ends_with(end) {
            body.truncate(body.len() - end.len());
            return Some(body);
        }
    }
    None
}

/// `[` の後ろから、対になる `]` までを取る。 `]` は落とす。
fn take_bracket(chars: &mut Peekable<Chars<'_>>) -> String {
    let mut label = String::new();
    let mut depth = 1;
    for c in chars.by_ref() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        label.push(c);
    }
    label
}

/// 角括弧の後ろを見て、リンク（画像）になるかを決める。
///
/// なるなら参照先を読み飛ばす。 参照形式は定義があるときだけリンクになる——
/// 定義の無い `[語]` は、表示でも角括弧のまま残る文字である。
fn resolve_target(chars: &mut Peekable<Chars<'_>>, label: &str, refs: &Refs) -> bool {
    match chars.peek() {
        Some('(') => {
            skip_link_target(chars);
            true
        }
        // `[語][鍵]` と `[語][]`。 鍵に定義が無ければ `[語]` 単独の省略形にもならない。
        Some('[') => {
            let mut look = chars.clone();
            look.next();
            let mut key = String::new();
            let mut closed = false;
            for c in look.by_ref() {
                if c == ']' {
                    closed = true;
                    break;
                }
                key.push(c);
            }
            if !closed {
                return refs.contains(&ref_key(label));
            }
            let key = if key.trim().is_empty() { label } else { &key };
            if refs.contains(&ref_key(key)) {
                *chars = look;
                return true;
            }
            false
        }
        _ => refs.contains(&ref_key(label)),
    }
}

/// インラインリンクの参照先を読み飛ばす。地の文には入らない。
fn skip_link_target(chars: &mut Peekable<Chars<'_>>) {
    if chars.peek() != Some(&'(') {
        return;
    }
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

/// 最初の `~` を読んだ後ろが、閉じる `~~` を持つ取り消し線か。
fn closes_strikethrough(chars: &Peekable<Chars<'_>>) -> bool {
    let rest: String = chars.clone().skip(1).collect();
    rest.starts_with(|c: char| !c.is_whitespace() && c != '~') && rest.contains("~~")
}

/// `&` の後ろが文字参照なら、`;` の手前までを返す。
fn char_reference(chars: &Peekable<Chars<'_>>) -> Option<String> {
    let name: String = chars.clone().take(33).take_while(|&c| c != ';').collect();
    if name.chars().count() == 33 || chars.clone().nth(name.chars().count()) != Some(';') {
        return None;
    }
    let ok = if let Some(num) = name.strip_prefix('#') {
        if let Some(hex) = num.strip_prefix(['x', 'X']) {
            (1..=6).contains(&hex.len()) && hex.chars().all(|c| c.is_ascii_hexdigit())
        } else {
            (1..=7).contains(&num.len()) && num.chars().all(|c| c.is_ascii_digit())
        }
    } else {
        name.len() >= 2
            && name.starts_with(|c: char| c.is_ascii_alphabetic())
            && name.chars().all(|c| c.is_ascii_alphanumeric())
    };
    ok.then_some(name)
}

/// 数値の文字参照が指す文字。名前の文字参照なら `None`。
///
/// 文字にならない値（0、サロゲート、範囲外）は U+FFFD にする。CommonMark と同じである。
fn numeric_reference(name: &str) -> Option<char> {
    let num = name.strip_prefix('#')?;
    let code = match num.strip_prefix(['x', 'X']) {
        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
        None => num.parse::<u32>().ok()?,
    };
    Some(
        char::from_u32(code)
            .filter(|&c| c != '\0')
            .unwrap_or('\u{FFFD}'),
    )
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

/// コードブロックの囲みの開きなら、記号と数を返す。
fn fence_open(line: &str) -> Option<(char, usize)> {
    let t = line.trim_start();
    let mark = t.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let n = t.chars().take_while(|&c| c == mark).count();
    (n >= 3).then_some((mark, n))
}

/// 開きと同じ記号が、開き以上の数だけ並ぶ行か。
///
/// 数を見ないと、囲みの書き方を説明する記事で外側の ```` が内側の ``` で閉じ、
/// 内側のコードが地の文に流れる。 後ろに文字が続く行を閉じと見るのは、これまでの
/// 読み方を変えないためである。
fn closes_fence(line: &str, (mark, n): (char, usize)) -> bool {
    line.trim_start().chars().take_while(|&c| c == mark).count() >= n
}

/// 見出しの下線なら深さを返す。`=` は 1、`-` は 2。
fn setext_underline(line: &str) -> Option<u8> {
    if line.len() - line.trim_start_matches(' ').len() > 3 {
        return None;
    }
    let t = line.trim();
    if t.is_empty() {
        return None;
    }
    if t.chars().all(|c| c == '=') {
        Some(1)
    } else if t.chars().all(|c| c == '-') {
        Some(2)
    } else {
        None
    }
}

/// コードブロックの字下げを外す。空白 4 つかタブ 1 つ。
fn strip_code_indent(line: &str) -> Option<&str> {
    line.strip_prefix("    ")
        .or_else(|| line.strip_prefix('\t'))
}

/// `[名前]: 参照先 "題"` の行なら名前を返す。
///
/// 参照先の後ろに題でない文字が続くなら定義ではない。 `[追記]: 直した。` のような
/// 地の文を定義として飲み込まない。
fn reference_definition(line: &str) -> Option<&str> {
    if line.len() - line.trim_start_matches(' ').len() > 3 {
        return None;
    }
    let rest = line.trim_start().strip_prefix('[')?;
    // `[^1]:` は脚注の定義である。
    if rest.starts_with('^') {
        return None;
    }
    let end = rest.find("]:")?;
    let label = &rest[..end];
    if label.trim().is_empty() || label.contains(['[', ']']) {
        return None;
    }
    let target = rest[end + 2..].trim_start();
    let after = if let Some(r) = target.strip_prefix('<') {
        &r[r.find('>')? + 1..]
    } else {
        let e = target.find(char::is_whitespace).unwrap_or(target.len());
        if e == 0 {
            return None;
        }
        &target[e..]
    };
    let title = after.trim();
    let quoted = title.len() >= 2
        && [('"', '"'), ('\'', '\''), ('(', ')')]
            .iter()
            .any(|&(o, c)| title.starts_with(o) && title.ends_with(c));
    (title.is_empty() || quoted).then_some(label)
}

/// 参照の名前を揃える。大小と空白の入れ方は区別しない。
fn ref_key(label: &str) -> String {
    label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// `<details>` の開き札で始まる行なら、札の後ろの残りを返す。
fn details_open(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let head = t.get(..8)?;
    if !head.eq_ignore_ascii_case("<details") {
        return None;
    }
    let rest = &t[8..];
    if !rest.starts_with(|c: char| c == '>' || c.is_whitespace()) {
        return None;
    }
    Some(&rest[rest.find('>')? + 1..])
}

fn is_details_close(line: &str) -> bool {
    line.trim().eq_ignore_ascii_case("</details>")
}

/// 折りたたみの中身の頭にある `<summary>` を外す。
///
/// 題は本文に入れない。 `:::details 題` の題も入らない——記法ごとに変えると、
/// 同じ折りたたみが書き方だけで違う値になる。
fn strip_summary<'a>(body: &[&'a str]) -> Result<Vec<&'a str>, Refusal> {
    let Some(start) = body.iter().position(|l| !l.trim().is_empty()) else {
        return Ok(body.to_vec());
    };
    let head = body[start].trim_start();
    if !head
        .get(..8)
        .is_some_and(|h| h.eq_ignore_ascii_case("<summary"))
    {
        return Ok(body.to_vec());
    }
    for (i, l) in body.iter().enumerate().skip(start) {
        let lower = l.to_ascii_lowercase();
        if let Some(e) = lower.find("</summary>") {
            let tail = &l[e + "</summary>".len()..];
            let mut out = Vec::with_capacity(body.len());
            if !tail.trim().is_empty() {
                out.push(tail);
            }
            out.extend_from_slice(&body[i + 1..]);
            return Ok(out);
        }
    }
    Err(Refusal::Broken {
        detail: "<summary> が閉じていない".into(),
    })
}

/// task list の印なら返す。`[ ]` `[x]` `[X]`。
fn task_marker(text: &str) -> Option<&str> {
    let mark = text.get(..3)?;
    let is_mark = matches!(mark, "[ ]" | "[x]" | "[X]");
    let ends = text[3..].is_empty() || text[3..].starts_with(char::is_whitespace);
    (is_mark && ends).then_some(mark)
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
        let d = parse(md).unwrap();
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
        let d = parse(md).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Quote]);
        let inner: Vec<Kind> = d.nodes[0].children.iter().map(|n| n.kind).collect();
        assert_eq!(inner, vec![Kind::Paragraph, Kind::Bullet], "{inner:?}");
    }

    #[test]
    fn 引用の中のコードは地の文に混ざらない() {
        // 畳めば `let x = 1;` が地の文に入り、記号の率が題材で動く。
        let md = "> 例である。\n>\n> ```\n> let x = 1;\n> ```\n";
        let d = parse(md).unwrap();
        let joined: String = d.prose().iter().map(|s| s.text.clone()).collect();
        assert!(!joined.contains("let x"), "{joined:?}");
    }

    #[test]
    fn important_は警告に落ちる() {
        let d = parse("> [!IMPORTANT]\n> 読み飛ばすな。\n").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Warning]);
    }

    #[test]
    fn 対応表に無い_alert_は断る() {
        let md = "> [!HINT]\n> これは何か。\n";
        let e = parse(md).unwrap_err();
        assert!(matches!(e, Refusal::UnknownMarkup { .. }), "{e:?}");
    }

    #[test]
    fn 普通の引用は引用である() {
        let d = parse("> 引用である。\n").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Quote]);
    }

    #[test]
    fn directive_は補足と警告に分かれる() {
        let md = ":::note\n補足。\n:::\n\n:::warning\n警告。\n:::\n";
        let d = parse(md).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Note, Kind::Warning]);
    }

    #[test]
    fn zenn_の_message_は補足で_alert_つきは警告() {
        let md = ":::message\n補足。\n:::\n\n:::message alert\n警告。\n:::\n";
        let d = parse(md).unwrap();
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
            let r = parse(md);
            // 断るのはよい。回り続けるのが駄目である。
            let _ = r;
        }
    }

    #[test]
    fn 進まない行があれば断る() {
        // 不変条件そのものを試す。壊れたら止まらずに断る。
        let md = ":::\n";
        let d = parse(md);
        // `:::` だけの行は directive の開きではない。段落として消費される。
        assert!(d.is_ok(), "{d:?}");
    }

    #[test]
    fn 対応表に無い_directive_は断る() {
        // 地の文に流せば `:::` が記号として数えられ、中身が段落として数えられる。
        // エラーにならない。
        let md = ":::hint\n補足である。\n:::\n";
        let e = parse(md).unwrap_err();
        assert!(matches!(e, Refusal::UnknownMarkup { .. }), "{e:?}");
    }

    #[test]
    fn alert_と_directive_は同じ文書に並べられる() {
        // 2 つの記法は構文として重ならない。 だから方言を分けて名乗らせる
        // 理由が無い——分ければ、名乗り違えたときに Alert が引用に化ける。
        let md = "> [!NOTE]\n> 補足。\n\n:::message alert\n警告。\n:::\n";
        let d = parse(md).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Note, Kind::Warning]);
    }

    #[test]
    fn 字下げした閉じでも閉じる() {
        // 開きは字下げを許す。 閉じだけを行頭に縛れば、字下げして開いた
        // directive は決して閉じられない。整形器が箇条書きの直後の `:::` を
        // 下げることは実素材で普通に起きる。
        let md = ":::message\n- あ\n- い\n  :::\n";
        let doc = parse(md).expect("通る");
        assert_eq!(doc.nodes.len(), 1);
        assert_eq!(doc.nodes[0].kind, Kind::Note);
    }

    #[test]
    fn 字下げして開いた_directive_も閉じる() {
        let md = "  :::message\n  補足である。\n  :::\n";
        let doc = parse(md).expect("通る");
        assert_eq!(doc.nodes[0].kind, Kind::Note);
    }

    #[test]
    fn 閉じていない_directive_は断る() {
        let e = parse(":::note\n補足。\n").unwrap_err();
        assert!(matches!(e, Refusal::Broken { .. }), "{e:?}");
    }

    #[test]
    fn 閉じていないコードブロックは断る() {
        let e = parse("```rust\nlet x = 1;\n").unwrap_err();
        assert!(matches!(e, Refusal::Broken { .. }), "{e:?}");
    }

    #[test]
    fn 長い囲みは短い記号では閉じない() {
        // 囲みの書き方を説明する記事は、外側を 4 つ以上で囲む。 3 つで閉じれば
        // 内側のコードが地の文に流れる。
        let md = "````\n```mermaid\ngraph TD\n    A[Markdown] --> B[Arto]\n```\n````\n\n続き。\n";
        let d = parse(md).unwrap();
        assert_eq!(kinds(&d), vec![Kind::CodeBlock, Kind::Paragraph]);
        assert!(
            d.nodes[0].text.contains("graph TD"),
            "{:?}",
            d.nodes[0].text
        );
    }

    #[test]
    fn コードブロックの中身は地の文に入らない() {
        let md = "説明。\n\n```rust\nlet x = 1;\n```\n\n続き。\n";
        let d = parse(md).unwrap();
        let p = d.prose();
        assert_eq!(p.len(), 2);
        assert!(!p.iter().any(|s| s.text.contains("let")));
    }

    #[test]
    fn 箇条書きの記法は潰す() {
        // `-` と `*` と `+` は記法の違いであって書きぶりではない。
        for m in ['-', '*', '+'] {
            let d = parse(format!("{m} ひとつめ\n{m} ふたつめ\n")).unwrap();
            assert_eq!(kinds(&d), vec![Kind::Bullet], "{m} が箇条書きにならない");
            assert_eq!(d.items().len(), 2);
        }
    }

    #[test]
    fn 番号リストは箇条書きと分けたままにする() {
        // どちらを選ぶかは書き手の選択で、別の指標が測る。
        let d = parse("1. ひとつめ\n2. ふたつめ\n").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Ordered]);
    }

    #[test]
    fn 表はセルごとに_1_本になる() {
        let md = "| 機能 | あり |\n| --- | --- |\n| リモート | o |\n";
        let d = parse(md).unwrap();
        let p = d.prose();
        // 区切り行は落ちる。`o` だけのセルは地の文に入らない。
        let texts: Vec<&str> = p.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, vec!["機能", "あり", "リモート"], "{texts:?}");
    }

    #[test]
    fn 見出しの深さは記法から取る() {
        let d = parse("# 章\n## 節\n### 項\n").unwrap();
        assert_eq!(d.heading_depth(&d.nodes[0]), Some(1));
        assert_eq!(d.heading_depth(&d.nodes[2]), Some(3));
    }

    #[test]
    fn 空白の無い井桁は見出しではない() {
        let d = parse("#タグではない\n").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Paragraph]);
    }

    #[test]
    fn インラインコードの中身は落ちる() {
        let d = parse("設定は `--force` である。\n").unwrap();
        // 跡に空白を残さない。**両側の空白ごと落ちる。**
        assert_eq!(d.nodes[0].text, "設定はである。");
        // 和欧のあいだなら、表示されるぶんの空白 1 個が残る。
        let d = parse("run the `--force` flag\n").unwrap();
        assert_eq!(d.nodes[0].text, "run the flag");
    }

    #[test]
    fn リンクは中身だけ残り参照先は落ちる() {
        let d = parse("詳細は [こちら](https://example.com) を見る。\n").unwrap();
        assert_eq!(d.nodes[0].text, "詳細は こちら を見る。");
    }

    #[test]
    fn 参照形式のリンクも参照先が落ちる() {
        let md = "[Netrw][] と [Fern][fern] を比べる。\n\n[netrw]: https://example.com/netrw\n[fern]: https://example.com/fern\n";
        let d = parse(md).unwrap();
        assert_eq!(d.nodes[0].text, "Netrw と Fern を比べる。");
    }

    #[test]
    fn 参照定義の行は何も残さない() {
        // 残せば URL が地の文に入り、記号と英字の率が参照の数で動く。
        let md = "詳細は [こちら][ref] を見る。\n\n[ref]: https://example.com \"題\"\n";
        let d = parse(md).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Paragraph]);
        let joined: String = d.prose().iter().map(|s| s.text.clone()).collect();
        assert!(!joined.contains("example"), "{joined:?}");
    }

    #[test]
    fn 参照先の後ろに地の文が続く行は定義ではない() {
        // 定義として飲み込めば、書き手の文が消える。
        let d = parse("[追記]: 直した。 後で見る。\n").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Paragraph]);
        assert_eq!(d.nodes[0].text, "[追記]: 直した。 後で見る。");
    }

    #[test]
    fn 定義のある省略形の参照はリンクになる() {
        let md = "[VimConf] に出た。\n\n[VimConf]: https://vimconf.org\n";
        let d = parse(md).unwrap();
        assert_eq!(d.nodes[0].text, "VimConf に出た。");
        assert!(d.nodes[0].children.iter().any(|c| c.kind == Kind::Link));
    }

    #[test]
    fn 定義の無い角括弧はリンクではない() {
        // 表示でも角括弧のまま残る。リンクとして数えれば、リンクの密度が
        // 角括弧を使う書き手ほど上がる。
        let d = parse("[追記] 直した。\n").unwrap();
        assert_eq!(d.nodes[0].text, "[追記] 直した。");
        assert!(!d.nodes[0].children.iter().any(|c| c.kind == Kind::Link));
    }

    #[test]
    fn 参照は定義より前でも後でも解決する() {
        let md =
            "[前]: https://example.com/a\n\n[前] と [後] を見る。\n\n[後]: https://example.com/b\n";
        let d = parse(md).unwrap();
        let links = d.nodes[0]
            .children
            .iter()
            .filter(|c| c.kind == Kind::Link)
            .count();
        assert_eq!(links, 2);
    }

    #[test]
    fn 自動リンクはリンクになり_url_は地の文に入らない() {
        let d = parse("詳細は <https://example.com/a> を見る。\n").unwrap();
        assert!(d.nodes[0].children.iter().any(|c| c.kind == Kind::Link));
        assert!(
            !d.nodes[0].text.contains("example"),
            "{:?}",
            d.nodes[0].text
        );
        let d = parse("連絡は <foo@example.com> へ。\n").unwrap();
        assert!(d.nodes[0].children.iter().any(|c| c.kind == Kind::Link));
    }

    #[test]
    fn 札になれない不等号は地の文である() {
        // 直後が名前を始められない `<` は札ではない。 断れば矢印を書いた記事が落ちる。
        let d = parse("a <- b -> c の順に流れる。\n").unwrap();
        assert_eq!(d.nodes[0].text, "a <- b -> c の順に流れる。");
    }

    #[test]
    fn 下線の見出しを読む() {
        // 知らずに読むと見出しが 0 になり、見出しの文字列が段落として数えられる。
        let d = parse("章\n===\n\n本文である。\n\n節\n---\n").unwrap();
        assert_eq!(
            kinds(&d),
            vec![Kind::Heading, Kind::Paragraph, Kind::Heading]
        );
        assert_eq!(d.nodes[0].text, "章");
        assert_eq!(d.nodes[0].raw_depth, Some(1));
        assert_eq!(d.nodes[2].raw_depth, Some(2));
    }

    #[test]
    fn 空行のあとの横線は区切り線である() {
        let d = parse("本文である。\n\n---\n").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Paragraph, Kind::Divider]);
    }

    #[test]
    fn 字下げ_4_つはコードブロックである() {
        let md = "説明である。\n\n    let x = 1;\n\n    let y = 2;\n\n続きである。\n";
        let d = parse(md).unwrap();
        assert_eq!(
            kinds(&d),
            vec![Kind::Paragraph, Kind::CodeBlock, Kind::Paragraph]
        );
        assert_eq!(d.nodes[1].text, "let x = 1;\n\nlet y = 2;");
        let joined: String = d.prose().iter().map(|s| s.text.clone()).collect();
        assert!(!joined.contains("let"), "{joined:?}");
    }

    #[test]
    fn 段落の続きの字下げはコードではない() {
        let d = parse("説明である。\n    続きである。\n").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Paragraph]);
    }

    #[test]
    fn 項目の続きの字下げはコードではない() {
        // 項目の中身の続きである。コードにすれば、書き手の文が地の文から消える。
        let d = parse("- 項目である。\n\n    続きである。\n").unwrap();
        assert!(!kinds(&d).contains(&Kind::CodeBlock), "{:?}", kinds(&d));
    }

    #[test]
    fn task_list_は断る() {
        // 落ちる先の node が無い。リンクとして数えれば、リンクの密度が狂う。
        for md in ["- [ ] やる\n", "- [x] やった\n", "1. [X] やった\n"] {
            let e = parse(md).unwrap_err();
            assert!(matches!(e, Refusal::UnknownMarkup { .. }), "{md:?} {e:?}");
        }
    }

    #[test]
    fn 取り消し線は断る() {
        let e = parse("これは ~~消した~~ 文である。\n").unwrap_err();
        assert!(matches!(e, Refusal::UnknownMarkup { .. }), "{e:?}");
    }

    #[test]
    fn 対にならないチルダは地の文である() {
        let d = parse("1~2 回か、~~ だけの行である。\n").unwrap();
        assert_eq!(d.nodes[0].text, "1~2 回か、~~ だけの行である。");
    }

    #[test]
    fn エスケープは表示される記号にする() {
        // 表示のとおりの文字列を測る。 `\` を残せば記号として数えられる。
        let d = parse("識別子は trace\\_id と span\\_id である。\n").unwrap();
        assert_eq!(d.nodes[0].text, "識別子は trace_id と span_id である。");
    }

    #[test]
    fn エスケープした記号は記法にならない() {
        let d = parse(
            "これは \\*\\*強調ではない\\*\\* 文で \\[角括弧\\](a) と \\`記号\\` と \\<em> である。\n",
        )
        .unwrap();
        assert_eq!(
            d.nodes[0].text,
            "これは **強調ではない** 文で [角括弧](a) と `記号` と <em> である。"
        );
        assert!(d.nodes[0].children.is_empty(), "{:?}", d.nodes[0].children);
    }

    #[test]
    fn 逆斜線そのものもエスケープできる() {
        let d = parse("置き場は C:\\\\Users である。\n").unwrap();
        assert_eq!(d.nodes[0].text, "置き場は C:\\Users である。");
    }

    #[test]
    fn 記号の前に無い逆斜線は地の文である() {
        let d = parse("置き場は C:\\Users である。\n").unwrap();
        assert_eq!(d.nodes[0].text, "置き場は C:\\Users である。");
    }

    #[test]
    fn 名前の文字参照は断る() {
        // 名前から文字を引く表を持たない。 残せば `&amp;` が記号として数えられる。
        let e = parse("A &amp; B である。\n").unwrap_err();
        assert!(matches!(e, Refusal::UnknownMarkup { .. }), "{e:?}");
    }

    #[test]
    fn 数値の文字参照は表示される文字にする() {
        for md in [
            "&#12354; である。\n",
            "&#x3042; である。\n",
            "&#X3042; である。\n",
        ] {
            let d = parse(md).unwrap();
            assert_eq!(d.nodes[0].text, "あ である。", "{md:?}");
        }
    }

    #[test]
    fn 文字にならない数値の文字参照は置換文字にする() {
        // CommonMark と同じ。 0 とサロゲートと範囲外は U+FFFD になる。
        let d = parse("&#0; と &#xD800; と &#1114112; である。\n").unwrap();
        assert_eq!(d.nodes[0].text, "\u{FFFD} と \u{FFFD} と \u{FFFD} である。");
    }

    #[test]
    fn 文字参照で書いた記号は記法にならない() {
        let d = parse("これは &#42;&#42;強調ではない&#42;&#42; である。\n").unwrap();
        assert_eq!(d.nodes[0].text, "これは **強調ではない** である。");
        assert!(d.nodes[0].children.is_empty());
    }

    #[test]
    fn 参照にならないアンパサンドは地の文である() {
        let d = parse("R&D と A & B である。\n").unwrap();
        assert_eq!(d.nodes[0].text, "R&D と A & B である。");
    }

    #[test]
    fn details_は折りたたみになる() {
        let md = "<details>\n<summary>題</summary>\n\n中身である。\n\n- 項目\n\n</details>\n";
        let d = parse(md).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Details]);
        let inner: Vec<Kind> = d.nodes[0].children.iter().map(|n| n.kind).collect();
        assert_eq!(inner, vec![Kind::Paragraph, Kind::Bullet], "{inner:?}");
    }

    #[test]
    fn details_の_summary_は地の文に入らない() {
        // `:::details 題` の題と同じ扱いにする。 記法ごとに変えると、
        // 同じ折りたたみが書き方だけで違う値になる。
        let md = "<details><summary>題である</summary>\n\n中身である。\n\n</details>\n";
        let d = parse(md).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Details]);
        let joined: String = d.prose().iter().map(|s| s.text.clone()).collect();
        assert_eq!(joined, "中身である。");
    }

    #[test]
    fn summary_の無い_details_も折りたたみになる() {
        let d = parse("<details>\n中身である。\n</details>\n").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Details]);
        assert_eq!(d.nodes[0].children[0].text, "中身である。");
    }

    #[test]
    fn 一行で閉じる_details_も折りたたみになる() {
        let d = parse("<details><summary>題</summary>中身である。</details>\n\n後。\n").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Details, Kind::Paragraph]);
        assert_eq!(d.nodes[0].children[0].text, "中身である。");
    }

    #[test]
    fn 入れ子の_details_は内側で閉じない() {
        let md = "<details>\n\n外。\n\n<details>\n\n内。\n\n</details>\n\n</details>\n\n後。\n";
        let d = parse(md).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Details, Kind::Paragraph]);
        let inner: Vec<Kind> = d.nodes[0].children.iter().map(|n| n.kind).collect();
        assert_eq!(inner, vec![Kind::Paragraph, Kind::Details], "{inner:?}");
    }

    #[test]
    fn コードの中の閉じ札では閉じない() {
        let md = "<details>\n\n```html\n</details>\n```\n\n</details>\n";
        let d = parse(md).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Details]);
        assert_eq!(d.nodes[0].children[0].kind, Kind::CodeBlock);
    }

    #[test]
    fn 閉じていない_details_は断る() {
        let e = parse("<details>\n中身である。\n").unwrap_err();
        assert!(matches!(e, Refusal::Broken { .. }), "{e:?}");
    }

    #[test]
    fn details_以外のブロックの_html_は断る() {
        // 認めれば、同じ補足や表に 2 通りの書き方ができる。 HTML で書くなら
        // `.html` として入れる。
        for md in [
            "<aside>\n補足である。\n</aside>\n",
            "<div>中身である。</div>\n",
            "<p>段落である。</p>\n",
            "<table><tr><td>升目</td></tr></table>\n",
            "<blockquote>引用である。</blockquote>\n",
            "本文の途中に <details> がある。\n",
        ] {
            let e = parse(md).unwrap_err();
            assert!(matches!(e, Refusal::UnknownMarkup { .. }), "{md:?} {e:?}");
        }
    }

    #[test]
    fn html_の画像は断る() {
        let e = parse("図である。<img src=\"a.png\">\n").unwrap_err();
        assert!(matches!(e, Refusal::UnknownMarkup { .. }), "{e:?}");
    }

    #[test]
    fn html_の_code_は中身ごと落ちる() {
        // 中身を残せば、バッククォートで書いたときと違う地の文になる。
        let d = parse("設定は <code>--force</code> である。\n").unwrap();
        assert_eq!(d.nodes[0].text, "設定はである。");
        assert!(d.nodes[0]
            .children
            .iter()
            .any(|c| c.kind == Kind::InlineCode));
    }

    #[test]
    fn 画像は代替文字ごと落ちる() {
        let d = parse("図。![構成図](a.png)\n").unwrap();
        assert_eq!(d.nodes[0].text, "図。");
    }

    #[test]
    fn 強調の記法は潰れて中身が残る() {
        let d = parse("ここが **大事** である。\n").unwrap();
        assert_eq!(d.nodes[0].text, "ここが 大事 である。");
        let d = parse("ここが __大事__ である。\n").unwrap();
        assert_eq!(d.nodes[0].text, "ここが 大事 である。");
    }

    #[test]
    fn 対応表にある_html_の札は潰れて中身が残る() {
        // Markdown の中にインライン HTML は来る。落とさなければ `<strong>` が
        // 地の文に入り、記号の率が上がる。
        let d = parse("ここが <em>大事</em> である。\n").unwrap();
        assert_eq!(d.nodes[0].text, "ここが 大事 である。");
    }

    #[test]
    fn 属性つきの札も潰れる() {
        let d = parse("詳細は <a href=\"https://example.com\">こちら</a>。\n").unwrap();
        assert_eq!(d.nodes[0].text, "詳細は こちら。");
    }

    #[test]
    fn 対応表に無い_html_の札は断る() {
        let e = parse("これは <blink>点滅</blink> する。\n").unwrap_err();
        assert!(matches!(e, Refusal::UnknownMarkup { .. }), "{e:?}");
    }

    #[test]
    fn html_の要素名でない札は地の文である() {
        // 型の引数を書いた地の文である。 HTML の札として断れば、技術記事が丸ごと入らない。
        for (md, want) in [
            (
                "戻り値は Box<dyn Trait> である。\n",
                "戻り値は Box<dyn Trait> である。",
            ),
            ("型は Vec<T> である。\n", "型は Vec<T> である。"),
            ("型は Result<T, E> である。\n", "型は Result<T, E> である。"),
            (
                "型は Result<(), Box<dyn Error>> である。\n",
                "型は Result<(), Box<dyn Error>> である。",
            ),
        ] {
            let d = parse(md).unwrap();
            assert_eq!(d.nodes[0].text, want, "{md:?}");
        }
    }

    #[test]
    fn 要素名でない札の後ろの記法は潰れる() {
        // 札でないなら、`<` の後ろも地の文として読む。
        let d = parse("型は Vec<T> で **大事** である。\n").unwrap();
        assert_eq!(d.nodes[0].text, "型は Vec<T> で 大事 である。");
        assert!(d.nodes[0].children.iter().any(|c| c.kind == Kind::Emphasis));
    }

    #[test]
    fn 要素名の札は対応表に無ければ断る() {
        for md in [
            "本文の途中に <aside> がある。\n",
            "これは <blink>点滅</blink> する。\n",
            "これは <span>範囲</span> である。\n",
        ] {
            let e = parse(md).unwrap_err();
            assert!(matches!(e, Refusal::UnknownMarkup { .. }), "{md:?} {e:?}");
        }
    }

    #[test]
    fn 閉じていない不等号は地の文である() {
        // `a < b` は記法ではない。断ってはいけない。
        let d = parse("条件は a < b である。\n").unwrap();
        assert_eq!(d.nodes[0].text, "条件は a < b である。");
    }

    #[test]
    fn 強調は数えるための_node_になる() {
        // 記法を落とすだけだと、強調の指標が永久に 0 になる。
        // エラーにならないので、実際の記事に当てるまで気付けない。
        let stars = "\u{2A}\u{2A}";
        let md = format!("ここが {stars}大事{stars} である。\n");
        let d = parse(&md).unwrap();
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
        let d = parse(&md).unwrap();
        let p = d.prose();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].text, "大事である。", "地の文に 1 度だけ入る");
    }

    #[test]
    fn html_の強調も数える() {
        let d = parse("ここが <em>大事</em> である。\n").unwrap();
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
        let d = parse(&md).unwrap();
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
        let d = parse("設定は `--force` で、詳細は [こちら](https://example.com)。\n").unwrap();
        let kinds: Vec<Kind> = d.nodes[0].children.iter().map(|c| c.kind).collect();
        assert!(kinds.contains(&Kind::InlineCode), "{kinds:?}");
        assert!(kinds.contains(&Kind::Link), "{kinds:?}");
    }

    #[test]
    fn 表記は潰さない() {
        // 字種・字幅・空白の入れ方はそのまま残る。
        let md = "全角（かっこ）と半角(paren)、〜 と ～、… と ……。Rust と Ｒｕｓｔ。\n";
        let d = parse(md).unwrap();
        assert_eq!(
            d.nodes[0].text,
            "全角（かっこ）と半角(paren)、〜 と ～、… と ……。Rust と Ｒｕｓｔ。"
        );
    }

    #[test]
    fn front_matter_は本文ではない() {
        let md = "---\ntitle: 題\n---\n\n本文である。\n";
        let d = parse(md).unwrap();
        assert_eq!(kinds(&d), vec![Kind::Paragraph]);
        assert_eq!(d.nodes[0].text, "本文である。");
    }

    #[test]
    fn 区切り線は箇条書きではない() {
        let d = parse("---\n").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Divider]);
    }

    #[test]
    fn 脚注の定義は脚注になる() {
        let d = parse("[^1]: これは脚注である。\n").unwrap();
        assert_eq!(kinds(&d), vec![Kind::Footnote]);
        assert_eq!(d.nodes[0].text, "これは脚注である。");
    }

    #[test]
    fn 入れ子の項目は親の項目として数えない() {
        let md = "- 外側\n  - 内側\n";
        let d = parse(md).unwrap();
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
        let d = parse("ひとつめ。\n\nふたつめ。\n").unwrap();
        assert_eq!(d.paragraphs().len(), 2);
    }

    #[test]
    fn 段落は見出しや箇条書きで切れる() {
        let md = "本文。\n# 見出し\n- 項目\n| 表 |\n";
        let d = parse(md).unwrap();
        assert_eq!(
            kinds(&d),
            vec![Kind::Paragraph, Kind::Heading, Kind::Bullet, Kind::Table]
        );
    }

    #[test]
    fn 同じ内容は書式が違っても同じ正規形になる() {
        // 揃えなければ、測っているのは書き手ではなく取り込み元である。
        let a = parse("- ひとつ\n- ふたつ\n").unwrap();
        let b = parse("* ひとつ\n* ふたつ\n").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn 決定的である() {
        let md = "# 章\n\n本文である。\n\n> [!TIP]\n> 補足。\n";
        let a = parse(md).unwrap();
        let b = parse(md).unwrap();
        assert_eq!(a, b);
    }
}
