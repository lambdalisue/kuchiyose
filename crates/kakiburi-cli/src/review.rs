//! `kakiburi review`。2 つのカセットから目盛りを組み立て、草稿を検める
//! （[review](../../../docs/design/200-command.md#review)）。
//!
//! 組み立ては草稿を読む前に終える。 組み立ての入力は 2 つのカセットの統計値だけで、
//! 草稿は入らない（[守られるのはクレートの中だけである](../../../docs/design/000-architecture.md#守られるのはクレートの中だけである)）。
//! 型では止まらないので、組み立ての関数を差し替えられる形にして[試験](run_with)で見張る。
//!
//! 草稿を 2 本以上渡せば、目盛りを 1 度だけ組み立てて全部に同じものを当て、
//! 1 本 1 行で並べる（[草稿を並べる](../../../docs/design/200-command.md#草稿を並べる)）。

use std::collections::BTreeMap;

use kakiburi_cassette::json::Value;
use kakiburi_cassette::{MuteKind, Tuning};
use kakiburi_doc::Document;
use kakiburi_metrics::morph::{Analyzed, Analyzer};
use kakiburi_metrics::Measured;
use kakiburi_review::{judge, Observed, Outcome, Range, Review, Side, Verdict};
use kakiburi_scale::assembly::{self, Assembly, Built};
use kakiburi_scale::{Band, Divergence, Sample, Scale};

use crate::cassettes::{Opened, Origin};
use crate::exit::Exit;
use crate::pair::{self, Assemble};
use crate::remedies::FromDefinitions;
use crate::{analyzer, environment, folder, machine, with_commas};

/// 検める。
pub fn run(args: &[String]) -> Exit {
    run_with(args, &assembly::assemble_stats)
}

/// 引数。
struct Args {
    drafts: Vec<String>,
    cassette: String,
    baseline: Option<String>,
    json: bool,
    values: bool,
}

fn parse_args(args: &[String]) -> Result<Args, Exit> {
    let mut drafts: Vec<String> = Vec::new();
    let mut cassette = None;
    let mut baseline = None;
    let mut json = false;
    let mut values = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            flag @ ("--cassette" | "--baseline") => {
                let Some(v) = args.get(i + 1) else {
                    eprintln!("{flag} にカセットが要る");
                    return Err(Exit::Usage);
                };
                if flag == "--cassette" {
                    cassette = Some(v.clone());
                } else {
                    baseline = Some(v.clone());
                }
                i += 2;
            }
            "--json" => {
                json = true;
                i += 1;
            }
            "--values" => {
                values = true;
                i += 1;
            }
            other if other.starts_with('-') => {
                eprintln!("知らない引数: {other}");
                return Err(Exit::Usage);
            }
            other => {
                drafts.push(other.to_owned());
                i += 1;
            }
        }
    }
    if drafts.is_empty() {
        eprintln!("草稿を渡す");
        return Err(Exit::Usage);
    }
    let Some(cassette) = cassette else {
        eprintln!("--cassette が要る");
        return Err(Exit::Usage);
    };
    Ok(Args {
        drafts,
        cassette,
        baseline,
        json,
        values,
    })
}

/// 組み立ての関数を差し替えて検める。
///
/// 目盛りは 1 度だけ組み立て、それは草稿を読む前である。
pub fn run_with(args: &[String], assemble: &Assemble<'_>) -> Exit {
    let a = match parse_args(args) {
        Ok(a) => a,
        Err(e) => return e,
    };
    // 拡張子だけは先に見る。 使い方の誤りを、重い組み立ての後に言わない。
    for draft in &a.drafts {
        if let Err(e) = folder::source_or_refuse(draft) {
            return e;
        }
    }
    let defs = FromDefinitions::load();
    let pair = match pair::open(
        Origin::File(a.cassette.clone()),
        pair::baseline_origin(a.baseline.clone()),
        &defs,
    ) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let built = pair.assemble(assemble, &defs);

    // 草稿はここで初めて読む。
    let mut docs: Vec<Draft> = Vec::with_capacity(a.drafts.len());
    for name in &a.drafts {
        match folder::read(name) {
            Ok(doc) => docs.push(Draft {
                name: name.clone(),
                source: folder::source_of(name).map_or("", |s| s.name()),
                doc,
            }),
            Err(e) => return e,
        }
    }
    let head = Head {
        target: &pair.target,
        baseline: &pair.baseline,
        assembly: &built,
    };
    head.print(a.json);

    // 組み立てられなかった、自己検査が崩れた——どちらも正常な状態で、判定できないを返す。
    let usable = match &built.outcome {
        Ok(b) => match pair::self_check_failure(b.self_check.as_ref()) {
            Some(reason) => Err(reason),
            None => Ok(b),
        },
        Err(stopped) => Err(format!("目盛りが組み立てられない: {}", stopped.error)),
    };
    let rows: Vec<Row> = docs
        .iter()
        .map(|d| match &usable {
            Ok(b) => Row::Judged(Box::new(evaluate(
                d,
                b,
                &head.target.cassette.tuning,
                &defs,
            ))),
            Err(reason) => Row::Unknown {
                name: d.name.clone(),
                source: d.source,
                reason: reason.clone(),
            },
        })
        .collect();

    if let [row] = rows.as_slice() {
        single(&a, &head, row)
    } else {
        many(&a, &head, &rows, usable.ok())
    }
}

/// 読んだ草稿。
struct Draft {
    name: String,
    source: &'static str,
    doc: Document,
}

/// 1 本ぶんの結果。
enum Row {
    /// 目盛りに載せて検めた。
    Judged(Box<Judged>),
    /// 目盛りが使えないので、検める前に判定できないと決まった。
    Unknown {
        name: String,
        source: &'static str,
        reason: String,
    },
}

impl Row {
    fn name(&self) -> &str {
        match self {
            Row::Judged(j) => &j.name,
            Row::Unknown { name, .. } => name,
        }
    }

    fn outcome(&self) -> Outcome {
        match self {
            Row::Judged(j) => j.result.outcome.clone(),
            Row::Unknown { reason, .. } => {
                let mut o = judge(&[], None, None, &[]);
                o.reason.clone_from(reason);
                o
            }
        }
    }

    fn got(&self) -> Option<&kakiburi_scale::Measured> {
        match self {
            Row::Judged(j) => Some(&j.got),
            Row::Unknown { .. } => None,
        }
    }
}

/// どの組み合わせで検めたかを名乗る部分。どの出口にも同じものを出す。
struct Head<'a> {
    target: &'a Opened,
    baseline: &'a Opened,
    assembly: &'a Assembly,
}

impl Head<'_> {
    /// 人向けに名乗る。`--json` のときは何も出さない——stdout を JSON だけにする。
    fn print(&self, json: bool) {
        if json {
            return;
        }
        let t = &self.target.cassette;
        println!("本人: {}", self.target.named());
        println!("{}", self.baseline.name_line("基準"));
        println!("{}", pair::register_note(&self.assembly.register));
        if let Some(note) = pair::length_window_note(self.assembly) {
            println!("{note}");
        }
        let muted = muted_lines(&t.tuning);
        if !muted.is_empty() {
            println!("本人のカセットで無効にしているもの");
            for line in muted {
                println!("  {line}");
            }
        }
        println!();
    }

    /// 道具向けに名乗る欄。
    fn fields(&self) -> Vec<(String, Value)> {
        let (t, b) = (&self.target.cassette, &self.baseline.cassette);
        let a = self.assembly;
        let selection = a.outcome.as_ref().ok().map_or(Value::Null, |b| {
            let s = &b.scale.selection;
            Value::obj([
                (
                    "person_partners".to_owned(),
                    machine::strings(&s.person_partners),
                ),
                (
                    "person_points".to_owned(),
                    machine::strings(&s.person_points),
                ),
                (
                    "baseline_partners".to_owned(),
                    machine::strings(&s.baseline_partners),
                ),
                (
                    "baseline_points".to_owned(),
                    machine::strings(&s.baseline_points),
                ),
            ])
        });
        #[allow(clippy::cast_precision_loss)]
        let tried = Value::Number(a.tried as f64);
        vec![
            ("scene".to_owned(), Value::s(&t.scene)),
            ("content_hash".to_owned(), Value::s(t.stats.content_hash())),
            ("baseline_scene".to_owned(), Value::s(&b.scene)),
            (
                "baseline_content_hash".to_owned(),
                Value::s(b.stats.content_hash()),
            ),
            (
                "baseline_bundled".to_owned(),
                Value::Bool(self.baseline.origin == Origin::Bundled),
            ),
            ("tuning".to_owned(), t.tuning.to_json()),
            (
                "assembly".to_owned(),
                Value::obj([
                    (
                        "register".to_owned(),
                        Value::s(pair::register_note(&a.register)),
                    ),
                    ("picked".to_owned(), machine::strings(&a.picked)),
                    ("length_window".to_owned(), pair::length_window_json(a)),
                    (
                        "bundles".to_owned(),
                        Value::Array(a.bundles.iter().map(|g| machine::strings(g)).collect()),
                    ),
                    ("tried".to_owned(), tried),
                    ("selection".to_owned(), selection),
                ]),
            ),
            (
                // 較正の設定は名前と値の組で入れる。 判定が変わったとき、どの閾値が
                // 変わったかを名指せるようにする。
                "settings".to_owned(),
                Value::obj(
                    environment::calibration_settings()
                        .into_iter()
                        .map(|(k, v)| (k, Value::s(v))),
                ),
            ),
        ]
    }
}

/// 無効にしている対象を並べる。無効にしたことを忘れたまま使わないためである。
fn muted_lines(t: &Tuning) -> Vec<String> {
    let mut out = Vec::new();
    for kind in MuteKind::ALL {
        if t.mute_kinds.contains(&kind) {
            out.push(format!("{}: 種類ごと無効", kind.name()));
        }
        let muted = t.muted(kind);
        if !muted.is_empty() {
            out.push(format!("{}: {}", kind.name(), muted.join("、")));
        }
    }
    if let Some(f) = &t.first_person {
        out.push(format!("一人称の申告: {f}"));
    }
    if let Some(r) = t.register {
        out.push(format!("文体の申告: {}", r.name()));
    }
    out
}

/// 草稿 1 本を目盛りに載せた結果。出し方とは切り離して持つ。
struct Judged {
    name: String,
    source: &'static str,
    got: kakiburi_scale::Measured,
    /// 畳む前の、系統ごとの距離。
    distances: Vec<(String, f64)>,
    diverging: Vec<Divergence>,
    /// 前に出す指標の本数。
    directives: usize,
    /// 草稿の延べ語数。解析できなければ `None`。
    tokens: Option<usize>,
    /// 短すぎて測れないなら、その言い方。
    short: Option<String>,
    /// 指示できる指標ごとの値。
    measured: Vec<(String, Measured)>,
    /// 指示できる指標ごとの本人の幅。効くかを判定した指標だけにある。
    ranges: BTreeMap<String, (f64, f64)>,
    bands: Bands,
    result: Review,
}

/// 帯。`--values` で値と並べる。
#[derive(Clone, Copy)]
struct Bands {
    matching: Band,
    humanness: Band,
}

/// 草稿を測り、組み立て終えた目盛りに載せて判定する。
#[allow(clippy::too_many_lines)]
fn evaluate(d: &Draft, built: &Built, tuning: &Tuning, defs: &FromDefinitions) -> Judged {
    let (doc, scale): (&Document, &Scale) = (&d.doc, &built.scale);
    let lindera = analyzer::resolve();
    let an = Some(&lindera as &dyn Analyzer);
    let sample = Sample {
        name: &d.name,
        document: doc,
    };
    let got = kakiburi_scale::measure_against(scale, sample, an);

    // 照合値のどこが違うのかを言えるようにする。 次元が語として読める系統だけを見る。
    let readable = [
        kakiburi_metrics::System::FunctionWord,
        kakiburi_metrics::System::Comma,
        kakiburi_metrics::System::CharType,
    ];
    // 畳む前の距離を出す。 照合値は 5 つの距離を重みで畳んだものなので、
    // 畳んだあとだけではどこが動いたか分からない。
    let distances = kakiburi_scale::assemble::distances_against(scale, sample, an);
    let diverging = kakiburi_scale::diverging(scale, sample, an, &readable, 12);
    let side = |band: Band, value: Option<f64>| -> Option<Side> {
        value.map(|v| match band.judge(v) {
            kakiburi_scale::Verdict::Pass => Side::Human,
            kakiburi_scale::Verdict::Fail => Side::Machine,
            kakiburi_scale::Verdict::Unknown => Side::InBand,
        })
    };
    let humanness = side(scale.humanness_band, got.humanness);
    // 代わりの値で出した照合値も帯に照らす。
    let matching = side(
        scale.band,
        got.matching
            .or(got.matching_substituted.as_ref().map(|s| s.value)),
    );

    // 検める側も本人のカセットの語のまとめ方で割る。 違う割り方をすれば、
    // 比べたものに意味が無い（[同じ測り方で測る](../../../docs/spec/300-revise.md#同じ測り方で測る)）。
    let analyzed_now = Analyzed::with_lexicon(&doc.prose(), &lindera, &scale.lexicon).ok();
    let measured_now = measured_with(doc, analyzed_now.as_ref());
    // 0 段目。書き方。カセットを見ない。
    let inspections: Vec<kakiburi_review::Inspected> = defs
        .inspections()
        .into_iter()
        .map(|(name, upper, limit, remedy)| {
            let value = measured_now
                .iter()
                .find(|(n, _)| *n == name)
                .and_then(|(_, m)| m.value());
            let where_ = match name.as_str() {
                "語を割る読点" => analyzed_now
                    .as_ref()
                    .map(|a| a.split_commas().join("」「"))
                    .filter(|s| !s.is_empty())
                    .map(|s| format!("「{s}」。")),
                _ => None,
            };
            kakiburi_review::Inspected {
                broken: match value {
                    Some(v) => format!(
                        "{name}が {} で、線の {limit} を超えている。{}{remedy}",
                        number(v),
                        where_.unwrap_or_default()
                    ),
                    None => format!("{name}が線の {limit} を超えている。{remedy}"),
                },
                name,
                value,
                limit,
                upper,
            }
        })
        .collect();
    let observe = |e: &kakiburi_scale::Effective| -> Option<Observed> {
        let (_, m) = measured_now.iter().find(|(n, _)| *n == e.name)?;
        Some(Observed {
            name: e.name.clone(),
            value: m.value(),
            range: Range {
                low: e.low,
                high: e.high,
                units: e.units,
            },
            lower: defs.lower_rule(&e.name, e.rate),
            direct: defs.is_direct(&e.name),
        })
    };
    // 前に出す指標。 効くと判定されたものから、層 3 と無効にしたものを除く
    // （[前に出す指標](../../../docs/spec/300-revise.md#前に出す指標)）。判定も指摘もこの集合から取る。
    let muted = |name: &str| tuning.is_muted(MuteKind::Metric, name);
    let candidates: Vec<&kakiburi_scale::Effective> = built
        .effective
        .iter()
        .filter(|e| e.works())
        // 検査は幅で見ない。比べる先が本人ではない。
        .filter(|e| !defs.is_inspection(&e.name))
        // 層 3 は指摘にも判定にも使わない。止めた理由を言えないものは止めない。
        .filter(|e| !defs.is_layer_three(&e.name))
        .collect();
    let directives: Vec<Observed> = candidates
        .iter()
        .filter(|e| !muted(&e.name))
        .filter_map(|e| observe(e))
        .collect();
    let directives_muted = !candidates.is_empty() && candidates.iter().all(|e| muted(&e.name));
    // 一貫しているだけの軸も見る。判定はしない、指摘にだけ出す。
    let habits: Vec<Observed> = built
        .effective
        .iter()
        .filter(|e| e.narrow_only())
        .filter(|e| !muted(&e.name))
        .filter(|e| !defs.is_layer_three(&e.name))
        .filter(|e| !defs.is_inspection(&e.name))
        .filter_map(observe)
        .collect();
    // 人らしさの直し方は、指標ごとの値から組む。
    let by_metric: Vec<kakiburi_review::HumannessObserved> = got
        .humanness_by_metric
        .iter()
        .map(|m| kakiburi_review::HumannessObserved {
            name: m.name.clone(),
            value: m.value,
            raise: m.raise,
            // 長い繰り返しにだけ添える。
            phrases: if m.name == "長い繰り返し" {
                scale.phrases.clone()
            } else {
                Vec::new()
            },
            overused: m.overused.clone(),
            once_only: m.once_only.clone(),
            effect: m.effect,
            target: m.target,
        })
        .collect();
    let apart: Vec<kakiburi_review::MatchingObserved> = diverging
        .iter()
        .map(|d| kakiburi_review::MatchingObserved {
            system: d.system.clone(),
            dim: d.dim.clone(),
            mine: d.mine,
            theirs: d.theirs,
            effect: d.effect(),
            examples: d.examples.clone(),
            spots: d.spots.clone(),
        })
        .collect();
    // 型が使われているか。 地の文から探す——記法の外にある並びは型ではない。
    let joined = kakiburi_metrics::humanness::joined(&doc.prose());
    let ja = doc.japanese_chars();
    let density = |times: usize| -> f64 {
        #[allow(clippy::cast_precision_loss)]
        if ja == 0 {
            0.0
        } else {
            1000.0 * times as f64 / ja as f64
        }
    };
    let spots_of = |needle: &str| spots(&joined, needle);
    let seen =
        |ks: &[kakiburi_scale::assemble::Kata], kind: MuteKind| -> Vec<kakiburi_review::Kata> {
            ks.iter()
                // 本人が無効にした型は渡さない。 無効にした基準の型は知らせに出さない。
                .filter(|k| !tuning.is_muted(kind, &k.shown()))
                .map(|k| {
                    let (spots, times) = spots_of(&k.text);
                    kakiburi_review::Kata {
                        spots,
                        times,
                        density: density(times),
                        ceiling: k.ceiling,
                        base: k.base,
                        // 穴あきは、固定部が 2 つともこの順で同じ段落にあれば使われている。
                        used: match &k.tail {
                            Some(t) => joined.split('\n').any(|line| {
                                line.find(&k.text)
                                    .and_then(|i| line[i + k.text.len()..].find(t.as_str()))
                                    .is_some()
                            }),
                            None => joined.contains(&k.text),
                        },
                        text: k.shown(),
                        rate: k.rate,
                        at: k.at,
                    }
                })
                .collect()
        };
    // 直し方に載せた言い回しも、同じ見方で数える。 使いすぎを止めるためである。
    let offered = scale.phrase_ceilings.iter().map(|(text, ceiling)| {
        let (spots, times) = spots_of(text);
        kakiburi_review::Kata {
            text: text.clone(),
            rate: 0.0,
            base: 0.0,
            at: 0.0,
            used: times > 0,
            spots,
            times,
            density: density(times),
            ceiling: *ceiling,
        }
    });
    // この文章が繰り返している言い回しも、同じ物差しに乗せる。 本人の上限は
    // 本人のカセットの言い回しの表から引く。表に無いものは上限 0 で、本人が
    // 一度も使っていないことがそのまま指摘になる。
    let repeated = repeated_in_draft(analyzed_now.as_ref())
        .into_iter()
        .filter(|(text, _)| {
            !scale
                .phrase_ceilings
                .iter()
                .any(|(p, _)| p == text || text.contains(p) || p.contains(text))
        })
        .map(|(text, times)| {
            let (spots, _) = spots_of(&text);
            let ceiling = built.phrase_ceilings.get(&text).copied().unwrap_or(0.0);
            kakiburi_review::Kata {
                text,
                rate: 0.0,
                base: 0.0,
                at: 0.0,
                used: true,
                spots,
                times,
                density: density(times),
                ceiling,
            }
        });
    let phrases: Vec<kakiburi_review::Kata> = offered.chain(repeated).collect();
    let katas = seen(&scale.katas, MuteKind::Kata);
    let machine_katas = seen(&scale.machine_katas, MuteKind::BaselineKata);
    // 語は文字列ではなく語彙素で照らす。
    let here = kakiburi_metrics::word::goi(analyzed_now.as_ref());
    let machine_gois: Vec<kakiburi_review::Goi> = scale
        .machine_gois
        .iter()
        .filter(|g| !tuning.is_muted(MuteKind::BaselineGoi, &g.text))
        .map(|g| kakiburi_review::Goi {
            times: here.get(&g.text).map_or(0, |(_, n)| *n),
            text: g.text.clone(),
            rate: g.rate,
            base: g.base,
            theirs: g.theirs.clone(),
        })
        .collect();
    // 一人称の申告があれば、数えた結果より申告を採る
    // （[一人称は別に見る](../../../docs/spec/300-revise.md#一人称は別に見る)）。
    let first_person: Vec<(String, f64)> = match &tuning.first_person {
        Some(declared) => vec![(declared.clone(), 1.0)],
        None => scale.first_person.clone(),
    };
    let draft_opening = doc.opening().map(|k| k.name().to_owned());
    let draft_first_person: Vec<(String, usize)> =
        kakiburi_metrics::word::first_person(analyzed_now.as_ref())
            .into_iter()
            .collect();
    let tokens = analyzed_now.as_ref().map(Analyzed::tokens);
    let short = too_short(doc, tokens);
    let result = kakiburi_review::review(
        &kakiburi_review::Observations {
            inspections: &inspections,
            humanness,
            matching,
            matching_substituted: got.matching_substituted.as_ref().map(|s| s.system.as_str()),
            too_short: short.as_deref(),
            directives: &directives,
            directives_muted,
            habits: &habits,
            humanness_by_metric: &by_metric,
            diverging: &apart,
            katas: &katas,
            machine_katas: &machine_katas,
            machine_gois: &machine_gois,
            phrases: &phrases,
            first_person: &first_person,
            draft_first_person: &draft_first_person,
            opening: &scale.opening,
            draft_opening: draft_opening.as_deref(),
        },
        defs,
    );
    Judged {
        name: d.name.clone(),
        source: d.source,
        got,
        distances,
        diverging,
        directives: directives.len(),
        tokens,
        short,
        measured: measured_now,
        ranges: built
            .effective
            .iter()
            .map(|e| (e.name.clone(), (e.low, e.high)))
            .collect(),
        bands: Bands {
            matching: scale.band,
            humanness: scale.humanness_band,
        },
        result,
    }
}

/// 草稿 1 本のとき。判定と指摘を全部出す。
fn single(a: &Args, head: &Head<'_>, row: &Row) -> Exit {
    let outcome = row.outcome();
    let exit = Exit::from_verdict(outcome.verdict);
    let j = match row {
        Row::Judged(j) => j,
        Row::Unknown { source, reason, .. } => {
            if a.json {
                let mut fields = head.fields();
                fields.extend(unknown_fields(row.name(), source, &outcome));
                if a.values {
                    fields.push(("values".to_owned(), Value::Null));
                }
                println!("{}", Value::obj(fields).write());
            } else {
                println!("判定: 判定できない");
                println!("止まった段: {}", outcome.stage.name());
                println!("理由: {reason}");
                if a.values {
                    println!();
                    println!("測った値: 目盛りが無いので、幅や帯と並べて出せない");
                }
            }
            return exit;
        }
    };

    if a.json {
        let mut fields = head.fields();
        fields.extend(judged_fields(j, a.values));
        println!("{}", Value::obj(fields).write());
        return exit;
    }

    let got = &j.got;
    let result = &j.result;
    println!("前に出す指標: {} 本", j.directives);
    println!();
    // 長さで黙るなら、長さで黙ると言う。
    if let Some(n) = j.tokens {
        let floor = kakiburi_metrics::floor::TOKENS;
        if n < floor {
            println!(
                "延べ {n} 語。下限 {floor} 語に届かないので測れない——{} 語ぶん足りない",
                floor - n
            );
        } else if n < floor + floor / 5 {
            println!("延べ {n} 語。下限 {floor} 語に近い——これ以上削ると測れなくなる");
        }
    }
    println!("基準との距離: {}", shown(got.humanness));
    if !got.missing_humanness.is_empty() {
        println!("  測れていない次元: {}", got.missing_humanness.join("、"));
    }
    println!("照合値: {}", shown(got.matching));
    if !got.missing_systems.is_empty() {
        println!("  測れていない系統: {}", got.missing_systems.join("、"));
    }
    if let Some(s) = &got.matching_substituted {
        println!("{}", substituted_note(s));
    }
    println!();
    println!("判定: {}", verdict_name(result.outcome.verdict));
    println!("止まった段: {}", result.outcome.stage.name());
    println!("理由: {}", result.outcome.reason);
    let section = |title: &str, lines: &[String]| {
        if lines.is_empty() {
            return;
        }
        println!();
        println!("{title} {} 本", lines.len());
        for l in lines {
            println!("  - {l}");
        }
    };
    let points: Vec<String> = result.points.iter().map(|p| p.prose()).collect();
    section("指摘", &points);
    section("照合の直し方", &result.matching);
    section("人らしさの直し方", &result.humanness);
    // ここから下は判定に使っていない。 見出しを 1 本入れて、止める指摘と
    // 止めない知らせを分ける。
    if result.has_aside() {
        println!();
        println!("ここから下は判定に使っていない。直すかどうかは書き手が決める。");
    }
    let habits: Vec<String> = result.habits.iter().map(|h| h.prose()).collect();
    section("本人の癖から外れているところ", &habits);
    section("使われていない型", &result.katas);
    section("残っている基準の型", &result.machine_katas);
    section("残っている基準の語", &result.machine_gois);
    section("使いすぎている言い回し", &result.overused_katas);
    section("本人と違う一人称", &result.first_person);
    if !result.opening.is_empty() {
        println!();
        println!("書き出しの構造");
        for o in &result.opening {
            println!("  - {o}");
        }
    }
    if a.values {
        println!();
        for line in values_lines(j) {
            println!("{line}");
        }
    }
    exit
}

/// 判定できないと先に決まった 1 本の、道具向けの欄。
///
/// 早く抜けても欄は同じである。 欄が消えれば、読む側は「出なかった」と
/// 「そもそも無い」を区別できない。
fn unknown_fields(name: &str, source: &str, outcome: &Outcome) -> Vec<(String, Value)> {
    vec![
        ("draft".to_owned(), Value::s(name)),
        ("source".to_owned(), Value::s(source)),
        (
            "verdict".to_owned(),
            Value::s(verdict_name(outcome.verdict)),
        ),
        ("stage".to_owned(), Value::s(outcome.stage.name())),
        ("reason".to_owned(), Value::s(&outcome.reason)),
        ("baseline_distance".to_owned(), Value::Null),
        ("missing_humanness".to_owned(), Value::Array(vec![])),
        ("matching".to_owned(), Value::Null),
        ("matching_substituted".to_owned(), Value::Null),
        ("missing_systems".to_owned(), Value::Array(vec![])),
        ("too_short".to_owned(), Value::Null),
        ("directives".to_owned(), Value::Number(0.0)),
        ("points".to_owned(), Value::Array(vec![])),
        ("humanness_points".to_owned(), Value::Array(vec![])),
        ("baseline_distance_by_metric".to_owned(), Value::obj([])),
        ("matching_points".to_owned(), Value::Array(vec![])),
        ("matching_by_dim".to_owned(), Value::Array(vec![])),
    ]
}

/// 検めた 1 本の、道具向けの欄。
fn judged_fields(j: &Judged, values: bool) -> Vec<(String, Value)> {
    #[allow(clippy::cast_precision_loss)]
    let n = |v: usize| Value::Number(v as f64);
    let (got, result) = (&j.got, &j.result);
    let mut fields = vec![
        ("draft".to_owned(), Value::s(&j.name)),
        ("source".to_owned(), Value::s(j.source)),
        (
            "verdict".to_owned(),
            Value::s(verdict_name(result.outcome.verdict)),
        ),
        ("stage".to_owned(), Value::s(result.outcome.stage.name())),
        ("reason".to_owned(), Value::s(&result.outcome.reason)),
        ("baseline_distance".to_owned(), machine::number(got.humanness)),
        (
            "missing_humanness".to_owned(),
            machine::strings(&got.missing_humanness),
        ),
        (
            // 指標ごとの観測と向き。 合算した 1 つの値だけでは、どの指標が
            // 隔たりを担っているかを言えない。
            "baseline_distance_by_metric".to_owned(),
            Value::obj(got.humanness_by_metric.iter().map(|m| {
                (
                    m.name.clone(),
                    Value::obj([
                        ("value".to_owned(), Value::Number(m.value)),
                        ("raise".to_owned(), Value::Bool(m.raise)),
                    ]),
                )
            })),
        ),
        ("matching".to_owned(), machine::number(got.matching)),
        (
            // `matching` と同じ欄に入れない。 同じ目盛りに載っていない値を、
            // 読む側が区別できなくなる。
            "matching_substituted".to_owned(),
            got.matching_substituted.as_ref().map_or(Value::Null, |s| {
                Value::obj([
                    ("system".to_owned(), Value::s(&s.system)),
                    ("value".to_owned(), Value::Number(s.value)),
                    (
                        "sigma".to_owned(),
                        Value::Number(kakiburi_scale::calibrate::SUBSTITUTE_SIGMA),
                    ),
                ])
            }),
        ),
        (
            "missing_systems".to_owned(),
            machine::strings(&got.missing_systems),
        ),
        (
            "too_short".to_owned(),
            j.short.as_deref().map_or(Value::Null, Value::s),
        ),
        ("directives".to_owned(), n(j.directives)),
        (
            "distances".to_owned(),
            Value::obj(
                j.distances
                    .iter()
                    .map(|(k, v)| (k.clone(), Value::Number(*v))),
            ),
        ),
        ("katas".to_owned(), machine::strings(&result.katas)),
        (
            "baseline_katas".to_owned(),
            machine::strings(&result.machine_katas),
        ),
        (
            "baseline_gois".to_owned(),
            machine::strings(&result.machine_gois),
        ),
        (
            "first_person".to_owned(),
            machine::strings(&result.first_person),
        ),
        ("opening".to_owned(), machine::strings(&result.opening)),
        (
            // 指摘は結果であって断り書きではない。 ここに入れる。
            "points".to_owned(),
            Value::Array(result.points.iter().map(|p| Value::s(p.prose())).collect()),
        ),
        (
            // 書きぶりの枠と混ぜない。
            "humanness_points".to_owned(),
            machine::strings(&result.humanness),
        ),
        (
            "matching_points".to_owned(),
            machine::strings(&result.matching),
        ),
        (
            "matching_by_dim".to_owned(),
            Value::Array(
                j.diverging
                    .iter()
                    .map(|d| {
                        Value::obj([
                            ("system".to_owned(), Value::s(&d.system)),
                            ("dim".to_owned(), Value::s(&d.dim)),
                            ("mine".to_owned(), Value::Number(d.mine)),
                            ("theirs".to_owned(), Value::Number(d.theirs)),
                            ("outside".to_owned(), Value::Number(d.outside())),
                            ("effect".to_owned(), Value::Number(d.effect())),
                        ])
                    })
                    .collect(),
            ),
        ),
    ];
    if values {
        fields.push(("values".to_owned(), values_json(j)));
    }
    fields
}

/// 測った値を全部。幅や帯と並べる（[測った値を出す](../../../docs/design/200-command.md#測った値を出す)）。
fn values_json(j: &Judged) -> Value {
    Value::obj([
        (
            "bands".to_owned(),
            Value::obj([
                ("matching".to_owned(), machine::band(j.bands.matching)),
                ("baseline_distance".to_owned(), machine::band(j.bands.humanness)),
            ]),
        ),
        (
            "systems".to_owned(),
            Value::obj(
                j.distances
                    .iter()
                    .map(|(k, v)| (k.clone(), Value::Number(*v))),
            ),
        ),
        (
            "baseline_distance_by_metric".to_owned(),
            Value::obj(
                j.got
                    .humanness_metrics
                    .iter()
                    .map(|(k, v)| (k.clone(), Value::Number(*v))),
            ),
        ),
        (
            "baseline_distance_by_dim".to_owned(),
            Value::obj(
                j.got
                    .humanness_dims
                    .iter()
                    .map(|(k, v)| (k.clone(), machine::number(*v))),
            ),
        ),
        (
            "directives".to_owned(),
            Value::obj(j.measured.iter().map(|(name, m)| {
                let range = j.ranges.get(name);
                (
                    name.clone(),
                    Value::obj([
                        ("value".to_owned(), machine::number(m.value())),
                        (
                            "excluded".to_owned(),
                            m.unmeasured().map_or(Value::Null, |u| Value::s(u.name())),
                        ),
                        ("low".to_owned(), machine::number(range.map(|r| r.0))),
                        ("high".to_owned(), machine::number(range.map(|r| r.1))),
                    ]),
                )
            })),
        ),
    ])
}

/// 測った値を全部、人向けに。
fn values_lines(j: &Judged) -> Vec<String> {
    let band = |name: &str, b: Band| {
        format!(
            "  {name}の帯: 天井 {:.3}〜{:.3} / 床 {:.3}〜{:.3}（{}）",
            b.ceiling.low,
            b.ceiling.high,
            b.floor.low,
            b.floor.high,
            if b.separated() {
                "分かれている"
            } else {
                "重なっている"
            }
        )
    };
    let mut out = vec![
        "測った値".to_owned(),
        band("照合値", j.bands.matching),
        band("基準との距離", j.bands.humanness),
        "  系統ごとの距離（相手集合との中央値）".to_owned(),
    ];
    out.extend(j.distances.iter().map(|(k, v)| format!("    {k}: {v:.3}")));
    out.push("  基準との距離（指標ごと）".to_owned());
    if j.got.humanness_metrics.is_empty() {
        out.push("    出ていない（測れていない次元がある）".to_owned());
    }
    out.extend(
        j.got
            .humanness_metrics
            .iter()
            .map(|(k, v)| format!("    {k}: {v:.3}")),
    );
    out.push("  基準との距離の材料（次元ごとの測った量）".to_owned());
    out.extend(j.got.humanness_dims.iter().map(|(k, v)| {
        format!(
            "    {k}: {}",
            v.map_or_else(|| "—".to_owned(), |x| format!("{x:.3}"))
        )
    }));
    out.push("  指示できる指標（幅は本人の単位の最小〜最大）".to_owned());
    out.extend(j.measured.iter().map(|(name, m)| {
        let value = match (m.value(), m.unmeasured()) {
            (Some(v), _) => format!("{v:.3}"),
            (None, Some(why)) => format!("—（{}）", why.name()),
            (None, None) => "—".to_owned(),
        };
        match j.ranges.get(name) {
            Some((lo, hi)) => format!("    {name}: {value}（幅 {lo:.3}〜{hi:.3}）"),
            None => format!("    {name}: {value}"),
        }
    }));
    out
}

/// 草稿が 2 本以上のとき。1 本 1 行で判定と主な値だけを並べる。
///
/// 指摘は出さない。 読みたい草稿は 1 本だけ渡して検め直す。
fn many(a: &Args, head: &Head<'_>, rows: &[Row], built: Option<&Built>) -> Exit {
    let outcomes: Vec<Outcome> = rows.iter().map(Row::outcome).collect();
    let exit = combined_exit(outcomes.iter().map(|o| o.verdict));
    if a.json {
        let drafts: Vec<Value> = rows
            .iter()
            .zip(&outcomes)
            .map(|(row, outcome)| match row {
                Row::Judged(j) => Value::obj(judged_fields(j, a.values)),
                Row::Unknown { name, source, .. } => {
                    let mut f = unknown_fields(name, source, outcome);
                    if a.values {
                        f.push(("values".to_owned(), Value::Null));
                    }
                    Value::obj(f)
                }
            })
            .collect();
        let mut fields = head.fields();
        fields.push(("drafts".to_owned(), Value::Array(drafts)));
        fields.push((
            "scatter".to_owned(),
            built.map_or(Value::Null, |b| scatter_json(rows, &b.scale)),
        ));
        println!("{}", Value::obj(fields).write());
        return exit;
    }
    if a.values {
        eprintln!("草稿が 2 本以上のときは、測った値を --json の中に草稿ごとに出す");
    }
    let table: Vec<[String; 5]> = rows
        .iter()
        .zip(&outcomes)
        .map(|(row, o)| {
            let got = row.got();
            // 目盛りが使えなければ、どの段にも入っていない。 段の名前を出すと、
            // その段で止まったように読める。
            let stage = if o.verdict == Verdict::Pass || got.is_none() {
                "—".to_owned()
            } else {
                o.stage.name().to_owned()
            };
            [
                row.name().to_owned(),
                verdict_name(o.verdict).to_owned(),
                stage,
                short_value(got.and_then(|g| g.matching)),
                short_value(got.and_then(|g| g.humanness)),
            ]
        })
        .collect();
    let header = ["草稿", "判定", "止まった段", "照合値", "基準との距離"].map(str::to_owned);
    for line in aligned(&header, &table) {
        println!("{line}");
    }
    println!();
    // 目盛りが使えないなら、理由は全部の草稿で同じである。 1 度だけ言う。
    if built.is_none() {
        if let Some(o) = outcomes.first() {
            println!("理由: {}", o.reason);
        }
        return exit;
    }
    for (row, o) in rows.iter().zip(&outcomes) {
        println!("{}: {}", row.name(), o.reason);
    }
    exit
}

/// 並べた草稿の終了コード。1 本でも通らないなら 1、それ以外で 1 本でも判定できないなら 2。
fn combined_exit(verdicts: impl IntoIterator<Item = Verdict>) -> Exit {
    let vs: Vec<Verdict> = verdicts.into_iter().collect();
    if vs.contains(&Verdict::Fail) {
        Exit::Fail
    } else if vs.contains(&Verdict::Unknown) {
        Exit::Unknown
    } else {
        Exit::Pass
    }
}

/// 表の値。出ていないものは `—` にする。0 と混ぜない。
fn short_value(v: Option<f64>) -> String {
    v.map_or_else(|| "—".to_owned(), |x| format!("{x:.3}"))
}

/// 列を揃える。幅は端末で見える幅で数える——和文は 1 字が 2 桁を取る。
fn aligned(header: &[String; 5], rows: &[[String; 5]]) -> Vec<String> {
    let width = |s: &str| console::measure_text_width(s);
    let mut widths = [0usize; 5];
    for r in std::iter::once(header).chain(rows) {
        for (w, cell) in widths.iter_mut().zip(r) {
            *w = (*w).max(width(cell));
        }
    }
    std::iter::once(header)
        .chain(rows)
        .map(|r| {
            let cells: Vec<String> = r
                .iter()
                .enumerate()
                .map(|(i, cell)| {
                    let pad = " ".repeat(widths[i] - width(cell));
                    // 数の列は右に揃える。 符号の有無で桁がずれない。
                    if i >= 3 {
                        format!("{pad}{cell}")
                    } else {
                        format!("{cell}{pad}")
                    }
                })
                .collect();
            cells.join("  ").trim_end().to_owned()
        })
        .collect()
}

/// 並べた草稿の散らばりを、天井と比べる（[直すと差が均されるのを見張る](../../../docs/spec/300-revise.md#直すと差が均されるのを見張る)）。
///
/// 別々の文書を同じ周回数だけ回したものを並べたときに意味がある。 同じ文書の周回
/// どうしは題材も構成も共有しているので、天井より狭く出るのが当たり前である。
fn scatter_json(rows: &[Row], scale: &Scale) -> Value {
    let one = |values: Vec<f64>, ceiling: kakiburi_scale::Ends| -> Value {
        let spread = spread_of(&values);
        let ceiling_spread = ceiling.spread();
        Value::obj([
            (
                "values".to_owned(),
                Value::Array(values.iter().map(|v| Value::Number(*v)).collect()),
            ),
            ("spread".to_owned(), machine::number(spread)),
            ("sd".to_owned(), machine::number(sd_of(&values))),
            (
                "ceiling".to_owned(),
                Value::obj([
                    ("low".to_owned(), Value::Number(ceiling.low)),
                    ("high".to_owned(), Value::Number(ceiling.high)),
                    ("spread".to_owned(), Value::Number(ceiling_spread)),
                ]),
            ),
            (
                "ratio".to_owned(),
                machine::number(
                    spread
                        .filter(|_| ceiling_spread > 0.0)
                        .map(|s| s / ceiling_spread),
                ),
            ),
        ])
    };
    let collect = |f: &dyn Fn(&kakiburi_scale::Measured) -> Option<f64>| -> Vec<f64> {
        rows.iter().filter_map(Row::got).filter_map(f).collect()
    };
    Value::obj([
        (
            "matching".to_owned(),
            one(collect(&|g| g.matching), scale.band.ceiling),
        ),
        (
            "baseline_distance".to_owned(),
            one(collect(&|g| g.humanness), scale.humanness_band.ceiling),
        ),
    ])
}

/// 最大と最小の差。2 本に満たなければ出さない。
fn spread_of(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Some(hi - lo)
}

/// 標本の標準偏差。2 本に満たなければ出さない。
fn sd_of(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let ss: f64 = values.iter().map(|v| (v - mean).powi(2)).sum();
    Some((ss / (n - 1.0)).sqrt())
}

/// 地の文で `needle` が出ている箇所と回数。「言い換えろ」と言うなら、どこを言い換えるのかを言う。
fn spots(joined: &str, needle: &str) -> (Vec<String>, usize) {
    const SHOWN: usize = 2;
    const AROUND: usize = 12;
    let (mut out, mut times) = (Vec::new(), 0usize);
    let pat: Vec<char> = needle.chars().collect();
    if pat.is_empty() {
        return (out, 0);
    }
    for line in joined.split('\n') {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0usize;
        while i + pat.len() <= chars.len() {
            if chars[i..i + pat.len()] != pat[..] {
                i += 1;
                continue;
            }
            times += 1;
            if out.len() < SHOWN {
                let from = i.saturating_sub(AROUND);
                let to = (i + pat.len() + AROUND).min(chars.len());
                out.push(chars[from..to].iter().collect::<String>());
            }
            i += pat.len();
        }
    }
    (out, times)
}

/// この文章が繰り返していると言う回数の下限。
///
/// 繰り返していないものは見ない。 1 度きりの言い回しは書きぶりではなく、
/// その文章の題材である。
const DRAFT_REPEAT: usize = 3;

/// この文章が繰り返している言い回しと、その回数。
///
/// 上限を渡す先を、道具が勧めた言い回しに限らない。 受け取った側は勧められていない
/// 言い回しでも足す——人らしさを通すために語尾を揃えるのが、いちばん安い手だからである。
fn repeated_in_draft(analyzed: Option<&Analyzed>) -> Vec<(String, usize)> {
    let mut n: BTreeMap<String, usize> = BTreeMap::new();
    for (g, _, _) in
        kakiburi_metrics::word::grams_with_position(analyzed, &kakiburi_scale::assemble::KATA_N)
    {
        *n.entry(g).or_insert(0) += 1;
    }
    let mut out: Vec<(String, usize)> = n
        .into_iter()
        .filter(|(text, times)| {
            *times >= DRAFT_REPEAT && text.chars().count() >= kakiburi_scale::stats::PHRASE_CHARS
        })
        .collect();
    // 長いほうを先に採る。 短い並びはその一部なので、両方出すと同じ指摘が重なる。
    out.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| b.0.chars().count().cmp(&a.0.chars().count()))
            .then_with(|| a.0.cmp(&b.0))
    });
    let mut kept: Vec<(String, usize)> = Vec::new();
    for (text, times) in out {
        if kept.iter().any(|(k, _)| k.contains(&text)) {
            continue;
        }
        kept.push((text, times));
    }
    kept
}

/// 測れる指標。解析器を要るものも含める。
///
/// 解析器が無ければ、要る軸は[道具が無い](Measured::ToolMissing)になる
/// ——0 を返さないし、名前も落とさない。
pub fn measured_with(doc: &Document, analyzed: Option<&Analyzed>) -> Vec<(String, Measured)> {
    kakiburi_metrics::directive::measure(doc, analyzed)
        .into_iter()
        .map(|(name, c)| (name, c.measured()))
        .collect()
}

/// 素材の下限に届かない短さを、判定の理由に載る形で返す。届いていれば `None`。
///
/// 字数を先に見る。 下限は字数だけが先行研究由来で、語数はそこから決めている。
fn too_short(doc: &Document, tokens: Option<usize>) -> Option<String> {
    let chars = doc.japanese_chars();
    let floor = kakiburi_metrics::floor::JAPANESE_CHARS;
    if chars < floor {
        return Some(format!(
            "地の文の日本語 {} 字 / 下限 {} 字",
            with_commas(chars),
            with_commas(floor)
        ));
    }
    let n = tokens?;
    let floor = kakiburi_metrics::floor::TOKENS;
    (n < floor).then(|| {
        format!(
            "延べ {} 語 / 下限 {} 語",
            with_commas(n),
            with_commas(floor)
        )
    })
}

/// 照合値を代わりの値で出したことを言う 1 行。
fn substituted_note(s: &kakiburi_scale::Substituted) -> String {
    format!(
        "  {} が測れないので、機械の側へ標準偏差 {} 個ぶん寄せた代わりの値を置いて出した照合値: {}（人の側に出たときだけ使う）",
        s.system,
        kakiburi_scale::calibrate::SUBSTITUTE_SIGMA,
        shown(Some(s.value))
    )
}

/// 値か、出ていないことを書く。0 と混ぜない。
fn shown(v: Option<f64>) -> String {
    v.map_or_else(|| "出ていない".to_owned(), |x| format!("{x:.3}"))
}

/// 3 値の名前。
fn verdict_name(v: Verdict) -> &'static str {
    match v {
        Verdict::Pass => "通る",
        Verdict::Fail => "通らない",
        Verdict::Unknown => "判定できない",
    }
}

/// 数を散文に載せる。個数を `1.0000` と書かない。
fn number(v: f64) -> String {
    if (v - v.round()).abs() < f64::EPSILON {
        format!("{v:.0}")
    } else {
        format!("{v:.4}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cassettes;
    use crate::fixture;
    use crate::testdir::TempDir;
    use kakiburi_cassette::Register;
    use kakiburi_doc::node::{Kind, Node};
    use kakiburi_scale::stats::CassetteStats;
    use std::cell::RefCell;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    fn paragraph(text: &str) -> Document {
        Document::new(vec![Node::leaf(Kind::Paragraph, text)])
    }

    /// 本人と基準のカセットを作る。
    fn cassettes(dir: &TempDir) -> (String, String) {
        let person = fixture::write_corpus(dir, "本人", false);
        let baseline = fixture::write_corpus(dir, "基準", true);
        let (p, b) = (dir.join("本人.kb"), dir.join("基準.kb"));
        for (folder, out) in [(&person, &p), (&baseline, &b)] {
            assert_eq!(
                crate::run(&args(&["cassette", "build", folder, "-o", out])),
                Exit::Pass
            );
        }
        (p, b)
    }

    #[test]
    fn 字数が下限に届かなければ字数で短いと言う() {
        let doc = paragraph("短い。");
        let got = too_short(&doc, Some(5000)).expect("短い");
        assert!(got.contains("字"), "{got}");
    }

    #[test]
    fn 字数が届いても語数が届かなければ語数で短いと言う() {
        let doc = paragraph(&"あ".repeat(1200));
        let got = too_short(&doc, Some(10)).expect("短い");
        assert!(got.contains("語"), "{got}");
    }

    #[test]
    fn 下限に届いていれば短いと言わない() {
        let doc = paragraph(&"あ".repeat(1200));
        assert_eq!(too_short(&doc, Some(5000)), None);
        assert_eq!(too_short(&doc, None), None, "語数が出ないのは短さではない");
    }

    #[test]
    fn 組み立ては草稿を読む前にちょうど_1_度走り渡るのは_2_つのカセットだけである() {
        // 型で止まらない唯一の境界である。 草稿は組み立ての関数が呼ばれるまで
        // 存在しない——先に読めば、読めずに 65 で終わる。
        let dir = TempDir::new("assemble-once");
        let (p, b) = cassettes(&dir);
        let draft = dir.join("草稿.md");
        let calls: RefCell<Vec<(usize, usize)>> = RefCell::new(Vec::new());
        let spy = |t: &CassetteStats,
                   base: &CassetteStats,
                   tuning: assembly::Tuning,
                   by: &dyn Fn(&str) -> bool| {
            assert!(
                !std::path::Path::new(&draft).exists(),
                "組み立てる前に草稿がある"
            );
            calls
                .borrow_mut()
                .push((t.documents.len(), base.documents.len()));
            std::fs::write(&draft, "これは、そうだ、と思う。\n").expect("書ける");
            assembly::assemble_stats(t, base, tuning, by)
        };
        let got = run_with(&args(&[&draft, "--cassette", &p, "--baseline", &b]), &spy);
        assert_eq!(got, Exit::Unknown, "短い草稿なので判定できない");
        assert_eq!(*calls.borrow(), vec![(10, 10)]);
    }

    #[test]
    fn 草稿を並べても組み立ては草稿を読む前に_1_度だけ走る() {
        // 草稿ごとに組み立て直せば、同じ組み合わせでも比べられなくなる。
        let dir = TempDir::new("assemble-once-many");
        let (p, b) = cassettes(&dir);
        let drafts = [dir.join("草稿1.md"), dir.join("草稿2.md")];
        let calls = RefCell::new(0usize);
        let spy = |t: &CassetteStats,
                   base: &CassetteStats,
                   tuning: assembly::Tuning,
                   by: &dyn Fn(&str) -> bool| {
            for d in &drafts {
                assert!(
                    !std::path::Path::new(d).exists(),
                    "組み立てる前に草稿がある"
                );
                std::fs::write(d, "これは、そうだ、と思う。\n").expect("書ける");
            }
            *calls.borrow_mut() += 1;
            assembly::assemble_stats(t, base, tuning, by)
        };
        let got = run_with(
            &args(&[&drafts[0], &drafts[1], "--cassette", &p, "--baseline", &b]),
            &spy,
        );
        assert_eq!(got, Exit::Unknown);
        assert_eq!(*calls.borrow(), 1);
    }

    #[test]
    fn 違う草稿を渡しても組み立てた目盛りは同じである() {
        let dir = TempDir::new("same-scale");
        let (p, b) = cassettes(&dir);
        let seen: RefCell<Vec<Assembly>> = RefCell::new(Vec::new());
        let spy = |t: &CassetteStats,
                   base: &CassetteStats,
                   tuning: assembly::Tuning,
                   by: &dyn Fn(&str) -> bool| {
            let a = assembly::assemble_stats(t, base, tuning, by);
            seen.borrow_mut().push(a.clone());
            a
        };
        for (i, body) in ["これは、そうだ。\n", "あれは、ちがう、と思う。\n"]
            .iter()
            .enumerate()
        {
            let draft = dir.write(&format!("草稿{i}.md"), body);
            run_with(&args(&[&draft, "--cassette", &p, "--baseline", &b]), &spy);
        }
        let seen = seen.borrow();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0], seen[1]);
    }

    /// 組み立てた結果を控えながら検める。
    fn review_seen(v: &[String]) -> (Exit, Assembly) {
        let seen: RefCell<Option<Assembly>> = RefCell::new(None);
        let spy = |t: &CassetteStats,
                   base: &CassetteStats,
                   tuning: assembly::Tuning,
                   by: &dyn Fn(&str) -> bool| {
            let a = assembly::assemble_stats(t, base, tuning, by);
            *seen.borrow_mut() = Some(a.clone());
            a
        };
        let exit = run_with(v, &spy);
        (exit, seen.into_inner().expect("組み立てが走った"))
    }

    #[test]
    fn 崩れていない組み合わせでは自己検査は何も言わない() {
        // 自己検査は組み立てるたびに走り、崩れていたときだけ判定できないの理由になる。
        let dir = TempDir::new("self-check");
        let (p, b) = cassettes(&dir);
        let draft = dir.write("草稿.md", "これは、そうだ。\n");
        let (_, a) = review_seen(&args(&[&draft, "--cassette", &p, "--baseline", &b]));
        let built = a.outcome.expect("組み立ては通る");
        assert!(built.self_check.is_some(), "走っている");
        assert_eq!(pair::self_check_failure(built.self_check.as_ref()), None);
    }

    #[test]
    fn 短い草稿は判定できないで返る() {
        let dir = TempDir::new("review-short");
        let (p, b) = cassettes(&dir);
        let draft = dir.write("草稿.md", "これは、そうだ、と思う。\n");
        for extra in [&[][..], &["--json"], &["--values"], &["--json", "--values"]] {
            let mut v = args(&["review", &draft, "--cassette", &p, "--baseline", &b]);
            v.extend(extra.iter().map(|s| (*s).to_owned()));
            assert_eq!(crate::run(&v), Exit::Unknown, "{extra:?}");
        }
    }

    #[test]
    fn 草稿を並べれば_1_本でも判定できないなら_2_で終わる() {
        let dir = TempDir::new("review-many");
        let (p, b) = cassettes(&dir);
        let one = dir.write("草稿1.md", "これは、そうだ、と思う。\n");
        let two = dir.write("草稿2.html", "<p>あれは、ちがう、と思う。</p>\n");
        for extra in [&[][..], &["--json"], &["--json", "--values"]] {
            let mut v = args(&["review", &one, &two, "--cassette", &p, "--baseline", &b]);
            v.extend(extra.iter().map(|s| (*s).to_owned()));
            assert_eq!(crate::run(&v), Exit::Unknown, "{extra:?}");
        }
    }

    #[test]
    fn 並べた草稿の終了コードは通らないを先に見る() {
        use Verdict::{Fail, Pass, Unknown};
        assert_eq!(combined_exit([Pass, Pass]), Exit::Pass);
        assert_eq!(combined_exit([Pass, Unknown]), Exit::Unknown);
        assert_eq!(combined_exit([Unknown, Fail, Pass]), Exit::Fail);
    }

    #[test]
    fn 表は和文の幅で揃える() {
        let header = ["草稿", "判定", "止まった段", "照合値", "基準との距離"].map(str::to_owned);
        let rows = [
            ["a.md", "通らない", "照合値", "0.412", "-0.903"].map(str::to_owned),
            ["draft-2.md", "判定できない", "基準との距離", "1.087", "—"].map(str::to_owned),
        ];
        let lines = aligned(&header, &rows);
        assert_eq!(lines.len(), 3);
        // 数の列の終わりが揃う。
        let ends: Vec<usize> = lines
            .iter()
            .map(|l| console::measure_text_width(l))
            .collect();
        assert!(ends.iter().all(|e| *e == ends[0]), "{lines:#?}");
        assert!(
            lines[2].starts_with("draft-2.md  判定できない  基準との距離"),
            "{lines:#?}"
        );
    }

    #[test]
    fn 散らばりは_2_本から出る() {
        assert_eq!(spread_of(&[1.0]), None);
        assert_eq!(spread_of(&[1.0, 3.0, 2.0]), Some(2.0));
        assert_eq!(sd_of(&[1.0]), None);
        let sd = sd_of(&[1.0, 3.0]).unwrap();
        assert!((sd - std::f64::consts::SQRT_2).abs() < 1e-12, "{sd}");
    }

    #[test]
    fn 指紋の合わないカセットでは検めない() {
        // 判定できないではなく使う前の問題である。
        let dir = TempDir::new("review-mismatch");
        let (p, b) = cassettes(&dir);
        let mut c = cassettes::read_file(&b).unwrap();
        c.inputs.compressor.version = "0.0.0".into();
        kakiburi_cassette::save::save(&b, &c, Some(c.generation)).unwrap();
        let draft = dir.write("草稿.md", "これは、そうだ。\n");
        assert_eq!(
            crate::run(&args(&[
                "review",
                &draft,
                "--cassette",
                &p,
                "--baseline",
                &b
            ])),
            Exit::FingerprintMismatch
        );
    }

    #[test]
    fn 壊れたカセットでは検めない() {
        let dir = TempDir::new("review-broken");
        let broken = dir.write("壊れた.kb", "PK");
        let draft = dir.write("草稿.md", "これは、そうだ。\n");
        assert_eq!(
            crate::run(&args(&["review", &draft, "--cassette", &broken])),
            Exit::Unreadable
        );
    }

    #[test]
    fn 知らない拡張子の草稿は使い方の誤りである() {
        let dir = TempDir::new("review-ext");
        let draft = dir.write("草稿.txt", "これは、そうだ。\n");
        let ok = dir.write("草稿.md", "これは、そうだ。\n");
        assert_eq!(
            crate::run(&args(&["review", &draft, "--cassette", "無い.kb"])),
            Exit::Usage
        );
        assert_eq!(
            crate::run(&args(&["review", &ok, &draft, "--cassette", "無い.kb"])),
            Exit::Usage,
            "並べた中に 1 本でもあれば、カセットを読む前に断る"
        );
    }

    #[test]
    fn 素材を消しても検められる() {
        // 検めが本文に頼っていないこと。 素材のフォルダを消してから検める。
        let dir = TempDir::new("review-no-source");
        let (p, b) = cassettes(&dir);
        std::fs::remove_dir_all(dir.join("本人")).unwrap();
        std::fs::remove_dir_all(dir.join("基準")).unwrap();
        let draft = dir.write("草稿.md", "これは、そうだ。\n");
        assert_eq!(
            crate::run(&args(&[
                "review",
                &draft,
                "--cassette",
                &p,
                "--baseline",
                &b
            ])),
            Exit::Unknown
        );
    }

    #[test]
    fn 基準を省けば同梱の基準で検める() {
        let dir = TempDir::new("review-bundled");
        let person = fixture::write_corpus(&dir, "本人", false);
        let p = dir.join("本人.kb");
        assert_eq!(
            crate::run(&args(&["cassette", "build", &person, "-o", &p])),
            Exit::Pass
        );
        let draft = dir.write("草稿.md", "これは、そうだ。\n");
        let got = crate::run(&args(&["review", &draft, "--cassette", &p]));
        assert!(got.code() < 64, "使う前の問題にならない: {got:?}");
    }

    #[test]
    fn 無効にしたものは並べて見せる() {
        let mut t = Tuning::default();
        t.mute
            .get_mut(&MuteKind::Metric)
            .unwrap()
            .insert("強調".into());
        t.mute_kinds.insert(MuteKind::BaselineKata);
        t.register = Some(Register::Plain);
        let lines = muted_lines(&t);
        assert!(lines.iter().any(|l| l.contains("強調")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("基準の型")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("plain")), "{lines:?}");
        assert!(muted_lines(&Tuning::default()).is_empty());
    }

    #[test]
    fn 繰り返しは長いほうを先に採り短い一部を重ねない() {
        let a = kakiburi_metrics::morph::Analyzed::of(
            &paragraph(&"ことができます。".repeat(4)).prose(),
            &fixture::Chars,
        )
        .ok();
        let got = repeated_in_draft(a.as_ref());
        for (i, (x, _)) in got.iter().enumerate() {
            for (y, _) in &got[i + 1..] {
                assert!(!x.contains(y.as_str()), "{x} と {y} が重なっている");
            }
        }
    }
}
