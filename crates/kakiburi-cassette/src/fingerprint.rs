//! 指紋。<strong>何で測ったかを残す。</strong>
//!
//! 仕様が「一部だけを混ぜない」と警告している。混ぜ忘れても<strong>エラーにならず、
//! 古い値が黙って使われる。</strong>
//!
//! <strong>だから指紋を作る関数は、全部の材料を引数に取る。</strong> 1 つでも欠ければ
//! 組み立てられない——型がそれを強制する。

use std::collections::BTreeMap;

/// 指紋の材料。<strong>すべての欄が必須である。</strong>
///
/// `Option` を持たない。持たせれば、混ぜ忘れが `None` として通ってしまう。
/// <strong>まだ使わないものも、使わないと書いて渡す。</strong>
///
/// <strong>共通部分と場面ごとの部分に割れる。</strong> 1 カセットが 1 人になって、
/// 道具と定義はカセット全体で 1 つ、語彙と人が決めたことは場面ごとになったからである。
/// <strong>割らなければ、2 つ目の場面を `build` した時点で 1 つ目の語彙が消える</strong>
/// ——指紋は同じ形のまま、中身だけが入れ替わる。
#[derive(Debug, Clone, PartialEq)]
pub struct Inputs {
    /// 共通部分。<strong>カセット全体で 1 つ。</strong>
    pub common: Common,
    /// 場面ごとの部分。<strong>鍵が場面である。</strong>
    pub scenes: BTreeMap<String, SceneInputs>,
}

/// 指紋の材料のうち、カセット全体で 1 つのもの。
///
/// <strong>道具と実装と定義である。</strong> 場面を増やしても変わらず、変われば
/// どの場面の値も過去と比べられない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Common {
    /// 指標の定義そのもの。登録簿から。
    pub metric_definitions: String,
    /// 数える単位の定義。地の文に何が入るか、日本語の文字の範囲。
    pub unit_definitions: String,
    /// 形態素解析器の辞書と版。
    pub morphology: Tool,
    /// 係り受け解析器の辞書と版。<strong>保留中。使わないなら `Tool::unused`。</strong>
    pub dependency: Tool,
    /// 圧縮器と設定。
    pub compressor: Tool,
    /// 外部の表の版。語の文体値、文末表現の辞書、Unicode。
    pub external_tables: BTreeMap<String, String>,
    /// 取り込み元の種類・変換の実装と版・対応表。
    pub normalization: Normalization,
}

/// 指紋の材料のうち、場面ごとに変わるもの。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SceneInputs {
    /// 固定した語彙。系統ごとの次元の並び。
    pub vocabulary: BTreeMap<String, Vec<String>>,
    /// 固定した z 得点の平均と標準偏差。
    pub z_scores: BTreeMap<String, Vec<(f64, f64)>>,
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
    /// 使わない道具。<strong>「未設定」と「使わない」を分ける。</strong>
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
    /// 版。<strong>モデル名だけでは足りない。</strong>
    pub version: String,
    /// 推論設定。
    pub params: BTreeMap<String, String>,
    /// 題材。<strong>言葉づかいだけで帯が動くので、外さない。</strong>
    pub topics: Vec<String>,
}

/// 指紋。
#[derive(Debug, Clone, PartialEq)]
pub struct Fingerprint {
    /// 材料。<strong>平文で残す</strong>——ハッシュだけでは、何が変わったかが分からない。
    pub inputs: Inputs,
    /// 材料から作った文字列。比べるのはこれ。
    digest: String,
}

impl Fingerprint {
    /// 組み立てる。<strong>材料をすべて受け取る。</strong>
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
    /// <strong>道具と実装と定義である。</strong> ここが変われば、どの場面の値も過去と
    /// 比べられない。
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

    /// 1 つの場面のどこが違うか。
    ///
    /// <strong>片方にしか無い場面は「場面ごとの材料」として違う。</strong> 空対空にすると、
    /// 場面を消したことが差として出ない。
    #[must_use]
    pub fn scene_differences(&self, other: &Self, scene: &str) -> Vec<&'static str> {
        let (a, b) = (
            self.inputs.scenes.get(scene),
            other.inputs.scenes.get(scene),
        );
        let (Some(a), Some(b)) = (a, b) else {
            return if a.is_none() && b.is_none() {
                vec![]
            } else {
                vec!["場面ごとの材料"]
            };
        };
        let mut out = Vec::new();
        if a.vocabulary != b.vocabulary {
            out.push("固定した語彙");
        }
        if a.z_scores != b.z_scores {
            out.push("固定した z 得点");
        }
        if a.baseline != b.baseline {
            out.push("基準の作り方");
        }
        if a.decided != b.decided {
            out.push("人が決めたこと");
        }
        out
    }

    /// どの材料が違うか。<strong>変わったことだけでなく、何が変わったかを言う。</strong>
    ///
    /// 場面ごとの材料には場面の名前を添える。
    #[must_use]
    pub fn differences(&self, other: &Self) -> Vec<String> {
        let mut out: Vec<String> = self
            .common_differences(other)
            .into_iter()
            .map(str::to_owned)
            .collect();
        let mut scenes: Vec<&String> = self
            .inputs
            .scenes
            .keys()
            .chain(other.inputs.scenes.keys())
            .collect();
        scenes.sort_unstable();
        scenes.dedup();
        for s in scenes {
            for what in self.scene_differences(other, s) {
                out.push(format!("{s} の{what}"));
            }
        }
        out
    }

    /// 比べるための文字列。
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

impl Inputs {
    /// 決定的な文字列にする。<strong>並び順を固定する</strong>ので、作り直しても同じものが出る。
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
        // <strong>場面ごとの材料は、場面の名前を添えて並べる。</strong> 名前を落とせば、
        // 2 つの場面の中身を入れ替えても同じ指紋になる。
        for (scene, i) in &self.scenes {
            s.push_str(&format!("== {scene} ==\n語彙\n"));
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
                    sources: vec!["directive-markdown".into()],
                    implementation: "kakiburi-normalize".into(),
                    version: "0.0.0".into(),
                    mapping: [("message".to_owned(), "補足".to_owned())].into(),
                },
            },
            scenes: [(
                "技術記事".to_owned(),
                SceneInputs {
                    vocabulary: [("文字bigram".to_owned(), vec!["あい".to_owned()])].into(),
                    z_scores: [("文字bigram".to_owned(), vec![(0.1, 0.2)])].into(),
                    baseline: Baseline {
                        model: "some-model".into(),
                        version: "2026-01".into(),
                        params: [("temperature".to_owned(), "1.0".to_owned())].into(),
                        topics: vec!["Vim のファイラー".into()],
                    },
                    decided: [("落とす定型".to_owned(), "この記事では".to_owned())].into(),
                },
            )]
            .into(),
        }
    }

    /// 場面ごとの材料を書き換える。
    fn scene_of(i: &mut Inputs) -> &mut SceneInputs {
        i.scenes.get_mut("技術記事").expect("在る")
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
        scene_of(&mut i)
            .vocabulary
            .insert("読点の打ち方".to_owned(), vec!["で".to_owned()]);
        assert!(!base.matches(&Fingerprint::build(i)), "語彙");

        let mut i = inputs();
        scene_of(&mut i)
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
        scene_of(&mut i).baseline.version = "2026-02".into();
        assert!(!base.matches(&Fingerprint::build(i)), "基準の版");

        let mut i = inputs();
        scene_of(&mut i).baseline.topics.push("GPG 鍵".into());
        assert!(!base.matches(&Fingerprint::build(i)), "基準の題材");

        let mut i = inputs();
        scene_of(&mut i)
            .decided
            .insert("落とす定型".to_owned(), "この文章では".to_owned());
        assert!(!base.matches(&Fingerprint::build(i)), "人が決めたこと");

        // <strong>場面が増えれば指紋が変わる。</strong>
        let mut i = inputs();
        i.scenes
            .insert("チャット".to_owned(), SceneInputs::default());
        assert!(!base.matches(&Fingerprint::build(i)), "場面が増えた");
    }

    #[test]
    fn 場面の中身を入れ替えても同じ指紋にならない() {
        // 場面の名前を落とすと、2 つの場面の中身を入れ替えても同じ指紋になる。
        let mut a = inputs();
        a.scenes.insert(
            "チャット".to_owned(),
            SceneInputs {
                vocabulary: [("文字bigram".to_owned(), vec!["うえ".to_owned()])].into(),
                ..SceneInputs::default()
            },
        );
        let mut b = a.clone();
        let x = b.scenes.get("技術記事").unwrap().clone();
        let y = b.scenes.get("チャット").unwrap().clone();
        b.scenes.insert("技術記事".to_owned(), y);
        b.scenes.insert("チャット".to_owned(), x);
        assert!(!Fingerprint::build(a).matches(&Fingerprint::build(b)));
    }

    #[test]
    fn 何が変わったかを言う() {
        // ハッシュだけでは、変わったことは分かっても何が変わったかが分からない。
        let base = Fingerprint::build(inputs());
        let mut i = inputs();
        i.common.morphology.version = "3.2.0".into();
        scene_of(&mut i).baseline.version = "2026-02".into();
        let other = Fingerprint::build(i);
        assert_eq!(
            base.differences(&other),
            vec!["形態素解析器", "技術記事 の基準の作り方"]
        );
    }

    #[test]
    fn 共通部分と場面ごとの部分を別に照らせる() {
        // <strong>道具が変わったのか語彙が変わったのかが、構造で分かれる。</strong>
        let base = Fingerprint::build(inputs());
        let mut i = inputs();
        scene_of(&mut i).baseline.version = "2026-02".into();
        let other = Fingerprint::build(i);
        assert!(
            base.common_differences(&other).is_empty(),
            "道具は変わっていない"
        );
        assert_eq!(
            base.scene_differences(&other, "技術記事"),
            vec!["基準の作り方"]
        );
        assert!(
            base.scene_differences(&other, "チャット").is_empty(),
            "どちらにも無い場面は違わない"
        );
    }

    #[test]
    fn 片方にしか無い場面は違いとして出る() {
        // 空対空にすると、場面を消したことが差として出ない。
        let base = Fingerprint::build(inputs());
        let mut i = inputs();
        i.scenes.remove("技術記事");
        let other = Fingerprint::build(i);
        assert_eq!(
            base.scene_differences(&other, "技術記事"),
            vec!["場面ごとの材料"]
        );
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
        assert_eq!(f.inputs.scenes["技術記事"].baseline.model, "some-model");
        assert!(!f.digest().is_empty());
    }

    #[test]
    fn 題材を外せない() {
        // 型が Vec を要求する。外すという選択肢が無い。
        let i = inputs();
        assert!(
            !i.scenes["技術記事"].baseline.topics.is_empty(),
            "題材が空でも組めるが、欄は消せない"
        );
    }
}
