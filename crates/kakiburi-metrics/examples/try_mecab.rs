//! MeCab の口が実際に繋がるかを見る。
//!
//! ```text
//! cargo run --example try_mecab
//! ```
//!
//! <strong>IPADic を名乗ると断られる。</strong> UniDic を名乗ると通る——だが体系が実際に
//! UniDic でなければ、語彙素で引く指標が静かに落ちる。<strong>名乗りは人の責任である。</strong>

use kakiburi_doc::node::{Kind, Node};
use kakiburi_doc::Document;
use kakiburi_metrics::mecab::Mecab;
use kakiburi_metrics::morph::{check, Analyzed, Analyzer};
use kakiburi_metrics::word;

fn main() {
    let doc = Document::new(vec![Node::leaf(
        Kind::Paragraph,
        "これは日本語の文章である。しかし、ここは違う。",
    )]);
    let prose = doc.prose();

    println!("=== IPADic を名乗る ===");
    let ipadic = Mecab::other("mecab", "IPADic", "2.7.0");
    match check(&ipadic) {
        Ok(()) => println!("通った（起きてはいけない）"),
        Err(e) => println!("断られた: {e}"),
    }

    println!();
    println!("=== UniDic を名乗る ===");
    // <strong>辞書の経路は環境から取る。</strong> 名乗りは人の責任である。
    let dicdir = std::env::var("KAKIBURI_UNIDIC").unwrap_or_default();
    println!("辞書: {}", if dicdir.is_empty() { "既定" } else { &dicdir });
    let claimed = Mecab::unidic("mecab", dicdir, "2.1.2");
    match check(&claimed) {
        Ok(()) => println!("名乗りは通る"),
        Err(e) => println!("断られた: {e}"),
    }
    let ms = claimed.analyze("これは日本語の文章である。");
    if ms.is_empty() {
        println!("MeCab を呼べなかった。形態素 0 なので除外に掛かる。");
        return;
    }
    println!("形態素 {} 個", ms.len());
    for m in ms.iter().take(8) {
        println!("  {:<8} {:<8} {}", m.surface, m.pos1, m.lemma);
    }

    println!();
    match Analyzed::of(&prose, &claimed) {
        Ok(a) => {
            let c = word::function_words(&a);
            println!("機能語 {} 種", c.len());
            for (k, v) in c.iter().take(8) {
                println!("  {k} {v}");
            }
            println!();
            println!("延べ {} 語", a.tokens());
            println!("延べ 1,000 語に届くか: {}", a.enough_tokens());
        }
        Err(e) => println!("測れない: {e}"),
    }
}
