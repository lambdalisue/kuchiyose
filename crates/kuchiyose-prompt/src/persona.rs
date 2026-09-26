//! ペルソナの形（[仕様](../../../docs/spec/400-write.md#ペルソナ)）。
//!
//! 見出しだけを固定し、項目の書き方は自由にする。 見出しが自由だと、プロンプトの
//! どこに置けばよいかが決まらず、取り込みも「何かが書いてある」ことしか確かめられない。

use std::collections::BTreeMap;

/// 見出しと、その下に何を書くか。この順に並べる。
///
/// ペルソナを作らせるプロンプトも、取り込むときの確かめも、ここから引く。
pub const HEADINGS: [(&str, &str); 5] = [
    ("大事にすること", "何を良しとするか。判断の拠り所"),
    ("読み手", "誰に向けて書くか。前提にしている知識"),
    (
        "話の運び方",
        "何を先に言うか、結論をどこに置くか、例をどう使うか",
    ),
    ("書かないこと", "意図して避けている話題や言い方"),
    ("よく扱う領域", "繰り返し書いている題材の領域"),
];

/// 引用に要る、日本語の字数の下限。暫定値である。
///
/// 短い引用はどの記事にも現れて、確かめたことにならない。 一般論の項目が短い引用で
/// 素通りする例を見て導き直す。
pub const QUOTE_MIN_CHARS: usize = 8;

/// 読んだペルソナ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Persona {
    /// 見出しごとの節。[見出し](HEADINGS)の順に全部ある。
    pub sections: Vec<Section>,
}

/// 見出し 1 つぶん。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// 見出しの名前。
    pub heading: String,
    /// 見出しの下に書いてあるもの。前後の空行を除いて、そのまま持つ。
    pub body: String,
    /// 項目。
    pub items: Vec<Item>,
}

/// 項目。`-` の箇条 1 つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// 項目の文。
    pub text: String,
    /// 下の箇条に付けた引用。
    pub quotes: Vec<Quote>,
}

/// 引用。`「…」（単位の名前）` の形である。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quote {
    /// 記事の文字列。
    pub text: String,
    /// 単位の名前。素材のファイル名から拡張子を除いたもの。
    pub unit: String,
}

/// 形が決めたとおりでない。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShapeError {
    /// 欠けた見出し。
    pub missing: Vec<String>,
    /// 表に無い、または 2 度目の見出し。
    pub extra: Vec<String>,
    /// 見出しは揃っているが、順が違う。
    pub out_of_order: bool,
    /// 最初の見出しより前に、題（`#`）でない本文がある。
    pub preamble: bool,
    /// 本文が空である。
    pub empty: bool,
}

impl ShapeError {
    fn is_ok(&self) -> bool {
        self.missing.is_empty()
            && self.extra.is_empty()
            && !self.out_of_order
            && !self.preamble
            && !self.empty
    }
}

impl std::fmt::Display for ShapeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if self.empty {
            parts.push("本文が空である。外すなら --remove を使う".to_owned());
        }
        if !self.missing.is_empty() {
            parts.push(format!("欠けた見出し: {}", self.missing.join("、")));
        }
        if !self.extra.is_empty() {
            parts.push(format!("余計な見出し: {}", self.extra.join("、")));
        }
        if self.out_of_order {
            parts.push(format!(
                "見出しの順が違う。{} の順に並べる",
                names().join("、")
            ));
        }
        if self.preamble {
            parts.push("最初の見出しより前に本文がある。置けるのは `#` の題だけ".to_owned());
        }
        write!(f, "{}", parts.join("。"))
    }
}

impl std::error::Error for ShapeError {}

fn names() -> Vec<&'static str> {
    HEADINGS.iter().map(|(n, _)| *n).collect()
}

/// 行がコードブロックの囲みか。
fn is_fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

/// `## 見出し` なら見出しの名前。
fn heading_of(line: &str) -> Option<&str> {
    line.strip_prefix("## ").map(str::trim)
}

/// `- 「…」（…）` なら引用。
fn quote_of(line: &str) -> Option<Quote> {
    let rest = line.trim_start().strip_prefix("- ")?.trim_end();
    let rest = rest.strip_prefix('「')?;
    let rest = rest.strip_suffix('）')?;
    let split = rest.rfind("」（")?;
    Some(Quote {
        text: rest[..split].to_owned(),
        unit: rest[split + "」（".len()..].to_owned(),
    })
}

/// 読む。見出しが決めたとおりに揃っていなければ断る。
///
/// # Errors
///
/// 欠けた見出し、余計な見出し、順の違い、見出しより前の本文、空の本文を名指して断る。
pub fn parse(markdown: &str) -> Result<Persona, ShapeError> {
    let mut err = ShapeError {
        empty: markdown.trim().is_empty(),
        ..ShapeError::default()
    };
    let mut found: Vec<(String, Vec<&str>)> = Vec::new();
    let mut fenced = false;
    for line in markdown.lines() {
        if is_fence(line) {
            fenced = !fenced;
        }
        match (fenced, heading_of(line), found.last_mut()) {
            (false, Some(name), _) => found.push((name.to_owned(), Vec::new())),
            (_, _, Some((_, body))) => body.push(line),
            (_, _, None) => {
                let t = line.trim();
                if !t.is_empty() && !t.starts_with("# ") {
                    err.preamble = true;
                }
            }
        }
    }
    let known = names();
    let mut seen: Vec<&str> = Vec::new();
    for (name, _) in &found {
        match known.iter().find(|k| *k == name) {
            Some(k) if !seen.contains(k) => seen.push(k),
            _ => err.extra.push(name.clone()),
        }
    }
    err.missing = known
        .iter()
        .filter(|k| !seen.contains(k))
        .map(|k| (*k).to_owned())
        .collect();
    err.out_of_order = err.missing.is_empty() && err.extra.is_empty() && seen != known;
    if !err.is_ok() {
        return Err(err);
    }
    Ok(Persona {
        sections: found
            .into_iter()
            .map(|(heading, lines)| Section {
                body: lines.join("\n").trim_matches('\n').trim_end().to_owned(),
                items: items(&lines),
                heading,
            })
            .collect(),
    })
}

/// 節の本文から項目を取り出す。
///
/// 字下げの無い `- ` が項目を始め、その下の `- 「…」（…）` が引用である。
/// ほかの行は項目の文の続きとして読む。
fn items(lines: &[&str]) -> Vec<Item> {
    let mut out: Vec<Item> = Vec::new();
    let mut fenced = false;
    for line in lines {
        if is_fence(line) {
            fenced = !fenced;
        }
        if fenced || line.trim().is_empty() {
            continue;
        }
        if let Some(text) = line.strip_prefix("- ") {
            out.push(Item {
                text: text.trim().to_owned(),
                quotes: Vec::new(),
            });
            continue;
        }
        let Some(item) = out.last_mut() else {
            continue;
        };
        match quote_of(line) {
            Some(q) if line.starts_with(char::is_whitespace) => item.quotes.push(q),
            _ => {
                item.text.push(' ');
                item.text.push_str(line.trim());
            }
        }
    }
    out
}

/// 引用が解決しない理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// 名指した単位が素材のフォルダに無い。
    NoUnit,
    /// 単位はあるが、その文字列がどの node にも無い。
    NotFound,
    /// 地の文の日本語が[下限](QUOTE_MIN_CHARS)に届かない。
    TooShort,
}

impl Reason {
    /// 名前。
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Reason::NoUnit => "単位が無い",
            Reason::NotFound => "文字列が無い",
            Reason::TooShort => "短すぎる",
        }
    }
}

/// 解決しない引用 1 つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unresolved {
    /// 項目の見出し。
    pub heading: String,
    /// 項目の文。
    pub item: String,
    /// 引用。
    pub quote: Quote,
    /// 理由。
    pub reason: Reason,
}

/// 引用を照らした結果。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Citations {
    /// 解決しない引用。現れる順。
    pub unresolved: Vec<Unresolved>,
    /// 解決する引用を 1 つも持たない項目。見出しと項目の文。現れる順。
    pub unsupported: Vec<(String, String)>,
}

/// 空白類の並びを 1 つの空白にする。ほかは 1 文字も変えない。
fn squeeze(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !space {
                out.push(' ');
            }
            space = true;
        } else {
            out.push(c);
            space = false;
        }
    }
    out
}

/// 引用が素材に実在するかを照らす。
///
/// `units` は単位の名前から、その単位を正規化したあとの node ごとの地の文の文字列への表。
/// 引用は 1 つの node の文字列に部分文字列として現れなければならない。 途中を `…` で
/// 省いた引用は、記事にそのまま無いので解決しない。 `japanese` は日本語の字数を数える
/// 関数で、単位の定義を持つ側から渡す。
///
/// 確かめられるのは、引用が記事にあることまでである。 引用が項目を支えているか、
/// 項目がその人の考えとして正しいかは確かめない。
#[must_use]
pub fn check(
    persona: &Persona,
    units: &BTreeMap<String, Vec<String>>,
    japanese: &dyn Fn(&str) -> usize,
) -> Citations {
    let mut out = Citations::default();
    let squeezed: BTreeMap<&str, Vec<String>> = units
        .iter()
        .map(|(k, v)| (k.as_str(), v.iter().map(|s| squeeze(s)).collect()))
        .collect();
    for section in &persona.sections {
        for item in &section.items {
            let mut resolved = 0usize;
            for q in &item.quotes {
                let reason = if japanese(&q.text) < QUOTE_MIN_CHARS {
                    Some(Reason::TooShort)
                } else {
                    match squeezed.get(q.unit.as_str()) {
                        None => Some(Reason::NoUnit),
                        Some(nodes) => {
                            let needle = squeeze(&q.text);
                            (!nodes.iter().any(|n| n.contains(&needle))).then_some(Reason::NotFound)
                        }
                    }
                };
                match reason {
                    None => resolved += 1,
                    Some(reason) => out.unresolved.push(Unresolved {
                        heading: section.heading.clone(),
                        item: item.text.clone(),
                        quote: q.clone(),
                        reason,
                    }),
                }
            }
            if resolved == 0 {
                out.unsupported
                    .push((section.heading.clone(), item.text.clone()));
            }
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const PERSONA: &str = "\
# ありすえ

## 大事にすること

- 手を動かして確かめたことだけを書く
  - 「実際に手元で動かしてみると」（2024-05-vim-filer）
  - 「試した限りでは、この設定で足りました」（2023-11-denops）

## 読み手

- Vim を毎日使う人
  - 「毎日 Vim を開いている人なら」（2024-05-vim-filer）

## 話の運び方

## 書かないこと

- 他人の道具をけなさない

## よく扱う領域

- Vim と Deno
";

    fn japanese(s: &str) -> usize {
        s.chars()
            .filter(|c| {
                ('\u{3040}'..='\u{30FF}').contains(c) || ('\u{4E00}'..='\u{9FFF}').contains(c)
            })
            .count()
    }

    #[test]
    fn 見出しと項目と引用を取り出す() {
        let p = parse(PERSONA).expect("読める");
        let headings: Vec<&str> = p.sections.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(headings, names());
        let first = &p.sections[0];
        assert_eq!(first.items.len(), 1);
        assert_eq!(first.items[0].text, "手を動かして確かめたことだけを書く");
        assert_eq!(
            first.items[0].quotes[1],
            Quote {
                text: "試した限りでは、この設定で足りました".into(),
                unit: "2023-11-denops".into(),
            }
        );
        assert!(
            p.sections[2].body.is_empty(),
            "見出しの下が空なのはかまわない"
        );
        assert!(first.body.starts_with("- 手を動かして"), "{}", first.body);
    }

    #[test]
    fn 欠けた見出しと余計な見出しを名指して断る() {
        let md = PERSONA.replace("## 読み手", "## 想定読者");
        let e = parse(&md).unwrap_err();
        assert_eq!(e.missing, vec!["読み手".to_owned()]);
        assert_eq!(e.extra, vec!["想定読者".to_owned()]);
        let msg = e.to_string();
        assert!(msg.contains("読み手") && msg.contains("想定読者"), "{msg}");
    }

    #[test]
    fn 同じ見出しが_2_度あれば余計な見出しである() {
        let md = format!("{PERSONA}\n## 読み手\n\n- もう 1 つ\n");
        assert_eq!(parse(&md).unwrap_err().extra, vec!["読み手".to_owned()]);
    }

    #[test]
    fn 順の違う見出しは断る() {
        let md = PERSONA
            .replace("## 書かないこと", "## 仮")
            .replace("## よく扱う領域", "## 書かないこと")
            .replace("## 仮", "## よく扱う領域");
        let e = parse(&md).unwrap_err();
        assert!(e.out_of_order, "{e:?}");
    }

    #[test]
    fn 見出しより前の本文は断るが題は置ける() {
        assert!(parse(&format!("メモ\n{PERSONA}")).unwrap_err().preamble);
        assert!(parse(PERSONA).is_ok(), "題の `#` は置ける");
    }

    #[test]
    fn 空のペルソナは取り込まない() {
        assert!(parse("  \n").unwrap_err().empty);
    }

    #[test]
    fn コードブロックの中の見出しは見出しではない() {
        let md = PERSONA.replace(
            "- Vim と Deno\n",
            "- Vim と Deno\n\n```markdown\n## 例\n```\n",
        );
        assert!(parse(&md).is_ok(), "{:?}", parse(&md));
    }

    fn units() -> BTreeMap<String, Vec<String>> {
        [
            (
                "2024-05-vim-filer".to_owned(),
                vec![
                    "実際に手元で動かしてみると、思ったより速かった。".to_owned(),
                    "毎日 \t Vim を開いている人なら知っている。".to_owned(),
                ],
            ),
            (
                "2023-11-denops".to_owned(),
                vec!["試した限りでは、この設定で足りました。".to_owned()],
            ),
        ]
        .into()
    }

    #[test]
    fn 空白類の並びは_1_つの空白として比べる() {
        let p = parse(PERSONA).unwrap();
        let c = check(&p, &units(), &japanese);
        assert!(c.unresolved.is_empty(), "{c:?}");
    }

    #[test]
    fn 記事に無い引用と短すぎる引用と無い単位を指す引用は解決しないと名指す() {
        let md = PERSONA
            .replace("（2023-11-denops）", "（2099-01-none）")
            .replace("「実際に手元で動かしてみると」", "「実際に手元で…みると」")
            .replace("「毎日 Vim を開いている人なら」", "「毎日 Vim」");
        let p = parse(&md).unwrap();
        let c = check(&p, &units(), &japanese);
        let reasons: Vec<(String, Reason)> = c
            .unresolved
            .iter()
            .map(|u| (u.quote.unit.clone(), u.reason))
            .collect();
        assert_eq!(
            reasons,
            vec![
                ("2024-05-vim-filer".to_owned(), Reason::NotFound),
                ("2099-01-none".to_owned(), Reason::NoUnit),
                ("2024-05-vim-filer".to_owned(), Reason::TooShort),
            ]
        );
    }

    #[test]
    fn 解決する引用を持たない項目を名指す() {
        let p = parse(PERSONA).unwrap();
        let c = check(&p, &units(), &japanese);
        assert_eq!(
            c.unsupported,
            vec![
                (
                    "書かないこと".to_owned(),
                    "他人の道具をけなさない".to_owned()
                ),
                ("よく扱う領域".to_owned(), "Vim と Deno".to_owned()),
            ]
        );
    }
}
