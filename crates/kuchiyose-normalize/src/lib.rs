//! 入力を正規形にする。記法から意味への対応表。
//!
//! 取り込み元ごとの実装を持つ唯一の場所である。GitHub の Alert を引用より先に
//! 認識する、といった取り込み元固有の知識をここに閉じこめる。
//!
//! 決定的である。 推測しない。落ちない入力は断る。

pub mod html;
pub mod markdown;
pub mod markup;
pub mod refuse;
pub mod source;
pub mod space;

use kuchiyose_doc::Document;

pub use refuse::Refusal;
pub use source::{Source, Writable};

/// 変換の版。指紋に出る。
///
/// 同じ入力から違う正規形が出るように変換を変えたら手で上げる。記法の認識、
/// 空白の扱い、断る条件がそれに当たる。 升目の意味の変更は
/// [`source::MAPPING_VERSION`] が別に持つ。
///
/// パッケージの版は使わない。 版は変換と無関係に上がるので、指紋に入れれば
/// 変換が同じでも版を上げるたびに形代が全部使えなくなり、変換を変えても
/// 版を上げるまでは指紋が動かない。
pub const REVISION: &str = "変換 1";

/// 取り込む。読んで、断るかを決める。
///
/// 取り込み元は内容から判定しない。 呼ぶ側が指定する——推測を混ぜれば
/// 決定的でなくなる。
pub fn normalize(input: impl AsRef<str>, source: Source) -> Result<Document, Refusal> {
    let doc = match source {
        Source::Markdown => markdown::parse(input)?,
        Source::Html => html::parse(input)?,
    };
    refuse::admit(&doc)?;
    Ok(doc)
}

/// この取り込み元で測れる node か。
///
/// 書けない記法の値は 0 ではなく「測れない」である。0 を並べれば、その書き手は
/// 補足を使わない人だと判定される。
#[must_use]
pub fn measurable(source: Source, kind: kuchiyose_doc::node::Kind) -> bool {
    source.writable(kind) == Writable::Yes
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuchiyose_doc::node::Kind;

    #[test]
    fn 日本語以外が主なら取り込みで断る() {
        let e = normalize("this is english only text\n", Source::Markdown).unwrap_err();
        assert!(matches!(e, Refusal::NotJapanese { .. }), "{e:?}");
    }

    #[test]
    fn 日本語の文書は通る() {
        let md = "# 題\n\nこれは日本語の文章である。十分な長さを持っている。\n";
        let d = normalize(md, Source::Markdown).unwrap();
        assert_eq!(d.paragraphs().len(), 1);
    }

    #[test]
    fn 書けない記法は測れないになる() {
        assert!(!measurable(Source::Html, Kind::Warning));
        assert!(measurable(Source::Markdown, Kind::Note));
    }

    #[test]
    fn html_も通る() {
        let html = "<h1>題</h1><p>これは日本語の文章である。十分な長さを持っている。</p>";
        let d = normalize(html, Source::Html).unwrap();
        assert_eq!(d.paragraphs().len(), 1);
        assert_eq!(d.sections().len(), 1);
    }

    #[test]
    fn html_の警告は測れない() {
        // 対応表に無いので、書けない升目である。0 ではない。
        assert!(!measurable(Source::Html, Kind::Warning));
        // aside は補足に落ちる。
        let d = normalize("<aside>補足である。</aside>", Source::Html).unwrap();
        assert_eq!(d.nodes[0].kind, Kind::Note);
    }

    #[test]
    fn 取り込み元ごとに正規形が変わる() {
        // 同じ意味を違う記法で書いても、同じ node に落ちる。
        let gh = normalize(
            "> [!NOTE]\n> 補足である。十分に長い日本語の文章。\n",
            Source::Markdown,
        )
        .unwrap();
        let dir = normalize(
            ":::note\n補足である。十分に長い日本語の文章。\n:::\n",
            Source::Markdown,
        )
        .unwrap();
        let h = normalize(
            "<aside>補足である。十分に長い日本語の文章。</aside>",
            Source::Html,
        )
        .unwrap();
        for d in [&gh, &dir, &h] {
            assert_eq!(d.nodes[0].kind, Kind::Note, "記法が違っても補足になる");
        }
    }
}
