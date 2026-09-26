//! いまの道具が出す指紋の材料と、較正の設定。
//!
//! 材料はすべて渡さないと[指紋](kakiburi_cassette::Fingerprint)を組み立てられない。
//! 混ぜ忘れは型が止める。

use std::collections::BTreeMap;

use kakiburi_cassette::{Cassette, Inputs, Normalization, Tool};

use crate::analyzer;
use crate::remedies::FromDefinitions;

/// いまの道具の材料。読んだ取り込み元の種類だけは呼ぶ側が渡す。
///
/// 取り込み元はカセットが何を読んだかであって、道具ではない。 作るときに
/// 読んだものを入れ、照らすときには見ない（[`Fingerprint::differences`](kakiburi_cassette::Fingerprint::differences)）。
#[must_use]
pub fn inputs(defs: &FromDefinitions, sources: Vec<String>) -> Inputs {
    Inputs {
        // 本数を指紋にしない。 同じ本数のまま数え方・除外・直し方を変えれば、
        // 値の意味が変わったのに指紋が動かず、古い統計値が使い回される。
        metric_definitions: defs.digest(),
        unit_definitions: format!(
            "kakiburi-doc {} / Unicode {}",
            env!("CARGO_PKG_VERSION"),
            kakiburi_doc::text::UNICODE_VERSION,
        ),
        morphology: analyzer::tool(),
        // まだ使わないものも、使わないと書いて渡す。
        dependency: Tool::unused(),
        compressor: analyzer::compressor(),
        external_tables: external_tables(),
        normalization: Normalization {
            sources,
            implementation: "kakiburi-normalize".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            mapping: normalization_mapping(),
        },
        measurement: measurement(),
    }
}

/// カセットの指紋を、いまの道具と照らす。合わなければ何が違うかを言う。
///
/// # Errors
///
/// 合わない材料の名前を返す。
pub fn check(c: &Cassette, defs: &FromDefinitions) -> Result<(), Vec<String>> {
    let diff = c.fingerprint().differences(&inputs(defs, Vec::new()));
    if diff.is_empty() {
        Ok(())
    } else {
        Err(diff)
    }
}

/// 窓・下限・言い回しの長さ・語のまとめ方の線。指標の側と目盛りの側が自分の分を返す。
fn measurement() -> BTreeMap<String, String> {
    kakiburi_metrics::measurement_settings()
        .into_iter()
        .chain(kakiburi_scale::stats::measurement_settings())
        .map(|(k, v)| (k.to_owned(), v))
        .collect()
}

/// 外部の表の版。名前だけでは足りない。
///
/// 版が上がれば区画や推奨列が増え、同じ本文から違う値が出る。
fn external_tables() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    out.insert(
        "Unicode".to_owned(),
        kakiburi_doc::text::UNICODE_VERSION.to_owned(),
    );
    // 暫定の表も、暫定と書いて残す。 正規の表に替えたら値が変わる。
    out.insert(
        "絵文字の表".to_owned(),
        kakiburi_metrics::symbol::EMOJI_RANGES_VERSION.to_owned(),
    );
    // 使わない表も、使わないと書いて残す。 空にすると、あとで足したときに
    // 「もともと無かった」のか「混ぜ忘れた」のかが分からない。
    out.insert("語の文体値の表".to_owned(), "使わない".to_owned());
    out.insert("文末表現の辞書".to_owned(), "使わない".to_owned());
    out
}

/// 適用した対応表。版だけでは足りない。
///
/// 升目の中身を変えても版を上げ忘れれば、指紋が同じまま別の木が出る。
fn normalization_mapping() -> BTreeMap<String, String> {
    kakiburi_normalize::Source::all()
        .into_iter()
        .map(|s| (s.name().to_owned(), s.mapping_digest()))
        .collect()
}

/// 目盛りと判定を決める較正の設定と閾値。判定と一緒に平文で出す。
///
/// 各クレートが自分の分を返す。 ここで定数を並べ直すと、定数を足したときに
/// 並べ忘れてもエラーにならない。
///
/// 指紋には入れない（[較正の設定](../../../docs/spec/200-extract.md#較正の設定は判定と一緒に平文で出す)）。
#[must_use]
pub fn calibration_settings() -> BTreeMap<String, String> {
    kakiburi_scale::settings()
        .into_iter()
        .chain(kakiburi_review::settings())
        .map(|(k, v)| (k.to_owned(), v))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kakiburi_cassette::{Stats, Tuning};

    fn cassette() -> Cassette {
        let defs = FromDefinitions::load();
        Cassette::new(
            "試験",
            inputs(&defs, vec!["markdown".into()]),
            Stats::default(),
            Tuning::default(),
        )
    }

    #[test]
    fn 作った直後の指紋はいまの道具と合う() {
        // 合わなければ 1 度も使えないカセットが出来上がる。
        assert_eq!(check(&cassette(), &FromDefinitions::load()), Ok(()));
    }

    #[test]
    fn 解析器の版が違えば形態素解析器を名指して合わない() {
        let mut c = cassette();
        c.inputs.morphology.version = "0.0".into();
        assert_eq!(
            check(&c, &FromDefinitions::load()),
            Err(vec!["形態素解析器".to_owned()])
        );
    }

    #[test]
    fn 測り方の設定が違えば設定の名前を名指して合わない() {
        let mut c = cassette();
        c.inputs
            .measurement
            .insert("metrics::floor::TOKENS".into(), "1".into());
        assert_eq!(
            check(&c, &FromDefinitions::load()),
            Err(vec!["測り方の設定: metrics::floor::TOKENS".to_owned()])
        );
    }

    #[test]
    fn 較正の設定は指紋の材料に入らない() {
        // 入れれば、閾値を 1 つ動かしただけで人からもらったカセットが全部使えなくなる。
        let c = cassette();
        for name in calibration_settings().keys() {
            assert!(
                !c.inputs.measurement.contains_key(name),
                "{name} が指紋に入っている"
            );
        }
    }

    #[test]
    fn 測り方の設定は窓と下限と言い回しと語のまとめ方を覆う() {
        let m = measurement();
        for name in [
            "metrics::humanness::WINDOW",
            "metrics::floor::JAPANESE_CHARS",
            "scale::stats::KATA_N",
            "metrics::lexicon::BOUND",
        ] {
            assert!(m.contains_key(name), "{name} が無い");
        }
    }
}
