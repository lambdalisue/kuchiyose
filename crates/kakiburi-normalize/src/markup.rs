//! 記法から意味への対応表。
//!
//! ここがいちばん危ない。 GitHub の Alert 記法は引用の記法の上に建っている。
//! 素朴に CommonMark として解釈すると引用になり、引用の密度が実際より高く出て、
//! 補足の密度に 0 が並ぶ。エラーにならない。 値が出て、判定が回り、結果だけが違う。

use kakiburi_doc::node::Kind;

/// Alert の名前から node を引く。
///
/// `IMPORTANT` を警告に入れるのは、書き手が「読み飛ばすな」と示している側だから
/// である。補足は本筋から外れることを示す記法で、向きが逆になる。
#[must_use]
pub fn alert_kind(name: impl AsRef<str>) -> Option<Kind> {
    match name.as_ref() {
        "NOTE" | "TIP" => Some(Kind::Note),
        "WARNING" | "CAUTION" | "IMPORTANT" => Some(Kind::Warning),
        _ => None,
    }
}

/// directive の名前から node を引く。
///
/// 名前は修飾を伴うことがある——Zenn の `:::message alert` は警告で、修飾の無い
/// `:::message` は補足である。だから名前だけでは決まらない。
#[must_use]
pub fn directive_kind(name: impl AsRef<str>) -> Option<Kind> {
    let raw = name.as_ref().trim();
    let mut parts = raw.split_whitespace();
    let head = parts.next()?;
    let modifier = parts.next();
    match (head, modifier) {
        ("note" | "tip", _) => Some(Kind::Note),
        ("warning" | "caution", _) => Some(Kind::Warning),
        // Zenn。`alert` が付くと警告になる。
        ("message", Some("alert")) => Some(Kind::Warning),
        ("message", None) => Some(Kind::Note),
        ("footnote", _) => Some(Kind::Footnote),
        ("details", _) => Some(Kind::Details),
        _ => None,
    }
}

/// HTML の要素名から node を引く。
#[must_use]
pub fn html_kind(tag: impl AsRef<str>) -> Option<Kind> {
    match tag.as_ref() {
        "aside" => Some(Kind::Note),
        "details" => Some(Kind::Details),
        "table" => Some(Kind::Table),
        "td" | "th" => Some(Kind::Cell),
        "blockquote" => Some(Kind::Quote),
        "p" => Some(Kind::Paragraph),
        "ul" => Some(Kind::Bullet),
        "ol" => Some(Kind::Ordered),
        "li" => Some(Kind::Item),
        // `<pre>` だけがコードブロックである。 段落の中の ``` まで
        // コードブロックにすると、インラインコードが永久に 0 になり、
        // コードブロックの密度は地の文に混ざらない記号のぶんだけ跳ね上がる。
        "pre" => Some(Kind::CodeBlock),
        "code" => Some(Kind::InlineCode),
        "hr" => Some(Kind::Divider),
        "img" => Some(Kind::Image),
        "em" | "strong" | "b" | "i" => Some(Kind::Emphasis),
        "a" => Some(Kind::Link),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => Some(Kind::Heading),
        _ => None,
    }
}

/// node を作らず、中身を親へ透かす要素か。
///
/// 行と区分は node ではない——[文書の形](../../../docs/spec/020-document.md#文書は-node-でできている)は
/// 表がセルを持つと定めており、あいだに段を置かない。node にすると、`<table>` と
/// `<tr>` の両方が表として数えられ、ふつうの表 1 つで表の密度が何倍にもなる。
///
/// ``` も `<pre>` の直下では透かす（[`html_kind`] を参照）が、そちらは
/// 文脈で決まるのでここには入らない。
#[must_use]
pub fn is_transparent(tag: impl AsRef<str>) -> bool {
    matches!(tag.as_ref(), "thead" | "tbody" | "tfoot" | "tr")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_と_tip_は補足() {
        assert_eq!(alert_kind("NOTE"), Some(Kind::Note));
        assert_eq!(alert_kind("TIP"), Some(Kind::Note));
    }

    #[test]
    fn important_は警告である() {
        // 書き手が「読み飛ばすな」と示している側。補足は向きが逆になる。
        assert_eq!(alert_kind("IMPORTANT"), Some(Kind::Warning));
        assert_eq!(alert_kind("WARNING"), Some(Kind::Warning));
        assert_eq!(alert_kind("CAUTION"), Some(Kind::Warning));
    }

    #[test]
    fn 対応表に無い_alert_は引けない() {
        // 断る根拠になる。推測して補足に落とさない。
        assert_eq!(alert_kind("HINT"), None);
        assert_eq!(alert_kind("note"), None, "大小は区別する");
    }

    #[test]
    fn directive_も同じ向きに落ちる() {
        assert_eq!(directive_kind("note"), Some(Kind::Note));
        assert_eq!(directive_kind("tip"), Some(Kind::Note));
        assert_eq!(directive_kind("warning"), Some(Kind::Warning));
        assert_eq!(directive_kind("caution"), Some(Kind::Warning));
        assert_eq!(directive_kind("details"), Some(Kind::Details));
        assert_eq!(directive_kind("footnote"), Some(Kind::Footnote));
    }

    #[test]
    fn aside_は補足である() {
        assert_eq!(html_kind("aside"), Some(Kind::Note));
    }

    #[test]
    fn html_の対応表に警告は無い() {
        // 「書けない」升目。0 ではなく測れないになる。
        assert!(!html_kind("aside").iter().any(|&k| k == Kind::Warning));
        assert_eq!(html_kind("warning"), None);
    }
}
