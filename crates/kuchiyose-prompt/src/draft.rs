//! 代筆のプロンプト（[仕様](../../../docs/spec/400-write.md#代筆のプロンプト)）。
//!
//! | 順 | 中身 |
//! | --- | --- |
//! | 1 | 役目。その人の代わりに書くこと、内容は使う人と詰めてから書くこと |
//! | 2 | ペルソナ。無ければこの節を置かない |
//! | 3 | 文体の事実 |
//! | 4 | 要約 |
//! | 5 | 保存先 |
//!
//! 文体の事実は目安として渡し、全部を合わせにいかせない。 プロンプトで表面は寄らない
//! ので、合わせようとして内容が崩れるほうが損である。表面は周回が寄せる。

use std::fmt::Write as _;

use crate::persona::Persona;
use crate::{fence_for, with_commas};

/// 型と避ける言い回しを、それぞれ何本まで渡すか。
///
/// [一度に渡すのは 3 本か 4 本](../../../docs/spec/010-strategy.md#一度に渡すのは-3-本か-4-本)
/// と同じ理由である。多く並べると受け取った側が扱いきれない。
pub const MAX_PHRASES: usize = 4;

/// 文体。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Register {
    /// です・ます。
    Polite,
    /// だ・である。
    Plain,
}

/// 文体の事実。数えた結果か、本人の申告か。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterFact {
    /// 文体。
    pub register: Register,
    /// 申告か。
    pub declared: bool,
}

/// 一人称。
#[derive(Debug, Clone, PartialEq)]
pub struct FirstPerson {
    /// 語。
    pub word: String,
    /// 使った記事の割合。申告なら `None`。
    pub rate: Option<f64>,
}

/// 本人の型。
#[derive(Debug, Clone, PartialEq)]
pub struct Kata {
    /// 言い回し。
    pub text: String,
    /// 使った記事の割合。
    pub rate: f64,
    /// 使う位置が偏っていれば、その場所（書き出し、結び）。
    pub place: Option<String>,
    /// 1 本の中で使う、日本語 1,000 字あたりの最大。
    pub ceiling: f64,
}

/// 避ける言い回しの種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvoidKind {
    /// 基準の型。語の並び。
    Kata,
    /// 基準の語。語彙素。
    Goi,
}

/// 避ける言い回し。基準がよく使い、本人は使わない。
#[derive(Debug, Clone, PartialEq)]
pub struct Avoid {
    /// 言い回しか語彙素。
    pub text: String,
    /// 種類。
    pub kind: AvoidKind,
    /// 基準の記事のうち、これを使う割合。
    pub rate: f64,
    /// 基準の語なら、本人が同じ品詞でよく使う語。
    pub instead: Vec<String>,
}

/// 地の文の長さ。本人の記事の日本語の字数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Length {
    /// 中央値。
    pub median: usize,
    /// 最短。
    pub low: usize,
    /// 最長。
    pub high: usize,
}

/// 段落の組み方に関わる指示できる指標の、本人の幅。
#[derive(Debug, Clone, PartialEq)]
pub struct Spread {
    /// 指標の名前。
    pub name: String,
    /// 単位。
    pub unit: String,
    /// 最小。
    pub low: f64,
    /// 最大。
    pub high: f64,
}

/// 文体の事実。形代と基準から組み立て層が取り出して渡す。
///
/// 基準が要るもの（型、避ける言い回し）は、目盛りが組み立てられなければ空で渡る。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StyleFacts {
    /// 文体。
    pub register: Option<RegisterFact>,
    /// 一人称。
    pub first_person: Option<FirstPerson>,
    /// 本人の型。無効にしたものは入れない。
    pub katas: Vec<Kata>,
    /// 避ける言い回し。無効にしたものは入れない。
    pub avoid: Vec<Avoid>,
    /// 地の文の長さ。
    pub length: Option<Length>,
    /// 段落の組み方。
    pub paragraphs: Vec<Spread>,
}

/// 代筆のプロンプトの材料。
#[derive(Debug, Clone, Copy)]
pub struct DraftRequest<'a> {
    /// ペルソナ。形代が持たなければ `None`。
    pub persona: Option<&'a Persona>,
    /// 文体の事実。
    pub facts: &'a StyleFacts,
    /// 要約。使う人が書いたもの。
    pub brief: &'a str,
    /// 草稿の保存先。
    pub save_path: &'a str,
}

/// 値の大きい順、並んだら文字列の順で、上から `MAX_PHRASES` 本。
fn top<T>(items: &[T], rate: impl Fn(&T) -> f64, text: impl Fn(&T) -> &str) -> Vec<&T> {
    let mut v: Vec<&T> = items.iter().collect();
    v.sort_by(|a, b| {
        rate(b)
            .partial_cmp(&rate(a))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| text(a).cmp(text(b)))
    });
    v.truncate(MAX_PHRASES);
    v
}

fn percent(rate: f64) -> String {
    format!("{:.0}%", rate * 100.0)
}

fn facts_section(f: &StyleFacts) -> String {
    let mut out = String::new();
    if let Some(r) = f.register {
        let (name, how) = match r.register {
            Register::Polite => ("敬体", "です・ます"),
            Register::Plain => ("常体", "だ・である"),
        };
        let source = if r.declared {
            "本人の申告"
        } else {
            "数えた結果"
        };
        let _ = writeln!(out, "- 文体: {name}（{how}）で書く。{source}。");
    }
    if let Some(p) = &f.first_person {
        let source = p.rate.map_or_else(
            || "本人の申告".to_owned(),
            |r| format!("{} の記事で使っている", percent(r)),
        );
        let _ = writeln!(out, "- 一人称: 「{}」。{source}。", p.word);
    }
    if let Some(l) = f.length {
        let _ = writeln!(
            out,
            "- 長さ: 地の文の日本語は、中央値 {} 字、短いもので {} 字、長いもので {} 字。",
            with_commas(l.median),
            with_commas(l.low),
            with_commas(l.high)
        );
    }
    if !f.paragraphs.is_empty() {
        let _ = writeln!(out, "- 段落の組み方。この人の記事の幅は次のとおり。");
        for s in &f.paragraphs {
            let _ = writeln!(
                out,
                "  - {}: {:.2}〜{:.2}（{}）",
                s.name, s.low, s.high, s.unit
            );
        }
    }
    let katas = top(&f.katas, |k| k.rate, |k| &k.text);
    if !katas.is_empty() {
        let _ = writeln!(
            out,
            "- この人がよく使う言い回し。使うなら、日本語 1,000 字あたりの上限を超えない。"
        );
        for k in katas {
            let place = k
                .place
                .as_deref()
                .map_or_else(String::new, |p| format!("{p}で使う。"));
            let _ = writeln!(
                out,
                "  - 「{}」: {} の記事で使っている。{place}1,000 字あたり {:.1} 回まで。",
                k.text,
                percent(k.rate),
                k.ceiling
            );
        }
    }
    let avoid = top(&f.avoid, |a| a.rate, |a| &a.text);
    if !avoid.is_empty() {
        let _ = writeln!(
            out,
            "- 避ける言い回し。基準の文章（LLM が素で書いた文章）がよく使い、この人は使わない。"
        );
        for a in avoid {
            let kind = match a.kind {
                AvoidKind::Kata => "言い回し",
                AvoidKind::Goi => "語",
            };
            let instead = if a.instead.is_empty() {
                String::new()
            } else {
                format!(
                    "この人が同じ品詞でよく使うのは「{}」。",
                    a.instead.join("」「")
                )
            };
            let _ = writeln!(
                out,
                "  - {kind}「{}」: 基準の {} が使う。{instead}",
                a.text,
                percent(a.rate)
            );
        }
    }
    out
}

/// 代筆のプロンプトを組み立てる。同じ材料からは同じバイト列が出る。
#[must_use]
pub fn draft_prompt(r: &DraftRequest<'_>) -> String {
    let mut out = String::new();
    out.push_str("# 代筆の依頼\n\n");
    out.push_str(
        "あなたは、この人の代わりに文章を書く。いきなり書かない。まず使う人と対話して、\
         何を、誰に向けて、どういう順で書くかを詰める。内容が固まってから草稿を書く。\n",
    );
    if let Some(p) = r.persona {
        out.push_str("\n## ペルソナ\n\n");
        out.push_str(
            "この人が何を大事にし、誰に向けて、どう話を運ぶか。内容と構成と考え方は\
             ここに寄せる。\n",
        );
        for s in &p.sections {
            let _ = write!(out, "\n### {}\n", s.heading);
            if !s.body.is_empty() {
                let _ = write!(out, "\n{}\n", s.body);
            }
        }
    }
    let facts = facts_section(r.facts);
    if !facts.is_empty() {
        out.push_str("\n## 文体の事実\n\n");
        out.push_str(
            "この人の記事を数えて取り出した事実である。目安であって、全部を合わせに\
             いかなくてよい。合わせようとして内容を崩さない。表面の書きぶりは、書いた\
             あとに kuchiyose が検めて寄せる。\n\n",
        );
        out.push_str(&facts);
    }
    out.push_str("\n## 要約\n\n使う人が書いた、何を書きたいかである。\n\n");
    let fence = fence_for(r.brief);
    let _ = writeln!(out, "{fence}text\n{}\n{fence}", r.brief.trim_end());
    out.push_str("\n## 保存先\n\n");
    let _ = writeln!(
        out,
        "内容が固まったら、草稿の全文を Markdown で次の経路に書く。ほかの経路には書かない。\
         既にあるファイルを上書きしない。\n\n- {}\n\n書いたら、書いたことを使う人に伝えて\
         対話を終える。",
        r.save_path
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persona::{parse, tests::PERSONA};

    fn facts() -> StyleFacts {
        let kata = |text: &str, rate: f64, place: Option<&str>| Kata {
            text: text.into(),
            rate,
            place: place.map(str::to_owned),
            ceiling: 1.9,
        };
        StyleFacts {
            register: Some(RegisterFact {
                register: Register::Polite,
                declared: false,
            }),
            first_person: Some(FirstPerson {
                word: "僕".into(),
                rate: Some(0.57),
            }),
            katas: vec![
                kata("と思います。", 0.62, Some("結び")),
                kata("ですね。", 0.3, None),
                kata("どうも、", 0.8, Some("書き出し")),
                kata("かなと", 0.3, None),
                kata("ちなみに", 0.1, None),
            ],
            avoid: vec![
                Avoid {
                    text: "のではなく、".into(),
                    kind: AvoidKind::Kata,
                    rate: 0.56,
                    instead: Vec::new(),
                },
                Avoid {
                    text: "地味".into(),
                    kind: AvoidKind::Goi,
                    rate: 0.2,
                    instead: vec!["ささやか".into()],
                },
            ],
            length: Some(Length {
                median: 3120,
                low: 1200,
                high: 6000,
            }),
            paragraphs: vec![Spread {
                name: "段落あたりの文数".into(),
                unit: "無次元".into(),
                low: 1.8,
                high: 3.2,
            }],
        }
    }

    fn request<'a>(persona: Option<&'a Persona>, facts: &'a StyleFacts) -> DraftRequest<'a> {
        DraftRequest {
            persona,
            facts,
            brief: "Vim の Denops について",
            save_path: "/tmp/draft.md",
        }
    }

    #[test]
    fn 同じ材料からは同じバイト列が出る() {
        let p = parse(PERSONA).unwrap();
        let f = facts();
        assert_eq!(
            draft_prompt(&request(Some(&p), &f)),
            draft_prompt(&request(Some(&p), &f))
        );
    }

    #[test]
    fn 役目_ペルソナ_文体の事実_要約_保存先の順に並ぶ() {
        let p = parse(PERSONA).unwrap();
        let f = facts();
        let got = draft_prompt(&request(Some(&p), &f));
        let at = |s: &str| got.find(s).unwrap_or_else(|| panic!("{s} が無い:\n{got}"));
        let order = [
            at("対話して"),
            at("## ペルソナ"),
            at("### 大事にすること"),
            at("## 文体の事実"),
            at("## 要約"),
            at("## 保存先"),
            at("/tmp/draft.md"),
        ];
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{got}");
    }

    #[test]
    fn ペルソナが無ければその節を置かない() {
        let f = facts();
        let got = draft_prompt(&request(None, &f));
        assert!(!got.contains("## ペルソナ"), "{got}");
    }

    #[test]
    fn 型と避ける言い回しは割合の高い順に_4_本までで上限を必ず添える() {
        let f = facts();
        let got = draft_prompt(&request(None, &f));
        assert!(!got.contains("ちなみに"), "5 本目は渡さない:\n{got}");
        let first = got.find("どうも、").unwrap();
        let second = got.find("と思います。").unwrap();
        let tie_a = got.find("「かなと」").unwrap();
        let tie_b = got.find("「ですね。」").unwrap();
        assert!(first < second && second < tie_a && tie_a < tie_b, "{got}");
        assert!(got.contains("1,000 字あたり 1.9 回まで"), "{got}");
        assert!(got.contains("結びで使う"), "{got}");
        assert!(got.contains("「ささやか」"), "{got}");
    }

    #[test]
    fn 事実は目安であり表面は周回が寄せると言う() {
        let f = facts();
        let got = draft_prompt(&request(None, &f));
        assert!(got.contains("目安"), "{got}");
        assert!(got.contains("kuchiyose が検めて寄せる"), "{got}");
    }

    #[test]
    fn 事実が_1_つも無ければ文体の事実の節を置かない() {
        let f = StyleFacts::default();
        let got = draft_prompt(&request(None, &f));
        assert!(!got.contains("## 文体の事実"), "{got}");
    }

    #[test]
    fn 形が固定されている() {
        // 決めたとおりにできているかを、出力そのもので固定する。 変えるなら、
        // ここを書き換えることで気付く。
        let p = parse(PERSONA).unwrap();
        let f = facts();
        let got = draft_prompt(&request(Some(&p), &f));
        let want = "\
# 代筆の依頼

あなたは、この人の代わりに文章を書く。いきなり書かない。まず使う人と対話して、何を、誰に向けて、どういう順で書くかを詰める。内容が固まってから草稿を書く。

## ペルソナ

この人が何を大事にし、誰に向けて、どう話を運ぶか。内容と構成と考え方はここに寄せる。

### 大事にすること

- 手を動かして確かめたことだけを書く
  - 「実際に手元で動かしてみると」（2024-05-vim-filer）
  - 「試した限りでは、この設定で足りました」（2023-11-denops）

### 読み手

- Vim を毎日使う人
  - 「毎日 Vim を開いている人なら」（2024-05-vim-filer）

### 話の運び方

### 書かないこと

- 他人の道具をけなさない

### よく扱う領域

- Vim と Deno

## 文体の事実

この人の記事を数えて取り出した事実である。目安であって、全部を合わせにいかなくてよい。合わせようとして内容を崩さない。表面の書きぶりは、書いたあとに kuchiyose が検めて寄せる。

- 文体: 敬体（です・ます）で書く。数えた結果。
- 一人称: 「僕」。57% の記事で使っている。
- 長さ: 地の文の日本語は、中央値 3,120 字、短いもので 1,200 字、長いもので 6,000 字。
- 段落の組み方。この人の記事の幅は次のとおり。
  - 段落あたりの文数: 1.80〜3.20（無次元）
- この人がよく使う言い回し。使うなら、日本語 1,000 字あたりの上限を超えない。
  - 「どうも、」: 80% の記事で使っている。書き出しで使う。1,000 字あたり 1.9 回まで。
  - 「と思います。」: 62% の記事で使っている。結びで使う。1,000 字あたり 1.9 回まで。
  - 「かなと」: 30% の記事で使っている。1,000 字あたり 1.9 回まで。
  - 「ですね。」: 30% の記事で使っている。1,000 字あたり 1.9 回まで。
- 避ける言い回し。基準の文章（LLM が素で書いた文章）がよく使い、この人は使わない。
  - 言い回し「のではなく、」: 基準の 56% が使う。
  - 語「地味」: 基準の 20% が使う。この人が同じ品詞でよく使うのは「ささやか」。

## 要約

使う人が書いた、何を書きたいかである。

```text
Vim の Denops について
```

## 保存先

内容が固まったら、草稿の全文を Markdown で次の経路に書く。ほかの経路には書かない。既にあるファイルを上書きしない。

- /tmp/draft.md

書いたら、書いたことを使う人に伝えて対話を終える。
";
        assert_eq!(got, want);
    }
}
