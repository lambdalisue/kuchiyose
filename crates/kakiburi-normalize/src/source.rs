//! 取り込み元と、その対応表。
//!
//! 記法は取り込み元の事情である。だから記法から意味への対応表は、ここが持つ。

use kakiburi_doc::node::Kind;

/// 取り込み元の種類。
///
/// 内容から判定しない。拡張子で決まる粒度にとどめる——推測を混ぜれば決定的でなくなる。
///
/// Markdown を方言に分けない。 Alert（`> [!NOTE]`）と directive（`:::note`）は構文として
/// 重ならないので、1 つの読み方で両方を認識できる。分ければ名乗り違えが生まれ、
/// directive の側で読んだ `> [!NOTE]` は黙って引用に化ける。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    /// Markdown + Alert 記法（`> [!NOTE]`）+ directive 記法（`:::note`）。
    Markdown,
    /// HTML。
    Html,
}

/// 記法で書けるか。
///
/// 0 と「測れない」を区別する。 0 を並べれば、その書き手は補足を使わない人だと
/// 判定される。書けない記法の升目が、そのまま「測れない」を返す場所である。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Writable {
    /// 書ける。値が 0 なら「使わなかった」である。
    Yes,
    /// 書けない。値は「測っていない」であって 0 ではない。
    No,
}

/// 対応表の版。升目の意味を変えたら手で上げる。
///
/// 升目そのものも指紋に出るので、上げ忘れても中身の差で気付ける。逆も同じで、
/// 升目が同じまま解釈だけを変えたときは、この版でしか気付けない。
pub const MAPPING_VERSION: &str = "対応表 3";

/// 升目が分かれる node。これ以外はどの取り込み元でも書ける。
const GATED: [Kind; 5] = [
    Kind::Note,
    Kind::Warning,
    Kind::Footnote,
    Kind::Details,
    Kind::Table,
];

impl Source {
    /// 扱う取り込み元をすべて。指紋は全部の升目を写す。
    #[must_use]
    pub fn all() -> [Source; 2] {
        [Source::Markdown, Source::Html]
    }

    /// 升目の中身。指紋に入る。
    ///
    /// 実装の版だけでは足りない。 升目を書き換えて版を上げ忘れれば、指紋が同じ
    /// まま別の木が出る——そして「0」と「測れない」の出方が静かに入れ替わる。
    ///
    /// 平文で残す。ハッシュだけでは、どの升目が変わったかが分からない。
    ///
    /// 版も一緒に持つ。 手で上げる版と、機械が出す升目の両方が変わらないときだけ、
    /// 過去の値と比べてよい。
    #[must_use]
    pub fn mapping_digest(self) -> String {
        let cells: String = GATED
            .iter()
            .map(|&k| match self.writable(k) {
                Writable::Yes => '1',
                Writable::No => '0',
            })
            .collect();
        format!("{MAPPING_VERSION} 補足警告脚注折表:{cells}")
    }

    /// 名前。指紋に入る。
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Source::Markdown => "markdown",
            Source::Html => "html",
        }
    }

    /// 名前から引く。対応表に無い名前は受けない。
    #[must_use]
    pub fn from_name(name: impl AsRef<str>) -> Option<Self> {
        match name.as_ref() {
            "markdown" => Some(Source::Markdown),
            "html" => Some(Source::Html),
            _ => None,
        }
    }

    /// この取り込み元で、その node を記法として書けるか。
    ///
    /// 仕様の升目をそのまま写す。書けない升目は「測れない」を返す根拠になる。
    #[must_use]
    pub fn writable(self, kind: Kind) -> Writable {
        use Kind::{Details, Footnote, Note, Table};
        let yes = match self {
            Source::Markdown => true,
            // HTML に警告の記法は無い。`<aside>` は補足に落ちる。
            Source::Html => matches!(kind, Note | Footnote | Details | Table),
        };
        // 上の 5 つ以外は、どの取り込み元でも書ける記法がある。
        if yes || !GATED.contains(&kind) {
            Writable::Yes
        } else {
            Writable::No
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 名前は往復する() {
        for s in [Source::Markdown, Source::Html] {
            assert_eq!(Source::from_name(s.name()), Some(s));
        }
    }

    #[test]
    fn 対応表に無い名前は受けない() {
        assert_eq!(Source::from_name("github-markdown"), None);
        assert_eq!(Source::from_name("directive-markdown"), None);
        assert_eq!(Source::from_name("plain-markdown"), None);
        assert_eq!(Source::from_name("rst"), None);
    }

    #[test]
    fn html_には警告の記法が無い() {
        assert_eq!(Source::Html.writable(Kind::Warning), Writable::No);
        assert_eq!(Source::Html.writable(Kind::Note), Writable::Yes);
        assert_eq!(Source::Html.writable(Kind::Footnote), Writable::Yes);
        assert_eq!(Source::Html.writable(Kind::Details), Writable::Yes);
        assert_eq!(Source::Html.writable(Kind::Table), Writable::Yes);
    }

    #[test]
    fn 段落や見出しはどの取り込み元でも書ける() {
        for s in [Source::Markdown, Source::Html] {
            for k in [Kind::Paragraph, Kind::Heading, Kind::Bullet, Kind::Quote] {
                assert_eq!(s.writable(k), Writable::Yes, "{s:?} の {k:?}");
            }
        }
    }
}
