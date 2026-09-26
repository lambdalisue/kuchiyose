//! 通らないものは断る。
//!
//! 正規形に落ちない入力がある。落ちないまま無理に通さない。黙って一部を落として
//! 通せば、落ちた分だけ値が狂った文書が、正常な文書と同じ顔でコーパスに入る。

use kuchiyose_doc::text;
use kuchiyose_doc::Document;

/// 日本語が主だと認める下限。
///
/// 暫定値である。 地の文の作りを変えたら導き直しになる——分母が変わるので、
/// 同じ 3 割が同じものを断らない。
pub const JAPANESE_FLOOR: f64 = 0.3;

/// 断る理由。
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// 記法が壊れていて解釈できない。
    Broken {
        /// どこで壊れているか。
        detail: String,
    },
    /// 対応表に無い記法が使われている。対応表を足してから通す。
    UnknownMarkup {
        /// 見つけた記法。
        markup: String,
    },
    /// 日本語以外が主。
    NotJapanese {
        /// 地の文に残った文字のうち、日本語の文字の割合。
        ratio: f64,
    },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::Broken { detail } => write!(f, "記法が壊れている: {detail}"),
            Refusal::UnknownMarkup { markup } => {
                write!(f, "対応表に無い記法: {markup}。対応表を足してから通す")
            }
            Refusal::NotJapanese { ratio } => {
                write!(f, "日本語以外が主: 日本語の文字が {:.0}%", ratio * 100.0)
            }
        }
    }
}

impl std::error::Error for Refusal {}

/// 日本語が主かを見る。
///
/// 言語判定の道具に頼らない。 判定は決定的でなければならない。
///
/// 分母から約物と空白を外す。コードの多い技術記事は英数字と記号が増えるが、それは
/// 題材であって言語ではない。分母は「地の文に残ったもの」である——落としたセルは
/// ここにも入らない。
#[must_use]
pub fn japanese_ratio(doc: &Document) -> f64 {
    let mut denominator = 0usize;
    let mut japanese = 0usize;
    for seg in doc.prose() {
        for c in seg.text.chars() {
            if text::is_japanese(c) {
                japanese += 1;
                denominator += 1;
            } else if !is_excluded_from_denominator(c) {
                denominator += 1;
            }
        }
    }
    if denominator == 0 {
        return 0.0;
    }
    japanese as f64 / denominator as f64
}

/// 分母から外す文字。約物と空白。
fn is_excluded_from_denominator(c: char) -> bool {
    c.is_whitespace() || is_punctuation(c)
}

/// 約物。
fn is_punctuation(c: char) -> bool {
    matches!(c,
        '\u{3000}'..='\u{303F}'   // CJK の約物（。、「」『』〜 など）
        | '\u{FF01}'..='\u{FF0F}' // ！＂＃…／
        | '\u{FF1A}'..='\u{FF20}' // ：；＜＝＞？＠
        | '\u{FF3B}'..='\u{FF40}'
        | '\u{FF5B}'..='\u{FF65}'
        | '\u{2010}'..='\u{2027}'
        | '\u{2030}'..='\u{205E}'
        | '!'..='/' | ':'..='@' | '['..='`' | '{'..='~'
    )
}

/// 断るかを決める。通るなら `Ok`。
pub fn admit(doc: &Document) -> Result<(), Refusal> {
    let ratio = japanese_ratio(doc);
    if ratio < JAPANESE_FLOOR {
        return Err(Refusal::NotJapanese { ratio });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuchiyose_doc::node::{Kind, Node};

    fn doc(texts: &[&str]) -> Document {
        Document::new(
            texts
                .iter()
                .map(|t| Node::leaf(Kind::Paragraph, *t))
                .collect(),
        )
    }

    #[test]
    fn 約物と空白は分母に入らない() {
        // 「これは大事」の 5 字 / 5 字 = 100%。読点も句点も空白も数えない。
        let d = doc(&["これは、 大事。"]);
        assert!(
            (japanese_ratio(&d) - 1.0).abs() < 1e-9,
            "{}",
            japanese_ratio(&d)
        );
    }

    #[test]
    fn 英数字は分母に入る() {
        // 「あ」1 字 / 「あ」+ "abc" = 1/4。
        let d = doc(&["あabc"]);
        assert!(
            (japanese_ratio(&d) - 0.25).abs() < 1e-9,
            "{}",
            japanese_ratio(&d)
        );
    }

    #[test]
    fn 日本語以外が主なら断る() {
        let d = doc(&["あ this is mostly english text here"]);
        assert!(matches!(admit(&d), Err(Refusal::NotJapanese { .. })));
    }

    #[test]
    fn 日本語が主なら通る() {
        let d = doc(&["これは日本語の文章である。Rust という語が混ざる。"]);
        assert!(admit(&d).is_ok(), "{}", japanese_ratio(&d));
    }

    #[test]
    fn 落としたセルは分母に入らない() {
        // 記号だけのセルは地の文に入らないので、分母にも入らない。
        // 残った散文が日本語なら通る——断る基準は「表が多い」ではない。
        let table = Node::branch(
            Kind::Table,
            vec![
                Node::leaf(Kind::Cell, "機能"),
                Node::leaf(Kind::Cell, "o"),
                Node::leaf(Kind::Cell, "-"),
                Node::leaf(Kind::Cell, "-"),
                Node::leaf(Kind::Cell, "-"),
                Node::leaf(Kind::Cell, "-"),
            ],
        );
        let d = Document::new(vec![table]);
        assert!(
            (japanese_ratio(&d) - 1.0).abs() < 1e-9,
            "{}",
            japanese_ratio(&d)
        );
        assert!(admit(&d).is_ok());
    }

    #[test]
    fn 地の文が空なら通さない() {
        let d = Document::new(vec![Node::leaf(Kind::CodeBlock, "let x = 1;")]);
        assert!((japanese_ratio(&d) - 0.0).abs() < 1e-9);
        assert!(matches!(admit(&d), Err(Refusal::NotJapanese { .. })));
    }

    #[test]
    fn 断る理由は読める形で出る() {
        let r = Refusal::UnknownMarkup {
            markup: "[!HINT]".into(),
        };
        assert!(r.to_string().contains("対応表を足してから"));
    }
}
