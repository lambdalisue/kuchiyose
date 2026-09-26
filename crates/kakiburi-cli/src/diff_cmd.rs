//! `kakiburi cassette diff`。2 つのカセットがどこで違うかを出す
//! （[cassette diff](../../../docs/design/200-command.md#cassette-diff)）。
//!
//! 1 つ目を本人の側、2 つ目を基準の側として、`review` と同じ手順で目盛りを組み立てる。
//! 本人のカセットで無効にしたものも外さずに出し、印を付ける——何が違うかを
//! 全部見るためのコマンドである。

use kakiburi_cassette::json::Value;
use kakiburi_cassette::{MuteKind, Tuning};
use kakiburi_scale::assembly::{self, Assembly, Built};
use kakiburi_scale::self_check::HIGHER_RATE_FLOOR;
use kakiburi_scale::{Band, Basis, Effective};

use crate::cassettes::{Opened, Origin};
use crate::exit::Exit;
use crate::pair::{self, Pair};
use crate::remedies::FromDefinitions;
use crate::tuning_cmd::phrase_id;
use crate::{environment, machine};

/// 2 つが見分けられるか。見分けられないなら、その理由。
///
/// 見分けられるのは、目盛りが組み立てられ、本人がいちばん高く出て、照合値の帯が
/// 分かれているときである。 基準との距離の帯は重なっても止めない——重なった帯は
/// 検めで判定できないとして出る、設計どおりの状態である。
pub fn apart(a: &Assembly) -> Result<(), Vec<String>> {
    let b = match &a.outcome {
        Ok(b) => b,
        Err(stopped) => return Err(vec![stopped.error.to_string()]),
    };
    let mut reasons = Vec::new();
    if let Some(r) = pair::self_check_failure(b.self_check.as_ref()) {
        reasons.push(r);
    }
    if !b.scale.band.separated() {
        reasons.push(format!(
            "照合値の帯が重なっている（{}）",
            band_text(b.scale.band)
        ));
    }
    if reasons.is_empty() {
        Ok(())
    } else {
        Err(reasons)
    }
}

/// 帯を 1 行にする。
fn band_text(b: Band) -> String {
    format!(
        "天井 {:.3}〜{:.3} / 床 {:.3}〜{:.3}",
        b.ceiling.low, b.ceiling.high, b.floor.low, b.floor.high
    )
}

/// `cassette diff`。
pub fn run(args: &[String]) -> Exit {
    let mut paths: Vec<&str> = Vec::new();
    let mut json = false;
    for a in args {
        match a.as_str() {
            "--json" => json = true,
            other if other.starts_with('-') => {
                eprintln!("知らない引数: {other}");
                return Exit::Usage;
            }
            other => paths.push(other),
        }
    }
    let [target, baseline] = paths.as_slice() else {
        eprintln!("カセットを 2 つ渡す。1 つ目が本人の側、2 つ目が基準の側");
        eprintln!("{}", crate::CASSETTE_DIFF);
        return Exit::Usage;
    };
    let defs = FromDefinitions::load();
    let pair = match pair::open(
        Origin::File((*target).to_owned()),
        Origin::File((*baseline).to_owned()),
        &defs,
    ) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let built = pair.assemble(&assembly::assemble_stats, &defs);
    let verdict = apart(&built);
    let exit = if verdict.is_ok() {
        Exit::Pass
    } else {
        Exit::Unknown
    };
    if json {
        println!("{}", to_json(&pair, &built, &verdict, &defs).write());
    } else {
        print(&pair, &built, &verdict, &defs);
    }
    exit
}

/// 無効にしているかの印。
fn mark(t: &Tuning, kind: MuteKind, text: &str) -> &'static str {
    if t.is_muted(kind, text) {
        "（無効にしている）"
    } else {
        ""
    }
}

/// 効くかの根拠を 1 行にする。
fn effective_line(e: &Effective, t: &Tuning, defs: &FromDefinitions) -> String {
    let basis = match e.basis {
        Basis::Spread { low, high } => format!(
            "本人 {:.3}〜{:.3} / 基準 {low:.3}〜{high:.3}",
            e.low, e.high
        ),
        Basis::Appearance { rate } => format!(
            "使った割合 本人 {:.0}% / 基準 {:.0}%",
            e.rate * 100.0,
            rate * 100.0
        ),
    };
    let layer = if defs.is_layer_three(&e.name) {
        "（層 3。判定に使わない）"
    } else {
        ""
    };
    format!(
        "{}: {basis}{layer}{}",
        e.name,
        mark(t, MuteKind::Metric, &e.name)
    )
}

/// 比べる相手が本人の幅でない検査は並べない。
fn compared<'a>(b: &'a Built, defs: &'a FromDefinitions) -> impl Iterator<Item = &'a Effective> {
    b.effective.iter().filter(|e| !defs.is_inspection(&e.name))
}

fn print(pair: &Pair, built: &Assembly, verdict: &Result<(), Vec<String>>, defs: &FromDefinitions) {
    let t = &pair.target.cassette.tuning;
    println!("{}", pair.target.name_line("本人"));
    println!("{}", pair.baseline.name_line("基準"));
    println!("{}", pair::register_note(&built.register));
    if let Some(note) = pair::length_window_note(built) {
        println!("{note}");
    }
    println!();
    match verdict {
        Ok(()) => println!("見分けられる"),
        Err(reasons) => {
            println!("見分けられない");
            for r in reasons {
                println!("  理由: {r}");
            }
        }
    }
    if let Ok(b) = &built.outcome {
        let s = &b.scale;
        println!("照合値の帯: {}", band_text(s.band));
        println!("基準との距離の帯: {}", band_text(s.humanness_band));
        if let Some(c) = &b.self_check {
            println!(
                "本人が高く出た対の割合: {:.3}（{} 対 / 下限 {HIGHER_RATE_FLOOR}）",
                c.rate, c.pairs
            );
        }
        println!();
        let works: Vec<String> = compared(b, defs)
            .filter(|e| e.works())
            .map(|e| effective_line(e, t, defs))
            .collect();
        let habits: Vec<String> = compared(b, defs)
            .filter(|e| e.narrow_only())
            .map(|e| effective_line(e, t, defs))
            .collect();
        section("効く指標", &works);
        section("本人の癖（幅は狭いが基準と離れていない）", &habits);
        let katas: Vec<String> = s
            .katas
            .iter()
            .map(|k| {
                let text = k.shown();
                format!(
                    "{} {text}（本人 {:.0}% / 基準 {:.0}%）{}",
                    phrase_id(MuteKind::Kata, &text),
                    k.rate * 100.0,
                    k.base * 100.0,
                    mark(t, MuteKind::Kata, &text)
                )
            })
            .collect();
        let machine_katas: Vec<String> = s
            .machine_katas
            .iter()
            .map(|k| {
                let text = k.shown();
                format!(
                    "{} {text}（基準 {:.0}% / 本人 {:.0}%）{}",
                    phrase_id(MuteKind::BaselineKata, &text),
                    k.rate * 100.0,
                    k.base * 100.0,
                    mark(t, MuteKind::BaselineKata, &text)
                )
            })
            .collect();
        let gois: Vec<String> = s
            .machine_gois
            .iter()
            .map(|g| {
                format!(
                    "{}（基準 {:.0}% / 本人 {:.0}%）{}",
                    g.text,
                    g.rate * 100.0,
                    g.base * 100.0,
                    mark(t, MuteKind::BaselineGoi, &g.text)
                )
            })
            .collect();
        section("型（本人だけが使う言い回し）", &katas);
        section("基準の型（基準だけが使う言い回し）", &machine_katas);
        section("基準の語（基準だけが使う語）", &gois);
    }
    println!();
    println!(
        "使った基準の文書: 文体と題材で選んだ {} 本、束ねた単位 {} 本（試した束ね方 {} 案）",
        built.picked.len(),
        built.bundles.len(),
        built.tried
    );
    for g in built.bundles.iter().filter(|g| g.len() > 1) {
        println!("  束: {}", g.join("+"));
    }
    if let Ok(b) = &built.outcome {
        let s = &b.scale.selection;
        println!("割り");
        println!("  本人の相手集合: {}", s.person_partners.join("、"));
        println!("  本人の測る分: {}", s.person_points.join("、"));
        println!("  基準の較正分: {}", s.baseline_partners.join("、"));
        println!("  基準の床の点: {}", s.baseline_points.join("、"));
    }
}

fn section(title: &str, lines: &[String]) {
    println!("{title} {} 本", lines.len());
    for l in lines {
        println!("  {l}");
    }
}

#[allow(clippy::too_many_lines)]
fn to_json(
    pair: &Pair,
    built: &Assembly,
    verdict: &Result<(), Vec<String>>,
    defs: &FromDefinitions,
) -> Value {
    let t = &pair.target.cassette.tuning;
    #[allow(clippy::cast_precision_loss)]
    let n = |v: usize| Value::Number(v as f64);
    let side = |o: &Opened| {
        Value::obj([
            ("cassette".to_owned(), Value::s(o.origin.label())),
            ("scene".to_owned(), Value::s(&o.cassette.scene)),
            (
                "content_hash".to_owned(),
                Value::s(o.cassette.stats.content_hash()),
            ),
        ])
    };
    let b = built.outcome.as_ref().ok();
    let kata = |kind: MuteKind, k: &kakiburi_scale::assemble::Kata| {
        let text = k.shown();
        Value::obj([
            ("id".to_owned(), Value::s(phrase_id(kind, &text))),
            ("text".to_owned(), Value::s(&text)),
            ("rate".to_owned(), Value::Number(k.rate)),
            ("base".to_owned(), Value::Number(k.base)),
            ("at".to_owned(), Value::Number(k.at)),
            ("muted".to_owned(), Value::Bool(t.is_muted(kind, &text))),
        ])
    };
    let effective = |e: &Effective| {
        let basis = match e.basis {
            Basis::Spread { low, high } => Value::obj([
                ("spread_low".to_owned(), Value::Number(low)),
                ("spread_high".to_owned(), Value::Number(high)),
            ]),
            Basis::Appearance { rate } => {
                Value::obj([("appearance_rate".to_owned(), Value::Number(rate))])
            }
        };
        Value::obj([
            ("name".to_owned(), Value::s(&e.name)),
            ("low".to_owned(), Value::Number(e.low)),
            ("high".to_owned(), Value::Number(e.high)),
            ("units".to_owned(), n(e.units)),
            ("rate".to_owned(), Value::Number(e.rate)),
            ("works".to_owned(), Value::Bool(e.works())),
            ("narrow_only".to_owned(), Value::Bool(e.narrow_only())),
            (
                "layer_three".to_owned(),
                Value::Bool(defs.is_layer_three(&e.name)),
            ),
            (
                "muted".to_owned(),
                Value::Bool(t.is_muted(MuteKind::Metric, &e.name)),
            ),
            ("baseline".to_owned(), basis),
        ])
    };
    Value::obj([
        ("target".to_owned(), side(&pair.target)),
        ("baseline".to_owned(), side(&pair.baseline)),
        (
            "register".to_owned(),
            Value::s(pair::register_note(&built.register)),
        ),
        ("distinguishable".to_owned(), Value::Bool(verdict.is_ok())),
        (
            "reasons".to_owned(),
            Value::Array(
                verdict
                    .as_ref()
                    .err()
                    .map(|r| r.iter().map(Value::s).collect())
                    .unwrap_or_default(),
            ),
        ),
        (
            "bands".to_owned(),
            b.map_or(Value::Null, |b| {
                Value::obj([
                    ("matching".to_owned(), machine::band(b.scale.band)),
                    (
                        "baseline_distance".to_owned(),
                        machine::band(b.scale.humanness_band),
                    ),
                ])
            }),
        ),
        (
            "self_check".to_owned(),
            b.and_then(|b| b.self_check.as_ref())
                .map_or(Value::Null, |c| {
                    Value::obj([
                        ("rate".to_owned(), Value::Number(c.rate)),
                        ("floor".to_owned(), Value::Number(HIGHER_RATE_FLOOR)),
                        ("pairs".to_owned(), n(c.pairs)),
                        ("passed".to_owned(), Value::Bool(c.passed())),
                        (
                            "inverted".to_owned(),
                            Value::Array(
                                c.inverted
                                    .iter()
                                    .map(|(p, pv, q, qv)| {
                                        Value::obj([
                                            ("person".to_owned(), Value::s(p)),
                                            ("person_value".to_owned(), Value::Number(*pv)),
                                            ("baseline".to_owned(), Value::s(q)),
                                            ("baseline_value".to_owned(), Value::Number(*qv)),
                                        ])
                                    })
                                    .collect(),
                            ),
                        ),
                    ])
                }),
        ),
        (
            "effective".to_owned(),
            Value::Array(
                b.map(|b| compared(b, defs).map(effective).collect())
                    .unwrap_or_default(),
            ),
        ),
        (
            "katas".to_owned(),
            Value::Array(
                b.map(|b| {
                    b.scale
                        .katas
                        .iter()
                        .map(|k| kata(MuteKind::Kata, k))
                        .collect()
                })
                .unwrap_or_default(),
            ),
        ),
        (
            "baseline_katas".to_owned(),
            Value::Array(
                b.map(|b| {
                    b.scale
                        .machine_katas
                        .iter()
                        .map(|k| kata(MuteKind::BaselineKata, k))
                        .collect()
                })
                .unwrap_or_default(),
            ),
        ),
        (
            "baseline_gois".to_owned(),
            Value::Array(
                b.map(|b| {
                    b.scale
                        .machine_gois
                        .iter()
                        .map(|g| {
                            Value::obj([
                                ("text".to_owned(), Value::s(&g.text)),
                                ("rate".to_owned(), Value::Number(g.rate)),
                                ("base".to_owned(), Value::Number(g.base)),
                                ("theirs".to_owned(), machine::strings(&g.theirs)),
                                (
                                    "muted".to_owned(),
                                    Value::Bool(t.is_muted(MuteKind::BaselineGoi, &g.text)),
                                ),
                            ])
                        })
                        .collect()
                })
                .unwrap_or_default(),
            ),
        ),
        (
            "assembly".to_owned(),
            Value::obj([
                ("picked".to_owned(), machine::strings(&built.picked)),
                ("length_window".to_owned(), pair::length_window_json(built)),
                (
                    "bundles".to_owned(),
                    Value::Array(built.bundles.iter().map(|g| machine::strings(g)).collect()),
                ),
                ("tried".to_owned(), n(built.tried)),
            ]),
        ),
        (
            "selection".to_owned(),
            b.map_or(Value::Null, |b| {
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
            }),
        ),
        (
            "settings".to_owned(),
            Value::obj(
                environment::calibration_settings()
                    .into_iter()
                    .map(|(k, v)| (k, Value::s(v))),
            ),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;
    use crate::testdir::TempDir;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

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
    fn 分かれる組み合わせは見分けられると言い_0_で終わる() {
        let dir = TempDir::new("diff");
        let (p, b) = cassettes(&dir);
        for extra in [&[][..], &["--json"]] {
            let mut v = args(&["cassette", "diff", &p, &b]);
            v.extend(extra.iter().map(|s| (*s).to_owned()));
            assert_eq!(crate::run(&v), Exit::Pass, "{extra:?}");
        }
    }

    #[test]
    fn 同じカセットどうしは見分けられないと言い_2_で終わる() {
        // 組み立てられないのは正常な状態である。 64 以上にしない。
        let dir = TempDir::new("diff-same");
        let (p, _) = cassettes(&dir);
        assert_eq!(
            crate::run(&args(&["cassette", "diff", &p, &p])),
            Exit::Unknown
        );
    }

    #[test]
    fn 組み立てられなければ止まった理由が見分けられない理由になる() {
        let dir = TempDir::new("diff-reason");
        let (p, _) = cassettes(&dir);
        let defs = FromDefinitions::load();
        let pair = pair::open(Origin::File(p.clone()), Origin::File(p.clone()), &defs).unwrap();
        let built = pair.assemble(&assembly::assemble_stats, &defs);
        let reasons = apart(&built).unwrap_err();
        assert!(!reasons.is_empty());
        assert!(reasons.iter().all(|r| !r.is_empty()));
    }

    #[test]
    fn カセットが_2_つでなければ使い方の誤りである() {
        let dir = TempDir::new("diff-usage");
        let (p, _) = cassettes(&dir);
        assert_eq!(crate::run(&args(&["cassette", "diff", &p])), Exit::Usage);
        assert_eq!(
            crate::run(&args(&["cassette", "diff", &p, &p, &p])),
            Exit::Usage
        );
        assert_eq!(
            crate::run(&args(&["cassette", "diff", &p, &dir.write("x.kb", "PK")])),
            Exit::Unreadable
        );
    }
}
