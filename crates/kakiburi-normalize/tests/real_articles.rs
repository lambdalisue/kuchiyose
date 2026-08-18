//! 実際の記事で通るかを見る。
//!
//! 素材は `.spike/` に置いてある（追跡していない）。無ければ飛ばす——
//! <strong>飛ばしたことを黙らない。</strong>

use kakiburi_normalize::{normalize, Source};

fn read_all(dir: &str) -> Vec<(String, String)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
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
    let files = read_all(".spike/baseline");
    if files.is_empty() {
        eprintln!("素材が無いので飛ばした: .spike/baseline");
        return;
    }
    for (name, body) in &files {
        let doc = normalize(body, Source::GithubMarkdown)
            .unwrap_or_else(|e| panic!("{name} が断られた: {e}"));
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
    let files = read_all(".spike/check");
    if files.is_empty() {
        eprintln!("素材が無いので飛ばした: .spike/check");
        return;
    }
    for (name, body) in &files {
        normalize(body, Source::GithubMarkdown)
            .unwrap_or_else(|e| panic!("{name} が断られた: {e}"));
    }
}

#[test]
fn 同じ入力からは同じ正規形が出る() {
    let files = read_all(".spike/baseline");
    if files.is_empty() {
        eprintln!("素材が無いので飛ばした");
        return;
    }
    for (name, body) in &files {
        let a = normalize(body, Source::GithubMarkdown).unwrap();
        let b = normalize(body, Source::GithubMarkdown).unwrap();
        assert_eq!(a, b, "{name} で正規形が揺れた");
    }
}

#[test]
fn 取り込み元を変えると正規形が変わりうる() {
    // Alert を持つかで補足と引用が入れ替わる。だから取り込み元は指紋に入る。
    let md = "> [!NOTE]\n> 補足である。これは十分に長い日本語の文章であり、\n> 断られない程度の分量を持っている。\n";
    let gh = normalize(md, Source::GithubMarkdown).unwrap();
    let plain = normalize(md, Source::PlainMarkdown).unwrap();
    assert_ne!(
        gh, plain,
        "取り込み元を変えても同じでは、対応表が効いていない"
    );
}
