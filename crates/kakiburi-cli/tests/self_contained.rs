//! 実行ファイルだけで動くことを確かめる。
//!
//! 解析器・辞書・基準のカセット・指標の定義を同梱している。 リポジトリの中で
//! 走らせる試験では、どれかを実行時に置き場から読んでいても通ってしまう。
//! だから実行ファイルを、何も無いディレクトリで走らせる。

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// 何も置いていない作業ディレクトリ。手放すときに片づける。
struct Empty(PathBuf);

impl Empty {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("kakiburi-試験-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("作れる");
        Self(dir)
    }
}

impl Drop for Empty {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn kakiburi(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kakiburi"))
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("走る")
}

/// 素材にする本物の文書。リポジトリの基準の文書を 10 本写す。
fn copy_documents(to: &Path) -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../baselines");
    let mut names: Vec<PathBuf> = std::fs::read_dir(&from)
        .expect("baselines がある")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    names.sort();
    std::fs::create_dir_all(to).expect("作れる");
    for p in names.iter().take(10) {
        std::fs::copy(p, to.join(p.file_name().expect("名前がある"))).expect("写せる");
    }
    names[10].clone()
}

#[test]
fn 何も無いディレクトリからでもカセットを作って検められる() {
    let work = Empty::new("self-contained-work");
    let cwd = Empty::new("self-contained-cwd");
    let corpus = work.0.join("本人");
    let draft_src = copy_documents(&corpus);
    let draft = work.0.join("草稿.md");
    std::fs::copy(draft_src, &draft).expect("写せる");
    let kb = work.0.join("本人.kb");

    let built = kakiburi(
        &cwd.0,
        &[
            "cassette",
            "build",
            corpus.to_str().unwrap(),
            "-o",
            kb.to_str().unwrap(),
        ],
    );
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );

    // 基準は同梱のものを使う。 定義を読めなければ指紋が同梱の基準と合わず 66 で終わる。
    let reviewed = kakiburi(
        &cwd.0,
        &[
            "review",
            draft.to_str().unwrap(),
            "--cassette",
            kb.to_str().unwrap(),
        ],
    );
    let stderr = String::from_utf8_lossy(&reviewed.stderr);
    let code = reviewed.status.code().expect("終了コードがある");
    assert!(code < 64, "使う前の問題で止まった（{code}）: {stderr}");
    assert!(!stderr.contains("定義ファイルが見つからない"), "{stderr}");
    assert_eq!(
        std::fs::read_dir(&cwd.0).expect("読める").count(),
        0,
        "作業ディレクトリに何も書かない"
    );
}
