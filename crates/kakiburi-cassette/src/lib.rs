//! カセット。<strong>3 つの層で持つ。</strong>
//!
//! | | |
//! | --- | --- |
//! | `decided/` | 人が決めたこと。<strong>作り直せない</strong> |
//! | `corpus/` | 揃えたあとの本文。<strong>作り直せない</strong> |
//! | `derived/` | 派生物。<strong>いつでも捨ててよい</strong> |
//!
//! <strong>`derived/` を丸ごと消しても、`corpus/` と `decided/` があれば同じものが作り直せる。</strong>

pub mod fingerprint;
pub mod json;
pub mod save;
pub mod store;
pub mod zip;

use std::collections::BTreeMap;

use kakiburi_doc::Document;

pub use fingerprint::{Baseline, Fingerprint, Inputs, Normalization, Tool};

/// 単位の役。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// 本人の文書。
    Person,
    /// 基準。LLM の既定出力。
    BaselineOutput,
    /// 他人の文書。<strong>無くてよい。</strong>
    Other,
}

impl Role {
    /// ディレクトリの名前。
    #[must_use]
    pub fn dir(self) -> &'static str {
        match self {
            Role::Person => "person",
            Role::BaselineOutput => "baseline",
            Role::Other => "other",
        }
    }
}

/// 人が決めたこと。<strong>作り直せない原本である。</strong>
#[derive(Debug, Clone, PartialEq)]
pub struct Decided {
    /// 場面。<strong>1 カセット 1 場面。</strong> 文章から当てにいかない。
    pub scene: String,
    /// 落とす定型。
    pub boilerplate: Vec<String>,
    /// 基準の作り方。
    pub baseline: Baseline,
    /// 指示して動くか。<strong>直させてみて初めて分かる。</strong>
    ///
    /// 書いていない指標は「未知」で、前に出す指標に入る。
    pub movement: BTreeMap<String, Movement>,
}

/// 指示して動くか。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Movement {
    /// 動く。
    Moves,
    /// 動かないと分かった。<strong>前に出す指標から外れる。</strong>
    Stuck,
}

/// 正規形の本文。<strong>作り直せない原本である。</strong>
#[derive(Debug, Clone, PartialEq)]
pub struct Corpus {
    /// 単位。<strong>役と名前の昇順で持つ。</strong>
    ///
    /// 並びを正典にしないと、書き出して読み戻したときに順が変わり、
    /// [作り直しても同じものが出る](../../../docs/design/300-test.md#作り直せることを試験する)
    /// が成り立たない。
    pub units: Vec<Unit>,
}

impl Corpus {
    /// 並びを正典にして作る。
    #[must_use]
    pub fn new(mut units: Vec<Unit>) -> Self {
        units.sort_by(|a, b| a.role.cmp(&b.role).then_with(|| a.name.cmp(&b.name)));
        Self { units }
    }
}

/// 1 単位。
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    /// 取り込んだ 1 本の名前。<strong>ファイル 1 つに 1 つ。</strong>
    ///
    /// カセット全体で一意である。<strong>役を跨いでも重複を許さない</strong>——役で名前空間を
    /// 分けると、本人の `Rust入門` と基準の `Rust入門` が別物として通る。
    pub name: String,
    /// 測る 1 単位の名前。<strong>束ねなければ [`name`](Self::name) と同じ。</strong>
    ///
    /// 1 文書では指標が意味を持たないほど短いものは、何本かをまとめて 1 単位にする。
    /// <strong>[10 単位の下限](../../../docs/spec/200-extract.md#対が何本あれば信じるか)は単位で
    /// 数える</strong>——ファイルで数えれば、束ねた分だけ実際より多く見える。
    pub unit: String,
    /// 役。
    pub role: Role,
    /// 正規形。
    pub document: Document,
}

/// 派生物。<strong>いつでも捨ててよい。</strong>
///
/// 型が `Option` なのは「まだ作っていない」を表すためである。<strong>捨てられることが
/// 型に出ている。</strong>
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Derived {
    /// 固定した語彙。
    pub vocabulary: Option<String>,
    /// 単位 × 指標の値。
    pub values: Option<String>,
    /// 幅・出現割合。
    pub spread: Option<String>,
    /// 相手集合の割り、重み。
    pub calibration: Option<String>,
    /// 天井・床・帯。
    pub scale: Option<String>,
    /// 効く指標、前に出す指標。
    pub effective: Option<String>,
}

impl Derived {
    /// 全部捨てる。
    #[must_use]
    pub fn dropped() -> Self {
        Self::default()
    }

    /// 目盛りができているか。
    ///
    /// <strong>できていないカセットは正常な状態である。</strong> 素材が足りずに作れなかったのは
    /// 異常ではなく、検めが判定できないを返す。
    #[must_use]
    pub fn has_scale(&self) -> bool {
        self.scale.is_some()
    }
}

/// カセット。
#[derive(Debug, Clone, PartialEq)]
pub struct Cassette {
    /// 版。
    pub version: u32,
    /// 世代。<strong>書くたびに 1 つ増える。</strong>
    ///
    /// 同時に 2 つが書くと、片方の変更が正常終了のまま消える。<strong>落ちるより悪い</strong>
    /// ——誰も気付かない。読んだときの世代と、置き換える直前の世代が同じことを
    /// 確かめて防ぐ。
    ///
    /// <strong>[指紋](Fingerprint)では検出できない。</strong> 指紋は測った条件を表すもので、
    /// 本文を差し替えても条件が同じなら変わらない。
    pub generation: u64,
    /// 場面。`decided` の写しではなく、`manifest` に出す値。
    pub scene: String,
    /// 指紋。
    pub fingerprint: Fingerprint,
    /// 暫定値が立っている箇所。
    ///
    /// <strong>空でないカセットは、判定に但し書きが付く。</strong> いまは常に立つ。
    pub provisional: Vec<String>,
    /// 人が決めたこと。
    pub decided: Decided,
    /// 正規形の本文。
    pub corpus: Corpus,
    /// 派生物。
    pub derived: Derived,
}

impl Cassette {
    /// 派生物を捨てる。<strong>原本は残る。</strong>
    pub fn drop_derived(&mut self) {
        self.derived = Derived::dropped();
    }

    /// 役ごとの単位。<strong>名前の昇順。</strong>
    #[must_use]
    pub fn units(&self, role: Role) -> Vec<&Unit> {
        let mut out: Vec<&Unit> = self
            .corpus
            .units
            .iter()
            .filter(|u| u.role == role)
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// 測る単位。<strong>束ねたものは 1 つにまとめて返す。</strong>
    ///
    /// <strong>文書の境界は node の境界として残す。</strong> 連結して 1 本の地の文にしない——
    /// [node を跨がない](../../../docs/spec/020-document.md#地の文は-1-本の文字列ではない)
    /// はずの bigram と文が、文書を跨いで繋がる。
    ///
    /// <strong>束の中の並びは `id` の昇順に固定する。</strong> 並びが変われば node の並びが変わり、
    /// 決定性が壊れる。
    #[must_use]
    pub fn bundles(&self, role: Role) -> Vec<(String, Document)> {
        let mut by_unit: BTreeMap<&str, Vec<&Unit>> = BTreeMap::new();
        for u in self.units(role) {
            by_unit.entry(u.unit.as_str()).or_default().push(u);
        }
        by_unit
            .into_iter()
            .map(|(unit, mut members)| {
                members.sort_by(|a, b| a.name.cmp(&b.name));
                let nodes = members
                    .iter()
                    .flat_map(|u| u.document.nodes.iter().cloned())
                    .collect();
                (unit.to_owned(), Document::new(nodes))
            })
            .collect()
    }

    /// 前に出す指標から外すか。
    ///
    /// <strong>書いていなければ「未知」で、前に出す指標に入る。</strong> 動かないと分かるまでは使う。
    #[must_use]
    pub fn is_stuck(&self, metric: &str) -> bool {
        self.decided.movement.get(metric) == Some(&Movement::Stuck)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kakiburi_doc::node::{Kind, Node};

    fn document(text: &str) -> Document {
        Document::new(vec![Node::leaf(Kind::Paragraph, text)])
    }

    fn cassette() -> Cassette {
        Cassette {
            version: 1,
            generation: 0,
            scene: "技術記事".into(),
            fingerprint: Fingerprint::build(Inputs {
                metric_definitions: "51 本".into(),
                unit_definitions: "版 1".into(),
                vocabulary: BTreeMap::new(),
                z_scores: BTreeMap::new(),
                morphology: Tool::unused(),
                dependency: Tool::unused(),
                compressor: Tool::unused(),
                external_tables: BTreeMap::new(),
                normalization: Normalization {
                    sources: vec!["directive-markdown".into()],
                    implementation: "kakiburi-normalize".into(),
                    version: "0.0.0".into(),
                    mapping: BTreeMap::new(),
                },
                baseline: Baseline {
                    model: "m".into(),
                    version: "v".into(),
                    params: BTreeMap::new(),
                    topics: vec!["t".into()],
                },
                decided: BTreeMap::new(),
            }),
            provisional: vec!["除外の既定".into()],
            decided: Decided {
                scene: "技術記事".into(),
                boilerplate: vec![],
                baseline: Baseline {
                    model: "m".into(),
                    version: "v".into(),
                    params: BTreeMap::new(),
                    topics: vec!["t".into()],
                },
                movement: [("笑い".to_owned(), Movement::Stuck)].into(),
            },
            corpus: Corpus::new(vec![
                Unit {
                    name: "p02".into(),
                    unit: "p02".into(),
                    role: Role::Person,
                    document: document("本人の文書である。"),
                },
                Unit {
                    name: "p01".into(),
                    unit: "p01".into(),
                    role: Role::Person,
                    document: document("もう 1 本の本人の文書。"),
                },
                Unit {
                    name: "b01".into(),
                    unit: "b01".into(),
                    role: Role::BaselineOutput,
                    document: document("基準の文書である。"),
                },
            ]),
            derived: Derived {
                scale: Some("天井と床".into()),
                ..Derived::default()
            },
        }
    }

    #[test]
    fn 役ごとに単位を引ける() {
        let c = cassette();
        assert_eq!(c.units(Role::Person).len(), 2);
        assert_eq!(c.units(Role::BaselineOutput).len(), 1);
        assert_eq!(c.units(Role::Other).len(), 0, "他人は無くてよい");
    }

    #[test]
    fn 単位は名前の昇順で並ぶ() {
        let c = cassette();
        let names: Vec<&str> = c
            .units(Role::Person)
            .iter()
            .map(|u| u.name.as_str())
            .collect();
        assert_eq!(names, vec!["p01", "p02"]);
    }

    #[test]
    fn 派生物を捨てても原本は残る() {
        let mut c = cassette();
        assert!(c.derived.has_scale());
        c.drop_derived();
        assert!(!c.derived.has_scale());
        assert_eq!(c.corpus.units.len(), 3, "本文は残る");
        assert_eq!(c.decided.scene, "技術記事", "決めたことも残る");
    }

    #[test]
    fn 目盛りが無いカセットは正常な状態である() {
        // 素材が足りずに作れなかったのは異常ではない。
        let mut c = cassette();
        c.drop_derived();
        assert!(!c.derived.has_scale());
    }

    #[test]
    fn 書いていない指標は未知で前に出す() {
        // 動かないと分かるまでは使う。
        let c = cassette();
        assert!(c.is_stuck("笑い"), "stuck と書いてある");
        assert!(!c.is_stuck("全角括弧"), "書いていなければ未知");
    }

    #[test]
    fn 暫定値が立っていることを持つ() {
        // いまは常に立つ。12 か所の閾値がまだ導き直されていない。
        let c = cassette();
        assert!(!c.provisional.is_empty());
    }

    #[test]
    fn 役のディレクトリ名は仕様どおり() {
        assert_eq!(Role::Person.dir(), "person");
        assert_eq!(Role::BaselineOutput.dir(), "baseline");
        assert_eq!(Role::Other.dir(), "other");
    }
}
