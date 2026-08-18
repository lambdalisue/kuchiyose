//! 目盛りを派生物として書き出し、読み戻す。
//!
//! <strong>ここが繋ぎ目である。</strong> 入れ物はカセット、組み立ては目盛り、繋ぐのはこちら——
//! 検めが目盛りを作り直せる経路を作らないために、この 3 つを分けている。
//!
//! <strong>読み戻したものが元と一致することを試験が確かめる。</strong> 一致しなければ、
//! 保存したカセットで測った値は、作ったときの値と違う。

use kakiburi_cassette::json::Value;
use kakiburi_scale::band::{Band, Ends};
use kakiburi_scale::calibrate::{Calibration, Weights};
use kakiburi_scale::humanness::HumannessScale;
use kakiburi_scale::vocabulary::{Frozen, FrozenSet};
use kakiburi_scale::Scale;

/// 数の並び。
fn numbers(v: &[f64]) -> Value {
    Value::Array(v.iter().copied().map(Value::Number).collect())
}

/// 文字列の並び。
fn strings(v: &[String]) -> Value {
    Value::Array(v.iter().map(Value::s).collect())
}

fn read_numbers(v: Option<&Value>) -> Option<Vec<f64>> {
    v?.as_array()?.iter().map(Value::as_f64).collect()
}

fn read_strings(v: Option<&Value>) -> Option<Vec<String>> {
    Some(
        v?.as_array()?
            .iter()
            .map(|x| x.as_str().unwrap_or_default().to_owned())
            .collect(),
    )
}

fn weights(w: &Weights) -> Value {
    Value::obj([
        ("切片".to_owned(), Value::Number(w.intercept())),
        ("傾き".to_owned(), numbers(w.slopes())),
    ])
}

fn read_weights(v: &Value) -> Option<Weights> {
    Some(Weights::restore(
        v.get("切片")?.as_f64()?,
        read_numbers(v.get("傾き"))?,
    ))
}

fn frozen(f: &Frozen) -> Value {
    Value::obj([
        ("次元".to_owned(), strings(f.dims())),
        ("平均".to_owned(), numbers(f.mean())),
        ("標準偏差".to_owned(), numbers(f.sd())),
    ])
}

fn read_frozen(v: &Value) -> Option<Frozen> {
    Frozen::restore(
        read_strings(v.get("次元"))?,
        read_numbers(v.get("平均"))?,
        read_numbers(v.get("標準偏差"))?,
    )
}

fn ends(e: Ends) -> Value {
    Value::obj([
        ("下端".to_owned(), Value::Number(e.low)),
        ("上端".to_owned(), Value::Number(e.high)),
    ])
}

fn read_ends(v: &Value) -> Option<Ends> {
    Some(Ends {
        low: v.get("下端")?.as_f64()?,
        high: v.get("上端")?.as_f64()?,
    })
}

fn band(b: Band) -> Value {
    Value::obj([
        ("天井".to_owned(), ends(b.ceiling)),
        ("床".to_owned(), ends(b.floor)),
    ])
}

fn read_band(v: &Value) -> Option<Band> {
    Some(Band {
        ceiling: read_ends(v.get("天井")?)?,
        floor: read_ends(v.get("床")?)?,
    })
}

/// 書き出す。
#[must_use]
pub fn write(s: &Scale) -> String {
    let frozen_v = Value::Array(
        s.frozen
            .iter()
            .map(|(name, set)| {
                Value::obj([
                    ("系統".to_owned(), Value::s(name)),
                    (
                        "部分".to_owned(),
                        Value::Array(set.parts().iter().map(frozen).collect()),
                    ),
                ])
            })
            .collect(),
    );
    let calibration = Value::obj([
        ("系統".to_owned(), strings(s.calibration.systems())),
        (
            "系統ごと".to_owned(),
            Value::Array(s.calibration.per_system().iter().map(weights).collect()),
        ),
        ("合算".to_owned(), weights(s.calibration.fusion())),
    ]);
    let humanness = Value::obj([
        (
            "次元ごと".to_owned(),
            Value::Array(s.humanness.per_dim().iter().map(weights).collect()),
        ),
        ("合算".to_owned(), weights(s.humanness.fusion())),
    ]);
    Value::obj([
        ("語彙".to_owned(), frozen_v),
        ("較正".to_owned(), calibration),
        ("帯".to_owned(), band(s.band)),
        ("相手集合".to_owned(), strings(&s.partners)),
        ("人らしさ".to_owned(), humanness),
        ("人らしさの帯".to_owned(), band(s.humanness_band)),
    ])
    .write()
}

/// 読み戻す。<strong>1 つでも欠けたら組み立てない。</strong>
///
/// 半端に組み立てれば、次元の意味がずれたまま距離を取ることになる。
#[must_use]
pub fn read(text: &str) -> Option<Scale> {
    let v = kakiburi_cassette::json::parse(text).ok()?;
    let mut frozen = Vec::new();
    for e in v.get("語彙")?.as_array()? {
        let name = e.get("系統")?.as_str()?.to_owned();
        let parts: Option<Vec<Frozen>> =
            e.get("部分")?.as_array()?.iter().map(read_frozen).collect();
        frozen.push((name, FrozenSet::from_parts(parts?)));
    }
    let c = v.get("較正")?;
    let per_system: Option<Vec<Weights>> = c
        .get("系統ごと")?
        .as_array()?
        .iter()
        .map(read_weights)
        .collect();
    let calibration = Calibration::restore(
        read_strings(c.get("系統"))?,
        per_system?,
        read_weights(c.get("合算")?)?,
    )?;
    let h = v.get("人らしさ")?;
    let per_dim: Option<Vec<Weights>> = h
        .get("次元ごと")?
        .as_array()?
        .iter()
        .map(read_weights)
        .collect();
    let humanness = HumannessScale::restore(per_dim?, read_weights(h.get("合算")?)?)?;
    Some(Scale {
        frozen,
        calibration,
        band: read_band(v.get("帯")?)?,
        partners: read_strings(v.get("相手集合"))?,
        humanness,
        humanness_band: read_band(v.get("人らしさの帯")?)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;

    #[test]
    fn 書き出して読み戻すと同じものになる() {
        // 一致しなければ、保存したカセットで測った値は作ったときの値と違う。
        let s = fixture::scale();
        let text = write(&s);
        let back = read(&text).expect("読み戻せる");
        assert_eq!(back, s);
    }

    #[test]
    fn 欠けていれば組み立てない() {
        // 半端に組み立てれば、次元の意味がずれたまま距離を取る。
        let text = write(&fixture::scale());
        let broken = text.replace("\"帯\"", "\"おび\"");
        assert!(read(&broken).is_none());
    }

    #[test]
    fn 読めない文字列は組み立てない() {
        assert!(read("{").is_none());
        assert!(read("{}").is_none());
    }
}
