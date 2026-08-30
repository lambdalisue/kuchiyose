//! 指標を決める。登録簿と計測。
//!
//! <strong>登録簿がこのクレートの本体である。</strong>「使う側は一覧を持たない」を守る唯一の場所。

pub mod definitions;
pub mod humanness;
pub mod lexicon;
pub mod matching;
pub mod mecab;
pub mod morph;
pub mod phrase;
pub mod registry;
pub mod structure;
pub mod symbol;
pub mod system;
pub mod tag;
pub mod word;

pub use definitions::Definition;
pub use humanness::Humanness;
pub use registry::{Entry, Registry, RegistryError};
pub use system::{Layer, System};
pub use tag::{Class, Direction, Tag, TagError};

/// 除外の既定。<strong>単位ごとの下限をここで 1 度だけ決める。</strong>
///
/// 同じことを 50 のファイルに書き写さない。定義ファイルが `除外` を書くのは、
/// <strong>既定と違うときだけ</strong>である。
pub mod floor {
    /// 日本語の文字。
    pub const JAPANESE_CHARS: usize = 1000;
    /// 延べ語数。<strong>字数と同じ値だが単位が違う</strong>——揃えて 1 つにしてはいけない。
    pub const TOKENS: usize = 1000;
    /// 生成した bigram の数。
    ///
    /// <strong>素材の下限とは別に掛かる。</strong> bigram は
    /// [node を跨がない](../../../docs/spec/020-document.md#地の文は-1-本の文字列ではない)ので、
    /// 形態素が 1,000 個あっても node がすべて 1 形態素なら 1 つも作られない。
    /// <strong>分母が 0 の分布は、0 が並んだ分布ではない。</strong>
    pub const BIGRAMS: usize = 1000;
    /// 対象の品詞に当たった形態素の数。
    ///
    /// 延べ 1,000 形態素あっても、機能語が 1 つも無ければ分布にならない。
    pub const FUNCTION_WORDS: usize = 100;
    /// 地の文のバイト数。<strong>圧縮器のヘッダが結果を支配しない大きさ。</strong>
    pub const PROSE_BYTES: usize = 2000;
    /// 文。
    pub const SENTENCES: usize = 20;
    /// 段落。
    pub const PARAGRAPHS: usize = 5;
    /// 節。
    pub const SECTIONS: usize = 3;
    /// node。見出し・箇条書き・表など。
    pub const NODES: usize = 3;
    /// 項目。
    pub const ITEMS: usize = 5;
}

/// 測った結果。
///
/// <strong>0 と「測っていない」は別である。</strong> 下限を下回った指標は 0 を返さない——
/// 0 は「使わなかった」という値である。
///
/// <strong>測っていない理由も 1 つにまとめない。</strong> 理由によって、使う側が取れる手が違う。
/// まとめて「短すぎる」として返せば、辞書を入れ忘れた環境が「素材が足りない」という
/// 顔で回り続け、<strong>素材をいくら足しても直らない。</strong>
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Measured {
    /// 値が出た。
    Value(f64),
    /// 下限を下回った。素材はあるが分母が足りない。<strong>長い文書を足せば直る。</strong>
    BelowFloor,
    /// 分母が 0。素材はあるが、その現象が 1 つも作られない。
    ///
    /// <strong>手は無い。</strong> その文書では測れない——node がすべて 1 単位なら bigram は
    /// 1 つも作られず、素材を足しても同じことが起きる。
    NoDenominator,
    /// この取り込み元では記法が書けない。<strong>別の取り込み元で集め直せば直る。</strong>
    NotWritable,
    /// 道具が無い。形態素解析器や外部の表が用意されていない。
    ///
    /// <strong>コーパスの性質ではなく環境の壊れである。</strong> 素材を足しても直らない。
    ToolMissing,
    /// 道具が失敗した。解析器が返さなかった。
    ///
    /// <strong>入力か道具の不具合である。</strong> 報告する。
    ToolFailed,
}

/// 測れない理由。<strong>使う側が取れる手で分ける。</strong>
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Unmeasured {
    /// 道具が失敗した。
    ToolFailed,
    /// 道具が無い。
    ToolMissing,
    /// 書けない記法。
    NotWritable,
    /// 分母が 0。
    NoDenominator,
    /// 下限未満。
    BelowFloor,
}

impl Unmeasured {
    /// 使う側に見せる名前。
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Unmeasured::ToolFailed => "道具が失敗した",
            Unmeasured::ToolMissing => "道具が無い",
            Unmeasured::NotWritable => "書けない記法",
            Unmeasured::NoDenominator => "分母が 0",
            Unmeasured::BelowFloor => "下限未満",
        }
    }

    /// コーパスの性質か。<strong>そうでなければ環境の壊れである。</strong>
    ///
    /// 環境の壊れは[出現割合の分母から外して済ませない](../../../docs/spec/200-extract.md#下限は使った割合で見る)
    /// ——外せば、辞書を入れ忘れたまままともな値が出ているように見える。
    #[must_use]
    pub fn is_corpus(self) -> bool {
        matches!(
            self,
            Unmeasured::NotWritable | Unmeasured::NoDenominator | Unmeasured::BelowFloor
        )
    }
}

impl Measured {
    /// 値があれば返す。
    #[must_use]
    pub fn value(self) -> Option<f64> {
        match self {
            Measured::Value(v) => Some(v),
            _ => None,
        }
    }

    /// 測れなかった理由。<strong>測れていれば `None`。</strong>
    #[must_use]
    pub fn unmeasured(self) -> Option<Unmeasured> {
        match self {
            Measured::Value(_) => None,
            Measured::BelowFloor => Some(Unmeasured::BelowFloor),
            Measured::NoDenominator => Some(Unmeasured::NoDenominator),
            Measured::NotWritable => Some(Unmeasured::NotWritable),
            Measured::ToolMissing => Some(Unmeasured::ToolMissing),
            Measured::ToolFailed => Some(Unmeasured::ToolFailed),
        }
    }

    /// 測れたか。
    #[must_use]
    pub fn is_measured(self) -> bool {
        matches!(self, Measured::Value(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 測っていないものは値を返さない() {
        // 0 を返せば「使わなかった」と読まれる。
        for m in [
            Measured::BelowFloor,
            Measured::NoDenominator,
            Measured::NotWritable,
            Measured::ToolMissing,
            Measured::ToolFailed,
        ] {
            assert_eq!(m.value(), None, "{m:?}");
        }
        assert_eq!(Measured::Value(0.0).value(), Some(0.0));
    }

    #[test]
    fn ゼロは測れた値である() {
        assert!(Measured::Value(0.0).is_measured());
        assert!(!Measured::BelowFloor.is_measured());
        assert_eq!(Measured::Value(0.0).unmeasured(), None);
    }

    #[test]
    fn 理由は_5_つに分かれる() {
        // まとめて返せば、辞書を入れ忘れた環境が「素材が足りない」という顔で回る。
        let all = [
            Measured::BelowFloor,
            Measured::NoDenominator,
            Measured::NotWritable,
            Measured::ToolMissing,
            Measured::ToolFailed,
        ];
        let mut reasons: Vec<Unmeasured> = all.iter().filter_map(|m| m.unmeasured()).collect();
        let before = reasons.len();
        reasons.sort_unstable();
        reasons.dedup();
        assert_eq!(reasons.len(), before, "5 つとも別の理由である");
        assert_eq!(before, 5);
    }

    #[test]
    fn 環境の壊れとコーパスの性質を分ける() {
        // 環境の壊れは素材を足しても直らない。分母から外して済ませてはいけない。
        assert!(Unmeasured::BelowFloor.is_corpus());
        assert!(Unmeasured::NoDenominator.is_corpus());
        assert!(Unmeasured::NotWritable.is_corpus());
        assert!(!Unmeasured::ToolMissing.is_corpus());
        assert!(!Unmeasured::ToolFailed.is_corpus());
    }

    #[test]
    fn 理由の順は環境の壊れが先である() {
        // 2 つ以上が当たったときは、直せる手がいちばん限られている側を返す。
        assert!(Unmeasured::ToolFailed < Unmeasured::ToolMissing);
        assert!(Unmeasured::ToolMissing < Unmeasured::NotWritable);
        assert!(Unmeasured::NotWritable < Unmeasured::NoDenominator);
        assert!(Unmeasured::NoDenominator < Unmeasured::BelowFloor);
    }
}
