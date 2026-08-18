//! カセット。<strong>3 つの層で持つ。</strong>
//!
//! | | |
//! | --- | --- |
//! | `decided/` | 人が決めたこと。<strong>作り直せない</strong> |
//! | `corpus/` | 揃えたあとの本文。<strong>作り直せない</strong> |
//! | `derived/` | 派生物。<strong>いつでも捨ててよい</strong> |
//!
//! <strong>`derived/` を丸ごと消しても、`corpus/` と `decided/` があれば同じものが作り直せる。</strong>
//!
//! <strong>1 カセットが 1 人である。</strong> 場面ごとの束を[トラック](Track)と呼び、
//! `decided/` と `derived/` はトラックごとに持つ。本文と道具と定義の版は共有する。
//!
//! <strong>[場面ごとに閉じる](../../../docs/spec/010-strategy.md#場面ごとに閉じる)は変えない。</strong>
//! 語彙も重みも帯も場面ごとに作る——動いたのは入れ物の境界だけである。

pub mod fingerprint;
pub mod json;
pub mod save;
pub mod store;
pub mod zip;

use std::collections::BTreeMap;

use kakiburi_doc::Document;

pub use fingerprint::{Baseline, Common, Fingerprint, Inputs, Normalization, SceneInputs, Tool};

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

/// 単位の所属。<strong>役と場面をひとつの決定にする。</strong>
///
/// <strong>別々の欄にすると、場面を持たない `other` が場面で絞る関数から漏れる道ができる。</strong>
/// `Option<String>` の場面を持たせれば、`None` を「まだ決めていない」と読む経路が
/// 書けてしまう——<strong>場面を持たないことは、決め終わった状態である。</strong>
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Belongs {
    /// 本人の文書。<strong>場面に属する。</strong>
    Person {
        /// 場面。
        scene: String,
    },
    /// 基準（LLM の既定出力）。<strong>場面に属する。</strong>
    Baseline {
        /// 場面。
        scene: String,
    },
    /// 他人の文書。<strong>場面を持たない。</strong>
    ///
    /// [人らしさの人の側](../../../docs/spec/200-extract.md#人らしさの境目は同じ材料から出る)
    /// にだけ効く。仕様はこの用途に限って場面を跨ぐことを許している——<strong>場面は人と
    /// 機械の別を跨がない</strong>からである。
    Other,
}

impl Belongs {
    /// 役。<strong>所属から導く。</strong>
    #[must_use]
    pub fn role(&self) -> Role {
        match self {
            Belongs::Person { .. } => Role::Person,
            Belongs::Baseline { .. } => Role::BaselineOutput,
            Belongs::Other => Role::Other,
        }
    }

    /// 場面。<strong>他人の文書は持たない。</strong>
    #[must_use]
    pub fn scene(&self) -> Option<&str> {
        match self {
            Belongs::Person { scene } | Belongs::Baseline { scene } => Some(scene),
            Belongs::Other => None,
        }
    }
}

/// 場面の名前として使えるか。
///
/// <strong>保存の中では階層の名前になる。</strong> `/` を許せば、`decided/技術/記事/` が
/// 「技術/記事」なのか「技術」の下の「記事」なのかを言えなくなる。<strong>空も断る</strong>
/// ——空の場面は「場面を決めていない」と見分けが付かない。
#[must_use]
pub fn scene_name_ok(scene: &str) -> bool {
    !scene.is_empty()
        && !scene.contains('/')
        && !scene.contains('\\')
        && scene != "."
        && scene != ".."
}

/// 場面ごとの束。<strong>1 本のカセットに何本か入る。</strong>
///
/// <strong>比喩を延ばせばトラックである。</strong> 再生するときは 1 本を選ぶ。
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// 人が決めたこと。
    pub decided: Decided,
    /// 派生物。
    pub derived: Derived,
}

impl Track {
    /// 何も決まっていないトラック。
    #[must_use]
    pub fn empty() -> Self {
        Self {
            decided: Decided::default(),
            derived: Derived::dropped(),
        }
    }
}

/// 人が決めたこと。<strong>作り直せない原本である。</strong>
///
/// <strong>場面は持たない。</strong> どの場面のものかは[トラック](Track)の鍵が言う——
/// 2 か所に置けば、食い違ったときに正本が決まらない。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Decided {
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
///
/// <strong>カセット全体で共有する。</strong> 場面はカセットではなく単位が持つ。
///
/// <strong>全単位をまとめて返す口を持たない。</strong> 持てば、場面を跨いで目盛りを作る経路が
/// 書けてしまう——いまは入れ物が別なので物理的に書けないことを、ここでは型で守る。
#[derive(Debug, Clone, PartialEq)]
pub struct Corpus {
    /// 単位。<strong>所属と名前の昇順で持つ。</strong>
    ///
    /// 並びを正典にしないと、書き出して読み戻したときに順が変わり、
    /// [作り直しても同じものが出る](../../../docs/design/300-test.md#作り直せることを試験する)
    /// が成り立たない。
    pub(crate) units: Vec<Unit>,
}

impl Corpus {
    /// 並びを正典にして作る。
    #[must_use]
    pub fn new(mut units: Vec<Unit>) -> Self {
        units.sort_by(|a, b| a.belongs.cmp(&b.belongs).then_with(|| a.name.cmp(&b.name)));
        Self { units }
    }

    /// 入っている単位の数。<strong>ファイルの数である。</strong>
    #[must_use]
    pub fn len(&self) -> usize {
        self.units.len()
    }

    /// 空か。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    /// 名前の一覧。<strong>本文は返さない。</strong>
    ///
    /// 名前の衝突を見るためだけのものである。
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.units.iter().map(|u| u.name.as_str())
    }

    /// 入っている場面。<strong>昇順。</strong>
    #[must_use]
    pub fn scenes(&self) -> Vec<&str> {
        let mut out: Vec<&str> = self
            .units
            .iter()
            .filter_map(|u| u.belongs.scene())
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// 1 つの場面の、1 つの役の単位。<strong>名前の昇順。</strong>
    ///
    /// <strong>他人の文書はここから出てこない。</strong> 場面を持たないので、どの場面を指しても
    /// 一致しない——[人らしさ用の口](Self::for_humanness)からだけ取れる。
    #[must_use]
    pub fn in_scene(&self, scene: &str, role: Role) -> Vec<&Unit> {
        let mut out: Vec<&Unit> = self
            .units
            .iter()
            .filter(|u| u.belongs.scene() == Some(scene) && u.belongs.role() == role)
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// 人らしさの人の側に足せる他人の文書。<strong>場面を跨ぐ唯一の口である。</strong>
    ///
    /// <strong>[較正の側に足す](../../../docs/spec/200-extract.md#人らしさの境目は同じ材料から出る)。</strong>
    /// 帯の側に足すと、帯の点の数が本人と基準で釣り合わなくなる。
    #[must_use]
    pub fn for_humanness(&self) -> Vec<&Unit> {
        let mut out: Vec<&Unit> = self
            .units
            .iter()
            .filter(|u| u.belongs == Belongs::Other)
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// 名前で 1 本引く。
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&Unit> {
        self.units.iter().find(|u| u.name == name)
    }

    /// 名前で 1 本を差し替える。<strong>無ければ何もしない。</strong>
    ///
    /// 並びが変わりうるので、置き換えたあとに正典の順へ戻す。
    pub fn replace(&mut self, name: &str, unit: Unit) -> bool {
        let Some(i) = self.units.iter().position(|u| u.name == name) else {
            return false;
        };
        self.units[i] = unit;
        let mut units = std::mem::take(&mut self.units);
        units.sort_by(|a, b| a.belongs.cmp(&b.belongs).then_with(|| a.name.cmp(&b.name)));
        self.units = units;
        true
    }

    /// 単位を足す。<strong>並びは正典に戻す。</strong>
    pub fn push(&mut self, unit: Unit) {
        self.units.push(unit);
        let mut units = std::mem::take(&mut self.units);
        units.sort_by(|a, b| a.belongs.cmp(&b.belongs).then_with(|| a.name.cmp(&b.name)));
        self.units = units;
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
    /// 所属。<strong>役と場面をひとつにしたもの。</strong>
    pub belongs: Belongs,
    /// 正規形。
    pub document: Document,
}

impl Unit {
    /// 役。
    #[must_use]
    pub fn role(&self) -> Role {
        self.belongs.role()
    }
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
    /// 指紋の<strong>共通部分</strong>。道具・実装・定義の版・取り込み元。
    ///
    /// <strong>場面ごとに変わるもの（語彙と z 得点、人が決めたこと）は入らない。</strong>
    /// それは[トラック](Track)の側にある。
    pub fingerprint: Fingerprint,
    /// 暫定値が立っている箇所。
    ///
    /// <strong>空でないカセットは、判定に但し書きが付く。</strong> いまは常に立つ。
    pub provisional: Vec<String>,
    /// 正規形の本文。<strong>共有する。</strong>
    pub corpus: Corpus,
    /// 場面ごとの束。<strong>鍵が場面である。</strong>
    pub tracks: BTreeMap<String, Track>,
}

impl Cassette {
    /// 1 つの場面の束。<strong>無ければ `None`。</strong>
    #[must_use]
    pub fn track(&self, scene: &str) -> Option<&Track> {
        self.tracks.get(scene)
    }

    /// 1 つの場面の束を、無ければ作って返す。
    pub fn track_mut(&mut self, scene: &str) -> &mut Track {
        self.tracks
            .entry(scene.to_owned())
            .or_insert_with(Track::empty)
    }

    /// 場面の一覧。<strong>昇順。</strong>
    ///
    /// <strong>トラックのある場面である。</strong> 本文だけが入っていて `build` していない場面も
    /// 含む——`add` がトラックを作るからである。
    #[must_use]
    pub fn scenes(&self) -> Vec<&str> {
        self.tracks.keys().map(String::as_str).collect()
    }

    /// 1 つの場面の派生物を捨てる。<strong>原本は残る。</strong>
    pub fn drop_derived(&mut self, scene: &str) {
        if let Some(t) = self.tracks.get_mut(scene) {
            t.derived = Derived::dropped();
        }
    }

    /// すべての場面の派生物を捨てる。
    ///
    /// <strong>本文が変われば、どの場面の派生物も信じられない。</strong> 語彙は場面ごとでも、
    /// 取り込み元と道具は共有だからである。
    pub fn drop_all_derived(&mut self) {
        for t in self.tracks.values_mut() {
            t.derived = Derived::dropped();
        }
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
    pub fn bundles(&self, scene: &str, role: Role) -> Vec<(String, Document)> {
        bundle(&self.corpus.in_scene(scene, role))
    }

    /// 人らしさの人の側に足せる他人の文書。<strong>束ねたものは 1 つにまとめて返す。</strong>
    #[must_use]
    pub fn other_bundles(&self) -> Vec<(String, Document)> {
        bundle(&self.corpus.for_humanness())
    }

    /// 前に出す指標から外すか。
    ///
    /// <strong>書いていなければ「未知」で、前に出す指標に入る。</strong> 動かないと分かるまでは使う。
    #[must_use]
    pub fn is_stuck(&self, scene: &str, metric: &str) -> bool {
        self.track(scene)
            .and_then(|t| t.decided.movement.get(metric))
            == Some(&Movement::Stuck)
    }
}

/// 束ねる。<strong>同じ `unit` を持つ文書を 1 本にする。</strong>
fn bundle(units: &[&Unit]) -> Vec<(String, Document)> {
    let mut by_unit: BTreeMap<&str, Vec<&Unit>> = BTreeMap::new();
    for u in units {
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

#[cfg(test)]
mod tests {
    use super::*;
    use kakiburi_doc::node::{Kind, Node};

    fn document(text: &str) -> Document {
        Document::new(vec![Node::leaf(Kind::Paragraph, text)])
    }

    fn cassette() -> Cassette {
        Cassette {
            version: store::VERSION,
            generation: 0,
            fingerprint: Fingerprint::build(Inputs {
                common: Common {
                    metric_definitions: "51 本".into(),
                    unit_definitions: "版 1".into(),
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
                },
                scenes: BTreeMap::new(),
            }),
            provisional: vec!["除外の既定".into()],
            corpus: Corpus::new(vec![
                Unit {
                    name: "p02".into(),
                    unit: "p02".into(),
                    belongs: Belongs::Person {
                        scene: "技術記事".into(),
                    },
                    document: document("本人の文書である。"),
                },
                Unit {
                    name: "p01".into(),
                    unit: "p01".into(),
                    belongs: Belongs::Person {
                        scene: "技術記事".into(),
                    },
                    document: document("もう 1 本の本人の文書。"),
                },
                Unit {
                    name: "c01".into(),
                    unit: "c01".into(),
                    belongs: Belongs::Person {
                        scene: "チャット".into(),
                    },
                    document: document("短いやつ。"),
                },
                Unit {
                    name: "b01".into(),
                    unit: "b01".into(),
                    belongs: Belongs::Baseline {
                        scene: "技術記事".into(),
                    },
                    document: document("基準の文書である。"),
                },
                Unit {
                    name: "o01".into(),
                    unit: "o01".into(),
                    belongs: Belongs::Other,
                    document: document("他人の文書である。"),
                },
            ]),
            tracks: [
                (
                    "技術記事".to_owned(),
                    Track {
                        decided: Decided {
                            boilerplate: vec![],
                            baseline: Baseline {
                                model: "m".into(),
                                version: "v".into(),
                                params: BTreeMap::new(),
                                topics: vec!["t".into()],
                            },
                            movement: [("笑い".to_owned(), Movement::Stuck)].into(),
                        },
                        derived: Derived {
                            scale: Some("天井と床".into()),
                            ..Derived::default()
                        },
                    },
                ),
                ("チャット".to_owned(), Track::empty()),
            ]
            .into(),
        }
    }

    #[test]
    fn 場面と役で単位を引ける() {
        let c = cassette();
        assert_eq!(c.corpus.in_scene("技術記事", Role::Person).len(), 2);
        assert_eq!(c.corpus.in_scene("技術記事", Role::BaselineOutput).len(), 1);
        assert_eq!(c.corpus.in_scene("チャット", Role::Person).len(), 1);
        assert_eq!(
            c.corpus.in_scene("チャット", Role::BaselineOutput).len(),
            0,
            "場面ごとに閉じる"
        );
    }

    #[test]
    fn 他人の文書は場面で絞る口から出てこない() {
        // 場面を跨げるのは人らしさの人の側だけである。**照合の側へ漏れれば、
        // 相手集合に他人が混ざる。**
        let c = cassette();
        for scene in ["技術記事", "チャット"] {
            for role in [Role::Person, Role::BaselineOutput, Role::Other] {
                assert!(
                    c.corpus
                        .in_scene(scene, role)
                        .iter()
                        .all(|u| u.belongs != Belongs::Other),
                    "{scene} / {role:?}"
                );
            }
        }
        assert_eq!(
            c.corpus.for_humanness().len(),
            1,
            "人らしさの口からは取れる"
        );
    }

    #[test]
    fn 単位は名前の昇順で並ぶ() {
        let c = cassette();
        let names: Vec<&str> = c
            .corpus
            .in_scene("技術記事", Role::Person)
            .iter()
            .map(|u| u.name.as_str())
            .collect();
        assert_eq!(names, vec!["p01", "p02"]);
    }

    #[test]
    fn 派生物を捨てても原本は残る() {
        let mut c = cassette();
        assert!(c.track("技術記事").unwrap().derived.has_scale());
        c.drop_derived("技術記事");
        assert!(!c.track("技術記事").unwrap().derived.has_scale());
        assert_eq!(c.corpus.len(), 5, "本文は残る");
        assert!(
            c.track("技術記事")
                .unwrap()
                .decided
                .movement
                .contains_key("笑い"),
            "決めたことも残る"
        );
    }

    #[test]
    fn 派生物は場面ごとに捨てる() {
        // 語彙も重みも帯も場面ごとに作るので、1 つの場面を作り直しても
        // ほかの場面の目盛りは生きている。
        let mut c = cassette();
        c.track_mut("チャット").derived.scale = Some("別の天井と床".into());
        c.drop_derived("技術記事");
        assert!(!c.track("技術記事").unwrap().derived.has_scale());
        assert!(c.track("チャット").unwrap().derived.has_scale());
    }

    #[test]
    fn 目盛りが無いカセットは正常な状態である() {
        // 素材が足りずに作れなかったのは異常ではない。
        let mut c = cassette();
        c.drop_all_derived();
        assert!(!c.track("技術記事").unwrap().derived.has_scale());
    }

    #[test]
    fn 書いていない指標は未知で前に出す() {
        // 動かないと分かるまでは使う。
        let c = cassette();
        assert!(c.is_stuck("技術記事", "笑い"), "stuck と書いてある");
        assert!(!c.is_stuck("技術記事", "全角括弧"), "書いていなければ未知");
        assert!(!c.is_stuck("チャット", "笑い"), "場面が違えば別に決め直す");
    }

    #[test]
    fn 場面の名前に階層の区切りを許さない() {
        // 保存の中では階層の名前になる。
        assert!(scene_name_ok("技術記事"));
        assert!(!scene_name_ok(""));
        assert!(!scene_name_ok("技術/記事"));
        assert!(!scene_name_ok(".."));
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
