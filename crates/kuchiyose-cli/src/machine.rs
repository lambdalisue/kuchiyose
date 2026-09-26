//! 道具向けの出口。人向けの表示は変えない。
//!
//! この道具は素材を集めて回す性質上、スクリプトや LLM から叩かれる回数のほうが
//! 多くなる。 人向けの出力は整形されているが、そこから値を取ろうとすると
//! ラベルの文言と桁揃えに依存した切り出しになる——文言を変えた瞬間に黙って
//! 壊れる。
//!
//! `null` と `0` を分ける。 人向け出力の `—` と `0.000` の区別を、そのまま
//! 写すだけである。
//!
//! 依存は増やさない。 書き出しだけなら
//! [`kuchiyose_katashiro::json`] にある。

use kuchiyose_katashiro::json::Value;

/// `f64` か `null`。
#[must_use]
pub fn number(v: Option<f64>) -> Value {
    v.map_or(Value::Null, Value::Number)
}

/// 文字列の並び。
#[must_use]
pub fn strings(v: &[String]) -> Value {
    Value::Array(v.iter().map(Value::s).collect())
}

/// 帯。天井と床の端と、分かれているか。
#[must_use]
pub fn band(b: kuchiyose_scale::Band) -> Value {
    let ends = |e: kuchiyose_scale::Ends| {
        Value::obj([
            ("low".to_owned(), Value::Number(e.low)),
            ("high".to_owned(), Value::Number(e.high)),
        ])
    };
    Value::obj([
        ("ceiling".to_owned(), ends(b.ceiling)),
        ("floor".to_owned(), ends(b.floor)),
        ("separated".to_owned(), Value::Bool(b.separated())),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 出ていない値は_0_ではなく_null_になる() {
        // 0 として出せば、測っていないことと 0 だったことが同じ顔で並ぶ。
        assert_eq!(number(None).write(), "null");
        assert_eq!(number(Some(0.0)).write(), "0");
    }

    #[test]
    fn 値は丸めずに出る() {
        // 人向けの表示は 3 桁に丸めるが、道具向けは丸めない。
        let back =
            kuchiyose_katashiro::json::parse(&number(Some(3.944_123)).write()).expect("読める");
        assert_eq!(back.as_f64(), Some(3.944_123));
    }
}
