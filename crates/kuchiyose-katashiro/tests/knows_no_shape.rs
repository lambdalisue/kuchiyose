//! 保存が統計値の形を知らないことを確かめる。
//!
//! **統計値はテキストとして抱えるだけである。** 統計値の型は `kuchiyose-scale` にあり、
//! 形代はその JSON を[作り直せるもの](../../../docs/design/100-katashiro.md#何を収めるか)
//! として持つ。
//!
//! **形を知れば、統計値の項目が増えるたびに保存の側が変わる。** 型では守れない——依存を
//! 足せば使えてしまう。だから **Cargo.toml を読んで確かめる。**

use std::path::{Path, PathBuf};

/// このクレートの `Cargo.toml`。
fn manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")
}

/// コメントを落とした本文。理由を書いた行に名前が出るので。
fn code() -> String {
    std::fs::read_to_string(manifest())
        .expect("Cargo.toml が読めない")
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn 指標にも目盛りにも依存しない() {
    let code = code();
    for name in [
        "kuchiyose-metrics",
        "kuchiyose-scale",
        "kuchiyose-review",
        "kuchiyose-prompt",
    ] {
        assert!(
            !code.contains(name),
            "kuchiyose-katashiro が {name} に依存している。\n\
             保存が統計値の形を知れば、項目が増えるたびに保存の側が変わる。"
        );
    }
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
        // flate2 は容器の entry を縮めるためだけにある。 統計値の形は知らない。
        vec!["kuchiyose-doc".to_owned(), "flate2".to_owned()],
        "依存が増えている。境界を跨ぐ漏れが無いかを確かめてから足す"
    );
}
