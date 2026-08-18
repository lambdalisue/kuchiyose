//! 検めが目盛りを作る側に依存していないことを確かめる。
//!
//! <strong>依存すると、検める文書を見てから重みや語彙を作り直せる経路が開く。</strong>
//! 仕様が「黙って壊れる」と名指しした 4 か所のうちの 1 つである。
//!
//! 型では守れない——依存を足せば使えてしまう。だから <strong>Cargo.toml を読んで確かめる。</strong>

use std::path::{Path, PathBuf};

/// このクレートの `Cargo.toml`。
fn manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")
}

#[test]
fn kakiburi_scale_に依存しない() {
    let body = std::fs::read_to_string(manifest()).expect("Cargo.toml が読めない");
    // コメントは落とす。理由を書いた行に名前が出るので。
    let code: String = body
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !code.contains("kakiburi-scale"),
        "kakiburi-review が kakiburi-scale に依存している。\n\
         検める文書を見てから重みや語彙を作り直せる経路が開く。"
    );
}

#[test]
fn 依存の一覧が意図どおりである() {
    let body = std::fs::read_to_string(manifest()).expect("Cargo.toml が読めない");
    let deps: Vec<String> = body
        .lines()
        .skip_while(|l| l.trim() != "[dependencies]")
        .skip(1)
        .take_while(|l| !l.trim_start().starts_with('['))
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split('=').next())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    assert_eq!(
        deps,
        vec!["kakiburi-metrics".to_owned()],
        "依存が増えている。境界を跨ぐ漏れが無いかを確かめてから足す"
    );
}
