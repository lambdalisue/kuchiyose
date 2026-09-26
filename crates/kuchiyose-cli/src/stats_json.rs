//! 統計値を JSON にし、読み戻す（[documents.jsonl](../../../docs/design/100-katashiro.md#documentsjsonl)）。
//!
//! 形代は統計値の形を知らない（[理由](../../../docs/design/000-architecture.md#形代は中身の形を知らない)）。
//! 型は `kuchiyose-scale` にあり、繋ぐのは組み立て層のここである。
//!
//! 読み戻したものは書く前と一致しなければならない。 一致しなければ、保存した
//! 形代から組み立てた目盛りは、測ったときの値から組み立てた目盛りと違う。
//!
//! 測れなかったことは値ではなく `excluded` で書く。 0 を書けば「使わなかった」と
//! 区別が付かなくなる。

use std::collections::{BTreeMap, BTreeSet};

use kuchiyose_katashiro::json::{self, Value};
use kuchiyose_katashiro::Stats;
use kuchiyose_metrics::directive::{Counted, Parts};
use kuchiyose_metrics::humanness::{HumannessWindows, Metric, Windows};
use kuchiyose_metrics::lexicon::Lexicon;
use kuchiyose_metrics::system::System;
use kuchiyose_metrics::word::Counts;
use kuchiyose_metrics::{Measured, Unmeasured};
use kuchiyose_scale::examples::ExampleTable;
use kuchiyose_scale::stats::{check_name, DocumentStats, KatashiroStats, Phrase, StatsError};

/// 統計値を形代に入れる形にする。行の並びは単位名の昇順である。
#[must_use]
pub fn write(stats: &KatashiroStats) -> Stats {
    let mut documents = String::new();
    for d in &stats.documents {
        documents.push_str(&document(d).write());
        documents.push('\n');
    }
    let lexicon = Value::obj([(
        "pairs".to_owned(),
        Value::Array(
            stats
                .lexicon
                .pairs()
                .into_iter()
                .map(|(a, b)| Value::Array(vec![Value::s(a), Value::s(b)]))
                .collect(),
        ),
    )])
    .write();
    Stats { documents, lexicon }
}

/// 形代の統計値を読み戻す。
///
/// # Errors
///
/// 形が合わなければ、どこが合わないかを返す。 欠けを既定で埋めない——埋めれば、
/// 測れなかったことが 0 として読み戻される。
///
/// 単位の名前が重なるか、名前に制御文字があれば断る。 形代は配り直すもので、
/// 測って書いたものとは限らない。 組み立ては名前で値を引くので、重なれば片方の
/// 値が黙って置き換わる。 制御文字は、測るときと同じ[確かめ](check_name)で断る。
pub fn read(stats: &Stats) -> Result<KatashiroStats, String> {
    let documents = stats
        .documents
        .lines()
        .enumerate()
        .map(|(i, line)| {
            let v = json::parse(line).map_err(|e| format!("{} 行目: {e}", i + 1))?;
            read_document(&v).map_err(|e| format!("{} 行目: {e}", i + 1))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (i, d) in documents.iter().enumerate() {
        check_name(&d.name).map_err(|e| format!("{} 行目: {e}", i + 1))?;
        if !seen.insert(&d.name) {
            let e = StatsError::DuplicateName(d.name.clone());
            return Err(format!("{} 行目: {e}", i + 1));
        }
    }
    let lexicon = json::parse(&stats.lexicon).map_err(|e| format!("語のまとめ方: {e}"))?;
    let pairs = lexicon
        .get("pairs")
        .and_then(Value::as_array)
        .ok_or("語のまとめ方に pairs が無い")?
        .iter()
        .map(|p| match p.as_array() {
            Some([Value::String(a), Value::String(b)]) => Ok((a.clone(), b.clone())),
            _ => Err("語のまとめ方の組が 2 つの文字列でない".to_owned()),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(KatashiroStats {
        documents,
        lexicon: Lexicon::from_pairs(pairs),
    })
}

#[allow(clippy::cast_precision_loss)]
fn n(v: usize) -> Value {
    Value::Number(v as f64)
}

fn strings<'a>(v: impl IntoIterator<Item = &'a String>) -> Value {
    Value::Array(v.into_iter().map(Value::s).collect())
}

fn counts(m: &BTreeMap<String, usize>) -> Value {
    Value::obj(m.iter().map(|(k, v)| (k.clone(), n(*v))))
}

fn document(d: &DocumentStats) -> Value {
    Value::obj([
        ("unit".to_owned(), Value::s(&d.name)),
        ("chars".to_owned(), n(d.chars)),
        ("chars_masked".to_owned(), n(d.masked_chars)),
        ("commas".to_owned(), n(d.commas)),
        ("tokens".to_owned(), d.tokens.map_or(Value::Null, n)),
        ("nodes".to_owned(), n(d.nodes)),
        (
            "polite_share".to_owned(),
            d.polite_share.map_or(Value::Null, Value::Number),
        ),
        ("content_words".to_owned(), strings(&d.content_words)),
        (
            "systems".to_owned(),
            Value::obj(d.systems.iter().map(|(s, parts)| {
                (
                    s.name().to_owned(),
                    Value::Array(parts.iter().map(counts).collect()),
                )
            })),
        ),
        ("windows".to_owned(), windows(&d.windows)),
        (
            "directives".to_owned(),
            Value::Array(
                d.directives
                    .iter()
                    .map(|(name, c)| directive(name, c))
                    .collect(),
            ),
        ),
        (
            "phrases".to_owned(),
            Value::Array(
                d.phrases
                    .iter()
                    .map(|(text, p)| {
                        Value::obj([
                            ("text".to_owned(), Value::s(text)),
                            ("first".to_owned(), Value::Number(p.first)),
                            ("count".to_owned(), n(p.count)),
                            (
                                "nodes".to_owned(),
                                Value::Array(p.nodes.iter().map(|x| n(*x)).collect()),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("recurring".to_owned(), strings(&d.recurring)),
        ("phrase_hits".to_owned(), counts(&d.phrase_hits)),
        (
            "lexemes".to_owned(),
            Value::obj(d.lexemes.iter().map(|(lemma, (pos, count))| {
                (
                    lemma.clone(),
                    Value::obj([
                        ("pos".to_owned(), Value::s(pos)),
                        ("count".to_owned(), n(*count)),
                    ]),
                )
            })),
        ),
        ("first_person".to_owned(), counts(&d.first_person)),
        (
            "opening".to_owned(),
            d.opening.as_deref().map_or(Value::Null, Value::s),
        ),
        (
            "examples".to_owned(),
            Value::Array(
                d.examples
                    .iter()
                    .map(|((system, dim), texts)| {
                        Value::obj([
                            ("system".to_owned(), Value::s(system)),
                            ("dim".to_owned(), Value::s(dim)),
                            ("texts".to_owned(), strings(texts)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

fn windows(w: &HumannessWindows) -> Value {
    Value::Array(
        w.values
            .iter()
            .map(|(metric, dims)| {
                Value::obj([
                    ("metric".to_owned(), Value::s(metric.name())),
                    (
                        "dims".to_owned(),
                        Value::Array(
                            dims.iter()
                                .map(|(dim, got)| {
                                    let mut pairs = vec![("dim".to_owned(), Value::s(dim))];
                                    match got {
                                        Ok(w) => {
                                            pairs.push(("taken".to_owned(), n(w.taken)));
                                            pairs.push((
                                                "values".to_owned(),
                                                Value::Array(
                                                    w.values
                                                        .iter()
                                                        .map(|v| Value::Number(*v))
                                                        .collect(),
                                                ),
                                            ));
                                        }
                                        Err(why) => {
                                            pairs.push((
                                                "excluded".to_owned(),
                                                Value::s(why.name()),
                                            ));
                                        }
                                    }
                                    Value::obj(pairs)
                                })
                                .collect(),
                        ),
                    ),
                ])
            })
            .collect(),
    )
}

/// 指示できる指標 1 本。値と、その元になった数。
///
/// 値と測れなかった理由も添える。 読む人のためであり、読み戻すときは数から
/// 求め直した値と照らす——食い違えば、どちらかが書き換えられている。
fn directive(name: &str, c: &Counted) -> Value {
    let mut pairs = vec![("name".to_owned(), Value::s(name))];
    if let Some(p) = c.parts() {
        pairs.push(("parts".to_owned(), parts(p)));
    }
    pairs.extend(measured(c.measured()));
    Value::obj(pairs)
}

fn measured(m: Measured) -> [(String, Value); 2] {
    [
        (
            "value".to_owned(),
            m.value().map_or(Value::Null, Value::Number),
        ),
        (
            "excluded".to_owned(),
            m.unmeasured().map_or(Value::Null, |u| Value::s(u.name())),
        ),
    ]
}

fn parts(p: &Parts) -> Value {
    match p {
        Parts::Ratio {
            num,
            den,
            per,
            floor,
            zero_is_absent,
        } => Value::obj([(
            "ratio".to_owned(),
            Value::obj([
                ("num".to_owned(), n(*num)),
                ("den".to_owned(), n(*den)),
                ("per".to_owned(), n(*per)),
                ("floor".to_owned(), n(*floor)),
                ("zero_is_absent".to_owned(), Value::Bool(*zero_is_absent)),
            ]),
        )]),
        Parts::Spread { values, floor } => Value::obj([(
            "spread".to_owned(),
            Value::obj([
                (
                    "values".to_owned(),
                    Value::Array(values.iter().map(|v| n(*v)).collect()),
                ),
                ("floor".to_owned(), n(*floor)),
            ]),
        )]),
        Parts::Count(c) => Value::obj([("count".to_owned(), n(*c))]),
    }
}

fn field<'a>(v: &'a Value, key: &str) -> Result<&'a Value, String> {
    v.get(key).ok_or_else(|| format!("{key} が無い"))
}

fn count(v: &Value, what: &str) -> Result<usize, String> {
    let x = v.as_f64().ok_or_else(|| format!("{what} が数でない"))?;
    #[allow(clippy::cast_precision_loss)]
    let ok = x.fract() == 0.0 && x >= 0.0 && x <= usize::MAX as f64;
    if !ok {
        return Err(format!("{what} が 0 以上の整数でない"));
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(x as usize)
}

fn number(v: &Value, what: &str) -> Result<f64, String> {
    v.as_f64().ok_or_else(|| format!("{what} が数でない"))
}

fn text(v: &Value, what: &str) -> Result<String, String> {
    v.as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{what} が文字列でない"))
}

fn array<'a>(v: &'a Value, what: &str) -> Result<&'a [Value], String> {
    v.as_array().ok_or_else(|| format!("{what} が配列でない"))
}

fn object<'a>(v: &'a Value, what: &str) -> Result<&'a BTreeMap<String, Value>, String> {
    match v {
        Value::Object(m) => Ok(m),
        _ => Err(format!("{what} が対象でない")),
    }
}

fn texts<C: FromIterator<String>>(v: &Value, what: &str) -> Result<C, String> {
    array(v, what)?.iter().map(|x| text(x, what)).collect()
}

fn read_counts(v: &Value, what: &str) -> Result<Counts, String> {
    object(v, what)?
        .iter()
        .map(|(k, x)| Ok((k.clone(), count(x, what)?)))
        .collect()
}

fn optional<T>(
    v: &Value,
    read: impl FnOnce(&Value) -> Result<T, String>,
) -> Result<Option<T>, String> {
    match v {
        Value::Null => Ok(None),
        other => read(other).map(Some),
    }
}

fn reason(v: &Value) -> Result<Unmeasured, String> {
    let name = text(v, "excluded")?;
    Unmeasured::from_name(&name).ok_or_else(|| format!("知らない理由: {name}"))
}

fn read_document(v: &Value) -> Result<DocumentStats, String> {
    let name = text(field(v, "unit")?, "unit")?;
    let systems = object(field(v, "systems")?, "systems")?
        .iter()
        .map(|(k, parts)| {
            let s = System::from_name(k).ok_or_else(|| format!("知らない系統: {k}"))?;
            let parts = array(parts, k)?
                .iter()
                .map(|p| read_counts(p, k))
                .collect::<Result<Vec<_>, _>>()?;
            Ok((s, parts))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let phrases = array(field(v, "phrases")?, "phrases")?
        .iter()
        .map(|p| {
            Ok((
                text(field(p, "text")?, "phrases.text")?,
                Phrase {
                    first: number(field(p, "first")?, "phrases.first")?,
                    count: count(field(p, "count")?, "phrases.count")?,
                    nodes: array(field(p, "nodes")?, "phrases.nodes")?
                        .iter()
                        .map(|x| count(x, "phrases.nodes"))
                        .collect::<Result<BTreeSet<_>, _>>()?,
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let lexemes = object(field(v, "lexemes")?, "lexemes")?
        .iter()
        .map(|(lemma, x)| {
            Ok((
                lemma.clone(),
                (
                    text(field(x, "pos")?, "lexemes.pos")?,
                    count(field(x, "count")?, "lexemes.count")?,
                ),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let examples: ExampleTable = array(field(v, "examples")?, "examples")?
        .iter()
        .map(|e| {
            Ok((
                (
                    text(field(e, "system")?, "examples.system")?,
                    text(field(e, "dim")?, "examples.dim")?,
                ),
                texts(field(e, "texts")?, "examples.texts")?,
            ))
        })
        .collect::<Result<_, String>>()?;
    Ok(DocumentStats {
        chars: count(field(v, "chars")?, "chars")?,
        masked_chars: count(field(v, "chars_masked")?, "chars_masked")?,
        commas: count(field(v, "commas")?, "commas")?,
        tokens: optional(field(v, "tokens")?, |x| count(x, "tokens"))?,
        nodes: count(field(v, "nodes")?, "nodes")?,
        polite_share: optional(field(v, "polite_share")?, |x| number(x, "polite_share"))?,
        content_words: texts(field(v, "content_words")?, "content_words")?,
        systems,
        windows: read_windows(field(v, "windows")?)?,
        directives: array(field(v, "directives")?, "directives")?
            .iter()
            .map(read_directive)
            .collect::<Result<_, _>>()?,
        phrases,
        recurring: texts(field(v, "recurring")?, "recurring")?,
        phrase_hits: read_counts(field(v, "phrase_hits")?, "phrase_hits")?,
        lexemes,
        first_person: read_counts(field(v, "first_person")?, "first_person")?,
        opening: optional(field(v, "opening")?, |x| text(x, "opening"))?,
        examples,
        name,
    })
}

fn read_windows(v: &Value) -> Result<HumannessWindows, String> {
    let values = array(v, "windows")?
        .iter()
        .map(|m| {
            let name = text(field(m, "metric")?, "windows.metric")?;
            let metric = Metric::from_name(&name)
                .ok_or_else(|| format!("知らない人らしさの指標: {name}"))?;
            let dims = array(field(m, "dims")?, "windows.dims")?
                .iter()
                .map(|d| {
                    let dim = text(field(d, "dim")?, "windows.dim")?;
                    let got = match d.get("excluded") {
                        Some(why) => Err(reason(why)?),
                        None => Ok(Windows {
                            taken: count(field(d, "taken")?, "windows.taken")?,
                            values: array(field(d, "values")?, "windows.values")?
                                .iter()
                                .map(|x| number(x, "windows.values"))
                                .collect::<Result<_, _>>()?,
                        }),
                    };
                    Ok((dim, got))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok((metric, dims))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(HumannessWindows { values })
}

fn read_directive(v: &Value) -> Result<(String, Counted), String> {
    let name = text(field(v, "name")?, "directives.name")?;
    let counted = match v.get("parts") {
        Some(p) => Counted::of(read_parts(p).map_err(|e| format!("{name}: {e}"))?),
        None => Counted::unmeasured(reason(field(v, "excluded")?)?),
    };
    // 添えた値と、数から求め直した値を照らす。
    let [(_, value), (_, excluded)] = measured(counted.measured());
    if field(v, "value")? != &value || field(v, "excluded")? != &excluded {
        return Err(format!("{name}: 値が元になった数と合わない"));
    }
    Ok((name, counted))
}

fn read_parts(v: &Value) -> Result<Parts, String> {
    if let Some(r) = v.get("ratio") {
        return Ok(Parts::Ratio {
            num: count(field(r, "num")?, "num")?,
            den: count(field(r, "den")?, "den")?,
            per: count(field(r, "per")?, "per")?,
            floor: count(field(r, "floor")?, "floor")?,
            zero_is_absent: field(r, "zero_is_absent")?
                .as_bool()
                .ok_or("zero_is_absent が真偽でない")?,
        });
    }
    if let Some(s) = v.get("spread") {
        return Ok(Parts::Spread {
            values: array(field(s, "values")?, "values")?
                .iter()
                .map(|x| count(x, "values"))
                .collect::<Result<_, _>>()?,
            floor: count(field(s, "floor")?, "floor")?,
        });
    }
    if let Some(c) = v.get("count") {
        return Ok(Parts::Count(count(c, "count")?));
    }
    Err("数の形が ratio / spread / count のどれでもない".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{self, Chars};
    use kuchiyose_scale::Sample;

    fn measured_stats() -> KatashiroStats {
        let docs = fixture::rich_corpus(3);
        let samples: Vec<Sample<'_>> = fixture::samples(&docs);
        KatashiroStats::measure(&samples, Some(&Chars)).expect("測れる")
    }

    #[test]
    fn 読み戻した統計値から組み立てた目盛りは書く前のものと一致する() {
        // 一致しなければ、保存した形代で検めた値が測ったときと違う。
        let side = |machine: bool| -> KatashiroStats {
            let docs: fixture::Named = (0..10)
                .map(|i| (format!("u{i:02}"), fixture::document(i, machine)))
                .collect();
            KatashiroStats::measure(&fixture::samples(&docs), Some(&Chars)).expect("測れる")
        };
        let (person, baseline) = (side(false), side(true));
        let by = |n: &str| n.contains("見出し");
        let tuning = kuchiyose_scale::assembly::Tuning::default();
        let before = kuchiyose_scale::assembly::assemble_stats(&person, &baseline, tuning, &by);
        assert!(before.outcome.is_ok(), "目盛りができる素材である");
        let back = |s: &KatashiroStats| read(&write(s)).expect("読み戻せる");
        let after = kuchiyose_scale::assembly::assemble_stats(
            &back(&person),
            &back(&baseline),
            tuning,
            &by,
        );
        assert_eq!(after, before);
    }

    #[test]
    fn 書いて読み戻すと同じ統計値が出る() {
        let stats = measured_stats();
        assert!(!stats.lexicon.is_empty(), "語のまとめ方も往復させる");
        assert!(
            stats.documents.iter().any(|d| !d.examples.is_empty()),
            "実例も往復させる"
        );
        assert_eq!(read(&write(&stats)), Ok(stats));
    }

    #[test]
    fn 測れなかったことは理由のまま往復する() {
        // 解析器が無ければ、要る指標は「道具が無い」になる。 0 として読み戻さない。
        let docs = fixture::rich_corpus(1);
        let stats = KatashiroStats::measure(&fixture::samples(&docs), None).expect("測れる");
        let text = write(&stats);
        assert!(
            text.documents.contains("\"excluded\":\"道具が無い\""),
            "{}",
            text.documents
        );
        assert_eq!(read(&text), Ok(stats));
    }

    #[test]
    fn 書き出しは決定的である() {
        assert_eq!(write(&measured_stats()), write(&measured_stats()));
    }

    #[test]
    fn 行は単位名の昇順に並ぶ() {
        let text = write(&measured_stats());
        let units: Vec<String> = text
            .documents
            .lines()
            .map(|l| {
                json::parse(l)
                    .unwrap()
                    .get("unit")
                    .and_then(Value::as_str)
                    .unwrap()
                    .to_owned()
            })
            .collect();
        let mut sorted = units.clone();
        sorted.sort();
        assert_eq!(units, sorted);
    }

    #[test]
    fn 欄が欠けていたら埋めずに断る() {
        let text = write(&measured_stats());
        let broken = Stats {
            documents: text.documents.replacen("\"tokens\"", "\"消した\"", 1),
            lexicon: text.lexicon,
        };
        let e = read(&broken).unwrap_err();
        assert!(e.contains("tokens"), "{e}");
    }

    #[test]
    fn 単位の名前が重なっていれば断る() {
        // 組み立ては名前で値を引く。 重なれば片方の値が黙って置き換わる。
        let mut stats = measured_stats();
        stats.documents[1].name = stats.documents[0].name.clone();
        let e = read(&write(&stats)).unwrap_err();
        assert!(e.contains("重なっている"), "{e}");
    }

    #[test]
    fn 単位の名前に制御文字があれば断る() {
        // 組み立ては基準の側の名前に NUL を前置して役を分ける。 本人の側の名前が
        // NUL で始まれば、基準の単位と同じ名前になる。
        for bad in ["\u{0}u00", "改\n行", "タブ\t"] {
            let mut stats = measured_stats();
            stats.documents[0].name = bad.to_owned();
            let e = read(&write(&stats)).unwrap_err();
            assert!(e.contains("制御文字"), "{bad:?}: {e}");
        }
    }

    #[test]
    fn 添えた値が元の数と食い違えば断る() {
        let stats = measured_stats();
        let (name, c) = stats.documents[0]
            .directives
            .iter()
            .find(|(_, c)| c.value().is_some())
            .expect("値の出る指標がある");
        let mut line = directive(name, c);
        if let Value::Object(m) = &mut line {
            m.insert("value".to_owned(), Value::Number(-1.0));
        }
        assert!(read_directive(&line).is_err());
    }
}
