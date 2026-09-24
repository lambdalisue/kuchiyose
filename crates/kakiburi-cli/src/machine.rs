//! 道具向けの出口。人向けの表示は変えない。
//!
//! この道具は素材を集めて回す性質上、スクリプトや LLM から叩かれる回数のほうが
//! 多くなる。 人向けの出力は整形されているが、そこから値を取ろうとすると
//! ラベルの文言と桁揃えに依存した切り出しになる——文言を変えた瞬間に黙って
//! 壊れる。
//!
//! `null` と `0` を分ける。 人向け出力の `—` と `0.000` の区別を、そのまま
//! 写すだけである。`why` を落とさない——落とせば、JSON にしたとたんに
//! 「測っていない」が消える。
//!
//! 依存は増やさない。 書き出しだけなら
//! [`kakiburi_cassette::json`] にある。

use kakiburi_cassette::json::Value;
use kakiburi_metrics::Measured;

/// 1 指標の値。測れなかったら `null` と理由。
#[must_use]
pub fn metric(name: &str, m: Measured) -> Value {
    let mut pairs = vec![("name".to_owned(), Value::s(name))];
    match m.unmeasured() {
        None => pairs.push((
            "value".to_owned(),
            Value::Number(m.value().unwrap_or_default()),
        )),
        Some(u) => {
            // 0 を入れない。 測っていないことと 0 だったことは違う。
            pairs.push(("value".to_owned(), Value::Null));
            pairs.push(("why".to_owned(), Value::s(u.name())));
        }
    }
    Value::obj(pairs)
}

/// 指標の並び。
#[must_use]
pub fn metrics(all: &[(String, Measured)]) -> Value {
    Value::Array(all.iter().map(|(n, m)| metric(n, *m)).collect())
}

/// 文書の形。
#[must_use]
pub fn structure(doc: &kakiburi_doc::Document) -> Value {
    #[allow(clippy::cast_precision_loss)]
    let n = |v: usize| Value::Number(v as f64);
    Value::obj([
        ("paragraphs".to_owned(), n(doc.paragraphs().len())),
        ("sentences".to_owned(), n(doc.sentences().len())),
        ("sections".to_owned(), n(doc.sections().len())),
        ("items".to_owned(), n(doc.items().len())),
    ])
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 測れていない指標は_null_と理由になる() {
        // 0 として出せば、測っていないことと 0 だったことが同じ顔で並ぶ。
        let v = metric("三点リーダの字数", Measured::BelowFloor);
        let text = v.write();
        assert!(text.contains("\"value\":null"), "{text}");
        assert!(text.contains("\"why\""), "{text}");
    }

    #[test]
    fn 測れた指標は生の値が出る() {
        // 人向けの表示は 3 桁に丸めるが、道具向けは丸めない。 丸めた値を
        // 読み戻して比べれば、直したのに動いていないことが見えなくなる。
        let v = metric("全角括弧", Measured::Value(3.944_123));
        let text = v.write();
        let back = kakiburi_cassette::json::parse(&text).expect("読める");
        assert_eq!(back.get("value").and_then(Value::as_f64), Some(3.944_123));
        assert!(back.get("why").is_none(), "{text}");
    }
}
