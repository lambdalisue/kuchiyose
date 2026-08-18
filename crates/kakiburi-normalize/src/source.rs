//! 取り込み元と、その対応表。
//!
//! 記法は取り込み元の事情である。だから記法から意味への対応表は、ここが持つ。

use kakiburi_doc::node::Kind;

/// 取り込み元の種類。
///
/// 内容から判定しない。呼ぶ側が指定する——推測を混ぜれば決定的でなくなる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    /// GitHub Flavored Markdown + Alert 記法。
    GithubMarkdown,
    /// CommonMark + directive 記法（`:::note`）。
    DirectiveMarkdown,
    /// HTML。
    Html,
    /// 素の CommonMark。
    PlainMarkdown,
}

/// 記法で書けるか。
///
/// <strong>0 と「測れない」を区別する。</strong> 0 を並べれば、その書き手は補足を使わない人だと
/// 判定される。書けない記法の升目が、そのまま「測れない」を返す場所である。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Writable {
    /// 書ける。値が 0 なら「使わなかった」である。
    Yes,
    /// 書けない。値は「測っていない」であって 0 ではない。
    No,
}

/// 対応表の版。<strong>升目の意味を変えたら手で上げる。</strong>
///
/// 升目そのものも指紋に出るので、上げ忘れても中身の差で気付ける。<strong>逆も同じで、
/// 升目が同じまま解釈だけを変えたときは、この版でしか気付けない。</strong>
pub const MAPPING_VERSION: &str = "対応表 1";

/// 升目が分かれる node。<strong>これ以外はどの取り込み元でも書ける。</strong>
const GATED: [Kind; 5] = [
    Kind::Note,
    Kind::Warning,
    Kind::Footnote,
    Kind::Details,
    Kind::Table,
];

impl Source {
    /// 扱う取り込み元をすべて。<strong>指紋は全部の升目を写す。</strong>
    #[must_use]
    pub fn all() -> [Source; 4] {
        [
            Source::GithubMarkdown,
            Source::DirectiveMarkdown,
            Source::Html,
            Source::PlainMarkdown,
        ]
    }

    /// 升目の中身。<strong>指紋に入る。</strong>
    ///
    /// <strong>実装の版だけでは足りない。</strong> 升目を書き換えて版を上げ忘れれば、指紋が同じ
    /// まま別の木が出る——そして「0」と「測れない」の出方が静かに入れ替わる。
    ///
    /// 平文で残す。ハッシュだけでは、<strong>どの升目が変わったか</strong>が分からない。
    ///
    /// <strong>版も一緒に持つ。</strong> 手で上げる版と、機械が出す升目の両方が変わらないときだけ、
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
            Source::GithubMarkdown => "github-markdown",
            Source::DirectiveMarkdown => "directive-markdown",
            Source::Html => "html",
            Source::PlainMarkdown => "plain-markdown",
        }
    }

    /// 名前から引く。対応表に無い名前は受けない。
    #[must_use]
    pub fn from_name(name: impl AsRef<str>) -> Option<Self> {
        match name.as_ref() {
            "github-markdown" => Some(Source::GithubMarkdown),
            "directive-markdown" => Some(Source::DirectiveMarkdown),
            "html" => Some(Source::Html),
            "plain-markdown" => Some(Source::PlainMarkdown),
            _ => None,
        }
    }

    /// この取り込み元で、その node を記法として書けるか。
    ///
    /// 仕様の升目をそのまま写す。書けない升目は「測れない」を返す根拠になる。
    #[must_use]
    pub fn writable(self, kind: Kind) -> Writable {
        use Kind::{Details, Footnote, Note, Table, Warning};
        let yes = match self {
            Source::GithubMarkdown | Source::DirectiveMarkdown => {
                matches!(kind, Note | Warning | Footnote | Details | Table)
            }
            // HTML に警告の記法は無い。`<aside>` は補足に落ちる。
            Source::Html => matches!(kind, Note | Footnote | Details | Table),
            // 素の CommonMark には 5 つとも無い。
            Source::PlainMarkdown => false,
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
        for s in [
            Source::GithubMarkdown,
            Source::DirectiveMarkdown,
            Source::Html,
            Source::PlainMarkdown,
        ] {
            assert_eq!(Source::from_name(s.name()), Some(s));
        }
    }

    #[test]
    fn 対応表に無い名前は受けない() {
        assert_eq!(Source::from_name("markdown"), None);
        assert_eq!(Source::from_name("rst"), None);
    }

    #[test]
    fn 素の_commonmark_では_5_つとも書けない() {
        // 0 を返せば「補足を使わない人」と判定される。ここは測れない。
        for k in [
            Kind::Note,
            Kind::Warning,
            Kind::Footnote,
            Kind::Details,
            Kind::Table,
        ] {
            assert_eq!(
                Source::PlainMarkdown.writable(k),
                Writable::No,
                "{k:?} は素の CommonMark で書けない"
            );
        }
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
        for s in [
            Source::GithubMarkdown,
            Source::DirectiveMarkdown,
            Source::Html,
            Source::PlainMarkdown,
        ] {
            for k in [Kind::Paragraph, Kind::Heading, Kind::Bullet, Kind::Quote] {
                assert_eq!(s.writable(k), Writable::Yes, "{s:?} の {k:?}");
            }
        }
    }
}
