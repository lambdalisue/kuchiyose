//! ディレクトリの Markdown を正規形に落として、単位の数を出す。
//!
//! ```text
//! cargo run --example normalize_dir -- <ディレクトリ>
//! ```

use kakiburi_doc::node::Kind;
use kakiburi_normalize::{normalize, Source};

fn main() {
    let dir = std::env::args().nth(1).expect("ディレクトリを渡す");
    let source = std::env::args().nth(2).map_or(Source::Markdown, |n| {
        Source::from_name(&n).expect("対応表に無い取り込み元")
    });
    println!("取り込み元 {}\n", source.name());
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("ディレクトリが読めない")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    files.sort();

    println!(
        "{:<38} {:>6} {:>5} {:>5} {:>4} {:>5} {:>4} {:>4}",
        "単位", "日本語", "段落", "文", "節", "項目", "補足", "警告"
    );
    println!("{}", "-".repeat(82));

    let (mut ok, mut refused) = (0usize, 0usize);
    for p in &files {
        let name: String = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .chars()
            .take(36)
            .collect();
        let body = std::fs::read_to_string(p).unwrap_or_default();
        match normalize(&body, source) {
            Ok(d) => {
                ok += 1;
                println!(
                    "{:<38} {:>6} {:>5} {:>5} {:>4} {:>5} {:>4} {:>4}",
                    name,
                    d.japanese_chars(),
                    d.paragraphs().len(),
                    d.sentences().len(),
                    d.sections().len(),
                    d.items().len(),
                    count(&d, Kind::Note),
                    count(&d, Kind::Warning),
                );
            }
            Err(e) => {
                refused += 1;
                println!("{name:<38} 断る: {e}");
            }
        }
    }
    println!("{}", "-".repeat(82));
    println!("通った {ok} / 断った {refused}");
}

fn count(doc: &kakiburi_doc::Document, kind: Kind) -> usize {
    fn walk(nodes: &[kakiburi_doc::node::Node], kind: Kind, n: &mut usize) {
        for x in nodes {
            if x.kind == kind {
                *n += 1;
            }
            walk(&x.children, kind, n);
        }
    }
    let mut n = 0;
    walk(&doc.nodes, kind, &mut n);
    n
}
