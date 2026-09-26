//! ディレクトリの Markdown を測って、指示できる指標の値を並べる。
//!
//! ```text
//! cargo run --example measure_dir -- <ディレクトリ> [取り込み元]
//! ```
//!
//! 0 と「測っていない」を区別して出す。 測っていないものは `—` で出る。

use kuchiyose_metrics::{directive, Measured};
use kuchiyose_normalize::{normalize, Source};

fn main() {
    let dir = std::env::args().nth(1).expect("ディレクトリを渡す");
    let source = std::env::args().nth(2).map_or(Source::Markdown, |n| {
        Source::from_name(&n).expect("対応表に無い取り込み元")
    });

    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("ディレクトリが読めない")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    files.sort();

    let mut docs = Vec::new();
    for p in &files {
        let name: String = p
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .chars()
            .take(10)
            .collect();
        let body = std::fs::read_to_string(p).unwrap_or_default();
        match normalize(&body, source) {
            Ok(d) => docs.push((name, d)),
            Err(e) => eprintln!("{name}: 断る: {e}"),
        }
    }
    if docs.is_empty() {
        eprintln!("測れる文書が無い");
        return;
    }

    print!("{:<26}", "指標");
    for (n, _) in &docs {
        print!("{n:>11}");
    }
    println!();
    println!("{}", "-".repeat(26 + 11 * docs.len()));

    // 一覧は登録簿の側が持つ。 解析器を渡さないので、要る指標は「道具無」と出る。
    let measured: Vec<Vec<(String, Measured)>> = docs
        .iter()
        .map(|(_, d)| {
            directive::measure(d, None)
                .into_iter()
                .map(|(n, c)| (n, c.measured()))
                .collect()
        })
        .collect();
    for (i, (name, _)) in measured[0].iter().enumerate() {
        print!("{name:<26}");
        for row in &measured {
            match row[i].1 {
                Measured::Value(v) => print!("{v:>11.3}"),
                Measured::BelowFloor => print!("{:>11}", "—"),
                Measured::NoDenominator => print!("{:>11}", "0/0"),
                Measured::NotWritable => print!("{:>11}", "×"),
                Measured::ToolMissing => print!("{:>11}", "道具無"),
                Measured::ToolFailed => print!("{:>11}", "道具失"),
            }
        }
        println!();
    }
    println!("{}", "-".repeat(26 + 11 * docs.len()));
    println!("測っていない印。どれも 0 ではない。");
    println!("  —      下限未満。長い文書を足せば直る");
    println!("  0/0    分母が 0。その文書では測れない");
    println!("  ×      書けない記法。別の取り込み元で集め直す");
    println!("  道具無 形態素解析器や外部の表が無い。環境を直す");
    println!("  道具失 道具が返さなかった。報告する");
}
