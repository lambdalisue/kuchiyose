//! 形代。1 つのフォルダの文書を測った統計値と、人が決めた調整を収める。
//!
//! | | |
//! | --- | --- |
//! | `tuning.json` | 調整。作り直せない原本である |
//! | `persona.md` | ペルソナ。作り直せない原本である。持たなければ entry が無い |
//! | `stats/` | 統計値。素材のフォルダから作り直せる |
//!
//! 本文も目盛りも持たない（[何を収めるか](../../../docs/design/100-katashiro.md#何を収めるか)）。
//! 統計値はテキストとして抱えるだけで、形を知らない
//! （[理由](../../../docs/design/000-architecture.md#形代は中身の形を知らない)）。
//!
//! 1 形代が 1 場面である（[場面ごとに閉じる](../../../docs/spec/010-strategy.md#場面ごとに閉じる)）。

pub mod fingerprint;
pub mod json;
pub mod save;
pub mod sha256;
pub mod store;
pub mod tuning;
pub mod zip;

pub use fingerprint::{Fingerprint, Inputs, Normalization, Tool};
pub use tuning::{MuteKind, Register, Tuning};

/// 場面の名前として使えるか。
///
/// 空を断る——空の場面は「場面を決めていない」と見分けが付かない。
#[must_use]
pub fn scene_name_ok(scene: &str) -> bool {
    !scene.trim().is_empty()
}

/// 統計値。形を知らないテキストのまま持つ。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stats {
    /// 文書ごとの統計値。1 行が 1 文書の JSON である。
    pub documents: String,
    /// 語のまとめ方。JSON である。
    pub lexicon: String,
}

impl Stats {
    /// `stats/` の中の名前と中身。名前の昇順。
    #[must_use]
    pub fn entries(&self) -> [(&'static str, &str); 2] {
        [
            (store::DOCUMENTS, self.documents.as_str()),
            (store::LEXICON, self.lexicon.as_str()),
        ]
    }

    /// 中身のハッシュ。基準として渡したときに、どの基準かを名指す。
    ///
    /// テキストのまま取る。 形を知らなくても取れる。名前と長さも混ぜる——
    /// 片方の末尾をもう片方の先頭へ移しただけで同じハッシュにならないようにする。
    #[must_use]
    pub fn content_hash(&self) -> String {
        let mut h = sha256::Sha256::default();
        for (name, body) in self.entries() {
            h.update(format!("{name}\t{}\n", body.len()));
            h.update(body);
        }
        format!("sha256:{}", h.hex())
    }
}

/// 形代。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Katashiro {
    /// 版。
    pub version: u32,
    /// 世代。書くたびに 1 つ増える。
    ///
    /// 同時に 2 つが書くと、片方の変更が正常終了のまま消える。落ちるより悪い
    /// ——誰も気付かない。読んだときの世代と、置き換える直前の世代が同じことを
    /// 確かめて防ぐ。
    ///
    /// [指紋](Fingerprint)では検出できない。 指紋は測った条件を表すもので、
    /// 調整を書き換えても変わらない。
    pub generation: u64,
    /// この統計値が何の場面のものか。名札である。
    ///
    /// 名前は人が付ける。中身と合っている保証は無い——だから
    /// [検めるたびに名乗る](../../../docs/spec/300-revise.md#場面を指定させる)。
    pub scene: String,
    /// 指紋の材料のうち、道具の部分。
    pub inputs: Inputs,
    /// 調整。作り直せない原本である。
    pub tuning: Tuning,
    /// 統計値。素材から作り直せる。
    pub stats: Stats,
    /// ペルソナ。取り込んだ Markdown をそのまま持つ。作り直せない原本である。
    ///
    /// 形は知らない。 見出しと引用を確かめるのは取り込む口である
    /// （[理由](../../../docs/design/000-architecture.md#形代は中身の形を知らない)）。
    /// 中身のハッシュにも指紋にも入れない——測った値を変えないからである。
    pub persona: Option<String>,
    /// 統計値を作った素材のフォルダの絶対経路。`katashiro build` が書く。
    ///
    /// 手元の事情なので、中身のハッシュにも指紋にも入れない。 人から受け取った
    /// 形代では、その経路は手元に無い。
    pub material: Option<String>,
}

impl Katashiro {
    /// 新しく作る。世代は 0 で、書けば 1 になる。
    #[must_use]
    pub fn new(scene: impl Into<String>, inputs: Inputs, stats: Stats, tuning: Tuning) -> Self {
        Self {
            version: store::VERSION,
            generation: 0,
            scene: scene.into(),
            inputs,
            tuning,
            stats,
            persona: None,
            material: None,
        }
    }

    /// 指紋。道具の部分と、統計値の中身のハッシュから作る。
    ///
    /// 持たずに毎回作る。 持てば、統計値を差し替えたのに指紋を作り直し忘れる道が開く。
    #[must_use]
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::build(self.inputs.clone(), self.stats.content_hash())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats() -> Stats {
        Stats {
            documents: "{\"unit\":\"a\"}\n".into(),
            lexicon: "{\"pairs\":[]}".into(),
        }
    }

    #[test]
    fn 場面の名前は空を断る() {
        // 空の場面は「場面を決めていない」と見分けが付かない。
        assert!(scene_name_ok("技術記事"));
        assert!(scene_name_ok("技術/記事"));
        assert!(!scene_name_ok(""));
        assert!(!scene_name_ok("   "));
    }

    #[test]
    fn 中身のハッシュは統計値だけから決まる() {
        let a = stats();
        assert_eq!(a.content_hash(), stats().content_hash());
        assert!(a.content_hash().starts_with("sha256:"));
        let mut b = stats();
        b.documents.push(' ');
        assert_ne!(a.content_hash(), b.content_hash());
    }

    #[test]
    fn 中身のハッシュは既知の値に固定されている() {
        // 取り方を変えれば、同じ基準を名指していた過去の出力と比べられなくなる。
        // 変えるなら、ここを書き換えることで気付く。
        assert_eq!(
            Stats::default().content_hash(),
            format!(
                "sha256:{}",
                sha256::hex("stats/documents.jsonl\t0\nstats/lexicon.json\t0\n")
            )
        );
        assert_eq!(
            Stats::default().content_hash(),
            "sha256:f19b76cf07a8f69b60ae75842d6585415cf8afb873e28b06e56c214b3c248e9a"
        );
    }

    #[test]
    fn 境目を動かしただけでは同じハッシュにならない() {
        let a = Stats {
            documents: "ab".into(),
            lexicon: "c".into(),
        };
        let b = Stats {
            documents: "a".into(),
            lexicon: "bc".into(),
        };
        assert_ne!(a.content_hash(), b.content_hash());
    }
}
