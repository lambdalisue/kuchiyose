//! 目盛りを組み立てる。素材から端まで通す 1 本の道である。
//!
//! 順序が守るべきものを持っている。
//!
//! | 順 | すること | 崩すと何が起きるか |
//! | --- | --- | --- |
//! | 1 | 測れた単位だけを取る | 除外に掛かった単位が対に入り、相手の本数が黙って 5 を割る |
//! | 2 | 長さの範囲を確かめる | 長さと連動する指標がすべて離れて見える |
//! | 3 | 割る前に語彙を固定する | 側ごとに次元の意味が変わる |
//! | 4 | 対を割り当てる | 較正と目盛りが対を共有し、いちばん危ない検査が無効になる |
//! | 5 | 較正して帯を作る | |
//!
//! 組み立てるのは測り終えた単位からである。 単位は文書ごとの
//! [統計値](crate::stats)から作る——本文から測る道も、1 本ずつ統計値にしてから通す。

use std::collections::{BTreeMap, BTreeSet};

use kuchiyose_doc::Document;
use kuchiyose_metrics::lexicon::Lexicon;
use kuchiyose_metrics::matching::{self, FOR_VERDICT};
use kuchiyose_metrics::morph::{Analyzed, Analyzer};
use kuchiyose_metrics::system::System;
use kuchiyose_metrics::Humanness;

use crate::band::Band;
use crate::calibrate::{median, Calibration};
use crate::examples::{examples_for, ExampleTable};
use crate::humanness::HumannessScale;
use crate::pairing::{pair, Pair};
use crate::split::{self, Unit};
use crate::stats::{phrases_in, Phrases};
use crate::vocabulary::{cosine_delta, Counts, FrozenSet};
use crate::{length_range_ok, Scale, ScaleError};

/// 素材 1 本。
#[derive(Debug, Clone, Copy)]
pub struct Sample<'a> {
    /// 単位の名前。ファイル名ではない。
    pub name: &'a str,
    /// 正規形。
    pub document: &'a Document,
}

/// 1 単位を測り終えた形。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Measurements {
    /// 系統ごとの部分ベクトルの数え上げ。測れなかった系統は入っていない。
    pub(crate) parts: BTreeMap<System, Vec<Counts>>,
    /// その単位の中で再来した言い回し。
    pub(crate) recurring: Vec<String>,
    /// その単位が繰り返しすぎている短い言い回し。検める 1 本でだけ測る。
    pub(crate) overused: Vec<String>,
    /// その単位で一度しか出てこない語。語を散らしている当のものである。検める 1 本でだけ測る。
    pub(crate) once_only: Vec<String>,
    /// その単位に現れる語の並びと、位置と node の番号。型を取り出す材料。
    pub(crate) phrases: Phrases,
    /// その単位に現れる[語](kuchiyose_metrics::word::goi)の語彙素と、品詞と回数。
    /// 語形が散っても 1 つに合流する。
    pub(crate) goi: BTreeMap<String, (String, usize)>,
    /// その単位に現れる[一人称](kuchiyose_metrics::word::FIRST_PERSON)と、その回数。
    pub(crate) first_person: BTreeMap<String, usize>,
    /// [書き出しの node の種類](kuchiyose_doc::Document::opening)の名前。
    pub(crate) opening: Option<String>,
    /// 人らしさの次元。
    pub(crate) humanness: Humanness,
    /// 地の文の日本語の文字数。長さの範囲に使う。
    pub(crate) chars: usize,
    /// 本人の側で再来した言い回しが、この単位の地の文に現れた回数。言い回しの上限に使う。
    ///
    /// [文節の並び](Self::phrases)では数えられない。 渡す言い回しは
    /// `ています。` のように文節にならないものを含むので、文節の並びと照らすと
    /// ほとんどが 0 になる——実測で、20 本のうち 15 本の上限が 0 だった。
    /// だから地の文を文字列として数える。
    pub(crate) phrase_hits: BTreeMap<String, usize>,
    /// 読める系統の次元ごとの実例。
    pub(crate) examples: ExampleTable,
}

/// 素材からコーパスの語を見つける。
///
/// 解析するだけで、測らない。 ここで測ってしまうと、割れたままの値が
/// 派生物に残る。
pub(crate) fn lexicon_of(samples: &[Sample<'_>], analyzer: Option<&dyn Analyzer>) -> Lexicon {
    let Some(a) = analyzer else {
        return Lexicon::default();
    };
    let units: Vec<Vec<Vec<kuchiyose_metrics::morph::Morpheme>>> = samples
        .iter()
        .filter_map(|s| {
            let prose = kuchiyose_doc::prose::mask_identifiers(&s.document.prose());
            Analyzed::of(&prose, a).ok().map(|x| x.segments().to_vec())
        })
        .collect();
    Lexicon::find(&units)
}

impl Measurements {
    /// 検める 1 本を本文から測る。
    ///
    /// 目盛りの材料は[統計値](crate::stats)から作る。 ここで測るのは、目盛りに
    /// 載せる側の 1 本だけである。
    pub(crate) fn of(
        sample: Sample<'_>,
        analyzer: Option<&dyn Analyzer>,
        lexicon: &Lexicon,
    ) -> Self {
        // 識別子を伏せてから測る。 `denops.vim` のような半角英字の連なりは
        // 書き手が選んだ書きぶりではなく題材が決めるもので、そのまま入れると
        // 同じ人が別の題材で書いた文章を「その人らしくない」と言う。
        //
        // 掛けるのはここだけである。 指示できる指標は和欧間スペースや半角英字
        // そのものを測るので、生の文から測り続ける。
        let prose = kuchiyose_doc::prose::mask_identifiers(&sample.document.prose());
        // 解析は 1 度だけ。 指標ごとに呼べば、外の実行ファイルを指標の数だけ起こす。
        let analyzed = analyzer.and_then(|a| Analyzed::with_lexicon(&prose, a, lexicon).ok());
        let mut parts = BTreeMap::new();
        for s in FOR_VERDICT {
            if let Some(p) = matching::parts(s, &prose, analyzed.as_ref()) {
                parts.insert(s, p);
            }
        }
        Self {
            parts,
            recurring: kuchiyose_metrics::humanness::recurring(
                analyzed.as_ref(),
                &kuchiyose_metrics::humanness::LONG_N,
            ),
            // 「減らせ」と言うなら、どれを減らすのかを言う。
            overused: kuchiyose_metrics::humanness::overused(
                analyzed.as_ref(),
                &kuchiyose_metrics::humanness::SHORT_N,
                OVERUSED,
            ),
            // 「散らすな」と言うなら、どれが散らしているのかを言う。
            once_only: kuchiyose_metrics::humanness::once_only(analyzed.as_ref(), ONCE_ONLY),
            phrases: phrases_in(analyzed.as_ref()),
            goi: kuchiyose_metrics::word::goi(analyzed.as_ref()),
            // どの一人称を選ぶかは、題材が変わっても動かない。
            first_person: kuchiyose_metrics::word::first_person(analyzed.as_ref()),
            // 書き出しに何を置くかも、題材ではなく書き手が決める。
            opening: sample
                .document
                .opening()
                .map(|k| k.name().to_owned()),
            humanness: Humanness::measure(&prose, analyzed.as_ref()),
            chars: sample.document.japanese_chars(),
            phrase_hits: BTreeMap::new(),
            examples: ExampleTable::new(),
        }
    }

    /// 判定に使う系統が全部測れたか。
    fn systems_measured(&self) -> bool {
        FOR_VERDICT.iter().all(|s| self.parts.contains_key(s))
    }

    /// 測れたかの内訳。
    pub(crate) fn report(&self, name: &str) -> Report {
        Report {
            name: name.to_owned(),
            chars: self.chars,
            missing_systems: FOR_VERDICT
                .iter()
                .filter(|sys| !self.parts.contains_key(sys))
                .map(|sys| sys.name().to_owned())
                .collect(),
            missing_humanness: self.humanness.missing(),
        }
    }
}

/// 1 単位が測れたかの内訳。
///
/// 止まったなら、どの単位のどこで止まったかを言う。 「10 本に届かない」だけでは、
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

/// 組み立てに渡す、測り終えた単位。
///
/// 名前は 2 つの欄を跨いで一意でなければならない——測った値を名前で引くので、
/// 重なれば片方の値がもう片方で黙って置き換わる。
pub(crate) struct Units {
    /// 本人の単位。
    pub(crate) person: Vec<(String, Measurements)>,
    /// 基準の単位。
    pub(crate) baseline: Vec<(String, Measurements)>,
    /// 本人の側の語のまとめ方。検める草稿もこれで測る。
    pub(crate) lexicon: Lexicon,
}

/// 測り終えた単位から組み立てる。
pub(crate) fn build(u: Units) -> Result<Scale, ScaleError> {
    let Units {
        person,
        baseline,
        lexicon,
    } = u;
    let as_unit = |name: &str, m: &Measurements| -> Unit {
        Unit {
            name: name.to_owned(),
            systems_measured: m.systems_measured(),
            humanness_measured: m.humanness.all_measured(),
        }
    };
    let person_units: Vec<Unit> = person.iter().map(|(n, m)| as_unit(n, m)).collect();
    let baseline_units: Vec<Unit> = baseline.iter().map(|(n, m)| as_unit(n, m)).collect();
    // 渡す側が一意にしたつもりでも、ここで確かめる。 名前は読み戻した形代から
    // 来るので、重ならないことを渡す側の約束に預けない。
    let mut measured: BTreeMap<String, Measurements> = BTreeMap::new();
    for (name, m) in person.into_iter().chain(baseline) {
        if measured.contains_key(&name) {
            return Err(ScaleError::DuplicateName(name));
        }
        measured.insert(name, m);
    }

    // 1. 測れた単位だけを取り、どちらも 5 ＋ 5 に届くことを確かめる。
    let person_split = split::split(&person_units).map_err(ScaleError::Split)?;
    let baseline_split = split::split(&baseline_units).map_err(ScaleError::Split)?;

    // 2. 長さの範囲。指標ごとではなく、その場面まるごと止める。
    //
    // 見るのは実際に使う単位の範囲である。 除外に掛かって目盛りに入らない単位を
    // 混ぜれば、比べていないものの長さで止まったり通ったりする。
    let chars = |s: &split::Split| -> Vec<usize> {
        s.partners
            .iter()
            .chain(&s.points)
            .map(|u| measured[&u.name].chars)
            .collect()
    };
    length_range_ok(&chars(&person_split), &chars(&baseline_split))?;

    // 3. 割る前に語彙を固定する。 全体から選ぶ——側ごとに違う語彙を使えば、
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
        // 平均と標準偏差は基準から取る。 本人を含む素材で標準化すると、
        // 本人の単位が過半を占めるぶん平均が本人のところに来て、
        // 本人の記事は原点に置かれる——z 得点に残るのは 1 本ごとの雑音だけになる。
        let reference: Vec<Vec<Counts>> = baseline_split
            .partners
            .iter()
            .chain(&baseline_split.points)
            .filter_map(|u| measured[&u.name].parts.get(&s).cloned())
            .collect();
        frozen.push((
            s.name().to_owned(),
            FrozenSet::fit_against(&all, &reference, &matching::limits(s)),
        ));
    }

    // 系統ごとの、単位 → z 得点のベクトル。
    let mut projected: BTreeMap<(String, String), Vec<f64>> = BTreeMap::new();
    for u in used.iter().copied() {
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

    // 4. 対を割り当てる。作り手を 1 つにしたうえで、それでも確かめる。
    let pairing = pair(&person_split, &baseline_split);
    if pairing.shares_pairs() {
        return Err(ScaleError::SharedPairs);
    }

    // 系統ごとの距離。片方でも投影できなければ対を捨てる。
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

    // 5. 天井と床。1 本につき 1 点——相手集合との照合値の中央値である。
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

    // 人らしさ。対ではなく単位で割る——1 本ごとに出る値で、対を作らない。
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
    // 較正には、帯に使う分を除いた全部を渡す。
    //
    // 帯の端を各側 5 点に固定したのは、最小・最大が n とともに外へ広がるから
    // である。回帰は漂わないので、同じ縛りを較正に掛ける理由が無い——
    // 10 単位で 4 つの重みを当てはめると、標本外で当たらない。
    //
    // 帯の点は除く。 較正に使った単位で帯を作れば、分離するように合わせた
    // ものの分離具合を見ることになる。
    let for_calibration = |all: &[Unit], points: &[Unit]| -> Vec<Unit> {
        all.iter()
            .filter(|u| u.usable() && !points.iter().any(|p| p.name == u.name))
            .cloned()
            .collect()
    };
    let human_units = for_calibration(&person_units, &person_split.points);
    let machine_units = for_calibration(&baseline_units, &baseline_split.points);

    // 測れなかったものは落とす。 次元が揃わない行を混ぜれば、列の数が
    // 行ごとに変わる。
    let humanness = HumannessScale::fit(&rows_of(&human_units), &rows_of(&machine_units));
    let side = |us: &[Unit]| -> Vec<f64> {
        us.iter()
            .filter_map(|u| humanness.value(&measured[&u.name].humanness.flat()).ok())
            .collect()
    };
    let human = side(&person_split.points);
    let machine = side(&baseline_split.points);
    // 重なりでは止めない。 人らしさの帯が全体を覆うのは設計どおりの結果で、
    // 当てはまらないことが判定不能として出る。止めれば自己診断が消える。
    let humanness_band = Band::build_humanness(&human, &machine).map_err(|source| {
        ScaleError::Band(Box::new(crate::BandStop {
            source,
            which: "基準との距離",
            ceiling: human.clone(),
            floor: machine.clone(),
            // 照合の帯は作れている。 止まったのは人らしさの側である。
            built: Some(band),
        }))
    })?;

    // 本人の代表値を先に作る。 効く量を数える相手である。
    let humanness_target: Vec<(String, f64)> = {
        let rows: Vec<Vec<(&str, f64)>> = person_units
            .iter()
            .filter(|u| u.usable())
            .filter_map(|u| {
                humanness
                    .by_metric(&measured[&u.name].humanness.flat())
                    .ok()
            })
            .collect();
        kuchiyose_metrics::humanness::Metric::ALL
            .into_iter()
            .enumerate()
            .filter_map(|(j, m)| {
                let mut col: Vec<f64> = rows
                    .iter()
                    .filter_map(|r| r.get(j).map(|(_, v)| *v))
                    .collect();
                if col.is_empty() {
                    return None;
                }
                col.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                Some((m.name().to_owned(), col[col.len() / 2]))
            })
            .collect()
    };

    // 相手集合のベクトルを取り置く。 検めるときに本文が無くても照合値を
    // 出せるようにするためである。
    let partner_vectors: Vec<crate::PartnerVector> = person_split
        .partners
        .iter()
        .map(|u| {
            let m = &measured[&u.name];
            let parts = frozen
                .iter()
                .filter_map(|(name, set)| {
                    let system = System::from_name(name)?;
                    Some((name.clone(), set.project(m.parts.get(&system)?)?))
                })
                .collect();
            (u.name.clone(), parts)
        })
        .collect();

    // 実例も取り置く。 検めるときに本文へ読みに行く道が無い。
    let partner_examples: Vec<&ExampleTable> = person_split
        .partners
        .iter()
        .map(|u| &measured[&u.name].examples)
        .collect();
    let examples = examples_for(&frozen, &partner_examples);

    let phrases = recurring_phrases(&person_units, &measured);
    Ok(Scale {
        frozen,
        calibration,
        band,
        examples,
        partner_vectors,
        selection: {
            let names = |us: &[Unit]| us.iter().map(|u| u.name.clone()).collect();
            crate::Selection {
                person_partners: names(&person_split.partners),
                person_points: names(&person_split.points),
                baseline_partners: names(&baseline_split.partners),
                baseline_points: names(&baseline_split.points),
            }
        },
        humanness,
        humanness_band,
        // 作るのはここだけである。 検めが作り直せる形にしておくと、検める文書を
        // 見てから言い回しを選び直す経路が書ける。
        humanness_target,
        // 渡した言い回しには上限も渡す。 繰り返せとだけ言えば、行きすぎる。
        phrase_ceilings: ceilings_of(&phrases, &person_units, &measured),
        phrases,
        lexicon,
        katas: katas_of(&person_units, &machine_units, &measured, KATAS),
        // 役を入れ替えて、もう 1 度回す。 同じ仕組みで機械の型が出る。
        machine_katas: katas_of(&machine_units, &person_units, &measured, MACHINE_KATAS),
        // 並びで割れた癖を、語彙素で拾い直す。
        machine_gois: gois_of(&machine_units, &person_units, &measured, MACHINE_GOIS),
        // 一人称はどちらを選ぶかだけが問われる。 相手側は要らない——
        // 本人が何を選ぶかが分かれば、草稿が別のものを選んだことが言える。
        first_person: first_person_of(&person_units, &measured),
        // 書き出しの構造も同じ形である。 閉じた集合のうちどれを選ぶかを持つ。
        opening: opening_of(&person_units, &measured),
    })
}

/// 言い回しごとの、本人が 1 本の中で使う上限。日本語 1,000 字あたりの最大。
///
/// 「繰り返せ」と言うなら、どこまで繰り返してよいかも言う。 言わなければ、
/// 受け取った側は本人の何倍も入れる。日本語は壊れないので検査では止まらない
/// ——本人の頻度と比べるしかない。
///
/// 本文をそのまま数える。 形態素の並びではなく文字列として数えるので、
/// 検める側が同じ数え方をできる。
fn ceilings_of(
    phrases: &[String],
    units: &[Unit],
    measured: &BTreeMap<String, Measurements>,
) -> Vec<(String, f64)> {
    let mut out = Vec::with_capacity(phrases.len());
    for p in phrases {
        let mut top = 0.0f64;
        for u in units {
            let Some(m) = measured.get(&u.name) else {
                continue;
            };
            if m.chars == 0 {
                continue;
            }
            let n = m.phrase_hits.get(p).copied().unwrap_or(0);
            #[allow(clippy::cast_precision_loss)]
            let r = 1000.0 * n as f64 / m.chars as f64;
            if r > top {
                top = r;
            }
        }
        out.push((p.clone(), top));
    }
    out
}

/// 何本の単位で再来したかで、言い回しを並べる。
///
/// 1 本にしか出ない言い回しは、その文書の題材が作ったものである。 2 本以上で
/// 再来したものだけを、その人の癖として残す。
fn recurring_phrases(units: &[Unit], measured: &BTreeMap<String, Measurements>) -> Vec<String> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for u in units {
        let Some(m) = measured.get(&u.name) else {
            continue;
        };
        for g in &m.recurring {
            *seen.entry(g.clone()).or_default() += 1;
        }
    }
    let mut out: Vec<(usize, String)> = seen
        .into_iter()
        .filter(|(_, k)| *k >= 2)
        .map(|(g, k)| (k, g))
        .collect();
    // 多い順。同じなら短い順、次に文字の順。 決めておかないと並びが実装で変わる。
    out.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.chars().count().cmp(&b.1.chars().count()))
            .then_with(|| a.1.cmp(&b.1))
    });
    out.truncate(PHRASES);
    out.into_iter().map(|(_, g)| g).collect()
}

/// 残す言い回しの数。暫定値である。
pub const PHRASES: usize = 20;

/// 型として見る語の並びの長さ。暫定値である。
///
/// 数えるのは[文節](kuchiyose_metrics::word)の数である——形態素の窓ではない。
/// 窓で切ると `が地味` `に分け` のような、言葉として立たない断片が候補を埋める。
///
/// 短すぎれば誰でも書く並びになり、長すぎれば 1 本にしか出てこない。
pub const KATA_N: [usize; 4] = [1, 2, 3, 4];

/// 型と認める、本人の単位に現れる割合。暫定値である。
pub const KATA_PERSON_MIN: f64 = 0.10;

/// 型と認める、基準の単位に現れる割合の上限。暫定値である。
///
/// みんなが書く並びは、その人のものではない。
pub const KATA_BASE_MAX: f64 = 0.05;

/// 残す型の数。種類ごとに数える。 暫定値である。
///
/// まとめて数えると、場所の型が押し出される——場所の型は珍しいので割合が低い。
pub const KATAS: usize = 6;

/// 残す機械の型の数。本人の型よりずっと多く持つ。 暫定値である。
///
/// 枠の意味が違う。 本人の型は「この文章に入っていないもの」を言うので、
/// 多く持つほど指摘が薄まる。機械の型は「この文章に出ているもの」だけを言うので、
/// 多く持っても指摘は増えない——持っていない並びは見つけられないだけである。
///
/// 実測で、6 本に絞ると割合の高い並びが枠を占め、珍しい言い回しが落ちた——
/// 基準 3 本すべてが使う「地味に」は 18 単位中 2 本（11%）で、
/// 56% の「のではなく、」に押し出されていた。
pub const MACHINE_KATAS: usize = 200;

/// 残す機械の語の数。暫定値である。
///
/// [機械の型](MACHINE_KATAS)と同じ理由で多く持つ——この文章に出ているものしか
/// 言わないので、持っても指摘は増えない。
pub const MACHINE_GOIS: usize = 200;

/// 場所の型と認める、位置のばらつきの上限。暫定値である。
///
/// 短い並びは、決まった場所で使うときだけ型である。 どこにでも出てくる短い
/// 並びは、その人の癖ではなく日本語である。
pub const KATA_TIGHT: f64 = 0.12;

/// 場所を選ばない型と認める、並びの長さの下限（文字）。暫定値である。
///
/// 長い並びはそれ自体が珍しいので、位置を問わない。
///
/// 相手側に 1 度も出てこないなら、短くても残す。 短い並びを落とすのは
/// 「どこにでも出てくる短い並びは日本語であって癖ではない」からだが、
/// 日本語なら相手側にも出てくる。 片側にしか出てこない短い並びは、
/// その側のものである——実測で、基準 3 本すべてが使う「地味に」を本人は
/// 50 単位で 1 度も使っておらず、3 文字なので落ちていた。
pub const KATA_LONG: usize = 6;

/// 穴あきの型に繋いでよい、部品の割合に対する下限。暫定値である。
///
/// 繋ぐと割合は必ず下がる。 穴あきは 2 つが同じ node に現れた回数でしか
/// 数えられないので、部品が単独でそれより広く出ていれば、繋ぐことは
/// その分を捨てることである。
///
/// 実測で、出現割合 0.706 の `僕は` が 0.176 の `思います。`〜`僕は` に
/// 食われていた。繋いだ先は型として意味をなさず、いちばん広く使われていた
/// 一人称が表から消えた。
pub const FRAME_KEEP: f64 = 0.5;

/// その人の型。
///
/// コーパスから見つける。 手で並べた定型ではない——道具は書き手を選ばないので、
/// 特定の言い回しを実装に持たない。
#[derive(Debug, Clone, PartialEq)]
pub struct Kata {
    /// 語の並び。
    pub text: String,
    /// 本人の単位のうち、これが現れた割合。
    pub rate: f64,
    /// 文書の中での位置の中央。0 に近ければ書き出しの型である。
    pub at: f64,
    /// 位置のばらつき。小さければ決まった場所で使う型である。
    pub spread: f64,
    /// 相手側の単位のうち、これが現れた割合。
    ///
    /// 短い並びを残してよいかを、これで決める（[長さの下限](KATA_LONG)）。
    pub base: f64,
    /// 本人が 1 本の中でこれを使う、日本語 1,000 字あたりの最大。
    ///
    /// 「本人の言い回しを繰り返せ」には上限が要る。 繰り返せとだけ言うと、
    /// 受け取った側は本人の何倍も入れる——実測で、本人が 42 本で 25 回しか
    /// 使わない `ことができます` を、直した 1 本に 10 回入れていた。
    ///
    /// 日本語は壊れないので検査では止まらない。 止めるならここで測るしかない。
    pub ceiling: f64,
    /// 穴あきの型なら、後ろの固定部。間は書き手が埋める。
    ///
    /// `どうも、` … `ありすえです。` のように、固定部が 2 つあって間が変わる
    /// 書き出しは、連続した並びとしては拾えない。
    pub tail: Option<String>,
}

impl Kata {
    /// 散文にする。穴あきなら、間があることを見せる。
    #[must_use]
    pub fn shown(&self) -> String {
        match &self.tail {
            Some(t) => format!("{}〜{t}", self.text),
            None => self.text.clone(),
        }
    }
}

/// 機械の語。基準がよく使い、本人が使わない[語](kuchiyose_metrics::word::goi)。
///
/// [型](Kata)が取りこぼすものを取る。 型は表層の並びをそのまま照合するので、
/// 同じ癖が語形ごとに割れて、どの綴りも床を割ることがある——実測で、基準の池 44 本の
/// うち `地味` は 9 本（20%）に出るのに、`地味に` という綴りは 2 本にしかなく、
/// 絞った後は 1 単位（5.6%）で床を割って一度も拾えなかった。
///
/// 本人の側は作らない。 型には「入っていない本人の型を使え」と言う向きが
/// あるが、語にそれは無い——形容詞を 1 つ足せと言われても直せない。
/// 言えるのは「この語はその人のものではない」だけである。
#[derive(Debug, Clone, PartialEq)]
pub struct Goi {
    /// 語彙素。
    pub text: String,
    /// 基準の単位のうち、これが現れた割合。
    pub rate: f64,
    /// 本人の単位のうち、これが現れた割合。
    pub base: f64,
    /// 本人が同じ品詞でよく使う語彙素。置き換える先である。
    ///
    /// 「別の言い方にする」だけでは直せない。 受け取った側は道具の外で
    /// 語を探すことになり、そこで選んだ語がまた本人の使わない語でありうる。
    ///
    /// 言い換えの辞書は持たない。 同義語を出すのではなく、その人が現に
    /// その品詞で何を使うかを並べる——選ぶのは書き手である。
    pub theirs: Vec<String>,
}

/// 置き換える先として並べる、本人の語の数。暫定値である。
///
/// 多く出すと選べない。[渡す軸は 3〜4 本が頂点](../../../docs/references/styleremix-2024.md)
/// という報告と同じ向きで、ここも絞る。
pub const GOI_THEIRS: usize = 5;

/// 機械の語を取り出す。
///
/// 条件は[型](katas_of)と同じものを、語彙素に当てる——
/// 基準の[一定割合以上](KATA_PERSON_MIN)に現れ、本人には[ほとんど現れない](KATA_BASE_MAX)。
/// 新しい暫定値を増やさない。 同じ規則を別の単位に当てているだけである。
/// 本人の一人称と、それが現れた単位の割合。多い順。
///
/// 回数ではなく単位の割合で見る。 1 本で何度も書く人と、毎回 1 度だけ書く人の
/// どちらも「その一人称を使う人」である。
fn first_person_of(person: &[Unit], measured: &BTreeMap<String, Measurements>) -> Vec<(String, f64)> {
    let mut df: BTreeMap<&str, usize> = BTreeMap::new();
    for u in person {
        let Some(m) = measured.get(&u.name) else {
            continue;
        };
        for name in m.first_person.keys() {
            *df.entry(name.as_str()).or_insert(0) += 1;
        }
    }
    #[allow(clippy::cast_precision_loss)]
    let n = person.len().max(1) as f64;
    let mut out: Vec<(String, f64)> = df
        .into_iter()
        .map(|(name, k)| {
            #[allow(clippy::cast_precision_loss)]
            let rate = k as f64 / n;
            (name.to_owned(), rate)
        })
        .collect();
    // 多い順。同じなら名前の順。 決めておかないと並びが実装で変わる。
    out.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    out
}

/// 本人の書き出しの種類と、その割合。多い順。
///
/// [一人称](first_person_of)と同じ形である。 閉じた集合のうちどれを選ぶかなので、
/// 選ばれなかったことがそのまま癖になる。
fn opening_of(person: &[Unit], measured: &BTreeMap<String, Measurements>) -> Vec<(String, f64)> {
    let mut df: BTreeMap<&str, usize> = BTreeMap::new();
    let mut n = 0usize;
    for u in person {
        let Some(m) = measured.get(&u.name) else {
            continue;
        };
        // 数えられなかった単位は分母にも入れない。 入れると、書き出しの
        // 割合が「node を持たない単位の多さ」で薄まる。
        let Some(kind) = m.opening.as_deref() else {
            continue;
        };
        n += 1;
        *df.entry(kind).or_insert(0) += 1;
    }
    #[allow(clippy::cast_precision_loss)]
    let total = n.max(1) as f64;
    let mut out: Vec<(String, f64)> = df
        .into_iter()
        .map(|(kind, k)| {
            #[allow(clippy::cast_precision_loss)]
            let rate = k as f64 / total;
            (kind.to_owned(), rate)
        })
        .collect();
    // 多い順。同じなら名前の順。 決めておかないと並びが実装で変わる。
    out.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    out
}

fn gois_of(
    baseline: &[Unit],
    person: &[Unit],
    measured: &BTreeMap<String, Measurements>,
    cap: usize,
) -> Vec<Goi> {
    let df = |units: &[Unit]| -> BTreeMap<&str, usize> {
        let mut out: BTreeMap<&str, usize> = BTreeMap::new();
        for u in units {
            let Some(m) = measured.get(&u.name) else {
                continue;
            };
            for g in m.goi.keys() {
                *out.entry(g.as_str()).or_insert(0) += 1;
            }
        }
        out
    };
    let theirs = df(baseline);
    let mine = df(person);
    // 置き換える先は本人の中から出す。 品詞ごとに、延べで多い順。
    let mut by_pos: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
    for u in person {
        let Some(m) = measured.get(&u.name) else {
            continue;
        };
        for (lemma, (pos, n)) in &m.goi {
            *by_pos
                .entry(pos.as_str())
                .or_default()
                .entry(lemma.as_str())
                .or_insert(0) += n;
        }
    }
    let pos_of = |lemma: &str| -> Option<&str> {
        baseline.iter().find_map(|u| {
            measured
                .get(&u.name)
                .and_then(|m| m.goi.get(lemma))
                .map(|(pos, _)| pos.as_str())
        })
    };
    let suggest = |lemma: &str| -> Vec<String> {
        let Some(pos) = pos_of(lemma) else {
            return Vec::new();
        };
        let Some(words) = by_pos.get(pos) else {
            return Vec::new();
        };
        // その語自身を候補にしない。 対象は本人が使う単位の割合で選ぶが、
        // 候補は延べで並べるので、1 本に固めて使った語は両方に入る
        // ——「X を言い換える。本人がよく使うのは X」という指示になる。
        let mut v: Vec<(&&str, &usize)> = words.iter().filter(|(w, _)| **w != lemma).collect();
        // 延べで多い順。同じなら語彙素の順。 決めておかないと並びが実装で変わる。
        v.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        v.into_iter()
            .take(GOI_THEIRS)
            .map(|(w, _)| (*w).to_owned())
            .collect()
    };
    #[allow(clippy::cast_precision_loss)]
    let (nb, np) = (baseline.len().max(1) as f64, person.len().max(1) as f64);
    let mut out: Vec<Goi> = theirs
        .into_iter()
        .filter_map(|(text, k)| {
            #[allow(clippy::cast_precision_loss)]
            let rate = k as f64 / nb;
            #[allow(clippy::cast_precision_loss)]
            let base = mine.get(text).copied().unwrap_or(0) as f64 / np;
            if rate < KATA_PERSON_MIN || base > KATA_BASE_MAX {
                return None;
            }
            Some(Goi {
                theirs: suggest(text),
                text: text.to_owned(),
                rate,
                base,
            })
        })
        .collect();
    // 広く使う順。同じなら語彙素の順。 決めておかないと並びが実装で変わる。
    out.sort_by(|a, b| {
        b.rate
            .partial_cmp(&a.rate)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.text.cmp(&b.text))
    });
    out.truncate(cap);
    out
}

/// 穴あきの型に繋ぐ。
///
/// 固定部が 2 つあって間が変わる書き出しは、連続した並びとしては拾えない。
/// 実測で、ある書き手の挨拶は 11 本の記事で `どうも、`〜`です。` の形をしていたが、
/// 間に入る一言が毎回違うので、1 つの並びとしては一度も繰り返されていなかった。
///
/// > 2 つの型が、本人の単位の[一定割合以上](KATA_PERSON_MIN)で同じ node に
/// > この順で現れるなら、繋げて 1 つの穴あきの型とする。
///
/// 繋いだら、部品は落とす。 3 本に分けて渡せば、受け取った側は 3 か所に
/// 挿しこむことになる。
fn frames_of(
    katas: &mut Vec<Kata>,
    person: &[Unit],
    baseline: &[Unit],
    measured: &BTreeMap<String, Measurements>,
) {
    // node ごとに、その node に現れた型を順番に持つ。
    let seen = |units: &[Unit]| -> Vec<BTreeSet<(usize, usize)>> {
        units
            .iter()
            .map(|u| {
                let mut out: BTreeSet<(usize, usize)> = BTreeSet::new();
                let Some(m) = measured.get(&u.name) else {
                    return out;
                };
                let mut at: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
                for (g, p) in &m.phrases {
                    if let Some(i) = katas.iter().position(|k| k.text == *g) {
                        for node in &p.nodes {
                            at.entry(*node).or_default().insert(i);
                        }
                    }
                }
                for here in at.values() {
                    for &a in here {
                        for &b in here {
                            if a != b {
                                out.insert((a, b));
                            }
                        }
                    }
                }
                out
            })
            .collect()
    };
    let mine = seen(person);
    let theirs = seen(baseline);
    #[allow(clippy::cast_precision_loss)]
    let (np, nb) = (person.len().max(1) as f64, baseline.len().max(1) as f64);

    let mut frames: Vec<(usize, usize, f64)> = Vec::new();
    for a in 0..katas.len() {
        for b in 0..katas.len() {
            // 重なっている 2 つを繋がない。 同じ型の切り出し方が違うだけのものを
            // 繋ぐと、`ご無沙汰して`〜`しております` のような穴あきができる。
            if a == b
                || overlaps(&katas[a].text, &katas[b].text)
                || chains(&katas[a].text, &katas[b].text)
            {
                continue;
            }
            #[allow(clippy::cast_precision_loss)]
            let rate = mine.iter().filter(|s| s.contains(&(a, b))).count() as f64 / np;
            #[allow(clippy::cast_precision_loss)]
            let base = theirs.iter().filter(|s| s.contains(&(a, b))).count() as f64 / nb;
            if rate >= KATA_PERSON_MIN
                && base <= KATA_BASE_MAX
                && frame_keeps(rate, katas[a].rate, katas[b].rate)
            {
                frames.push((a, b, rate));
            }
        }
    }
    frames.sort_by(|x, y| y.2.partial_cmp(&x.2).unwrap_or(std::cmp::Ordering::Equal));

    let mut used: BTreeSet<usize> = BTreeSet::new();
    let mut made: Vec<Kata> = Vec::new();
    for (a, b, rate) in frames {
        if used.contains(&a) || used.contains(&b) {
            continue;
        }
        used.insert(a);
        used.insert(b);
        made.push(Kata {
            text: katas[a].text.clone(),
            tail: Some(katas[b].text.clone()),
            rate,
            at: katas[a].at,
            spread: katas[a].spread.min(katas[b].spread),
            // 穴あきは 2 つとも相手側に出ないものから作る。
            base: katas[a].base.max(katas[b].base),
            // 穴あきは前の固定部の上限で見る。
            ceiling: katas[a].ceiling,
        });
    }
    if made.is_empty() {
        return;
    }
    let mut kept: Vec<Kata> = katas
        .iter()
        .enumerate()
        .filter(|(i, _)| !used.contains(i))
        .map(|(_, k)| k.clone())
        .collect();
    kept.extend(made);
    kept.sort_by(|a, b| {
        b.rate
            .partial_cmp(&a.rate)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.text.cmp(&b.text))
    });
    *katas = kept;
}

/// 穴あきに繋いで、部品を落としてよいか。
///
/// 繋ぐと割合は必ず下がる。 穴あきは 2 つが同じ node に現れた回数でしか
/// 数えられないので、部品が単独でそれより広く出ているぶんは、繋いだ時点で
/// 捨てている。
fn frame_keeps(frame: f64, a: f64, b: f64) -> bool {
    frame >= a * FRAME_KEEP && frame >= b * FRAME_KEEP
}

/// 前の終わりと後ろの始まりが重なっているか。
///
/// 繋がっている 1 つの言い回しを、穴あきの型にしない。
/// `ご無沙汰して` と `しております` は `して` で繋がっていて、間に何も入らない。
fn chains(a: &str, b: &str) -> bool {
    let touching = |a: &str, b: &str| {
        let x: Vec<char> = a.chars().collect();
        let y: Vec<char> = b.chars().collect();
        (2..=x.len().min(y.len())).any(|n| x[x.len() - n..] == y[..n])
    };
    // どちら向きでも見る。 同じ地の文から切り出した 2 つは、順番を入れ替えても
    // 同じ node に現れる。
    touching(a, b) || touching(b, a)
}

/// 位置のばらつき。母標準偏差である。
fn spread_of(ats: &[f64]) -> f64 {
    if ats.len() < 2 {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let n = ats.len() as f64;
    let mean = ats.iter().sum::<f64>() / n;
    (ats.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n).sqrt()
}

/// 2 つの並びが同じ型の切り出しか。
///
/// 片方がもう片方を含むか、端どうしが半分以上重なっているなら同じものとみなす。
fn overlaps(a: &str, b: &str) -> bool {
    if a.contains(b) || b.contains(a) {
        return true;
    }
    // 共通する並びが短いほうの半分を超えるなら、同じ型である。
    // 端どうしだけを見ると、真ん中で重なっているものを取りこぼす。
    let (x, y): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let want = x.len().min(y.len()) / 2 + 1;
    if want > x.len() {
        return false;
    }
    (0..=x.len() - want).any(|i| {
        let piece: String = x[i..i + want].iter().collect();
        b.contains(&piece)
    })
}

/// その人の型を取り出す。
///
/// > 本人の単位の[一定割合以上](KATA_PERSON_MIN)に現れ、
/// > 基準の単位には[ほとんど現れない](KATA_BASE_MAX)語の並び。
///
/// 2 つとも要る。 前者だけなら「ています。」のような誰でも書く並びが並び、
/// 後者だけなら 1 本にしかない偶然が並ぶ。
///
/// 長い並びを先に採り、その一部になる短い並びは落とす。 同じ型を長短で二重に
/// 数えない。
fn katas_of(
    person: &[Unit],
    baseline: &[Unit],
    measured: &BTreeMap<String, Measurements>,
    cap: usize,
) -> Vec<Kata> {
    let df = |units: &[Unit]| -> BTreeMap<String, (usize, Vec<f64>)> {
        let mut out: BTreeMap<String, (usize, Vec<f64>)> = BTreeMap::new();
        for u in units {
            let Some(m) = measured.get(&u.name) else {
                continue;
            };
            for (g, p) in &m.phrases {
                let e = out.entry(g.clone()).or_insert((0, Vec::new()));
                e.0 += 1;
                e.1.push(p.first);
            }
        }
        out
    };
    // 1 本の中で何回使うか。 何本に出るか（df）とは別の量である——
    // 「繰り返せ」と言うなら、どこまで繰り返してよいかを言わなければ、
    // 受け取った側は本人の何倍も入れる。
    let per_1000 = |units: &[Unit]| -> BTreeMap<String, f64> {
        let mut out: BTreeMap<String, f64> = BTreeMap::new();
        for u in units {
            let Some(m) = measured.get(&u.name) else {
                continue;
            };
            if m.chars == 0 {
                continue;
            }
            for (g, p) in &m.phrases {
                let n = p.count;
                #[allow(clippy::cast_precision_loss)]
                let r = 1000.0 * n as f64 / m.chars as f64;
                let e = out.entry(g.to_owned()).or_insert(0.0);
                if r > *e {
                    *e = r;
                }
            }
        }
        out
    };
    let ceilings = per_1000(person);
    let mine = df(person);
    let theirs = df(baseline);
    #[allow(clippy::cast_precision_loss)]
    let (np, nb) = (person.len().max(1) as f64, baseline.len().max(1) as f64);
    let mut cands: Vec<Kata> = mine
        .into_iter()
        .filter_map(|(text, (k, ats))| {
            #[allow(clippy::cast_precision_loss)]
            let rate = k as f64 / np;
            #[allow(clippy::cast_precision_loss)]
            let base = theirs.get(&text).map_or(0.0, |(b, _)| *b as f64) / nb;
            if rate < KATA_PERSON_MIN || base > KATA_BASE_MAX {
                return None;
            }
            // ひらがなを 1 つも含まない並びは題材である。
            //
            // 付属語も活用もひらがなで書かれる。 漢字とカタカナだけの並びは、
            // 言い方ではなく語そのもの——実測で、繰り返し出ている順に並べたとたん
            // `フロントエンド` が上位に来た。8 回出ていたが、それはその記事の題材である。
            //
            // [識別子を伏せる](kuchiyose_doc::prose::mask_identifiers)のと同じ理由で、
            // 題材が書きぶりの指摘に混ざるのを止める。
            if !text.chars().any(|c| ('\u{3041}'..='\u{309f}').contains(&c)) {
                return None;
            }
            let ceiling = ceilings.get(&text).copied().unwrap_or(0.0);
            Some(Kata {
                text,
                rate,
                at: median(&ats).unwrap_or(0.0),
                spread: spread_of(&ats),
                base,
                ceiling,
                tail: None,
            })
        })
        .collect();
    // 広く使う順。同じなら長さ、次に文字の順。 決めておかないと並びが実装で変わる。
    //
    // 長さを先に見てはいけない。 長い順に採ると、短くて頻度の高い型が枠から
    // 押し出される——実測で、11 本の記事に出てくる挨拶の書き出しが落ちていた。
    //
    cands.sort_by(|a, b| {
        b.rate
            .partial_cmp(&a.rate)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.text.chars().count().cmp(&a.text.chars().count()))
            .then_with(|| a.text.cmp(&b.text))
    });
    // 2 種類を別々に数える。
    //
    // | | 何を型とするか |
    // | --- | --- |
    // | 場所の型 | 位置が偏っているもの。短くてもよい |
    // | 言い回しの型 | 長いもの。位置は問わない |
    //
    // まとめて数えると、場所の型が押し出される——場所の型は珍しいので割合が低い。
    let mut out: Vec<Kata> = Vec::new();
    let (mut tight, mut loose) = (0usize, 0usize);
    for k in cands {
        let is_tight = k.spread <= KATA_TIGHT;
        let slot = if is_tight { &mut tight } else { &mut loose };
        // 相手側に 1 度も出てこないなら、短くても残す。
        let only_here = k.base <= 0.0;
        if *slot >= cap || (!is_tight && !only_here && k.text.chars().count() < KATA_LONG) {
            continue;
        }
        // 重なっている型を二重に持たない。 入っている場合だけでなく、
        // 端が重なっているだけでも落とす——`ご無沙汰しており` と
        // `無沙汰しております` は同じ型の切り出し方が違うだけである。
        if out.iter().any(|kept| overlaps(&kept.text, &k.text)) {
            continue;
        }
        *slot += 1;
        out.push(k);
    }
    frames_of(&mut out, person, baseline, measured);
    out.sort_by(|a, b| {
        b.rate
            .partial_cmp(&a.rate)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.text.cmp(&b.text))
    });
    out
}

/// 名指しする「繰り返しすぎ」の数。暫定値である。
pub const OVERUSED: usize = 3;

/// 名指しする「一度きりの語」の数。暫定値である。
pub const ONCE_ONLY: usize = 8;

/// 検める 1 本を、作り終えた目盛りに載せる。
///
/// 目盛りは作り直さない。 語彙も重みも受け取ったものを使う——検める文書を見てから
/// 作り直せる経路を持たない。
///
/// 相手集合は目盛りが持っている（[取り置いたベクトル](Scale::partner_vectors)）。
/// 照合値は相手集合との中央値で、本文は要らない——投影し終えた値だけで足りる。
#[must_use]
pub fn measure_against(
    scale: &Scale,
    target: Sample<'_>,
    analyzer: Option<&dyn Analyzer>,
) -> Measured {
    // 形代が持つ辞書で測る。 作ったときと違う割り方をすれば、
    // 比べたものに意味が無い。
    let t = Measurements::of(target, analyzer, &scale.lexicon);

    let vector = |m: &Measurements, name: &str| -> Option<Vec<f64>> {
        let system = System::from_name(name)?;
        let set = &scale.frozen.iter().find(|(n, _)| n == name)?.1;
        set.project(m.parts.get(&system)?)
    };
    let (matching, matching_substituted) = matching_median(scale, &|n| vector(&t, n));
    Measured {
        matching,
        matching_substituted,
        humanness: scale.humanness.value(&t.humanness.flat()).ok(),
        // 合算した 1 つの値では直し方を渡せない。「機械の側にある」としか
        // 言えず、どこをどうすればよいかが出てこない。
        //
        // 寄せる向きは較正から読む。 定義に固定すると、素材がその向きを
        // 支えていない形代で直し方に従うほど人らしさが下がる。
        //
        // 次元の向きが割れている指標は渡さない。 どちらへ動かせばよいかを
        // 言えないものを指示にしない。
        humanness_by_metric: {
            let toward = scale.humanness.toward_human();
            let all = scale
                .humanness
                .by_metric(&t.humanness.flat())
                .unwrap_or_default();
            let values: Vec<f64> = all.iter().map(|(_, v)| *v).collect();
            // いまの合算。 差を取る相手である。
            let now = scale.humanness.fuse(&values, None);
            all.iter()
                .enumerate()
                .filter_map(|(j, (n, v))| {
                    let (_, up) = toward.iter().find(|(m, _)| m == n)?;
                    // その指標だけを本人の代表値へ置いて、合算し直す。
                    //
                    // 直し方は 2 本以上出て、互いに正反対を指すことがある
                    // ——語彙を散らせと言う指標と、言い換えるなと言う指標が同時に
                    // 出る。どちらが勝つかを言わなければ、受け取った側は逆を選ぶ。
                    let target = scale
                        .humanness_target
                        .iter()
                        .find(|(m, _)| m == n)
                        .map_or(*v, |(_, x)| *x);
                    let effect = scale.humanness.fuse(&values, Some((j, target))) - now;
                    Some(HumannessByMetric {
                        name: (*n).to_owned(),
                        value: *v,
                        raise: *up,
                        // 減らす側では、この文章が繰り返しすぎているものを名指す。
                        // 増やす側で本人の言い回しを渡すのと表裏である。
                        overused: if *up { Vec::new() } else { t.overused.clone() },
                        // 散らすなと言う側でだけ渡す。 散らせと言う側に
                        // 「これが散らしている」を見せても使い道がない。
                        once_only: if *up { Vec::new() } else { t.once_only.clone() },
                        effect,
                        target,
                    })
                })
                .collect()
        },
        humanness_metrics: scale
            .humanness
            .by_metric(&t.humanness.flat())
            .unwrap_or_default()
            .into_iter()
            .map(|(n, v)| (n.to_owned(), v))
            .collect(),
        humanness_dims: t
            .humanness
            .flat()
            .into_iter()
            .map(|(n, m)| (n, m.value()))
            .collect(),
        missing_humanness: t.humanness.missing(),
        missing_systems: FOR_VERDICT
            .iter()
            .filter(|s| !t.parts.contains_key(s))
            .map(|s| s.name().to_owned())
            .collect(),
    }
}

/// 測り終えた 1 単位の照合値。相手集合との中央値で、系統が欠ければ出さない。
pub(crate) fn matching_of_unit(scale: &Scale, m: &Measurements) -> Option<f64> {
    let vector = |name: &str| -> Option<Vec<f64>> {
        let system = System::from_name(name)?;
        let set = &scale.frozen.iter().find(|(n, _)| n == name)?.1;
        set.project(m.parts.get(&system)?)
    };
    matching_median(scale, &vector).0
}

/// 欠けた系統に代わりの値を置いて出した照合値。
#[derive(Debug, Clone, PartialEq)]
pub struct Substituted {
    /// 欠けていた系統。
    pub system: String,
    /// 照合値。相手集合との中央値。
    pub value: f64,
}

/// 相手集合との照合値。
///
/// 検める文書の側で系統が 1 つだけ欠けていれば、その系統に
/// [代わりの値](crate::calibrate::SUBSTITUTE_SIGMA)を置いた値を別に返す。
/// 2 つ以上欠けていれば、どちらも出さない。
///
/// 欠けているかは相手ではなく検める文書で見る。 相手集合は目盛りを作った
/// 単位なので、系統がそろっている。
fn matching_median(
    scale: &Scale,
    vector: &dyn Fn(&str) -> Option<Vec<f64>>,
) -> (Option<f64>, Option<Substituted>) {
    let over_partners = |substitute: Option<&str>| -> Option<f64> {
        let values: Vec<f64> = scale
            .partner_vectors
            .iter()
            .filter_map(|(_, parts)| {
                let distance = |n: &str| -> Option<f64> {
                    let a = vector(n)?;
                    let b = &parts.iter().find(|(m, _)| m == n)?.1;
                    Some(cosine_delta(&a, b))
                };
                match substitute {
                    None => scale.calibration.matching_value(&distance),
                    Some(s) => scale.calibration.matching_value_substituting(&distance, s),
                }
                .ok()
            })
            .collect();
        median(&values)
    };
    let missing: Vec<&String> = scale
        .calibration
        .systems()
        .iter()
        .filter(|n| vector(n).is_none())
        .collect();
    match missing.as_slice() {
        [] => (over_partners(None), None),
        [one] => {
            let substituted = over_partners(Some(one)).map(|value| Substituted {
                system: (*one).clone(),
                value,
            });
            (None, substituted)
        }
        _ => (None, None),
    }
}

/// 指標 1 本ぶんの人らしさ値と、寄せる向き。
///
/// 並びの位置で意味を持たせない。 3 つ組で渡すと、受け取る側が位置の意味を
/// コメントで補うことになり、順番を入れ替えたときに型が何も言わない。
#[derive(Debug, Clone, PartialEq)]
pub struct HumannessByMetric {
    /// 指標の名前。
    pub name: String,
    /// その指標だけで見た人らしさ値。正が人の側、負が機械の側。
    pub value: f64,
    /// 人へ寄せる向き。`true` なら値を上げる。
    pub raise: bool,
    /// この文章が繰り返しすぎている言い回し。減らす側でだけ意味を持つ。
    pub overused: Vec<String>,
    /// この文章で一度しか出てこない語。語を散らしている当のものである。
    pub once_only: Vec<String>,
    /// 本人の代表値へ置いたときに、人らしさ値が動く量。
    ///
    /// 正反対を指す直し方が同時に出ることがある。 どちらが勝つかは、
    /// 動く量でしか言えない。
    pub effect: f64,
    /// 本人の代表値。
    ///
    /// 0 と比べてはいけない。 0 は人と機械の境目であって、その人の
    /// ところではない——4 指標とも境目より人の側にいるのに、合算では機械の側
    /// ということが実際に起きる。
    pub target: f64,
}

/// 系統の 1 次元ぶんの隔たり。
///
/// 系統そのものは指示にならない——「何番目かの次元を増やせ」は言葉にならない。
/// だが次元が語として読める系統なら、その 1 次元は指示になる——「あなたは
/// 『〜のだ』をよく使うが、この草稿には出てこない」は直せる。
#[derive(Debug, Clone, PartialEq)]
pub struct Divergence {
    /// どの系統か。
    pub system: String,
    /// どの次元か。語・記号・字種など、読める形である。
    pub dim: String,
    /// この文章の、標準化した値。
    pub mine: f64,
    /// 相手集合の、標準化した値の中央値。
    pub theirs: f64,
    /// 相手集合の、その次元での下端。
    pub low: f64,
    /// 相手集合の、その次元での上端。
    pub high: f64,
    /// その系統の、合算での重み。
    pub weight: f64,
    /// この次元を本人の値に置いたときに照合値が動く量。正なら近づく。
    pub effect: f64,
    /// この文章の、直す場所。
    ///
    /// 「減らせ」と言うなら、どれを減らすのかを言う。 実測で、間隔の次元は
    /// 場所を渡していなかったため、受け取った側が道具の外で数え直した
    /// ——しかも数え方を間違えた（道具は日本語の文字だけを数える）。
    pub spots: Vec<String>,
    /// 本人がその次元をどう書いているかの実例。
    ///
    /// 「増やせ」と言うだけでは、どこに置くのかが分からない。 実測では、
    /// 「空白を増やす」という指示を受けた側が本人の記事を自分で覗いて
    /// 打ち方を調べることになった。
    pub examples: Vec<String>,
}

impl Divergence {
    /// 本人の幅からのはみ出し。幅を 1 とした倍数。
    ///
    /// 畳めば消える信号がある。 系統の距離は数百次元のコサインなので、
    /// 1 次元が大きく動いても角度はわずかしか変わらない——実測で、敬体を常体に
    /// 変えた文章は `です` の次元が −12.2（本人 +2.2）まで動いたのに、
    /// 照合値は 0.23 しか動かなかった。
    ///
    /// だから次元の側でも見る。
    #[must_use]
    pub fn outside(&self) -> f64 {
        let spread = self.high - self.low;
        let over = if self.mine > self.high {
            self.mine - self.high
        } else if self.mine < self.low {
            self.low - self.mine
        } else {
            return 0.0;
        };
        if spread > 0.0 {
            over / spread
        } else {
            over
        }
    }

    /// 隔たりの大きさ。
    #[must_use]
    pub fn size(&self) -> f64 {
        (self.mine - self.theirs).abs()
    }

    /// この 1 次元を本人の値に置いたとき、照合値が実際に動く量。
    ///
    /// 当て推量で並べてはいけない。 隔たりの大きさでも、隔たり × 重みでも、
    /// 実際に動く量とは一致しない——次元の数が系統ごとに違うので、10 次元しか
    /// ない系統の 1 本は 500 次元の系統の 1 本よりずっと大きく効く。実測では、
    /// いちばん効く直し（文字種）が一度も上位に出てこなかった。
    ///
    /// だから数える。 その次元だけを本人の代表値に置き換えて照合値を出し直し、
    /// 差を取る。これは見込みではなく、そのまま効く量である。
    #[must_use]
    pub fn effect(&self) -> f64 {
        self.effect
    }

    /// 増やす側か。相手のほうが大きいなら増やす。
    #[must_use]
    pub fn raise(&self) -> bool {
        self.theirs > self.mine
    }
}

/// 照合値を出し直す。1 次元だけ置き換えられる。
///
/// `swap` に `(系統, 次元, 値)` を渡すと、その 1 次元だけを差し替えて測る。
/// 効く量を数えるための道具である——見込みではなく、そのまま動く量が出る。
fn matching_of(
    scale: &Scale,
    t: &Measurements,
    swap: Option<(System, usize, f64)>,
) -> Option<f64> {
    let vector = |m: &Measurements, name: &str| -> Option<Vec<f64>> {
        let system = System::from_name(name)?;
        let set = &scale.frozen.iter().find(|(n, _)| n == name)?.1;
        let parts = m.parts.get(&system)?;
        let Some((s, j, x)) = swap else {
            return set.project(parts);
        };
        if s != system {
            return set.project(parts);
        }
        // 投影した値を書き換えない。数え上げの側で動かす。
        //
        // 系統の次元はどれも相対頻度なので、1 つを減らせば残りの割合が上がる
        // ——読点を 1 つ外せば、外さなかった読点の取り分が増える。投影した値を
        // 直接書き換えると、実際には書けない直しを見積もることになる。
        let mut shifted: Vec<Counts> = parts.clone();
        let mut at = j;
        for (f, c) in set.parts().iter().zip(shifted.iter_mut()) {
            if at < f.len() {
                *c = f.shifted(c, at, f.unstandardize(at, x));
                break;
            }
            at -= f.len();
        }
        set.project(&shifted)
    };
    // 代わりの値で出した照合値でも動く量は測る。 検めがその値で 2 段目に
    // 止めたなら、直し方もその値から数えなければ食い違う。
    let (matching, substituted) = matching_median(scale, &|n| vector(t, n));
    matching.or(substituted.map(|s| s.value))
}

/// 相手集合との、系統ごとの距離。相手ごとの中央値である。
///
/// 照合値は 5 つの距離を重みで畳んだものなので、畳む前を見なければどこが動いたか
/// 分からない。 判定には使わない——出すだけである。
#[must_use]
pub fn distances_against(
    scale: &Scale,
    target: Sample<'_>,
    analyzer: Option<&dyn Analyzer>,
) -> Vec<(String, f64)> {
    let t = Measurements::of(target, analyzer, &scale.lexicon);
    let vector = |m: &Measurements, name: &str| -> Option<Vec<f64>> {
        let system = System::from_name(name)?;
        let set = &scale.frozen.iter().find(|(n, _)| n == name)?.1;
        set.project(m.parts.get(&system)?)
    };
    scale
        .frozen
        .iter()
        .filter_map(|(name, _)| {
            let ds: Vec<f64> = scale
                .partner_vectors
                .iter()
                .filter_map(|(_, parts)| {
                    let a = vector(&t, name)?;
                    let b = &parts.iter().find(|(m, _)| m == name)?.1;
                    Some(cosine_delta(&a, b))
                })
                .collect();
            Some((name.clone(), median(&ds)?))
        })
        .collect()
}

/// 相手集合からいちばん離れている次元を挙げる。
///
/// 照合値は 1 つの数なので、どこが違うのかを言えない。 帯の中で止まったときに
/// 何も出さなければ、受け取った側は動きようがない。
///
/// 読める系統だけを見る。 `systems` に渡すのは、次元が語や記号として読める
/// ものだけである——品詞 bigram の「名詞-助詞」を増やせとは言えない。
///
/// 相手は中央値で代表する。 平均だと 1 本の外れ値が代表を引っ張る。
#[must_use]
pub fn diverging(
    scale: &Scale,
    target: Sample<'_>,
    analyzer: Option<&dyn Analyzer>,
    systems: &[System],
    top: usize,
) -> Vec<Divergence> {
    let t = Measurements::of(target, analyzer, &scale.lexicon);

    let mut out: Vec<Divergence> = Vec::new();
    for system in systems {
        let name = system.name();
        let Some((_, set)) = scale.frozen.iter().find(|(n, _)| n == name) else {
            continue;
        };
        // 重みを引く。 重みの小さい系統をいくら直しても照合値は動かない。
        let weight = scale
            .calibration
            .systems()
            .iter()
            .position(|n| n == name)
            .and_then(|j| scale.calibration.fusion().slopes().get(j).copied())
            .unwrap_or(0.0);
        // 重みが 0 の系統は挙げない。 直しても動かないものを指示にしない。
        if weight <= 0.0 {
            continue;
        }
        let Some(mine) = t.parts.get(system).and_then(|c| set.project(c)) else {
            continue;
        };
        let theirs: Vec<Vec<f64>> = scale
            .partner_vectors
            .iter()
            .filter_map(|(_, parts)| parts.iter().find(|(m, _)| m == name).map(|(_, v)| v.clone()))
            .collect();
        if theirs.is_empty() {
            continue;
        }
        let dims: Vec<&str> = set
            .parts()
            .iter()
            .flat_map(|f| f.dims())
            .map(String::as_str)
            .collect();
        // いまの照合値を控えておく。 差を取る相手である。
        let now = matching_of(scale, &t, None);
        for (j, dim) in dims.iter().enumerate() {
            let Some(&m) = mine.get(j) else { continue };
            let mut col: Vec<f64> = theirs.iter().filter_map(|v| v.get(j).copied()).collect();
            if col.is_empty() {
                continue;
            }
            col.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let theirs_med = col[col.len() / 2];
            // その次元だけを本人の代表値に置いて、照合値を出し直す。
            let fixed = matching_of(scale, &t, Some((*system, j, theirs_med)));
            let (Some(a), Some(b)) = (now, fixed) else {
                continue;
            };
            out.push(Divergence {
                system: name.to_owned(),
                dim: (*dim).to_owned(),
                mine: m,
                theirs: theirs_med,
                low: col[0],
                high: col[col.len() - 1],
                weight,
                effect: b - a,
                spots: Vec::new(),
                examples: Vec::new(),
            });
        }
    }
    // 動く見込みの大きい順。同じなら系統・次元の名前順。 決めておかないと、
    // 上位が実装ごとに変わる。
    // 直しても近づかない次元は挙げない。 効かない指示を出さない。
    out.retain(|d| d.effect > 0.0);
    out.sort_by(|a, b| {
        b.effect()
            .partial_cmp(&a.effect())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.system.cmp(&b.system))
            .then_with(|| a.dim.cmp(&b.dim))
    });
    out.truncate(top);
    // 渡すぶんだけ実例を探す。 全次元で探すと、使われない実例のために
    // 相手集合を何度も読み直すことになる。
    for d in &mut out {
        // 目盛りが取り置いた実例を引く。 本文はもう手元に無い。
        d.examples = scale
            .examples
            .iter()
            .find(|(s, dim, _)| *s == d.system && *dim == d.dim)
            .map(|(_, _, v)| v.clone())
            .unwrap_or_default();
        // この文章のどこが、その次元を作っているか。
        d.spots = spots_of(&d.system, &d.dim, target);
    }
    out
}

/// この文章の、その次元を作っている場所。
///
/// 「減らせ」と言うなら、どれを減らすのかを言う。 本人の実例だけを渡しても、
/// 自分の文章のどこを直すのかは分からない。
fn spots_of(system: &str, dim: &str, target: Sample<'_>) -> Vec<String> {
    const WANT: usize = 3;
    if system != "読点の打ち方" || !(dim.ends_with('字') || dim.ends_with("字以上")) {
        return Vec::new();
    }
    let prose = kuchiyose_doc::prose::mask_identifiers(&target.document.prose());
    let mut out: Vec<String> = matching::comma_gaps(&prose)
        .into_iter()
        .filter(|(name, _)| name == dim)
        .map(|(_, ctx)| ctx)
        .collect();
    out.sort();
    out.dedup();
    out.truncate(WANT);
    out
}

/// 検める 1 本を測った結果。出なかったものは `None` である。
#[derive(Debug, Clone, PartialEq)]
pub struct Measured {
    /// 照合値。相手集合との中央値。
    pub matching: Option<f64>,
    /// 系統が 1 つだけ欠けたときの照合値。欠けた系統に機械の側の代わりの値を置いて出す。
    ///
    /// [照合値](Self::matching)と同じ欄に入れない。 同じ目盛りに載っていない
    /// ——機械の側へ寄せて置いたので、人の側に出たときにしか使えない。
    /// 欄を分けなければ、受け取った側はその区別を失う。
    pub matching_substituted: Option<Substituted>,
    /// 人らしさ値。
    pub humanness: Option<f64>,
    /// 指標ごとの人らしさ値と、人へ寄せる向き。
    ///
    /// 測れなければ空である。 直し方を渡す側が、測れていないことと機械の
    /// 側にあることを取り違えないようにする。
    pub humanness_by_metric: Vec<HumannessByMetric>,
    /// 指標ごとの人らしさ値。向きの割れた指標も含めて全部。1 次元でも欠ければ空である。
    ///
    /// [直し方の材料](Self::humanness_by_metric)は、向きを言えない指標を外す。
    /// 測った値を並べるときに外すと、合算に効いているのに見えない指標が残る。
    pub humanness_metrics: Vec<(String, f64)>,
    /// 人らしさの次元ごとの、測った量。測れなかった次元は `None`。
    pub humanness_dims: Vec<(String, Option<f64>)>,
    /// 測れなかった人らしさの次元。
    pub missing_humanness: Vec<String>,
    /// 測れなかった系統。
    pub missing_systems: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuchiyose_doc::node::{Kind, Node};

    use crate::testing::{document_with, Chars, Fixture};

    #[test]
    fn 広く使う型を狭い穴あきに食わせない() {
        // 実測で、出現割合 0.706 の `僕は` が 0.176 の `思います。`〜`僕は` に
        // 食われ、いちばん広く使われていた一人称が表から消えた。
        assert!(!frame_keeps(0.176, 0.551, 0.706));
        // 挨拶の穴あきは残る。 部品が単独で出るぶんは半分ほどである。
        assert!(frame_keeps(0.163, 0.306, 0.306));
        // 部品が穴あきの外に出ないなら、繋いで何も捨てていない。
        assert!(frame_keeps(0.3, 0.3, 0.3));
    }

    #[test]
    fn 素材が足りなければ目盛りを作らない() {
        // 作れないことは失敗ではない。判定できないが返る。
        let m = Fixture::new(4);
        let e = m.built(Some(&Chars)).unwrap_err();
        assert!(matches!(e, ScaleError::Split(_)), "{e}");
    }

    #[test]
    fn 解析器が無ければ系統が揃わない() {
        // 一部の系統が形態素を要る。抜いて合算しない。
        let m = Fixture::new(10);
        let e = m.built(None).unwrap_err();
        assert!(matches!(e, ScaleError::Split(_)), "{e}");
    }

    #[test]
    fn 端まで通ると目盛りができる() {
        let m = Fixture::new(10);
        let scale = m.built(Some(&Chars)).expect("目盛りができる");
        assert_eq!(scale.frozen.len(), 5, "判定に使う系統が揃う");
        assert_eq!(scale.partners().len(), 5);
        assert_eq!(scale.calibration.systems().len(), 5);
    }

    #[test]
    fn 語彙は割る前に全体から固定する() {
        // 側ごとに違う語彙を使えば、側ごとに次元の意味が変わる。
        let m = Fixture::new(10);
        let scale = m.built(Some(&Chars)).unwrap();
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
        let m = Fixture::new(10);
        let person = Fixture::samples(&m.person);
        let scale = m.built(Some(&Chars)).unwrap();
        // 相手集合は目盛りが持っている。 本文はもう要らない。
        assert_eq!(scale.partner_vectors.len(), 5);
        let got = measure_against(&scale, person[9], Some(&Chars));
        assert!(got.matching.is_some(), "照合値が出る");
        assert!(got.humanness.is_some(), "人らしさ値が出る");
        assert!(got.missing_systems.is_empty());
    }

    #[test]
    fn 測った値を全部出すために指標ごとと次元ごとの値を持つ() {
        // 向きの割れた指標は直し方から外れるが、値は出す。 出さなければ、
        // 合算に効いているのに見えない指標が残る。
        let m = Fixture::new(10);
        let person = Fixture::samples(&m.person);
        let scale = m.built(Some(&Chars)).unwrap();
        let got = measure_against(&scale, person[9], Some(&Chars));
        assert_eq!(
            got.humanness_metrics.len(),
            kuchiyose_metrics::humanness::Metric::ALL.len()
        );
        assert!(got.humanness_metrics.len() >= got.humanness_by_metric.len());
        assert_eq!(got.humanness_dims.len(), crate::humanness::dims().len());
        assert!(got.humanness_dims.iter().all(|(_, v)| v.is_some()));
    }

    #[test]
    fn 系統が欠ければ照合値を出さない() {
        let m = Fixture::new(10);
        let scale = m.built(Some(&Chars)).unwrap();
        // 短い文書は除外に掛かる。0 ではなく、出ないである。
        let short = Document::new(vec![Node::leaf(Kind::Paragraph, "短い。")]);
        let got = measure_against(
            &scale,
            Sample {
                name: "短い",
                document: &short,
            },
            Some(&Chars),
        );
        assert_eq!(got.matching, None);
        assert_eq!(got.matching_substituted, None, "2 つ以上欠ければ置かない");
        assert_eq!(got.missing_systems.len(), 5);
        assert_eq!(got.missing_humanness.len(), 14);
    }

    #[test]
    fn 系統が_1_つだけ欠ければ代わりの値で照合値を出す() {
        let m = Fixture::new(10);
        let scale = m.built(Some(&Chars)).unwrap();
        let no_comma = document_with(9, false, "");
        let got = measure_against(
            &scale,
            Sample {
                name: "読点なし",
                document: &no_comma,
            },
            Some(&Chars),
        );
        assert_eq!(got.missing_systems, vec![System::Comma.name().to_owned()]);
        assert_eq!(got.matching, None, "そろった照合値とは別に持つ");
        let s = got.matching_substituted.expect("代わりの値で出る");
        assert_eq!(s.system, System::Comma.name());
        assert!(s.value.is_finite());
    }

    #[test]
    fn そろっていれば代わりの値を置かない() {
        let m = Fixture::new(10);
        let person = Fixture::samples(&m.person);
        let scale = m.built(Some(&Chars)).unwrap();
        let got = measure_against(&scale, person[9], Some(&Chars));
        assert!(got.matching.is_some());
        assert_eq!(got.matching_substituted, None);
    }
}
