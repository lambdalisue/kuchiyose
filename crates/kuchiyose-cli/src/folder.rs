//! 文書を取る口。取り込み元は拡張子から決める（[規則](../../../docs/design/200-command.md#取り込み元は拡張子から決める)）。
//!
//! 文書を取るコマンドはどれもこの 1 つの規則で決める。 `katashiro build` がフォルダを
//! 読むときも、`review` が草稿を読むときも。

use std::path::Path;

use kuchiyose_doc::Document;
use kuchiyose_normalize::{normalize, Source};
use kuchiyose_scale::stats::check_name;

use crate::exit::Exit;

/// 読める拡張子。help と断りの文で同じ文を使う。
pub const EXTENSIONS: &str = "取り込み元は拡張子から決める——.md と .markdown は Markdown、.html と .htm は HTML。ほかは断る。";

/// 拡張子から取り込み元を決める。知らない拡張子なら `None`。
///
/// 中身は見ない。 Markdown の方言は 1 つなので、見て決めるものが無い。
///
/// 知らない拡張子を Markdown に倒さない。 倒せば、プレーンテキストや別の記法が
/// Markdown として読まれ、[取り違えて 0 が並ぶ](../../../docs/spec/030-normalize.md#取り込み元を間違えると0-が並ぶ)
/// ——エラーにならないまま値だけが狂う。
#[must_use]
pub fn source_of(path: &str) -> Option<Source> {
    match Path::new(path).extension()?.to_str()? {
        "md" | "markdown" => Some(Source::Markdown),
        "html" | "htm" => Some(Source::Html),
        _ => None,
    }
}

/// 1 本を渡す口で、取り込み元を決める。決まらなければ断る。
///
/// # Errors
///
/// 拡張子から取り込み元が決まらなければ、使い方の誤りとして断る。
pub fn source_or_refuse(path: &str) -> Result<Source, Exit> {
    source_of(path).ok_or_else(|| {
        eprintln!("断る: 取り込み元が拡張子から決まらない: {path}");
        eprintln!("{EXTENSIONS}");
        Exit::Usage
    })
}

/// 単位の名前。ファイル名の拡張子を除いた部分である。
#[must_use]
pub fn stem_of(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map_or_else(|| path.to_owned(), |s| s.to_string_lossy().into_owned())
}

/// フォルダの中の読める文書。下の階層まで見て、経路の昇順に並べる。
///
/// [取り込み元が決まる](source_of)ものだけを拾い、ほかは見ない。 素材の
/// フォルダには README や控えが混ざる——拾ってから断れば、
/// [1 本の断りでフォルダごと使えなくなる](load)。
///
/// 中の象徴リンクは、ファイルもディレクトリも辿らない。 素材のフォルダは人から
/// 受け取るものなので、辿ればフォルダの外の文書が素材に混ざり、自分を指す
/// リンクでは際限なく降りる。 渡したフォルダ自身がリンクであるのは構わない。
#[must_use]
pub fn readable_files(dir: &str) -> Vec<String> {
    fn walk(dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            // `DirEntry::file_type` はリンクを辿らない。 `Path::is_dir` は辿る。
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                walk(&path, out);
            } else if let Some(p) = path
                .to_str()
                .filter(|p| kind.is_file() && source_of(p).is_some())
            {
                out.push(p.to_owned());
            }
        }
    }
    let mut out = Vec::new();
    walk(Path::new(dir), &mut out);
    out.sort();
    out
}

/// 1 本を読んで正規形にする。読めなければ断る。
///
/// # Errors
///
/// 取り込み元が決まらない、読めない、正規化が断ったときに断る。
pub fn read(path: &str) -> Result<Document, Exit> {
    let source = source_or_refuse(path)?;
    read_as(path, source).map_err(|why| {
        eprintln!("{why}");
        Exit::Unreadable
    })
}

/// 取り込み元を決めたうえで 1 本を読む。断った理由を文で返し、何も出さない。
///
/// 周回が版を読むときに使う。 読めない版は失敗ではなく、捨てる版である。
///
/// # Errors
///
/// 読めない、正規化が断ったときに、理由を返す。
pub fn read_as(path: &str, source: Source) -> Result<Document, String> {
    let Ok(body) = std::fs::read_to_string(path) else {
        return Err(format!("読めない: {path}"));
    };
    normalize(&body, source).map_err(|e| format!("断る: {path}: {e}"))
}

/// 素材のファイルを単位にする。単位の名前はファイル名である。
///
/// 1 本でも断ったら何も返さない。 黙って一部を落として通せば、欠けたまま
/// 形代が出来上がる（[落ちない入力は断る](../../../docs/design/200-command.md#落ちない入力は断る)）。
///
/// 名前が重なれば断る。 下の階層にある同名のファイルは同じ単位名になる——
/// 黙って片方を落とせば、1 本が消えたまま測られる。
///
/// 名前に制御文字があっても断る。 読み戻す口が同じ[確かめ](check_name)で断るので、
/// 通せば書いた形代が使えない。
///
/// # Errors
///
/// 1 本でも読めないか、単位の名前が重なるか、名前に制御文字があれば断る。
pub fn load(files: &[String]) -> Result<Vec<(String, Document)>, Exit> {
    let mut out: Vec<(String, Document)> = Vec::with_capacity(files.len());
    for f in files {
        let doc = read(f).inspect_err(|_| {
            eprintln!("1 本でも断ったら使わない。欠けたまま次へ進まないため");
        })?;
        let name = stem_of(f);
        if let Err(e) = check_name(&name) {
            eprintln!("断る: {e}（{f:?}）");
            eprintln!("名前はファイル名である。 制御文字を含まない名前にする");
            return Err(Exit::Usage);
        }
        if out.iter().any(|(n, _)| *n == name) {
            eprintln!("断る: 単位の名前が重なっている: `{name}`（{f}）");
            eprintln!("名前はファイル名である。 分けたいならファイル名を分ける");
            return Err(Exit::Usage);
        }
        out.push((name, doc));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdir::TempDir;

    #[test]
    fn 取り込み元は拡張子だけで決まる() {
        assert_eq!(source_of("a.md"), Some(Source::Markdown));
        assert_eq!(source_of("a.markdown"), Some(Source::Markdown));
        assert_eq!(source_of("a.html"), Some(Source::Html));
        assert_eq!(source_of("a.htm"), Some(Source::Html));
        assert_eq!(source_of("a.txt"), None);
        assert_eq!(source_of("README"), None);
    }

    #[test]
    fn 知らない拡張子の_1_本は使い方の誤りである() {
        assert_eq!(source_or_refuse("a.txt"), Err(Exit::Usage));
    }

    #[test]
    fn フォルダでは知らない拡張子を拾わず下の階層まで見る() {
        let dir = TempDir::new("readable");
        dir.write("b.md", "本文");
        dir.write("a.html", "<p>本文</p>");
        dir.write("README", "説明");
        dir.write("控え.txt", "控え");
        dir.write("sub/c.markdown", "本文");
        let files: Vec<String> = readable_files(dir.path())
            .into_iter()
            .map(|f| f.trim_start_matches(dir.path()).to_owned())
            .collect();
        assert_eq!(files, vec!["/a.html", "/b.md", "/sub/c.markdown"]);
    }

    #[cfg(unix)]
    #[test]
    fn フォルダの中の象徴リンクはファイルでもディレクトリでも辿らない() {
        // 辿れば、フォルダの外の文書が素材に混ざる。
        let outside = TempDir::new("symlink-outside");
        outside.write("外.md", "本文");
        outside.write("外の階層/外2.md", "本文");
        let dir = TempDir::new("symlink-inside");
        dir.write("a.md", "本文");
        std::os::unix::fs::symlink(outside.join("外.md"), dir.join("リンク.md")).unwrap();
        std::os::unix::fs::symlink(outside.join("外の階層"), dir.join("リンクの階層")).unwrap();
        let files: Vec<String> = readable_files(dir.path())
            .into_iter()
            .map(|f| f.trim_start_matches(dir.path()).to_owned())
            .collect();
        assert_eq!(files, vec!["/a.md"]);
    }

    #[cfg(unix)]
    #[test]
    fn 自分を指す象徴リンクがあっても止まる() {
        // 辿れば、同じディレクトリを際限なく降りる。
        let dir = TempDir::new("symlink-loop");
        dir.write("sub/a.md", "本文");
        std::os::unix::fs::symlink(dir.path(), dir.join("sub/上")).unwrap();
        let files: Vec<String> = readable_files(dir.path())
            .into_iter()
            .map(|f| f.trim_start_matches(dir.path()).to_owned())
            .collect();
        assert_eq!(files, vec!["/sub/a.md"]);
    }

    #[test]
    fn 下の階層の同じ名前は単位の名前が重なるので断る() {
        let dir = TempDir::new("same-stem");
        dir.write("a.md", "これは、そうだ。");
        dir.write("sub/a.md", "これも、そうだ。");
        let files = readable_files(dir.path());
        assert_eq!(load(&files).err(), Some(Exit::Usage));
    }

    #[cfg(unix)]
    #[test]
    fn ファイル名に改行があれば単位の名前にできないので断る() {
        // 書いてから読み戻す口で断られれば、書いた形代が使えない。
        let dir = TempDir::new("control-name");
        dir.write("a.md", "これは、そうだ。");
        dir.write("改\n行.md", "これも、そうだ。");
        let files = readable_files(dir.path());
        assert_eq!(files.len(), 2, "{files:?}");
        assert_eq!(load(&files).err(), Some(Exit::Usage));
    }

    #[test]
    fn 読めない_1_本があれば何も返さない() {
        let dir = TempDir::new("unreadable");
        dir.write("a.md", "これは、そうだ。");
        let missing = format!("{}/消えた.md", dir.path());
        let files = vec![format!("{}/a.md", dir.path()), missing];
        assert_eq!(load(&files).err(), Some(Exit::Unreadable));
    }

    #[test]
    fn 単位の名前は拡張子を除いたファイル名である() {
        assert_eq!(
            stem_of("/x/y/2026-03-14-katashiro.md"),
            "2026-03-14-katashiro"
        );
    }
}
