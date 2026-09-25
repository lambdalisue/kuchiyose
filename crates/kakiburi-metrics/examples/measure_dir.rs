//! ディレクトリの Markdown を測って、指示できる指標の値を並べる。
//!
//! ```text
//! cargo run --example measure_dir -- <ディレクトリ> [取り込み元]
//! ```
//!
//! 0 と「測っていない」を区別して出す。 測っていないものは `—` で出る。

use kakiburi_doc::Document;
use kakiburi_metrics::{structure, symbol, Measured};
use kakiburi_normalize::{normalize, Source};

/// 名前と測り方の対。使う側は一覧を持たないので、ここに並べるのは例だけである。
type Metric = (&'static str, fn(&Document) -> Measured);

const METRICS: &[Metric] = &[
    ("全角括弧", |d| symbol::full_width_paren(&d.prose())),
    ("半角括弧", |d| symbol::half_width_paren(&d.prose())),
    ("鉤括弧", |d| symbol::corner_bracket(&d.prose())),
    ("感嘆符", |d| symbol::exclamation(&d.prose())),
    ("疑問符", |d| symbol::question(&d.prose())),
    ("三点リーダ", |d| symbol::ellipsis(&d.prose())),
    ("三点リーダの字数", |d| {
        symbol::ellipsis_doubled(&d.prose())
    }),
    ("中黒", |d| symbol::middle_dot(&d.prose())),
    ("波ダッシュ", |d| symbol::wave_dash(&d.prose())),
    ("数字の字幅", |d| symbol::digit_width(&d.prose())),
    ("感嘆符の字幅", |d| {
        symbol::exclamation_width(&d.prose())
    }),
    ("疑問符の字幅", |d| symbol::question_width(&d.prose())),
    ("和欧間スペース欠落", |d| {
        symbol::missing_space(&d.prose())
    }),
    ("笑い", |d| symbol::laughter(&d.prose())),
    ("絵文字", |d| symbol::emoji(&d.prose())),
    ("em dash", |d| symbol::em_dash(&d.prose())),
    ("見出し", structure::headings),
    ("深い見出し", structure::deep_headings),
    ("箇条書き", structure::bullets),
    ("番号リスト", structure::ordered_lists),
    ("表", structure::tables),
    ("引用", structure::quotes),
    ("補足", structure::notes),
    ("警告", structure::warnings),
    ("折りたたみ", structure::details),
    ("強調", structure::emphasis),
    ("コードブロック", structure::code_blocks),
    (
        "1 文だけの段落の割合",
        structure::single_sentence_paragraphs,
    ),
    ("段落あたりの文数", structure::sentences_per_paragraph),
    ("太字始まりの項目", structure::bold_leading_items),
    ("段落長の変動係数", structure::paragraph_length_cv),
    ("箇条書き項目長の変動係数", structure::item_length_cv),
    ("節の長さの変動係数", structure::section_length_cv),
];

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

    for (name, f) in METRICS {
        print!("{name:<26}");
        for (_, d) in &docs {
            match f(d) {
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
