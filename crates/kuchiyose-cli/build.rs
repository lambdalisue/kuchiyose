//! 指標の定義ファイルを実行ファイルに埋め込む一覧を作る。
//!
//! 定義ファイルは指摘の文と登録簿の出どころで、指紋にも入る。 実行時に置き場を
//! 探すと、リポジトリの外では見つからず、指摘が出ないうえに指紋が合わなくなる。
//!
//! 読むのはクレートの中の `definitions/` である。 リポジトリでは `docs/spec/metrics`
//! への symlink で、`cargo package` がリンク先の中身を実体として収める。 クレートの外を
//! 直に読むと、crates.io から組んだときに置き場が無い。

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("definitions");
    // ディレクトリを指せば、中のどのファイルが変わっても作り直される。
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{} が読めない: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .filter_map(|p| {
            let stem = p.file_stem()?.to_string_lossy().into_owned();
            let abs = p.canonicalize().ok()?.to_string_lossy().into_owned();
            Some((stem, abs))
        })
        .collect();
    // 空のまま埋め込むと、組めるのに指摘が出ず指紋も合わない実行ファイルができる。
    assert!(!files.is_empty(), "{} に定義ファイルが無い", dir.display());
    files.sort();
    let mut out = String::from("&[\n");
    for (stem, path) in &files {
        writeln!(out, "    ({stem:?}, include_str!({path:?})),").expect("書ける");
    }
    out.push(']');
    let target =
        Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR がある")).join("definitions.rs");
    std::fs::write(target, out).expect("書ける");
}
