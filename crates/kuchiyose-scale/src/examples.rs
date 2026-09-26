//! 本人がその次元をどう書いているかの実例。
//!
//! 「増やせ」と言うだけでは、どこに置くのかが分からない。 数値と向きだけを
//! 渡された側は、結局その人の文章を自分で読みに行くことになる——
//! 形代は[本文を持たない](../../../docs/spec/200-extract.md#本文を持たない)ので、
//! 読みに行く先が無い。
//!
//! だから文書を測るときに、文書ごとに拾っておく。 どの次元を訊かれるかは組み立てる
//! まで決まらないので、その文書が数えた次元の分を全部拾う。組み立てるときは、
//! 相手集合の文書の分を[並べる](merge)。
//!
//! 拾う次元は、その文書が数えたものに限る。 機能語の `ので` を、別の語の一部として
//! 現れた `ので` から拾わない。

use std::collections::{BTreeMap, BTreeSet};

use kuchiyose_doc::prose::Segment;
use kuchiyose_doc::Document;
use kuchiyose_metrics::matching::{self, CharType};
use kuchiyose_metrics::system::System;
use kuchiyose_metrics::word::Counts;

use crate::vocabulary::FrozenSet;

/// 実例を拾う系統。次元が語や記号として読めるものだけである。
///
/// 品詞 bigram の「名詞-助詞」を増やせとは言えない。
pub const EXAMPLE_SYSTEMS: [System; 3] = [System::FunctionWord, System::Comma, System::CharType];

/// 1 つの次元あたりに拾う実例の数。
pub(crate) const WANT: usize = 3;

/// 実例の前後に付ける字数。
pub(crate) const AROUND: usize = 6;

/// 1 文書の実例。系統の名前と次元の名前から、実例の並びへ。
pub type ExampleTable = BTreeMap<(String, String), Vec<String>>;

/// 1 文書の実例を拾う。
///
/// `masked` は識別子を伏せた地の文、`raw` はその文書の系統の数え上げである。
/// 読点の間隔は伏せた地の文から数える——系統がそう数えているからである。
#[must_use]
pub fn examples_in(
    doc: &Document,
    masked: &[Segment],
    raw: &BTreeMap<System, Vec<Counts>>,
) -> ExampleTable {
    let prose = doc.prose();
    let mut out = ExampleTable::new();
    for system in EXAMPLE_SYSTEMS {
        let name = system.name();
        for dim in dims_of(system, &prose, raw) {
            let found = if is_gap(name, &dim) {
                gaps_in(&dim, masked)
            } else {
                snippets_in(name, &dim, &prose)
            };
            if !found.is_empty() {
                out.insert((name.to_owned(), dim), found);
            }
        }
    }
    out
}

/// その文書で拾う次元。
fn dims_of(
    system: System,
    prose: &[Segment],
    raw: &BTreeMap<System, Vec<Counts>>,
) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = raw
        .get(&system)
        .into_iter()
        .flatten()
        .flat_map(|c| c.keys().cloned())
        .collect();
    match system {
        System::CharType => out.extend(CharType::ALL.iter().map(|t| t.name().to_owned())),
        // 読点の直前・直後は伏せた地の文で数えるので、伏せる前の字が次元に無いことが
        // ある。 実例は伏せる前の文から拾うので、そちらの字も拾う。
        System::Comma => {
            for seg in prose {
                let cs: Vec<char> = seg.text.chars().collect();
                for (i, c) in cs.iter().enumerate() {
                    if *c == '、' && i > 0 {
                        out.insert(cs[i - 1].to_string());
                    }
                }
            }
        }
        _ => {}
    }
    out
}

/// 読点の間隔の次元か。字面に現れないので、その間隔を作っている読点をまわりごと見せる。
fn is_gap(system: &str, dim: &str) -> bool {
    system == System::Comma.name() && (dim.ends_with('字') || dim.ends_with("字以上"))
}

/// 間隔の次元の実例。辞書順で先頭から拾う。
fn gaps_in(dim: &str, masked: &[Segment]) -> Vec<String> {
    let mut out: Vec<String> = matching::comma_gaps(masked)
        .into_iter()
        .filter(|(name, _)| name == dim)
        .map(|(_, ctx)| ctx)
        .collect();
    out.sort();
    out.dedup();
    out.truncate(WANT);
    out
}

/// 字面に現れる次元の実例。文書の頭から拾う。
fn snippets_in(system: &str, dim: &str, prose: &[Segment]) -> Vec<String> {
    // どこにでもある字は実例にならない。 ひらがなや漢字を 1 つ抜き出して
    // 見せても、どこをどう直せばよいかは何も伝わらない。
    if matches!((system, dim), ("文字種", "ひらがな" | "漢字" | "その他")) {
        return Vec::new();
    }
    // 次元が字面に現れるものと、字の種類を指すものを分ける。
    let hit: Box<dyn Fn(char) -> bool> = match (system, dim) {
        ("文字種", "空白") => Box::new(|c: char| c == ' ' || c == '\u{3000}'),
        ("文字種", "約物") => Box::new(|c: char| {
            matches!(
                c,
                '、' | '。' | '「' | '」' | '（' | '）' | '・' | '！' | '？'
            )
        }),
        ("文字種", "カタカナ") => {
            Box::new(|c: char| ('ァ'..='ヶ').contains(&c) || c == 'ー')
        }
        ("文字種", "半角数字") => Box::new(|c: char| c.is_ascii_digit()),
        ("文字種", "半角英字") => Box::new(|c: char| c.is_ascii_alphabetic()),
        _ => {
            // 字面に現れる次元。読点の直前・直後は 1 文字、機能語は語である。
            let needle: String = if system == System::Comma.name() {
                format!("{dim}、")
            } else {
                dim.to_owned()
            };
            return snippets(
                prose,
                &|t: &str| t.match_indices(&needle).map(|(i, _)| i).collect(),
                needle.chars().count(),
            );
        }
    };
    snippets(
        prose,
        &|t: &str| {
            t.char_indices()
                .filter(|(_, c)| hit(*c))
                .map(|(i, _)| i)
                .collect()
        },
        1,
    )
}

/// 見つけた位置のまわりを切り出す。文字の境で切る。
fn snippets(prose: &[Segment], find: &dyn Fn(&str) -> Vec<usize>, len: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for seg in prose {
        let cs: Vec<char> = seg.text.chars().collect();
        let byte_to_char: BTreeMap<usize, usize> = seg
            .text
            .char_indices()
            .enumerate()
            .map(|(k, (b, _))| (b, k))
            .collect();
        for b in find(&seg.text) {
            let Some(&k) = byte_to_char.get(&b) else {
                continue;
            };
            let lo = k.saturating_sub(AROUND);
            let hi = (k + len + AROUND).min(cs.len());
            let snip: String = cs[lo..hi].iter().collect();
            let snip = snip.trim().to_owned();
            if snip.chars().count() < 4 || out.contains(&snip) {
                continue;
            }
            out.push(snip);
            if out.len() >= WANT {
                return out;
            }
        }
    }
    out
}

/// 何本かの文書の実例を 1 つの次元にまとめる。並べた順に意味がある。
///
/// 字面に現れる次元は文書の順に先頭から拾い、間隔の次元は辞書順で先頭から拾う。
/// 文書ごとに拾った分をこうして並べると、全文書を通しで拾ったものと一致する。
#[must_use]
pub fn merge(system: &str, dim: &str, tables: &[&ExampleTable]) -> Vec<String> {
    let key = (system.to_owned(), dim.to_owned());
    let found = tables.iter().filter_map(|t| t.get(&key)).flatten();
    if is_gap(system, dim) {
        let mut out: Vec<String> = found.cloned().collect();
        out.sort();
        out.dedup();
        out.truncate(WANT);
        return out;
    }
    let mut out: Vec<String> = Vec::new();
    for s in found {
        if out.len() >= WANT {
            break;
        }
        if !out.contains(s) {
            out.push(s.clone());
        }
    }
    out
}

/// 固定した語彙の次元ごとに、相手集合の実例を拾う。
///
/// 作るのは組み立てのときだけである。 検めるときに本文へ読みに行く道が無い。
#[must_use]
pub fn examples_for(
    frozen: &[(String, FrozenSet)],
    partners: &[&ExampleTable],
) -> Vec<(String, String, Vec<String>)> {
    let mut out = Vec::new();
    for system in EXAMPLE_SYSTEMS {
        let name = system.name();
        let Some((_, set)) = frozen.iter().find(|(n, _)| n == name) else {
            continue;
        };
        for dim in set.parts().iter().flat_map(crate::vocabulary::Frozen::dims) {
            let found = merge(name, dim, partners);
            if found.is_empty() {
                continue;
            }
            out.push((name.to_owned(), dim.clone(), found));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuchiyose_doc::node::{Kind, Node};

    fn doc(t: &str) -> Document {
        Document::new(vec![Node::leaf(Kind::Paragraph, t)])
    }

    /// 1 文書から、次元 1 つの実例を拾う。
    fn found(system: System, dim: &str, d: &Document) -> Vec<String> {
        let masked = kuchiyose_doc::prose::mask_identifiers(&d.prose());
        let raw: BTreeMap<System, Vec<Counts>> = [(
            system,
            matching::raw_parts(system, &masked, None).unwrap_or_default(),
        )]
        .into_iter()
        .collect();
        let table = examples_in(d, &masked, &raw);
        merge(system.name(), dim, &[&table])
    }

    #[test]
    fn 空白の実例は打ち方が分かる形で出る() {
        // 「増やせ」と言うだけでは、どこに置くのかが分からない。
        let d = doc("今回は 2024/1/23 に行われた VimConf の話です。");
        let got = found(System::CharType, "空白", &d);
        assert!(!got.is_empty(), "{got:?}");
        assert!(got.iter().any(|x| x.contains(' ')), "{got:?}");
    }

    #[test]
    fn どこにでもある字は実例にしない() {
        // ひらがなを 1 つ抜き出して見せても、何も伝わらない。
        let d = doc("これはひらがなばかりの文である。");
        assert!(found(System::CharType, "ひらがな", &d).is_empty());
    }

    #[test]
    fn 読点の直前の字は読点ごと見せる() {
        let d = doc("そうなので、こうした。ならば、こうする。");
        let got = found(System::Comma, "で", &d);
        assert!(got.iter().any(|x| x.contains("で、")), "{got:?}");
    }

    #[test]
    fn 間隔の次元は読点をまわりごと見せる() {
        // 間隔は字として現れない。 その間隔を作っている読点を見せる。
        let d = doc("そうなので、こうした。");
        let got = found(System::Comma, "4字", &d);
        assert!(got.iter().any(|x| x.contains('、')), "{got:?}");
        assert!(found(System::Comma, "17字", &d).is_empty());
    }

    #[test]
    fn 文書ごとに拾って並べると通しで拾ったものと一致する() {
        // 相手集合の実例は、文書ごとの実例を並べて作る。 本文はもう無い。
        let a = doc("まずは、ここから。それから、あちらへ。");
        let b = doc("まずは、ここから。つぎは、そちらへ。最後に、戻る。");
        let joined = Document::new(a.nodes.iter().chain(&b.nodes).cloned().collect());
        let table = |d: &Document| {
            let masked = kuchiyose_doc::prose::mask_identifiers(&d.prose());
            let raw: BTreeMap<System, Vec<Counts>> = [(
                System::Comma,
                matching::raw_parts(System::Comma, &masked, None).unwrap(),
            )]
            .into_iter()
            .collect();
            examples_in(d, &masked, &raw)
        };
        let (ta, tb, tj) = (table(&a), table(&b), table(&joined));
        for dim in ["は", "ら", "ぎ", "に", "4字", "3字"] {
            assert_eq!(
                merge("読点の打ち方", dim, &[&ta, &tb]),
                merge("読点の打ち方", dim, &[&tj]),
                "{dim}"
            );
        }
    }
}
