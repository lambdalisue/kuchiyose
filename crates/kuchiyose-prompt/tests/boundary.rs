//! プロンプトと測る側が、互いを知らないことを確かめる。
//!
//! プロンプトの側から目盛りや判定を作り直す経路も、ペルソナが測った値や判定に
//! 混ざる経路も、依存を足せば書けてしまう。型では守れないので、Cargo.toml を読んで
//! 確かめる（[境界](../../../docs/design/000-architecture.md#kuchiyose-prompt)）。

use std::path::{Path, PathBuf};

fn manifest(krate: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(krate)
        .join("Cargo.toml")
}

/// `[dependencies]` の名前。コメントを落とす。理由を書いた行に名前が出るので。
fn dependencies(krate: &str) -> Vec<String> {
    let body = std::fs::read_to_string(manifest(krate)).expect("Cargo.toml が読めない");
    body.lines()
        .skip_while(|l| l.trim() != "[dependencies]")
        .skip(1)
        .take_while(|l| !l.trim_start().starts_with('['))
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split('=').next())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

#[test]
fn どのクレートにも依存しない() {
    assert_eq!(
        dependencies("kuchiyose-prompt"),
        Vec::<String>::new(),
        "kuchiyose-prompt が依存を持っている。文体の事実も判定の散文も、組み立て層が値にして渡す"
    );
}

#[test]
fn 測る側はプロンプトに依存しない() {
    for krate in [
        "kuchiyose-metrics",
        "kuchiyose-scale",
        "kuchiyose-review",
        "kuchiyose-katashiro",
    ] {
        assert!(
            !dependencies(krate).iter().any(|d| d == "kuchiyose-prompt"),
            "{krate} が kuchiyose-prompt に依存している。ペルソナが測る側に混ざる経路が書ける"
        );
    }
}
