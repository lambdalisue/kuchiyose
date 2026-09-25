//! カセット。1 人の 1 場面ぶんの目盛りである。
//!
//! | | |
//! | --- | --- |
//! | `decided/` | 人が決めたこと。作り直せない |
//! | `derived/` | 派生物。素材から作り直せる |
//!
//! 本文は持たない（[素材を正本にする](../../../docs/spec/200-extract.md#素材を正本にする)）。
//! 素材のフォルダが原本で、カセットはそこから作った目盛りだけを持つ。
//!
//! 1 カセットが 1 場面である（[場面ごとに閉じる](../../../docs/spec/010-strategy.md#場面ごとに閉じる)）。
//! 入れ物が境界そのものなので、場面を跨いだ目盛りは書けない。

pub mod fingerprint;
pub mod json;
pub mod save;
pub mod store;
pub mod zip;

use std::collections::BTreeMap;

pub use fingerprint::{Baseline, Common, Fingerprint, Inputs, Normalization, SceneInputs, Tool};

/// 単位の役。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// 本人の文書。
    Person,
    /// 基準。LLM の既定出力。
    BaselineOutput,
    /// 他人の文書。無くてよい。
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

/// 場面の名前として使えるか。
///
/// 空を断る——空の場面は「場面を決めていない」と見分けが付かない。
/// 1 カセットが 1 場面になって保存の中の階層名ではなくなったので、
/// `/` は断らない。
#[must_use]
pub fn scene_name_ok(scene: &str) -> bool {
    !scene.trim().is_empty()
}

/// 人が決めたこと。作り直せない原本である。
///
/// 場面は持たない。 どの場面のものかは[カセット](Cassette::scene)が言う——
/// 2 か所に置けば、食い違ったときに正本が決まらない。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Decided {
    /// 落とす定型。
    pub boilerplate: Vec<String>,
    /// 基準の作り方。
    pub baseline: Baseline,
    /// 指示して動くか。直させてみて初めて分かる。
    ///
    /// 書いていない指標は「未知」で、前に出す指標に入る。
    pub movement: BTreeMap<String, Movement>,
}

/// 指示して動くか。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Movement {
    /// 動く。
    Moves,
    /// 動かないと分かった。前に出す指標から外れる。
    Stuck,
}

/// 派生物。いつでも捨ててよい。
///
/// 型が `Option` なのは「まだ作っていない」を表すためである。捨てられることが
/// 型に出ている。
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
    /// 言い回し → 本人の上限。本文の代わりである。
    ///
    /// [繰り返せと言うなら上限も言う](../../../docs/spec/300-revise.md#繰り返せと言うなら上限も言う)
    /// は、どの言い回しを訊かれるかが検めるまで決まらないので、本文を走査していた。
    /// 2 つ以上の単位に現れる言い回しと上限だけに畳む。
    pub phrases: Option<String>,
}

impl Derived {
    /// 全部捨てる。
    #[must_use]
    pub fn dropped() -> Self {
        Self::default()
    }

    /// 目盛りができているか。
    ///
    /// できていないカセットは正常な状態である。 素材が足りずに作れなかったのは
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
    /// 世代。書くたびに 1 つ増える。
    ///
    /// 同時に 2 つが書くと、片方の変更が正常終了のまま消える。落ちるより悪い
    /// ——誰も気付かない。読んだときの世代と、置き換える直前の世代が同じことを
    /// 確かめて防ぐ。
    ///
    /// [指紋](Fingerprint)では検出できない。 指紋は測った条件を表すもので、
    /// 本文を差し替えても条件が同じなら変わらない。
    pub generation: u64,
    /// 指紋。道具・実装・定義の版・取り込み元・語彙・z 得点。
    pub fingerprint: Fingerprint,
    /// 暫定値が立っている箇所。
    ///
    /// 空でないカセットは、判定に但し書きが付く。 いまは常に立つ。
    pub provisional: Vec<String>,
    /// この目盛りが何の場面のものか。
    ///
    /// 1 カセットが 1 場面である（[決定](../../../docs/spec/010-strategy.md#場面ごとに閉じる)）。
    /// 入れ物が境界そのものなので、場面を跨いだ目盛りは書けない。
    ///
    /// 名前は人が付ける。中身と合っている保証は無い——だから
    /// [検めるたびに名乗る](../../../docs/spec/300-revise.md#場面を指定させる)。
    /// 道具が当てにいくためのものではない。
    pub scene: String,
    /// 人が決めたこと。作り直せない原本である。
    pub decided: Decided,
    /// 派生物。素材から作り直せる。
    pub derived: Derived,
}

impl Cassette {
    /// 前に出す指標から外すか。
    ///
    /// 書いていなければ「未知」で、前に出す指標に入る。 動かないと分かるまでは使う。
    #[must_use]
    pub fn is_stuck(&self, metric: &str) -> bool {
        self.decided.movement.get(metric) == Some(&Movement::Stuck)
    }

    /// 派生物を捨てる。決めたことは残る。
    pub fn drop_derived(&mut self) {
        self.derived = Derived::dropped();
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    
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
                        sources: vec!["markdown".into()],
                        implementation: "kakiburi-normalize".into(),
                        version: "0.0.0".into(),
                        mapping: BTreeMap::new(),
                    },
                },
                scene: SceneInputs::default(),
            }),
            provisional: vec!["除外の既定".into()],
            scene: "技術記事".into(),
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
        }
    }

    #[test]
    fn 派生物を捨てても決めたことは残る() {
        let mut c = cassette();
        assert!(c.derived.has_scale());
        c.drop_derived();
        assert!(!c.derived.has_scale());
        assert!(c.decided.movement.contains_key("笑い"), "決めたことは残る");
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
    fn 場面の名前は空を断る() {
        // 空の場面は「場面を決めていない」と見分けが付かない。
        // **入れ物の中の階層名ではなくなったので、`/` は断らない。**
        assert!(scene_name_ok("技術記事"));
        assert!(scene_name_ok("技術/記事"));
        assert!(!scene_name_ok(""));
        assert!(!scene_name_ok("   "));
    }

    #[test]
    fn 暫定値が立っていることを持つ() {
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
