//! 幅と出現割合、効くかの判定を派生物として書き出し、読み戻す。
//!
//! **検めはこれを読むだけである。** 幅も出現割合も効くかの判定も、検める時点で
//! 作り直さない——作り直せるなら、検める文書を見てから作り直す経路が書ける。
//!
//! **2 つのファイルに分ける。** `spread.json` が幅と出現割合、`effective.json` が
//! 効くかの判定である（[カセットの構造](../../../docs/design/100-cassette.md#derived--派生物)）。
//! 同じ値を両方に置かない——置けば、食い違ったときにどちらが正しいかを言えない。

use kakiburi_cassette::json::Value;
use kakiburi_scale::{Basis, Effective};

/// 幅と出現割合を書き出す。
#[must_use]
pub fn write_spread(rows: &[Effective]) -> String {
    Value::obj([(
        "指標".to_owned(),
        Value::Array(
            rows.iter()
                .map(|e| {
                    Value::obj([
                        ("名前".to_owned(), Value::s(&e.name)),
                        ("下端".to_owned(), Value::Number(e.low)),
                        ("上端".to_owned(), Value::Number(e.high)),
                        ("測れた単位".to_owned(), Value::Number(count(e.units))),
                        ("出現割合".to_owned(), Value::Number(e.rate)),
                    ])
                })
                .collect(),
        ),
    )])
    .write()
}

/// 効くかの判定を書き出す。
#[must_use]
pub fn write_effective(rows: &[Effective]) -> String {
    Value::obj([(
        "指標".to_owned(),
        Value::Array(
            rows.iter()
                .map(|e| {
                    // 比べた相手も残す。 本人の側は幅も出現割合も残っているが、
                    // 基準が残らなければ、判定が変わったときに「基準が変わったのか、
                    // 閾値を変えたのか」を言えない。
                    let mut fields = vec![
                        ("名前".to_owned(), Value::s(&e.name)),
                        ("幅が狭い".to_owned(), Value::Bool(e.narrow)),
                        ("基準から離れている".to_owned(), Value::Bool(e.distant)),
                    ];
                    match e.basis {
                        Basis::Spread { low, high } => fields.extend([
                            ("見方".to_owned(), Value::s("幅")),
                            ("基準の下端".to_owned(), Value::Number(low)),
                            ("基準の上端".to_owned(), Value::Number(high)),
                        ]),
                        Basis::Appearance { rate } => fields.extend([
                            ("見方".to_owned(), Value::s("出現割合")),
                            ("基準の出現割合".to_owned(), Value::Number(rate)),
                        ]),
                    }
                    Value::obj(fields)
                })
                .collect(),
        ),
    )])
    .write()
}

#[allow(clippy::cast_precision_loss)]
fn count(n: usize) -> f64 {
    n as f64
}

/// 2 つを読み戻して 1 つにする。
///
/// **片方にしか無い指標があれば読まない。** 幅だけあって判定が無い指標を通すと、
/// 「効かないと判定された」のか「判定が古い」のかが分からなくなる。
#[must_use]
pub fn read(spread: &str, effective: &str) -> Option<Vec<Effective>> {
    let s = kakiburi_cassette::json::parse(spread).ok()?;
    let e = kakiburi_cassette::json::parse(effective).ok()?;
    let judged = e.get("指標")?.as_array()?;
    let rows: Option<Vec<Effective>> = s
        .get("指標")?
        .as_array()?
        .iter()
        .map(|x| {
            let name = x.get("名前")?.as_str()?.to_owned();
            let j = judged
                .iter()
                .find(|y| y.get("名前").and_then(Value::as_str) == Some(&name))?;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            Some(Effective {
                low: x.get("下端")?.as_f64()?,
                high: x.get("上端")?.as_f64()?,
                units: x.get("測れた単位")?.as_f64()? as usize,
                rate: x.get("出現割合")?.as_f64()?,
                narrow: j.get("幅が狭い")?.as_bool()?,
                distant: j.get("基準から離れている")?.as_bool()?,
                basis: match j.get("見方")?.as_str()? {
                    "幅" => Basis::Spread {
                        low: j.get("基準の下端")?.as_f64()?,
                        high: j.get("基準の上端")?.as_f64()?,
                    },
                    "出現割合" => Basis::Appearance {
                        rate: j.get("基準の出現割合")?.as_f64()?,
                    },
                    // 知らない見方は読まない。 判定の根拠を読めないまま前に出せば、
                    // どの規則で選ばれたのかを誰も言えなくなる。
                    _ => return None,
                },
                name,
            })
        })
        .collect();
    let rows = rows?;
    // **数が合わなければ読まない。** 判定の側にだけある指標は、幅の出どころが無い。
    (rows.len() == judged.len()).then_some(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Effective> {
        vec![
            Effective {
                name: "三点リーダ".into(),
                low: 0.5,
                high: 2.0,
                units: 10,
                rate: 1.0,
                narrow: true,
                distant: true,
                basis: Basis::Spread {
                    low: 0.0,
                    high: 5.0,
                },
            },
            Effective {
                name: "絵文字".into(),
                low: 0.0,
                high: 0.0,
                units: 8,
                rate: 0.0,
                narrow: true,
                distant: false,
                basis: Basis::Appearance { rate: 0.75 },
            },
        ]
    }

    #[test]
    fn 書き出して読み戻すと同じものになる() {
        // 一致しなければ、検めが読む判定は build が出した判定ではない。
        let rows = sample();
        let got = read(&write_spread(&rows), &write_effective(&rows));
        assert_eq!(got, Some(rows));
    }

    #[test]
    fn 幅と判定は別のファイルに分ける() {
        // 同じ値を両方に置けば、食い違ったときにどちらが正しいかを言えない。
        let rows = sample();
        let spread = write_spread(&rows);
        let effective = write_effective(&rows);
        // **基準の側は別の値である。** 本人の下端と基準の下端を取り違えない。
        assert!(spread.contains("\"下端\"") && !spread.contains("幅が狭い"));
        assert!(effective.contains("幅が狭い") && !effective.contains("\"下端\""));
    }

    #[test]
    fn 片方にしか無ければ読まない() {
        let rows = sample();
        let only_one = write_effective(&rows[..1]);
        assert_eq!(read(&write_spread(&rows), &only_one), None);
    }

    #[test]
    fn 知らない見方は読まない() {
        // どの規則で選ばれたのかを言えないものを、前に出さない。
        let rows = sample();
        let broken = write_effective(&rows).replace("\"幅\"", "\"未知の見方\"");
        assert_eq!(read(&write_spread(&rows), &broken), None);
    }

    #[test]
    fn 欠けていれば読まない() {
        let rows = sample();
        assert_eq!(read("{}", &write_effective(&rows)), None);
        assert_eq!(read(&write_spread(&rows), "壊れている"), None);
    }
}
