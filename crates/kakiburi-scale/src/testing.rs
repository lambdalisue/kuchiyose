//! 試験のための素材。1 か所に置く。
//!
//! 目盛りを作るには本人 10 単位・基準 10 単位が要り、どちらも除外を越えなければ
//! ならない。その素材を試験ごとに書くと、除外の下限を変えたときに直し漏れる。

use kakiburi_doc::node::{Kind, Node};
use kakiburi_doc::Document;
use kakiburi_metrics::morph::{Analyzer, Dictionary, Morpheme};

use crate::assemble::Sample;

/// 試験用の解析器。UniDic を名乗り、字で切る。
///
/// 仕様の手続きを通すためのものであって、日本語を解析するものではない。
pub(crate) struct Chars;

impl Analyzer for Chars {
    fn dictionary(&self) -> Dictionary {
        Dictionary::UnidicShort
    }
    fn dictionary_version(&self) -> (String, String) {
        ("UniDic".into(), "試験".into())
    }
    fn analyze(&self, text: &str) -> Vec<Morpheme> {
        text.chars()
            .map(|c| {
                // 一人称と語彙素を見る指標のために、2 字だけ札を替える。
                let (pos1, lemma) = match c {
                    'は' | 'が' | 'の' | 'を' | 'に' => ("助詞", c.to_string()),
                    '。' | '、' => ("補助記号", c.to_string()),
                    '僕' => ("代名詞", c.to_string()),
                    '静' => ("形状詞", "静か".to_owned()),
                    _ => ("名詞", c.to_string()),
                };
                Morpheme {
                    surface: c.to_string(),
                    lemma,
                    pos1: pos1.into(),
                    pos2: "*".into(),
                }
            })
            .collect()
    }
}

/// 1 単位ぶんの文書。すべての除外を越える長さにする。
///
/// 長さは単位ごとに散らす。 揃えると広がりが 0 になり、長さの範囲の検査が
/// 「重なり 0」で止まる——両側が同じ範囲に散っている素材でなければ先へ進めない。
pub(crate) fn document(index: usize, machine: bool) -> Document {
    document_with(index, machine, "、")
}

/// 読点を `comma` に替えた文書。 空にすれば読点の系統だけが欠ける。
pub(crate) fn document_with(index: usize, machine: bool, comma: &str) -> Document {
    let seed = index * if machine { 17 } else { 13 };
    // 畳まれても下限に届く量にする。 作り物の文は繰り返しが強いので、
    // [コーパスから見つけた語](kakiburi_metrics::lexicon)が実素材より多く畳む。
    let nodes: Vec<Node> = (0..90 + index * 4)
        .map(|i| {
            // 骨格は両側で同じにする。 違えば、長さの差が両側の違いに混ざる。
            let (a, b, c, d) = if machine {
                // 機械の側。語を散らす——繰り返しが足りない側に出る。
                (
                    wordy(seed + i),
                    wordy(seed + i * 3),
                    wordy(seed + i * 7),
                    wordy(seed + i * 11),
                )
            } else {
                // 人の側。同じ言い回しを繰り返す。
                (wordy(seed % 2), wordy(0), wordy(1), wordy(seed % 3))
            };
            // 機能語を 5 つ含める。対象の形態素の下限を越えるためである——
            // 越えなければ機能語が測れず、判定に使う系統が揃わない。
            Node::leaf(
                Kind::Paragraph,
                format!("{a}は{comma}{b}の{c}を{d}に{a}が。"),
            )
        })
        .collect();
    Document::new(nodes)
}

/// 語のかわりに使う、種で変わるかな列。
pub(crate) fn wordy(n: usize) -> String {
    const KANA: [char; 10] = ['あ', 'か', 'さ', 'た', 'な', 'は', 'ま', 'や', 'ら', 'わ'];
    (0..3)
        .map(|i| KANA[(n / 10_usize.pow(i) + i as usize) % KANA.len()])
        .collect()
}

pub(crate) struct Fixture {
    pub(crate) person: Vec<(String, Document)>,
    pub(crate) baseline: Vec<(String, Document)>,
}

impl Fixture {
    pub(crate) fn new(n: usize) -> Self {
        Self {
            person: (0..n)
                .map(|i| (format!("p{i:02}"), document(i, false)))
                .collect(),
            baseline: (0..n)
                .map(|i| (format!("b{i:02}"), document(i, true)))
                .collect(),
        }
    }

    pub(crate) fn samples(pairs: &[(String, Document)]) -> Vec<Sample<'_>> {
        pairs
            .iter()
            .map(|(n, d)| Sample {
                name: n,
                document: d,
            })
            .collect()
    }
}

/// 構造を持つ 1 単位ぶんの文書。見出し・箇条書き・一人称・語彙素を含む。
///
/// 指示できる指標と、型・一人称・語の材料が、どれも値を持つようにする。
pub(crate) fn rich(index: usize) -> Document {
    let mut nodes = vec![Node::heading(1, format!("第{index}章のはなし"))];
    for i in 0..(12 + index) {
        if i % 4 == 0 {
            nodes.push(Node::heading(3, format!("小さな節{i}")));
        }
        nodes.push(Node::leaf(
            Kind::Paragraph,
            format!(
                "僕は{}を静かに書く。それは、{}のためです。まあ、{}ではない。",
                wordy(index * 7 + i),
                wordy(i),
                wordy(index + i * 3)
            ),
        ));
    }
    let items: Vec<Node> = (0..12)
        .map(|i| {
            Node::leaf(
                Kind::Item,
                format!("項目の{}は{}です", wordy(i), wordy(index + i)),
            )
        })
        .collect();
    nodes.push(Node::branch(Kind::Bullet, items));
    for i in 0..30 {
        nodes.push(Node::leaf(
            Kind::Paragraph,
            format!(
                "{}は、{}の{}を{}に。",
                wordy(i),
                wordy(index),
                wordy(i * 2),
                wordy(i + index)
            ),
        ));
    }
    Document::new(nodes)
}
