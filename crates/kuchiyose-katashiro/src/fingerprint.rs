//! 指紋。何で測ったかを残す。
//!
//! 仕様が「一部だけを混ぜない」と警告している。混ぜ忘れてもエラーにならず、
//! 古い値が黙って使われる。
//!
//! だから指紋を作る関数は、全部の材料を引数に取る。 1 つでも欠ければ
//! 組み立てられない——型がそれを強制する（[指紋は組み立てを型で守る](../../../docs/design/100-katashiro.md#指紋は組み立てを型で守る)）。
//!
//! 材料は 2 つに割れる。 道具の部分（[`Inputs`]）と、統計値の中身のハッシュである。
//! 照らすときは道具の部分だけを見る——中身が違うのは別の人の形代だからであって、
//! 並べてはいけない理由ではない。
//!
//! 較正の設定と調整は入れない。 どちらも統計値を変えないので、入れれば閾値を
//! 1 つ動かしただけで、人からもらった形代が全部使えなくなる。

use std::collections::{BTreeMap, BTreeSet};

use crate::sha256;

/// 指紋の材料のうち、道具の部分。すべての欄が必須である。
///
/// `Option` を持たない。 持たせれば、混ぜ忘れが `None` として通ってしまう。
/// まだ使わないものも、使わないと書いて渡す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inputs {
    /// 指標の定義そのもの。登録簿から。
    pub metric_definitions: String,
    /// 数える単位の定義。地の文に何が入るか、日本語の文字の範囲。
    ///
    /// 定義の版で表す。パッケージの版は測り方と無関係に上がるので使わない。
    pub unit_definitions: String,
    /// 形態素解析器の辞書と版。
    pub morphology: Tool,
    /// 係り受け解析器の辞書と版。保留中。使わないなら `Tool::unused`。
    pub dependency: Tool,
    /// 圧縮器と設定。
    pub compressor: Tool,
    /// 外部の表の版。語の文体値、文末表現の辞書、Unicode。
    pub external_tables: BTreeMap<String, String>,
    /// 取り込み元の種類・変換の実装と版・対応表。
    pub normalization: Normalization,
    /// 窓の大きさ・下限・言い回しの長さ・語のまとめ方の線。名前から値へ。
    ///
    /// 平文で持つ。 合わないときに、どの設定が変わったかを名指すためである。
    pub measurement: BTreeMap<String, String>,
}

/// 外部の道具。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    /// 名前。
    pub name: String,
    /// 版。
    pub version: String,
    /// 設定。辞書の名前や圧縮の水準。
    pub config: BTreeMap<String, String>,
}

impl Tool {
    /// 使わない道具。「未設定」と「使わない」を分ける。
    #[must_use]
    pub fn unused() -> Self {
        Self {
            name: "使わない".into(),
            version: String::new(),
            config: BTreeMap::new(),
        }
    }
}

/// 正規化の記録。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Normalization {
    /// 読んだ取り込み元の種類。
    pub sources: Vec<String>,
    /// 変換の実装。
    pub implementation: String,
    /// 変換の版。変換を変えたときに上げる版であって、パッケージの版ではない。
    pub version: String,
    /// 適用した対応表。
    pub mapping: BTreeMap<String, String>,
}

/// [測り方の設定](Inputs::measurement)が違うときの名前。
const MEASUREMENT: &str = "測り方の設定";

/// 指紋。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    /// 道具の部分。平文で残す——ハッシュだけでは、何が変わったかが分からない。
    pub inputs: Inputs,
    /// 統計値の中身のハッシュ。
    pub content_hash: String,
    digest: String,
}

impl Fingerprint {
    /// 組み立てる。材料をすべて受け取る。
    #[must_use]
    pub fn build(inputs: Inputs, content_hash: impl Into<String>) -> Self {
        let content_hash = content_hash.into();
        let mut h = sha256::Sha256::default();
        h.update(inputs.canonical());
        h.update("中身のハッシュ\t");
        h.update(&content_hash);
        let digest = format!("sha256:{}", h.hex());
        Self {
            inputs,
            content_hash,
            digest,
        }
    }

    /// 材料から作った文字列。
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// 道具の部分のどこが違うか。中身のハッシュは見ない。
    ///
    /// 読んだ取り込み元の種類も見ない。 それは形代が何を読んだかであって、
    /// 道具ではない——Markdown の素材から作った形代と HTML の素材から作った
    /// 形代は、同じ道具で並べられる。変換の実装・版・対応表は見る。
    ///
    /// 測り方の設定は名前まで言う。 「測り方の設定」とだけ言われても、どれが
    /// 変わったのかが分からない。
    #[must_use]
    pub fn differences(&self, other: &Inputs) -> Vec<String> {
        let (a, b) = (&self.inputs, other);
        let mut out = Vec::new();
        if a.metric_definitions != b.metric_definitions {
            out.push("指標の定義".to_owned());
        }
        if a.unit_definitions != b.unit_definitions {
            out.push("数える単位の定義".to_owned());
        }
        if a.morphology != b.morphology {
            out.push("形態素解析器".to_owned());
        }
        if a.dependency != b.dependency {
            out.push("係り受け解析器".to_owned());
        }
        if a.compressor != b.compressor {
            out.push("圧縮器".to_owned());
        }
        if a.external_tables != b.external_tables {
            out.push("外部の表".to_owned());
        }
        let (n, m) = (&a.normalization, &b.normalization);
        if (&n.implementation, &n.version, &n.mapping)
            != (&m.implementation, &m.version, &m.mapping)
        {
            out.push("正規化".to_owned());
        }
        let names: BTreeSet<&String> = a.measurement.keys().chain(b.measurement.keys()).collect();
        out.extend(
            names
                .into_iter()
                .filter(|k| a.measurement.get(*k) != b.measurement.get(*k))
                .map(|k| format!("{MEASUREMENT}: {k}")),
        );
        out
    }
}

impl Inputs {
    /// 決定的な文字列にする。並び順を固定するので、作り直しても同じものが出る。
    ///
    /// 可変長のものには長さを添える。 区切り文字で繋ぐと、値がその文字を
    /// 含んだときに別の材料が同じ文字列になる。
    fn canonical(&self) -> String {
        fn field(s: &mut String, label: &str, value: &str) {
            s.push_str(&format!("{label}\t{}:{value}\n", value.len()));
        }
        fn map(s: &mut String, label: &str, m: &BTreeMap<String, String>) {
            s.push_str(&format!("{label}\t{}\n", m.len()));
            for (k, v) in m {
                field(s, k, v);
            }
        }
        let mut s = String::new();
        field(&mut s, "指標の定義", &self.metric_definitions);
        field(&mut s, "単位の定義", &self.unit_definitions);
        for (label, t) in [
            ("形態素解析器", &self.morphology),
            ("係り受け解析器", &self.dependency),
            ("圧縮器", &self.compressor),
        ] {
            field(&mut s, label, &t.name);
            field(&mut s, "版", &t.version);
            map(&mut s, "設定", &t.config);
        }
        map(&mut s, "外部の表", &self.external_tables);
        let n = &self.normalization;
        s.push_str(&format!("取り込み元\t{}\n", n.sources.len()));
        for source in &n.sources {
            field(&mut s, "種類", source);
        }
        field(&mut s, "変換の実装", &n.implementation);
        field(&mut s, "変換の版", &n.version);
        map(&mut s, "対応表", &n.mapping);
        map(&mut s, MEASUREMENT, &self.measurement);
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> Inputs {
        Inputs {
            metric_definitions: "定義 51 本 fnv1a:0".into(),
            unit_definitions: "kuchiyose-doc 数える単位 1 / Unicode 15.1".into(),
            morphology: Tool {
                name: "Lindera".into(),
                version: "6.0".into(),
                config: [("辞書".to_owned(), "UniDic".to_owned())].into(),
            },
            dependency: Tool::unused(),
            compressor: Tool {
                name: "miniz_oxide".into(),
                version: "0.8.9".into(),
                config: [("水準".to_owned(), "6".to_owned())].into(),
            },
            external_tables: [("Unicode".to_owned(), "15.1".to_owned())].into(),
            normalization: Normalization {
                sources: vec!["markdown".into()],
                implementation: "kuchiyose-normalize".into(),
                version: "変換 1".into(),
                mapping: [("markdown".to_owned(), "表 1".to_owned())].into(),
            },
            measurement: [
                ("metrics::floor::TOKENS".to_owned(), "729".to_owned()),
                ("metrics::humanness::WINDOW".to_owned(), "729".to_owned()),
            ]
            .into(),
        }
    }

    const HASH: &str = "sha256:0000";

    #[test]
    fn 同じ材料からは同じ指紋が出る() {
        assert_eq!(
            Fingerprint::build(inputs(), HASH).digest(),
            Fingerprint::build(inputs(), HASH).digest()
        );
    }

    #[test]
    fn 指紋は_sha256_を名乗る() {
        let d = Fingerprint::build(inputs(), HASH);
        assert!(d.digest().starts_with("sha256:"), "{}", d.digest());
        assert_eq!(d.digest().len(), "sha256:".len() + 64);
    }

    /// 1 つだけ変えた材料で作った指紋が、元と違うことを確かめる。
    fn changes(label: &str, edit: impl FnOnce(&mut Inputs)) {
        let mut changed = inputs();
        edit(&mut changed);
        assert_ne!(
            Fingerprint::build(changed, HASH).digest(),
            Fingerprint::build(inputs(), HASH).digest(),
            "{label} を変えても指紋が動かない"
        );
    }

    #[test]
    fn 指標の定義を変えれば指紋が変わる() {
        changes("指標の定義", |i| i.metric_definitions.push('x'));
    }

    #[test]
    fn 数える単位の定義を変えれば指紋が変わる() {
        changes("数える単位の定義", |i| i.unit_definitions.push('x'));
    }

    #[test]
    fn 形態素解析器の辞書と版を変えれば指紋が変わる() {
        changes("形態素解析器の版", |i| {
            i.morphology.version = "6.1".into()
        });
        changes("形態素解析器の辞書", |i| {
            i.morphology.config.insert("辞書".into(), "IPADIC".into());
        });
    }

    #[test]
    fn 係り受け解析器を使うと決めたら指紋が変わる() {
        changes("係り受け解析器", |i| {
            i.dependency = Tool {
                name: "CaboCha".into(),
                version: "0.69".into(),
                config: BTreeMap::new(),
            };
        });
    }

    #[test]
    fn 圧縮器と設定を変えれば指紋が変わる() {
        changes("圧縮器の版", |i| {
            i.compressor.version = "0.8.10".into()
        });
        changes("圧縮の水準", |i| {
            i.compressor.config.insert("水準".into(), "9".into());
        });
    }

    #[test]
    fn 窓と下限と言い回しの設定を変えれば指紋が変わる() {
        changes("窓の大きさ", |i| {
            i.measurement
                .insert("metrics::humanness::WINDOW".into(), "1000".into());
        });
        changes("語のまとめ方の線", |i| {
            i.measurement
                .insert("metrics::lexicon::BOUND".into(), "0.8".into());
        });
    }

    #[test]
    fn 外部の表の版を変えれば指紋が変わる() {
        changes("Unicode の版", |i| {
            i.external_tables.insert("Unicode".into(), "16.0".into());
        });
    }

    #[test]
    fn 取り込み元と変換を変えれば指紋が変わる() {
        changes("取り込み元の種類", |i| {
            i.normalization.sources.push("html".into());
        });
        changes("変換の版", |i| {
            i.normalization.version = "変換 2".into()
        });
        changes("対応表", |i| {
            i.normalization.mapping.insert("html".into(), "表 1".into());
        });
    }

    #[test]
    fn 統計値の中身を変えれば指紋が変わる() {
        assert_ne!(
            Fingerprint::build(inputs(), "sha256:0000").digest(),
            Fingerprint::build(inputs(), "sha256:0001").digest()
        );
    }

    #[test]
    fn 区切り文字を含む値で別の材料と同じ文字列にならない() {
        // 長さを添えないと `a\tb` + `c` と `a` + `b\tc` が同じ文字列になる。
        let mut x = inputs();
        x.metric_definitions = "a\n単位の定義\tb".into();
        x.unit_definitions = "c".into();
        let mut y = inputs();
        y.metric_definitions = "a".into();
        y.unit_definitions = "b\n単位の定義\tc".into();
        assert_ne!(
            Fingerprint::build(x, HASH).digest(),
            Fingerprint::build(y, HASH).digest()
        );
    }

    #[test]
    fn 何が変わったかを言う() {
        let a = Fingerprint::build(inputs(), HASH);
        let mut other = inputs();
        other.morphology.version = "6.1".into();
        other.compressor.version = "0.8.10".into();
        assert_eq!(a.differences(&other), vec!["形態素解析器", "圧縮器"]);
    }

    #[test]
    fn どの測り方の設定が変わったかを言う() {
        let a = Fingerprint::build(inputs(), HASH);
        let mut other = inputs();
        other.measurement.remove("metrics::floor::TOKENS");
        other
            .measurement
            .insert("metrics::humanness::WINDOW".into(), "1000".into());
        assert_eq!(
            a.differences(&other),
            vec![
                "測り方の設定: metrics::floor::TOKENS",
                "測り方の設定: metrics::humanness::WINDOW",
            ]
        );
    }

    #[test]
    fn 中身と取り込み元の違いは道具の違いとして数えない() {
        // 別の人の形代を基準に渡せなければ、他人と比べられない。
        let a = Fingerprint::build(inputs(), "sha256:aaaa");
        let mut other = inputs();
        other.normalization.sources = vec!["html".into()];
        assert!(a.differences(&other).is_empty());
    }

    #[test]
    fn 使わない道具と未設定を分ける() {
        let unused = Tool::unused();
        assert!(!unused.name.is_empty(), "使わないと書く");
    }
}
