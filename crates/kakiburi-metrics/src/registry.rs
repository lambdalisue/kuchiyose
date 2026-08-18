//! 登録簿。<strong>このクレートの本体である。</strong>
//!
//! 「使う側は一覧を持たない」を守る唯一の場所。名前を 2 か所に書けば、片方を
//! 直したときにもう片方が古いまま残り、<strong>エラーにならない。</strong>

use crate::tag::{Tag, TagError};

/// 登録簿の 1 行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// 指標の名前。定義ファイルの 1 行目と同じ。
    pub name: String,
    /// 札。
    pub tag: Tag,
}

/// 登録簿。
#[derive(Debug, Clone, Default)]
pub struct Registry {
    entries: Vec<Entry>,
}

/// 登録簿に入れられない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// 札が読めない。
    Tag {
        /// どの指標か。
        name: String,
        /// 何が読めないか。
        source: TagError,
    },
    /// 同じ名前が 2 度出てきた。
    Duplicate {
        /// 重なった名前。
        name: String,
    },
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::Tag { name, source } => write!(f, "{name}: {source}"),
            RegistryError::Duplicate { name } => write!(f, "{name} が 2 度ある"),
        }
    }
}

impl std::error::Error for RegistryError {}

impl Registry {
    /// 空の登録簿。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 1 行足す。
    ///
    /// 同じ名前は 2 度入れられない——一覧が 2 か所にある状態を型で止める。
    pub fn insert(
        &mut self,
        name: impl Into<String>,
        tag_line: impl AsRef<str>,
    ) -> Result<(), RegistryError> {
        let name = name.into();
        if self.entries.iter().any(|e| e.name == name) {
            return Err(RegistryError::Duplicate { name });
        }
        let tag = Tag::parse(tag_line).map_err(|source| RegistryError::Tag {
            name: name.clone(),
            source,
        })?;
        self.entries.push(Entry { name, tag });
        Ok(())
    }

    /// 全部。
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// 名前で引く。
    #[must_use]
    pub fn get(&self, name: impl AsRef<str>) -> Option<&Entry> {
        let n = name.as_ref();
        self.entries.iter().find(|e| e.name == n)
    }

    /// 種別で絞る。<strong>使う側は一覧を持たず、ここに来る。</strong>
    pub fn by_kind<'a>(
        &'a self,
        pick: impl Fn(&Tag) -> bool + 'a,
    ) -> impl Iterator<Item = &'a Entry> {
        self.entries.iter().filter(move |e| pick(&e.tag))
    }

    /// 何本あるか。
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 空か。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::{Layer, System};
    use crate::tag::Class;

    fn sample() -> Registry {
        let mut r = Registry::new();
        r.insert("読点の打ち方", "照合 / 記号。").unwrap();
        r.insert(
            "接続詞直後の読点",
            "指示 / 読点の打ち方 / 記号 / 両側 / 割合。",
        )
        .unwrap();
        r.insert("脱線", "指示 / なし / 語 / 上限 / 日本語 1,000 字あたり。")
            .unwrap();
        r.insert("繰り返し", "人らしさ / 下限。").unwrap();
        r
    }

    #[test]
    fn 名前で引ける() {
        let r = sample();
        assert!(r.get("接続詞直後の読点").is_some());
        assert!(r.get("存在しない指標").is_none());
    }

    #[test]
    fn 同じ名前は_2_度入れられない() {
        // 一覧が 2 か所にある状態を型で止める。
        let mut r = sample();
        let e = r
            .insert("脱線", "指示 / なし / 語 / 上限 / 割合。")
            .unwrap_err();
        assert!(matches!(e, RegistryError::Duplicate { .. }), "{e:?}");
    }

    #[test]
    fn 読めない札は入らない() {
        let mut r = Registry::new();
        let e = r.insert("怪しい指標", "指示 / 記号 / 両側。").unwrap_err();
        assert!(matches!(e, RegistryError::Tag { .. }), "{e:?}");
        assert!(r.is_empty(), "半端に入れてはいけない");
    }

    #[test]
    fn 層は登録簿からも系統経由で引く() {
        let r = sample();
        let e = r.get("接続詞直後の読点").unwrap();
        assert_eq!(e.tag.layer(), Some(Layer::One), "読点の打ち方は層 1");
        let e = r.get("脱線").unwrap();
        assert_eq!(e.tag.layer(), Some(Layer::Three), "系統を名指しできない");
        let e = r.get("繰り返し").unwrap();
        assert_eq!(e.tag.layer(), None, "人らしさは層を持たない");
    }

    #[test]
    fn 種別で絞れる() {
        let r = sample();
        let matching: Vec<&str> = r
            .by_kind(|t| matches!(t, Tag::Matching { .. }))
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(matching, vec!["読点の打ち方"]);

        let humanness = r.by_kind(|t| matches!(t, Tag::Humanness { .. })).count();
        assert_eq!(humanness, 1);
    }

    #[test]
    fn 層_3_の指標は指摘から外せる() {
        let r = sample();
        let usable: Vec<&str> = r
            .by_kind(|t| matches!(t, Tag::Directive { .. }) && t.layer() != Some(Layer::Three))
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(usable, vec!["接続詞直後の読点"]);
    }

    #[test]
    fn 照合の系統は分類を持つ() {
        let r = sample();
        let Tag::Matching { class } = r.get("読点の打ち方").unwrap().tag else {
            panic!("照合であるべき");
        };
        assert_eq!(class, Class::Symbol);
    }

    #[test]
    fn 判定に使える系統は層_1_かつ保留でない() {
        assert!(System::Comma.usable_for_verdict());
        assert!(!System::BunsetsuPattern.usable_for_verdict());
        assert!(!System::Structure.usable_for_verdict());
    }
}
