//! 文体の事実を、形代と組み立てた目盛りから取り出す
//! （[文体の事実](../../../docs/spec/400-write.md#文体の事実)）。
//!
//! 取り出すのは組み立て層である。 `kuchiyose-prompt` は目盛りも判定も知らないので、
//! ここで平らな値にして渡す。形代の調整を当て、無効にした型・基準の型・基準の語は渡さない。

use std::collections::BTreeMap;

use kuchiyose_katashiro::{MuteKind, Register as Declared, Tuning};
use kuchiyose_prompt::{
    Avoid, AvoidKind, FirstPerson, Kata, Length, Register, RegisterFact, Spread, StyleFacts,
};
use kuchiyose_scale::assembly::Built;
use kuchiyose_scale::select::{self, RegisterCount};
use kuchiyose_scale::stats::KatashiroStats;

use crate::remedies::FromDefinitions;

/// 取り出す。`built` が無ければ（目盛りが組み立てられない）、基準が要る事実を置かない。
#[must_use]
pub fn style_facts(
    stats: &KatashiroStats,
    tuning: &Tuning,
    built: Option<&Built>,
    defs: &FromDefinitions,
) -> StyleFacts {
    StyleFacts {
        register: register(stats, tuning),
        first_person: first_person(stats, tuning),
        katas: built.map(|b| katas(b, tuning)).unwrap_or_default(),
        avoid: built.map(|b| avoid(b, tuning)).unwrap_or_default(),
        length: length(stats),
        paragraphs: paragraphs(stats, defs),
    }
}

/// 文体。申告があれば申告、無ければ分けられた文書の多い方。同数なら置かない。
fn register(stats: &KatashiroStats, tuning: &Tuning) -> Option<RegisterFact> {
    if let Some(r) = tuning.register {
        return Some(RegisterFact {
            register: match r {
                Declared::Polite => Register::Polite,
                Declared::Plain => Register::Plain,
            },
            declared: true,
        });
    }
    let counted = RegisterCount::of(stats.documents.iter().map(|d| d.polite_share)).majority()?;
    Some(RegisterFact {
        register: match counted {
            select::Register::Polite => Register::Polite,
            select::Register::Plain => Register::Plain,
        },
        declared: false,
    })
}

/// 一人称。申告があれば申告、無ければいちばん多くの記事で使うもの。
///
/// 回数ではなく記事の割合で見る。 1 本で何度も書く人と、毎回 1 度だけ書く人の
/// どちらも「その一人称を使う人」である。
fn first_person(stats: &KatashiroStats, tuning: &Tuning) -> Option<FirstPerson> {
    if let Some(word) = &tuning.first_person {
        return Some(FirstPerson {
            word: word.clone(),
            rate: None,
        });
    }
    let mut df: BTreeMap<&str, usize> = BTreeMap::new();
    for d in &stats.documents {
        for w in d.first_person.keys() {
            *df.entry(w.as_str()).or_default() += 1;
        }
    }
    // 多い順、並んだら文字列の順。
    let (word, n) = df
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0)))?;
    #[allow(clippy::cast_precision_loss)]
    let rate = n as f64 / stats.documents.len().max(1) as f64;
    Some(FirstPerson {
        word: word.to_owned(),
        rate: Some(rate),
    })
}

fn katas(b: &Built, tuning: &Tuning) -> Vec<Kata> {
    b.scale
        .katas
        .iter()
        .filter(|k| !tuning.is_muted(MuteKind::Kata, &k.shown()))
        .map(|k| Kata {
            text: k.shown(),
            rate: k.rate,
            place: kuchiyose_review::kata_place(k.at).map(str::to_owned),
            ceiling: k.ceiling,
        })
        .collect()
}

fn avoid(b: &Built, tuning: &Tuning) -> Vec<Avoid> {
    let katas = b
        .scale
        .machine_katas
        .iter()
        .filter(|k| !tuning.is_muted(MuteKind::BaselineKata, &k.shown()))
        .map(|k| Avoid {
            text: k.shown(),
            kind: AvoidKind::Kata,
            rate: k.rate,
            instead: Vec::new(),
        });
    let gois = b
        .scale
        .machine_gois
        .iter()
        .filter(|g| !tuning.is_muted(MuteKind::BaselineGoi, &g.text))
        .map(|g| Avoid {
            text: g.text.clone(),
            kind: AvoidKind::Goi,
            rate: g.rate,
            instead: g.theirs.clone(),
        });
    katas.chain(gois).collect()
}

/// 地の文の日本語の字数の中央値と範囲。偶数本なら下側の中央を採る。
fn length(stats: &KatashiroStats) -> Option<Length> {
    let mut chars: Vec<usize> = stats.documents.iter().map(|d| d.chars).collect();
    chars.sort_unstable();
    let median = *chars.get((chars.len().checked_sub(1)?) / 2)?;
    Some(Length {
        median,
        low: *chars.first()?,
        high: *chars.last()?,
    })
}

/// 段落の組み方に関わる指示できる指標の、本人の文書ごとの値の最小と最大。
fn paragraphs(stats: &KatashiroStats, defs: &FromDefinitions) -> Vec<Spread> {
    let mut seen: BTreeMap<String, (String, f64, f64)> = BTreeMap::new();
    for d in &stats.documents {
        for (name, counted) in &d.directives {
            let (Some(unit), Some(v)) = (defs.length_unit(name), counted.measured().value()) else {
                continue;
            };
            let e = seen
                .entry(name.clone())
                .or_insert_with(|| (unit.to_owned(), v, v));
            e.1 = e.1.min(v);
            e.2 = e.2.max(v);
        }
    }
    seen.into_iter()
        .map(|(name, (unit, low, high))| Spread {
            name,
            unit,
            low,
            high,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;

    fn stats() -> KatashiroStats {
        let docs = fixture::rich_corpus(4);
        KatashiroStats::measure(&fixture::samples(&docs), Some(&fixture::Chars)).expect("測れる")
    }

    #[test]
    fn 申告があれば数えた結果より申告を採る() {
        let s = stats();
        let t = Tuning {
            register: Some(Declared::Plain),
            first_person: Some("私".into()),
            ..Tuning::default()
        };
        let f = style_facts(&s, &t, None, &FromDefinitions::load());
        assert_eq!(
            f.register,
            Some(RegisterFact {
                register: Register::Plain,
                declared: true
            })
        );
        assert_eq!(
            f.first_person,
            Some(FirstPerson {
                word: "私".into(),
                rate: None
            })
        );
    }

    #[test]
    fn 目盛りが無ければ基準が要る事実を置かない() {
        let f = style_facts(&stats(), &Tuning::default(), None, &FromDefinitions::load());
        assert!(f.katas.is_empty());
        assert!(f.avoid.is_empty());
        assert!(f.length.is_some(), "本人の統計値だけで出るものは出す");
    }

    #[test]
    fn 長さは字数の中央値と範囲である() {
        let s = stats();
        let l = length(&s).expect("出る");
        let mut chars: Vec<usize> = s.documents.iter().map(|d| d.chars).collect();
        chars.sort_unstable();
        assert_eq!(l.low, chars[0]);
        assert_eq!(l.high, chars[3]);
        assert_eq!(l.median, chars[1]);
    }

    #[test]
    fn 段落の組み方は分類が長さの指標だけを札から選ぶ() {
        let defs = FromDefinitions::load();
        let f = style_facts(&stats(), &Tuning::default(), None, &defs);
        assert!(!f.paragraphs.is_empty(), "{:?}", f.paragraphs);
        for s in &f.paragraphs {
            assert!(defs.length_unit(&s.name).is_some(), "{}", s.name);
            assert!(s.low <= s.high);
        }
    }

    #[test]
    fn 一人称は回数ではなく記事の割合がいちばん高いものを採る() {
        let mut s = stats();
        for (i, d) in s.documents.iter_mut().enumerate() {
            d.first_person = match i {
                0 => [("私".to_owned(), 30)].into(),
                1 => BTreeMap::new(),
                _ => [("僕".to_owned(), 1)].into(),
            };
        }
        let f = style_facts(&s, &Tuning::default(), None, &FromDefinitions::load());
        assert_eq!(
            f.first_person,
            Some(FirstPerson {
                word: "僕".into(),
                rate: Some(0.5)
            })
        );
    }
}
