//! 目盛りを組み立てる。<strong>素材から端まで通す 1 本の道である。</strong>
//!
//! <strong>順序が守るべきものを持っている。</strong>
//!
//! | 順 | すること | 崩すと何が起きるか |
//! | --- | --- | --- |
//! | 1 | 測れた単位だけを取る | 除外に掛かった単位が対に入り、相手の本数が黙って 5 を割る |
//! | 2 | 長さの範囲を確かめる | 長さと連動する指標がすべて離れて見える |
//! | 3 | <strong>割る前に</strong>語彙を固定する | 側ごとに次元の意味が変わる |
//! | 4 | 対を割り当てる | 較正と目盛りが対を共有し、いちばん危ない検査が無効になる |
//! | 5 | 較正して帯を作る | |

use std::collections::BTreeMap;

use kakiburi_doc::Document;
use kakiburi_metrics::matching::{self, FOR_VERDICT};
use kakiburi_metrics::morph::{Analyzed, Analyzer};
use kakiburi_metrics::system::System;
use kakiburi_metrics::Humanness;

use crate::band::Band;
use crate::calibrate::{median, Calibration};
use crate::humanness::HumannessScale;
use crate::pairing::{pair, Pair};
use crate::split::{self, Unit};
use crate::vocabulary::{cosine_delta, Counts, FrozenSet};
use crate::{length_range_ok, Scale, ScaleError};

/// 素材 1 本。
#[derive(Debug, Clone, Copy)]
pub struct Sample<'a> {
    /// 単位の名前。<strong>ファイル名ではない。</strong>
    pub name: &'a str,
    /// 正規形。
    pub document: &'a Document,
}

/// 1 単位を測り終えた形。
struct Measurements {
    /// 系統ごとの部分ベクトルの数え上げ。<strong>測れなかった系統は入っていない。</strong>
    parts: BTreeMap<System, Vec<Counts>>,
    /// 人らしさの 12 次元。
    humanness: Humanness,
    /// 地の文の日本語の文字数。長さの範囲に使う。
    chars: usize,
}

impl Measurements {
    fn of(sample: Sample<'_>, analyzer: Option<&dyn Analyzer>) -> Self {
        let prose = sample.document.prose();
        // <strong>解析は 1 度だけ。</strong> 指標ごとに呼べば、外の実行ファイルを指標の数だけ起こす。
        let analyzed = analyzer.and_then(|a| Analyzed::of(&prose, a).ok());
        let mut parts = BTreeMap::new();
        for s in FOR_VERDICT {
            if let Some(p) = matching::parts(s, &prose, analyzed.as_ref()) {
                parts.insert(s, p);
            }
        }
        Self {
            parts,
            humanness: Humanness::measure(&prose, analyzed.as_ref()),
            chars: sample.document.japanese_chars(),
        }
    }

    /// 判定に使う 5 系統が全部測れたか。
    fn systems_measured(&self) -> bool {
        FOR_VERDICT.iter().all(|s| self.parts.contains_key(s))
    }
}

/// 1 単位が測れたかの内訳。
///
/// <strong>止まったなら、どの単位のどこで止まったかを言う。</strong> 「10 本に届かない」だけでは、
/// 素材を足すべきか、長さを揃えるべきか、辞書を入れるべきかが分からない。
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// 単位の名前。
    pub name: String,
    /// 地の文の日本語の文字数。
    pub chars: usize,
    /// 測れなかった系統。
    pub missing_systems: Vec<String>,
    /// 測れなかった人らしさの次元。
    pub missing_humanness: Vec<String>,
}

impl Report {
    /// 帯に使える単位か。
    #[must_use]
    pub fn usable(&self) -> bool {
        self.missing_systems.is_empty() && self.missing_humanness.is_empty()
    }
}

/// 素材を測って内訳を返す。<strong>目盛りは作らない。</strong>
#[must_use]
pub fn inspect(samples: &[Sample<'_>], analyzer: Option<&dyn Analyzer>) -> Vec<Report> {
    samples
        .iter()
        .map(|s| {
            let m = Measurements::of(*s, analyzer);
            Report {
                name: s.name.to_owned(),
                chars: m.chars,
                missing_systems: FOR_VERDICT
                    .iter()
                    .filter(|sys| !m.parts.contains_key(sys))
                    .map(|sys| sys.name().to_owned())
                    .collect(),
                missing_humanness: m.humanness.missing(),
            }
        })
        .collect()
}

/// 組み立てる。
///
/// <strong>作れないことは失敗ではない。</strong> 素材が足りなければ目盛りを作らず、検めが
/// 判定できないを返す——それが正しい振る舞いである。
pub fn assemble(
    person: &[Sample<'_>],
    baseline: &[Sample<'_>],
    analyzer: Option<&dyn Analyzer>,
) -> Result<Scale, ScaleError> {
    let measured: BTreeMap<String, Measurements> = person
        .iter()
        .chain(baseline.iter())
        .map(|s| ((*s.name).to_owned(), Measurements::of(*s, analyzer)))
        .collect();

    let unit = |s: &Sample<'_>| -> Unit {
        let m = &measured[s.name];
        Unit {
            name: s.name.to_owned(),
            systems_measured: m.systems_measured(),
            humanness_measured: m.humanness.all_measured(),
        }
    };
    let person_units: Vec<Unit> = person.iter().map(unit).collect();
    let baseline_units: Vec<Unit> = baseline.iter().map(unit).collect();

    // 1. 測れた単位だけを取り、どちらも 5 ＋ 5 に届くことを確かめる。
    let person_split = split::split(&person_units).map_err(ScaleError::Split)?;
    let baseline_split = split::split(&baseline_units).map_err(ScaleError::Split)?;

    // 2. 長さの範囲。<strong>指標ごとではなく、その場面まるごと止める。</strong>
    //
    // <strong>見るのは実際に使う単位の範囲である。</strong> 除外に掛かって目盛りに入らない単位を
    // 混ぜれば、比べていないものの長さで止まったり通ったりする。
    let chars = |s: &split::Split| -> Vec<usize> {
        s.partners
            .iter()
            .chain(&s.points)
            .map(|u| measured[&u.name].chars)
            .collect()
    };
    length_range_ok(&chars(&person_split), &chars(&baseline_split))?;

    // 3. <strong>割る前に語彙を固定する。</strong> 全体から選ぶ——側ごとに違う語彙を使えば、
    //    側ごとに次元の意味が変わる。
    let used: Vec<&Unit> = person_split
        .partners
        .iter()
        .chain(&person_split.points)
        .chain(&baseline_split.partners)
        .chain(&baseline_split.points)
        .collect();
    let mut frozen: Vec<(String, FrozenSet)> = Vec::with_capacity(FOR_VERDICT.len());
    for s in FOR_VERDICT {
        let all: Vec<Vec<Counts>> = used
            .iter()
            .filter_map(|u| measured[&u.name].parts.get(&s).cloned())
            .collect();
        frozen.push((
            s.name().to_owned(),
            FrozenSet::fit(&all, &matching::limits(s)),
        ));
    }

    // 系統ごとの、単位 → z 得点のベクトル。
    let mut projected: BTreeMap<(String, String), Vec<f64>> = BTreeMap::new();
    for u in &used {
        let m = &measured[&u.name];
        for (name, set) in &frozen {
            let Some(system) = System::from_name(name) else {
                continue;
            };
            let Some(parts) = m.parts.get(&system) else {
                continue;
            };
            if let Some(v) = set.project(parts) {
                projected.insert((name.clone(), u.name.clone()), v);
            }
        }
    }

    // 4. 対を割り当てる。<strong>作り手を 1 つにしたうえで、それでも確かめる。</strong>
    let pairing = pair(&person_split, &baseline_split);
    if pairing.shares_pairs() {
        return Err(ScaleError::SharedPairs);
    }

    // 系統ごとの距離。<strong>片方でも投影できなければ対を捨てる。</strong>
    let system_names: Vec<String> = frozen.iter().map(|(n, _)| n.clone()).collect();
    let distance = |system: &str, p: &Pair| -> Option<f64> {
        let a = projected.get(&(system.to_owned(), p.left.clone()))?;
        let b = projected.get(&(system.to_owned(), p.right.clone()))?;
        Some(cosine_delta(a, b))
    };
    let row =
        |p: &Pair| -> Option<Vec<f64>> { system_names.iter().map(|n| distance(n, p)).collect() };

    let mut rows = Vec::new();
    let mut labels = Vec::new();
    for p in &pairing.calibration_same {
        if let Some(r) = row(p) {
            rows.push(r);
            labels.push(1);
        }
    }
    for p in &pairing.calibration_different {
        if let Some(r) = row(p) {
            rows.push(r);
            labels.push(0);
        }
    }
    let calibration = Calibration::fit(&system_names, &rows, &labels);

    // 5. 天井と床。<strong>1 本につき 1 点</strong>——相手集合との照合値の中央値である。
    let point = |pairs: &[Pair]| -> Option<f64> {
        let values: Vec<f64> = pairs
            .iter()
            .filter_map(|p| calibration.matching_value(&|n| distance(n, p)).ok())
            .collect();
        median(&values)
    };
    let ceiling: Vec<f64> = pairing
        .ceiling
        .iter()
        .filter_map(|(_, ps)| point(ps))
        .collect();
    let floor: Vec<f64> = pairing
        .floor
        .iter()
        .filter_map(|(_, ps)| point(ps))
        .collect();
    let band = Band::build(&ceiling, &floor).map_err(|source| {
        ScaleError::Band(Box::new(crate::BandStop {
            source,
            which: "照合値",
            ceiling: ceiling.clone(),
            floor: floor.clone(),
            built: None,
        }))
    })?;

    // 人らしさ。<strong>対ではなく単位で割る</strong>——1 本ごとに出る値で、対を作らない。
    let rows_of = |us: &[Unit]| -> Vec<Vec<f64>> {
        us.iter()
            .map(|u| {
                measured[&u.name]
                    .humanness
                    .flat()
                    .into_iter()
                    .filter_map(|(_, m)| m.value())
                    .collect()
            })
            .collect()
    };
    let humanness = HumannessScale::fit(
        &rows_of(&person_split.partners),
        &rows_of(&baseline_split.partners),
    );
    let side = |us: &[Unit]| -> Vec<f64> {
        us.iter()
            .filter_map(|u| humanness.value(&measured[&u.name].humanness.flat()).ok())
            .collect()
    };
    let human = side(&person_split.points);
    let machine = side(&baseline_split.points);
    let humanness_band = Band::build(&human, &machine).map_err(|source| {
        ScaleError::Band(Box::new(crate::BandStop {
            source,
            which: "人らしさ値",
            ceiling: human.clone(),
            floor: machine.clone(),
            // <strong>照合の帯は作れている。</strong> 止まったのは人らしさの側である。
            built: Some(band),
        }))
    })?;

    Ok(Scale {
        frozen,
        calibration,
        band,
        partners: person_split
            .partners
            .iter()
            .map(|u| u.name.clone())
            .collect(),
        humanness,
        humanness_band,
    })
}

/// 検める 1 本を、作り終えた目盛りに載せる。
///
/// <strong>目盛りは作り直さない。</strong> 語彙も重みも受け取ったものを使う——検める文書を見てから
/// 作り直せる経路を持たない。
///
/// `partners` は相手集合の単位である。<strong>照合値は相手集合との中央値である。</strong>
#[must_use]
pub fn measure_against(
    scale: &Scale,
    target: Sample<'_>,
    partners: &[Sample<'_>],
    analyzer: Option<&dyn Analyzer>,
) -> Measured {
    let t = Measurements::of(target, analyzer);
    let ps: Vec<Measurements> = partners
        .iter()
        .map(|s| Measurements::of(*s, analyzer))
        .collect();

    let vector = |m: &Measurements, name: &str| -> Option<Vec<f64>> {
        let system = System::from_name(name)?;
        let set = &scale.frozen.iter().find(|(n, _)| n == name)?.1;
        set.project(m.parts.get(&system)?)
    };
    let values: Vec<f64> = ps
        .iter()
        .filter_map(|p| {
            scale
                .calibration
                .matching_value(&|n| {
                    let a = vector(&t, n)?;
                    let b = vector(p, n)?;
                    Some(cosine_delta(&a, &b))
                })
                .ok()
        })
        .collect();
    Measured {
        matching: median(&values),
        humanness: scale.humanness.value(&t.humanness.flat()).ok(),
        missing_humanness: t.humanness.missing(),
        missing_systems: FOR_VERDICT
            .iter()
            .filter(|s| !t.parts.contains_key(s))
            .map(|s| s.name().to_owned())
            .collect(),
    }
}

/// 検める 1 本を測った結果。<strong>出なかったものは `None` である。</strong>
#[derive(Debug, Clone, PartialEq)]
pub struct Measured {
    /// 照合値。<strong>相手集合との中央値。</strong>
    pub matching: Option<f64>,
    /// 人らしさ値。
    pub humanness: Option<f64>,
    /// 測れなかった人らしさの次元。
    pub missing_humanness: Vec<String>,
    /// 測れなかった系統。
    pub missing_systems: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kakiburi_doc::node::{Kind, Node};
    use kakiburi_metrics::morph::{Dictionary, Morpheme};

    /// 試験用の解析器。<strong>UniDic を名乗り、字で切る。</strong>
    ///
    /// 仕様の手続きを通すためのものであって、日本語を解析するものではない。
    struct Chars;

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
                    let pos1 = match c {
                        'は' | 'が' | 'の' | 'を' | 'に' => "助詞",
                        '。' | '、' => "補助記号",
                        _ => "名詞",
                    };
                    Morpheme {
                        surface: c.to_string(),
                        lemma: c.to_string(),
                        pos1: pos1.into(),
                        pos2: "*".into(),
                    }
                })
                .collect()
        }
    }

    /// 1 単位ぶんの文書。<strong>すべての除外を越える長さにする。</strong>
    ///
    /// <strong>長さは単位ごとに散らす。</strong> 揃えると広がりが 0 になり、長さの範囲の検査が
    /// 「重なり 0」で止まる——両側が同じ範囲に散っている素材でなければ先へ進めない。
    fn document(index: usize, machine: bool) -> Document {
        let seed = index * if machine { 17 } else { 13 };
        let nodes: Vec<Node> = (0..60 + index * 4)
            .map(|i| {
                // <strong>骨格は両側で同じにする。</strong> 違えば、長さの差が両側の違いに混ざる。
                let (a, b, c, d) = if machine {
                    // 機械の側。<strong>語を散らす</strong>——繰り返しが足りない側に出る。
                    (
                        wordy(seed + i),
                        wordy(seed + i * 3),
                        wordy(seed + i * 7),
                        wordy(seed + i * 11),
                    )
                } else {
                    // 人の側。<strong>同じ言い回しを繰り返す。</strong>
                    (wordy(seed % 2), wordy(0), wordy(1), wordy(seed % 3))
                };
                Node::leaf(Kind::Paragraph, format!("{a}は、{b}であり、{c}だと{d}。"))
            })
            .collect();
        Document::new(nodes)
    }

    /// 語のかわりに使う、種で変わるかな列。
    fn wordy(n: usize) -> String {
        const KANA: [char; 10] = ['あ', 'か', 'さ', 'た', 'な', 'は', 'ま', 'や', 'ら', 'わ'];
        (0..3)
            .map(|i| KANA[(n / 10_usize.pow(i) + i as usize) % KANA.len()])
            .collect()
    }

    struct Material {
        person: Vec<(String, Document)>,
        baseline: Vec<(String, Document)>,
    }

    impl Material {
        fn new(n: usize) -> Self {
            Self {
                person: (0..n)
                    .map(|i| (format!("p{i:02}"), document(i, false)))
                    .collect(),
                baseline: (0..n)
                    .map(|i| (format!("b{i:02}"), document(i, true)))
                    .collect(),
            }
        }

        fn samples(pairs: &[(String, Document)]) -> Vec<Sample<'_>> {
            pairs
                .iter()
                .map(|(n, d)| Sample {
                    name: n,
                    document: d,
                })
                .collect()
        }
    }

    #[test]
    fn 素材が足りなければ目盛りを作らない() {
        // 作れないことは失敗ではない。<strong>判定できないが返る。</strong>
        let m = Material::new(4);
        let e = assemble(
            &Material::samples(&m.person),
            &Material::samples(&m.baseline),
            Some(&Chars),
        )
        .unwrap_err();
        assert!(matches!(e, ScaleError::Split(_)), "{e}");
    }

    #[test]
    fn 解析器が無ければ系統が揃わない() {
        // 5 系統のうち 2 つが形態素を要る。<strong>抜いて合算しない。</strong>
        let m = Material::new(10);
        let e = assemble(
            &Material::samples(&m.person),
            &Material::samples(&m.baseline),
            None,
        )
        .unwrap_err();
        assert!(matches!(e, ScaleError::Split(_)), "{e}");
    }

    #[test]
    fn 端まで通ると目盛りができる() {
        let m = Material::new(10);
        let scale = assemble(
            &Material::samples(&m.person),
            &Material::samples(&m.baseline),
            Some(&Chars),
        )
        .expect("目盛りができる");
        assert_eq!(scale.frozen.len(), 5, "判定に使う 5 系統");
        assert_eq!(scale.partners.len(), 5);
        assert_eq!(scale.calibration.systems().len(), 5);
    }

    #[test]
    fn 語彙は割る前に全体から固定する() {
        // 側ごとに違う語彙を使えば、側ごとに次元の意味が変わる。
        let m = Material::new(10);
        let scale = assemble(
            &Material::samples(&m.person),
            &Material::samples(&m.baseline),
            Some(&Chars),
        )
        .unwrap();
        let (_, set) = scale
            .frozen
            .iter()
            .find(|(n, _)| n == System::CharType.name())
            .unwrap();
        assert_eq!(set.len(), 10, "文字種は 10 次元");
        let (_, comma) = scale
            .frozen
            .iter()
            .find(|(n, _)| n == System::Comma.name())
            .unwrap();
        assert_eq!(comma.parts().len(), 3, "読点は 3 つの部分ベクトル");
    }

    #[test]
    fn 検めは目盛りを受け取るだけである() {
        let m = Material::new(10);
        let person = Material::samples(&m.person);
        let scale = assemble(&person, &Material::samples(&m.baseline), Some(&Chars)).unwrap();
        // 相手集合の 5 本を名前で引く。
        let partners: Vec<Sample<'_>> = person
            .iter()
            .filter(|s| scale.partners.iter().any(|n| n == s.name))
            .copied()
            .collect();
        assert_eq!(partners.len(), 5);
        let got = measure_against(&scale, person[9], &partners, Some(&Chars));
        assert!(got.matching.is_some(), "照合値が出る");
        assert!(got.humanness.is_some(), "人らしさ値が出る");
        assert!(got.missing_systems.is_empty());
    }

    #[test]
    fn 系統が欠ければ照合値を出さない() {
        let m = Material::new(10);
        let person = Material::samples(&m.person);
        let scale = assemble(&person, &Material::samples(&m.baseline), Some(&Chars)).unwrap();
        let partners: Vec<Sample<'_>> = person
            .iter()
            .filter(|s| scale.partners.iter().any(|n| n == s.name))
            .copied()
            .collect();
        // 短い文書は除外に掛かる。<strong>0 ではなく、出ないである。</strong>
        let short = Document::new(vec![Node::leaf(Kind::Paragraph, "短い。")]);
        let got = measure_against(
            &scale,
            Sample {
                name: "短い",
                document: &short,
            },
            &partners,
            Some(&Chars),
        );
        assert_eq!(got.matching, None);
        assert_eq!(got.missing_systems.len(), 5);
        assert_eq!(got.missing_humanness.len(), 12);
    }
}
