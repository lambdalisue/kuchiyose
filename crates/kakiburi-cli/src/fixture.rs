//! 試験のための素材。1 か所に置く。
//!
//! 目盛りを作るには本人 10 単位・基準 10 単位が要り、どちらも除外を越えなければ
//! ならない。その素材を試験ごとに書くと、除外の下限を変えたときに直し漏れる。

use kakiburi_doc::node::{Kind, Node};
use kakiburi_doc::Document;
use kakiburi_metrics::morph::{Analyzer, Dictionary, Morpheme};
use kakiburi_scale::{assemble, Sample, Scale};

/// 試験用の解析器。UniDic を名乗り、字で切る。
///
/// 仕様の手続きを通すためのものであって、日本語を解析するものではない。
pub struct Chars;

impl Analyzer for Chars {
    fn dictionary(&self) -> Dictionary {
        Dictionary::UnidicShort
    }
    fn dictionary_version(&self) -> (String, String) {
        ("UniDic".into(), "試験".into())
    }
    fn analyze(&self, text: &str) -> Vec<Morpheme> {
        text.chars()
            .map(|c| Morpheme {
                surface: c.to_string(),
                lemma: c.to_string(),
                pos1: match c {
                    'は' | 'が' | 'の' | 'を' | 'に' => "助詞".into(),
                    '。' | '、' => "補助記号".into(),
                    _ => "名詞".into(),
                },
                pos2: "*".into(),
            })
            .collect()
    }
}

/// 1 単位ぶんの文書。
///
/// 骨格は両側で同じにする。 違えば、長さの差が両側の違いに混ざる。
/// 長さは単位ごとに散らす。 揃えると広がりが 0 になり、長さの範囲の検査で止まる。
pub fn document(index: usize, machine: bool) -> Document {
    let seed = index * if machine { 17 } else { 13 };
    // 畳まれても下限に届く量にする。 作り物の文は繰り返しが強いので、
    // [コーパスから見つけた語](kakiburi_metrics::lexicon)が実素材より多く畳む。
    let nodes: Vec<Node> = (0..90 + index * 4)
        .map(|i| {
            let (a, b, c, d) = if machine {
                // 機械の側。語を散らす——繰り返しが足りない側に出る。
                (
                    word(seed + i),
                    word(seed + i * 3),
                    word(seed + i * 7),
                    word(seed + i * 11),
                )
            } else {
                // 人の側。同じ言い回しを繰り返す。
                (word(seed % 2), word(0), word(1), word(seed % 3))
            };
            // 機能語を 5 つ含める。[対象の形態素の下限](kakiburi_metrics::floor::FUNCTION_WORDS)を
            // 越えるためである——越えなければ機能語が測れず、判定に使う系統が
            // 揃わない。
            Node::leaf(Kind::Paragraph, format!("{a}は、{b}の{c}を{d}に{a}が。"))
        })
        .collect();
    Document::new(nodes)
}

/// 語のかわりに使う、種で変わるかな列。
fn word(n: usize) -> String {
    const KANA: [char; 10] = ['あ', 'か', 'さ', 'た', 'な', 'は', 'ま', 'や', 'ら', 'わ'];
    (0..3)
        .map(|i| KANA[(n / 10_usize.pow(i) + i as usize) % KANA.len()])
        .collect()
}

/// 名前と文書の組。
pub type Named = Vec<(String, Document)>;

/// 名前と文書の組を 10 本ずつ。
pub fn corpus() -> (Named, Named) {
    (
        (0..10)
            .map(|i| (format!("p{i:02}"), document(i, false)))
            .collect(),
        (0..10)
            .map(|i| (format!("b{i:02}"), document(i, true)))
            .collect(),
    )
}

/// 素材の形にする。
pub fn samples(v: &[(String, Document)]) -> Vec<Sample<'_>> {
    v.iter()
        .map(|(n, d)| Sample {
            name: n,
            document: d,
        })
        .collect()
}

/// 目盛りを作る。作れなければ試験を落とす。
pub fn scale() -> Scale {
    let (person, baseline) = corpus();
    assemble(
        kakiburi_scale::assemble::Material {
            person: &samples(&person),
            baseline: &samples(&baseline),
            others: &[],
        },
        Some(&Chars),
    )
    .expect("目盛りができる")
}
