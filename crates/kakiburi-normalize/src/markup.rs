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

/// HTML の要素名。小文字で持つ。
///
/// HTML Living Standard の要素の索引と、廃止された要素（同じ規格の「Obsolete
/// features」の節）を合わせたものである。 廃止された要素も入れるのは、`<blink>` や
/// `<font>` を書く人がいまもいて、それを地の文として通すと札が記号として数えられる
/// からである。
///
/// Markdown の中で `<` と `>` に挟まれた語は、ここにある名前のときだけ札として扱う。
/// 無い名前は地の文である——技術記事の `Box<dyn Trait>` や `Vec<T>` は型の引数であって、
/// 札として断れば記事が丸ごと入らない。
///
/// 名前の大小は区別しない（HTML と同じ）。 だから `Option<U>` の `<U>` は下線の札になる。
pub const HTML_ELEMENTS: &[&str] = &[
    // 要素の索引。
    "a",
    "abbr",
    "address",
    "area",
    "article",
    "aside",
    "audio",
    "b",
    "base",
    "bdi",
    "bdo",
    "blockquote",
    "body",
    "br",
    "button",
    "canvas",
    "caption",
    "cite",
    "code",
    "col",
    "colgroup",
    "data",
    "datalist",
    "dd",
    "del",
    "details",
    "dfn",
    "dialog",
    "div",
    "dl",
    "dt",
    "em",
    "embed",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hgroup",
    "hr",
    "html",
    "i",
    "iframe",
    "img",
    "input",
    "ins",
    "kbd",
    "label",
    "legend",
    "li",
    "link",
    "main",
    "map",
    "mark",
    "math",
    "menu",
    "meta",
    "meter",
    "nav",
    "noscript",
    "object",
    "ol",
    "optgroup",
    "option",
    "output",
    "p",
    "picture",
    "pre",
    "progress",
    "q",
    "rp",
    "rt",
    "ruby",
    "s",
    "samp",
    "script",
    "search",
    "section",
    "select",
    "slot",
    "small",
    "source",
    "span",
    "strong",
    "style",
    "sub",
    "summary",
    "sup",
    "svg",
    "table",
    "tbody",
    "td",
    "template",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "time",
    "title",
    "tr",
    "track",
    "u",
    "ul",
    "var",
    "video",
    "wbr",
    // 廃止された要素。
    "acronym",
    "applet",
    "basefont",
    "bgsound",
    "big",
    "blink",
    "center",
    "dir",
    "font",
    "frame",
    "frameset",
    "image",
    "isindex",
    "keygen",
    "listing",
    "marquee",
    "menuitem",
    "multicol",
    "nextid",
    "nobr",
    "noembed",
    "noframes",
    "param",
    "plaintext",
    "rb",
    "rtc",
    "spacer",
    "strike",
    "tt",
    "xmp",
];

/// HTML の要素名か。大小は区別しない。
#[must_use]
pub fn is_html_element(name: impl AsRef<str>) -> bool {
    let name = name.as_ref().to_ascii_lowercase();
    HTML_ELEMENTS.contains(&name.as_str())
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
    fn 要素名は規格の名前だけである() {
        for name in ["em", "EM", "aside", "blink", "h6"] {
            assert!(is_html_element(name), "{name}");
        }
        for name in ["dyn", "t", "e", "string", "h7"] {
            assert!(!is_html_element(name), "{name}");
        }
    }

    #[test]
    fn 対応表の札はどれも要素名である() {
        // 対応表に要素名でない名前があれば、その札は地の文として通って引かれない。
        for name in HTML_ELEMENTS {
            assert_eq!(*name, name.to_ascii_lowercase(), "小文字で持つ");
        }
        for name in [
            "aside",
            "details",
            "table",
            "td",
            "th",
            "blockquote",
            "p",
            "ul",
            "ol",
            "li",
            "pre",
            "code",
            "hr",
            "img",
            "em",
            "strong",
            "b",
            "i",
            "a",
            "h1",
            "h6",
        ] {
            assert!(html_kind(name).is_some() && is_html_element(name), "{name}");
        }
    }

    #[test]
    fn html_の対応表に警告は無い() {
        // 「書けない」升目。0 ではなく測れないになる。
        assert!(!html_kind("aside").iter().any(|&k| k == Kind::Warning));
        assert_eq!(html_kind("warning"), None);
    }
}
