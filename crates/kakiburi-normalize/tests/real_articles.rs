//! 実際の記事で通るかを見る。
//!
//! 基準は同梱の `baselines/` を読むので、いつでも走る。 本人の記事は `.spike/` に
//! 置いてある（追跡していない）。無ければ飛ばす——飛ばしたことを黙らない。

use kakiburi_normalize::{normalize, Source};

fn read_all(dir: &str) -> Vec<(String, String)> {
    // 経路はリポジトリの根から取る。 cargo test は作業ディレクトリをクレートに
    // 置くので、相対のままだと素材があっても見つからず、毎回黙って飛ばしていた。
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
        return Vec::new();
    };
    let mut out: Vec<(String, String)> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .filter_map(|p| {
            let name = p.file_name()?.to_string_lossy().into_owned();
            let body = std::fs::read_to_string(&p).ok()?;
            Some((name, body))
        })
        .collect();
    out.sort();
    out
}

#[test]
fn 基準の記事はすべて通る() {
    let files = read_all("baselines");
    if files.is_empty() {
        eprintln!("素材が無いので飛ばした: baselines");
        return;
    }
    for (name, body) in &files {
        let doc =
            normalize(body, Source::Markdown).unwrap_or_else(|e| panic!("{name} が断られた: {e}"));
        assert!(
            doc.japanese_chars() >= 1000,
            "{name} の日本語が 1,000 字に届かない: {}",
            doc.japanese_chars()
        );
        assert!(!doc.paragraphs().is_empty(), "{name} に段落が無い");
        assert!(!doc.sentences().is_empty(), "{name} に文が無い");
    }
    eprintln!("基準 {} 本を通した", files.len());
}

#[test]
fn 検める文も通る() {
    let files = read_all(".spike/alisue/baseline");
    if files.is_empty() {
        eprintln!("素材が無いので飛ばした: .spike/alisue/baseline");
        return;
    }
    for (name, body) in &files {
        normalize(body, Source::Markdown).unwrap_or_else(|e| panic!("{name} が断られた: {e}"));
    }
}

#[test]
fn 同じ入力からは同じ正規形が出る() {
    let files = read_all("baselines");
    if files.is_empty() {
        eprintln!("素材が無いので飛ばした");
        return;
    }
    for (name, body) in &files {
        let a = normalize(body, Source::Markdown).unwrap();
        let b = normalize(body, Source::Markdown).unwrap();
        assert_eq!(a, b, "{name} で正規形が揺れた");
    }
}
