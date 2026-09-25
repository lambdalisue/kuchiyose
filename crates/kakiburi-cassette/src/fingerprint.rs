//! 指紋。何で測ったかを残す。
//!
//! 仕様が「一部だけを混ぜない」と警告している。混ぜ忘れてもエラーにならず、
//! 古い値が黙って使われる。
//!
//! だから指紋を作る関数は、全部の材料を引数に取る。 1 つでも欠ければ
//! 組み立てられない——型がそれを強制する。

use std::collections::BTreeMap;

/// 指紋の材料。すべての欄が必須である。
///
/// `Option` を持たない。持たせれば、混ぜ忘れが `None` として通ってしまう。
/// まだ使わないものも、使わないと書いて渡す。
///
/// 道具の部分と場面の部分に割れる。 道具と定義が変われば過去の値と
/// 比べられないが、語彙や基準が変わっただけなら道具は据え置きである。
/// 割らなければ、どちらが変わったのかを言えない。
#[derive(Debug, Clone, PartialEq)]
pub struct Inputs {
    /// 共通部分。道具と定義である。
    pub common: Common,
    /// 場面の部分。1 カセットが 1 場面なので 1 つだけ持つ。
    pub scene: SceneInputs,
}

/// 指紋の材料のうち、場面に依らないもの。
///
/// 道具と実装と定義である。 ここが変われば、どのカセットの値も
/// 過去と比べられない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Common {
    /// 指標の定義そのもの。登録簿から。
    pub metric_definitions: String,
    /// 数える単位の定義。地の文に何が入るか、日本語の文字の範囲。
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
}

/// 指紋の材料のうち、場面で変わるもの。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SceneInputs {
    /// 固定した語彙。系統ごとの次元の並び。
    pub vocabulary: BTreeMap<String, Vec<String>>,
    /// 固定した z 得点の平均と標準偏差。
    pub z_scores: BTreeMap<String, Vec<(f64, f64)>>,
    /// 単位をどう割ったか。束の構成と並びもここに出る。
    ///
    /// 鍵が割りの名前、値が単位の名前の並びである。
    /// [束ねた単位は中身を名前にする](../../../docs/spec/200-extract.md#短い文書は束ねる)
    /// ので、どの文書をどの順で束ねたかがそのまま入る。
    ///
    /// 割りが変われば帯が変わる。 入れなければ、同じ語彙のまま別の割りで
    /// 作った目盛りが同じ指紋を名乗る。
    pub selection: BTreeMap<String, Vec<String>>,
    /// 基準の LLM の版と推論設定。
    pub baseline: Baseline,
    /// 人が決めたこと。場面・落とす定型・指示して動くか。
    pub decided: BTreeMap<String, String>,
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
    /// 取り込み元の種類。
    pub sources: Vec<String>,
    /// 変換の実装。
    pub implementation: String,
    /// その版。
    pub version: String,
    /// 適用した対応表。
    pub mapping: BTreeMap<String, String>,
}

/// 基準の作り方。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Baseline {
    /// LLM の名前。
    pub model: String,
    /// 版。モデル名だけでは足りない。
    pub version: String,
    /// 推論設定。
    pub params: BTreeMap<String, String>,
    /// 題材。言葉づかいだけで帯が動くので、外さない。
    pub topics: Vec<String>,
}

/// 指紋。
#[derive(Debug, Clone, PartialEq)]
pub struct Fingerprint {
    /// 材料。平文で残す——ハッシュだけでは、何が変わったかが分からない。
    pub inputs: Inputs,
    /// 材料から作った文字列。比べるのはこれ。
    digest: String,
}

impl Fingerprint {
    /// 組み立てる。材料をすべて受け取る。
    #[must_use]
    pub fn build(inputs: Inputs) -> Self {
        let digest = inputs.canonical();
        Self { inputs, digest }
    }

    /// 同じ条件で測ったか。
    #[must_use]
    pub fn matches(&self, other: &Self) -> bool {
        self.digest == other.digest
    }

    /// 共通部分のどこが違うか。
    ///
    /// 道具と実装と定義である。 ここが変われば、どのカセットの値も
    /// 過去と比べられない。
    #[must_use]
    pub fn common_differences(&self, other: &Self) -> Vec<&'static str> {
        let a = &self.inputs.common;
        let b = &other.inputs.common;
        let mut out = Vec::new();
        if a.metric_definitions != b.metric_definitions {
            out.push("指標の定義");
        }
        if a.unit_definitions != b.unit_definitions {
            out.push("数える単位の定義");
        }
        if a.morphology != b.morphology {
            out.push("形態素解析器");
        }
        if a.dependency != b.dependency {
            out.push("係り受け解析器");
        }
        if a.compressor != b.compressor {
            out.push("圧縮器");
        }
        if a.external_tables != b.external_tables {
            out.push("外部の表");
        }
        if a.normalization != b.normalization {
            out.push("正規化");
        }
        out
    }

    /// 場面の部分のどこが違うか。
    #[must_use]
    pub fn scene_differences(&self, other: &Self) -> Vec<&'static str> {
        let (a, b) = (&self.inputs.scene, &other.inputs.scene);
        let mut out = Vec::new();
        if a.vocabulary != b.vocabulary {
            out.push("固定した語彙");
        }
        if a.z_scores != b.z_scores {
            out.push("固定した z 得点");
        }
        if a.selection != b.selection {
            out.push("単位の割り");
        }
        if a.baseline != b.baseline {
            out.push("基準の作り方");
        }
        if a.decided != b.decided {
            out.push("人が決めたこと");
        }
        out
    }

    /// どの材料が違うか。変わったことだけでなく、何が変わったかを言う。
    #[must_use]
    pub fn differences(&self, other: &Self) -> Vec<String> {
        self.common_differences(other)
            .into_iter()
            .chain(self.scene_differences(other))
            .map(str::to_owned)
            .collect()
    }

    /// 比べるための文字列。
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

impl Inputs {
    /// 決定的な文字列にする。並び順を固定するので、作り直しても同じものが出る。
    fn canonical(&self) -> String {
        let c = &self.common;
        let mut s = String::new();
        s.push_str("指標の定義\t");
        s.push_str(&c.metric_definitions);
        s.push_str("\n単位の定義\t");
        s.push_str(&c.unit_definitions);
        s.push('\n');
        for (label, t) in [
            ("形態素解析器", &c.morphology),
            ("係り受け解析器", &c.dependency),
            ("圧縮器", &c.compressor),
        ] {
            s.push_str(label);
            s.push('\t');
            s.push_str(&t.name);
            s.push('\t');
            s.push_str(&t.version);
            for (k, v) in &t.config {
                s.push_str(&format!("\t{k}={v}"));
            }
            s.push('\n');
        }
        s.push_str("外部の表\n");
        for (k, v) in &c.external_tables {
            s.push_str(&format!("{k}\t{v}\n"));
        }
        s.push_str("正規化\t");
        s.push_str(&c.normalization.sources.join(","));
        s.push('\t');
        s.push_str(&c.normalization.implementation);
        s.push('\t');
        s.push_str(&c.normalization.version);
        for (k, v) in &c.normalization.mapping {
            s.push_str(&format!("\t{k}={v}"));
        }
        s.push('\n');
        let i = &self.scene;
        s.push_str("語彙\n");
        for (k, v) in &i.vocabulary {
            s.push_str(k);
            s.push('\t');
            s.push_str(&v.join(","));
            s.push('\n');
        }
        s.push_str("z 得点\n");
        for (k, v) in &i.z_scores {
            s.push_str(k);
            for (m, sd) in v {
                s.push_str(&format!("\t{m:.17e}:{sd:.17e}"));
            }
            s.push('\n');
        }
        // 長さを添えて並べる。 区切り文字で繋ぐと、名前がその文字を
        // 含んだときに別の割りが同じ文字列になる——単位の名前は
        // ファイル名なので、区切りに使える文字はどれも名前に入りうる。
        s.push_str("割り\n");
        for (k, v) in &i.selection {
            s.push_str(k);
            for name in v {
                s.push_str(&format!("\t{}:{name}", name.len()));
            }
            s.push('\n');
        }
        s.push_str("基準\t");
        s.push_str(&i.baseline.model);
        s.push('\t');
        s.push_str(&i.baseline.version);
        for (k, v) in &i.baseline.params {
            s.push_str(&format!("\t{k}={v}"));
        }
        for t in &i.baseline.topics {
            s.push_str(&format!("\ttopic={t}"));
        }
        s.push_str("\n人が決めたこと\n");
        for (k, v) in &i.decided {
            s.push_str(&format!("{k}\t{v}\n"));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> Inputs {
        Inputs {
            common: Common {
                metric_definitions: "51 本".into(),
                unit_definitions: "020 の版 1".into(),
                morphology: Tool {
                    name: "UniDic".into(),
                    version: "3.1.0".into(),
                    config: BTreeMap::new(),
                },
                dependency: Tool::unused(),
                compressor: Tool {
                    name: "zstd".into(),
                    version: "1.5".into(),
                    config: [("level".to_owned(), "3".to_owned())].into(),
                },
                external_tables: [("Unicode".to_owned(), "15.1".to_owned())].into(),
                normalization: Normalization {
                    sources: vec!["markdown".into()],
                    implementation: "kakiburi-normalize".into(),
                    version: "0.0.0".into(),
                    mapping: [("message".to_owned(), "補足".to_owned())].into(),
                },
            },
            scene: SceneInputs {
                vocabulary: [("文字bigram".to_owned(), vec!["あい".to_owned()])].into(),
                z_scores: [("文字bigram".to_owned(), vec![(0.1, 0.2)])].into(),
                selection: [("本人の相手集合".to_owned(), vec!["p00".to_owned()])].into(),
                baseline: Baseline {
                    model: "some-model".into(),
                    version: "2026-01".into(),
                    params: [("temperature".to_owned(), "1.0".to_owned())].into(),
                    topics: vec!["Vim のファイラー".into()],
                },
                decided: [("落とす定型".to_owned(), "この記事では".to_owned())].into(),
            },
        }
    }

    #[test]
    fn 同じ材料からは同じ指紋が出る() {
        assert!(Fingerprint::build(inputs()).matches(&Fingerprint::build(inputs())));
    }

    #[test]
    fn 材料を_1_つ変えれば指紋が変わる() {
        // 入力の数だけ試験を書く。まとめて回すと、覆えていない入力が
        // 1 つあっても他が通って緑になる。
        let base = Fingerprint::build(inputs());

        let mut i = inputs();
        i.common.metric_definitions = "52 本".into();
        assert!(!base.matches(&Fingerprint::build(i)), "指標の定義");

        let mut i = inputs();
        i.common.unit_definitions = "020 の版 2".into();
        assert!(!base.matches(&Fingerprint::build(i)), "単位の定義");

        let mut i = inputs();
        i.scene
            .vocabulary
            .insert("読点の打ち方".to_owned(), vec!["で".to_owned()]);
        assert!(!base.matches(&Fingerprint::build(i)), "語彙");

        let mut i = inputs();
        i.scene
            .z_scores
            .insert("文字bigram".to_owned(), vec![(0.11, 0.2)]);
        assert!(!base.matches(&Fingerprint::build(i)), "z 得点");

        let mut i = inputs();
        i.common.morphology.version = "3.2.0".into();
        assert!(!base.matches(&Fingerprint::build(i)), "形態素解析器");

        let mut i = inputs();
        i.common.dependency = Tool {
            name: "CaboCha".into(),
            version: "0.69".into(),
            config: BTreeMap::new(),
        };
        assert!(!base.matches(&Fingerprint::build(i)), "係り受け解析器");

        let mut i = inputs();
        i.common
            .compressor
            .config
            .insert("level".to_owned(), "9".to_owned());
        assert!(!base.matches(&Fingerprint::build(i)), "圧縮器");

        let mut i = inputs();
        i.common
            .external_tables
            .insert("Unicode".to_owned(), "16.0".to_owned());
        assert!(!base.matches(&Fingerprint::build(i)), "外部の表");

        let mut i = inputs();
        i.common.normalization.version = "0.1.0".into();
        assert!(!base.matches(&Fingerprint::build(i)), "正規化");

        let mut i = inputs();
        i.scene
            .selection
            .insert("本人の相手集合".to_owned(), vec!["p01".to_owned()]);
        assert!(!base.matches(&Fingerprint::build(i)), "単位の割り");

        // 名前に区切り文字が入っても、別の割りは別の指紋になる。
        // 単位の名前はファイル名なので、区切りに使える文字はどれも名前に入りうる。
        let split_at = |names: Vec<&str>| {
            let mut i = inputs();
            i.scene.selection.insert(
                "本人の相手集合".to_owned(),
                names.into_iter().map(str::to_owned).collect(),
            );
            Fingerprint::build(i)
        };
        assert!(
            !split_at(vec!["a,b", "c"]).matches(&split_at(vec!["a", "b,c"])),
            "区切り文字を含む名前で割りが潰れている"
        );

        // 束ねた単位は中身を名前にする。 束ね方が変われば名前が変わり、
        // 割りが変わって指紋が動く。
        let mut i = inputs();
        i.scene
            .selection
            .insert("基準の較正分".to_owned(), vec!["b01+b02".to_owned()]);
        assert!(!base.matches(&Fingerprint::build(i)), "束の構成と並び");

        let mut i = inputs();
        i.scene.baseline.version = "2026-02".into();
        assert!(!base.matches(&Fingerprint::build(i)), "基準の版");

        let mut i = inputs();
        i.scene.baseline.topics.push("GPG 鍵".into());
        assert!(!base.matches(&Fingerprint::build(i)), "基準の題材");

        let mut i = inputs();
        i.scene
            .decided
            .insert("落とす定型".to_owned(), "この文章では".to_owned());
        assert!(!base.matches(&Fingerprint::build(i)), "人が決めたこと");
    }

    #[test]
    fn 何が変わったかを言う() {
        // ハッシュだけでは、変わったことは分かっても何が変わったかが分からない。
        let base = Fingerprint::build(inputs());
        let mut i = inputs();
        i.common.morphology.version = "3.2.0".into();
        i.scene.baseline.version = "2026-02".into();
        let other = Fingerprint::build(i);
        assert_eq!(base.differences(&other), vec!["形態素解析器", "基準の作り方"]);
    }

    #[test]
    fn 共通部分と場面の部分を別に照らせる() {
        // 道具が変わったのか語彙が変わったのかが、構造で分かれる。
        let base = Fingerprint::build(inputs());
        let mut i = inputs();
        i.scene.baseline.version = "2026-02".into();
        let other = Fingerprint::build(i);
        assert!(
            base.common_differences(&other).is_empty(),
            "道具は変わっていない"
        );
        assert_eq!(base.scene_differences(&other), vec!["基準の作り方"]);
    }

    #[test]
    fn 使わない道具と未設定を分ける() {
        // Option を持たせれば、混ぜ忘れが None として通る。
        let unused = Tool::unused();
        assert_eq!(unused.name, "使わない");
        assert!(unused.version.is_empty());
    }

    #[test]
    fn 材料は平文で残る() {
        let f = Fingerprint::build(inputs());
        assert_eq!(f.inputs.scene.baseline.model, "some-model");
        assert!(!f.digest().is_empty());
    }

    #[test]
    fn 題材を外せない() {
        // 型が Vec を要求する。外すという選択肢が無い。
        let i = inputs();
        assert!(
            !i.scene.baseline.topics.is_empty(),
            "題材が空でも組めるが、欄は消せない"
        );
    }
}
