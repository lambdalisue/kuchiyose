//! 指標を決める。登録簿と計測。
//!
//! <strong>登録簿がこのクレートの本体である。</strong>「使う側は一覧を持たない」を守る唯一の場所。

pub mod definitions;
pub mod humanness;
pub mod matching;
pub mod mecab;
pub mod morph;
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
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Measured {
    /// 値が出た。
    Value(f64),
    /// 下限を下回ったので測っていない。
    BelowFloor,
    /// この取り込み元では記法が書けないので測れない。
    NotWritable,
}

impl Measured {
    /// 値があれば返す。
    #[must_use]
    pub fn value(self) -> Option<f64> {
        match self {
            Measured::Value(v) => Some(v),
            Measured::BelowFloor | Measured::NotWritable => None,
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
        assert_eq!(Measured::BelowFloor.value(), None);
        assert_eq!(Measured::NotWritable.value(), None);
        assert_eq!(Measured::Value(0.0).value(), Some(0.0));
    }

    #[test]
    fn ゼロは測れた値である() {
        assert!(Measured::Value(0.0).is_measured());
        assert!(!Measured::BelowFloor.is_measured());
    }
}
