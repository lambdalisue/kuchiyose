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

use std::collections::{BTreeMap, BTreeSet};

use kakiburi_doc::Document;
use kakiburi_metrics::lexicon::Lexicon;
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
    /// 単位の名前。ファイル名ではない。
    pub name: &'a str,
    /// 正規形。
    pub document: &'a Document,
}

/// 1 単位を測り終えた形。
struct Measurements {
    /// 系統ごとの部分ベクトルの数え上げ。測れなかった系統は入っていない。
    parts: BTreeMap<System, Vec<Counts>>,
    /// その単位の中で再来した言い回し。
    recurring: Vec<String>,
    /// その単位が繰り返しすぎている短い言い回し。
    overused: Vec<String>,
    /// その単位で一度しか出てこない語。語を散らしている当のものである。
    once_only: Vec<String>,
    /// その単位に現れる語の並びと、位置と node の番号。型を取り出す材料。
    grams: Vec<(String, f64, usize)>,
    /// その単位に現れる[語](kakiburi_metrics::word::goi)の語彙素と、品詞と回数。
    /// 語形が散っても 1 つに合流する。
    goi: BTreeMap<String, (String, usize)>,
    /// 人らしさの次元。
    humanness: Humanness,
    /// 地の文の日本語の文字数。長さの範囲に使う。
    chars: usize,
    /// 地の文をつないだもの。言い回しの上限を文字列として数えるために持つ。
    ///
    /// [文節の並び](Self::grams)では数えられない。 渡す言い回しは
    /// `ています。` のように文節にならないものを含むので、文節の並びと照らすと
    /// ほとんどが 0 になる——実測で、20 本のうち 15 本の上限が 0 だった。
    text: String,
}

/// 本人の素材からコーパスの語を見つける。
///
/// 解析するだけで、測らない。 ここで測ってしまうと、割れたままの値が
/// 派生物に残る。
fn lexicon_of(person: &[Sample<'_>], analyzer: Option<&dyn Analyzer>) -> Lexicon {
    let Some(a) = analyzer else {
        return Lexicon::default();
    };
    let units: Vec<Vec<Vec<kakiburi_metrics::morph::Morpheme>>> = person
        .iter()
        .filter_map(|s| {
            let prose = kakiburi_doc::prose::mask_identifiers(&s.document.prose());
            Analyzed::of(&prose, a).ok().map(|x| x.segments().to_vec())
        })
        .collect();
    Lexicon::find(&units)
}

impl Measurements {
    fn of(sample: Sample<'_>, analyzer: Option<&dyn Analyzer>, lexicon: &Lexicon) -> Self {
        // 識別子を伏せてから測る。 `denops.vim` のような半角英字の連なりは
        // 書き手が選んだ書きぶりではなく題材が決めるもので、そのまま入れると
        // 同じ人が別の題材で書いた文章を「その人らしくない」と言う。
        //
        // 掛けるのはここだけである。 指示できる指標は和欧間スペースや半角英字
        // そのものを測るので、生の文から測り続ける。
        let prose = kakiburi_doc::prose::mask_identifiers(&sample.document.prose());
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
            // 長いほうだけを取る。 短い言い回しは誰でも繰り返すので、
            // 渡しても癖にならない。
            recurring: kakiburi_metrics::humanness::recurring(
                analyzed.as_ref(),
                &kakiburi_metrics::humanness::LONG_N,
            ),
            // 「減らせ」と言うなら、どれを減らすのかを言う。
            overused: kakiburi_metrics::humanness::overused(
                analyzed.as_ref(),
                &kakiburi_metrics::humanness::SHORT_N,
                OVERUSED,
            ),
            // 「散らすな」と言うなら、どれが散らしているのかを言う。
            once_only: kakiburi_metrics::humanness::once_only(analyzed.as_ref(), ONCE_ONLY),
            grams: kakiburi_metrics::word::grams_with_position(analyzed.as_ref(), &KATA_N),
            goi: kakiburi_metrics::word::goi(analyzed.as_ref()),
            humanness: Humanness::measure(&prose, analyzed.as_ref()),
            chars: sample.document.japanese_chars(),
            text: kakiburi_metrics::humanness::joined(&prose),
        }
    }

    /// 判定に使う系統が全部測れたか。
    fn systems_measured(&self) -> bool {
        FOR_VERDICT.iter().all(|s| self.parts.contains_key(s))
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

/// 素材を測って内訳を返す。目盛りは作らない。
#[must_use]
pub fn inspect(samples: &[Sample<'_>], analyzer: Option<&dyn Analyzer>) -> Vec<Report> {
    samples
        .iter()
        .map(|s| {
            let m = Measurements::of(*s, analyzer, &Lexicon::default());
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

/// 目盛りを作る材料。
///
/// 場面を跨げる素材を、跨げない素材と同じ配列に入れない。 入れれば、
/// 相手集合にも天井にも他人が混ざる道が開く——[場面ごとに閉じる](../../../docs/spec/010-strategy.md#場面ごとに閉じる)
/// が、呼ぶ側の注意だけで守られることになる。
///
/// 欄で分けたうえで、[`assemble`]は [`others`](Self::others) を人らしさの較正にしか
/// 渡さない。 型が全部を守るわけではないが、跨ぐ道が 1 本に絞られる。
#[derive(Debug, Clone, Copy)]
pub struct Material<'a> {
    /// 1 つの場面の本人の単位。
    pub person: &'a [Sample<'a>],
    /// 同じ場面の基準の単位。
    pub baseline: &'a [Sample<'a>],
    /// 他人の文書。場面を跨いでよい唯一の素材である。
    ///
    /// [人らしさの較正の人の側](../../../docs/spec/200-extract.md#人らしさの境目は同じ材料から出る)
    /// にだけ足す。帯の側には足さない——足せば、帯の点の数が本人と基準で
    /// 釣り合わなくなる。
    pub others: &'a [Sample<'a>],
}

/// 組み立てる。
///
/// 作れないことは失敗ではない。 素材が足りなければ目盛りを作らず、検めが
/// 判定できないを返す——それが正しい振る舞いである。
pub fn assemble(m: Material<'_>, analyzer: Option<&dyn Analyzer>) -> Result<Scale, ScaleError> {
    let (person, baseline) = (m.person, m.baseline);
    // 2 度測る。 1 度目でコーパスから語を見つけ、2 度目でその語を畳んで測る。
    //
    // 辞書に無い語は割れる。 書き手の名前も、その分野の言い回しも、解析器の
    // 辞書は知らない——割れたままだと、その語のところで機能語も品詞 bigram も型も
    // 狂う（[語](kakiburi_metrics::lexicon)）。
    //
    // 見つけるのは本人の素材からである。 基準から見つければ、基準の書きぶりが
    // 本人の測り方を決めることになる。
    let lexicon = lexicon_of(person, analyzer);
    let measured: BTreeMap<String, Measurements> = person
        .iter()
        .chain(baseline.iter())
        .chain(m.others.iter())
        .map(|s| {
            (
                (*s.name).to_owned(),
                Measurements::of(*s, analyzer, &lexicon),
            )
        })
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

    // 他人は系統が測れたものだけを使う。 投影できない単位を対にしても捨てられる。
    //
    // 語彙には入れない。 固定するのは本人と基準からで、他人はその語彙へ投影する
    // ——検めるときの草稿と同じ扱いである。他人の語で次元を決めれば、
    // 他人が何人来たかで本人の測り方が変わる。
    let other_units: Vec<Unit> = m
        .others
        .iter()
        .map(unit)
        .filter(|u| u.systems_measured)
        .collect();

    // 系統ごとの、単位 → z 得点のベクトル。
    let mut projected: BTreeMap<(String, String), Vec<f64>> = BTreeMap::new();
    for u in used.iter().copied().chain(other_units.iter()) {
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
    let pairing = pair(&person_split, &baseline_split, &other_units);
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
    // 他人の文書は較正の側にだけ足す。 人らしさの人の側は「誰の文章でも人が
    // 書いたものは人の側に落ちる」ので、素材が足りなければ混ぜてよい——
    // 場面は人と機械の別を跨がない。
    //
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
    let mut human_rows = rows_of(&human_units);
    for s in m.others {
        let h = &measured[s.name].humanness;
        if !h.all_measured() {
            continue;
        }
        human_rows.push(
            h.flat()
                .into_iter()
                .filter_map(|(_, v)| v.value())
                .collect(),
        );
    }
    let humanness = HumannessScale::fit(&human_rows, &rows_of(&machine_units));
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
            which: "人らしさ値",
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
        kakiburi_metrics::humanness::Metric::ALL
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
    // 出せるようにするためである——ここで作らなければ、素材を持っている瞬間が
    // 二度と来ない。
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
    let partner_samples: Vec<Sample<'_>> = m
        .person
        .iter()
        .filter(|s| person_split.partners.iter().any(|u| u.name == s.name))
        .copied()
        .collect();
    let examples = examples_for(&frozen, &partner_samples);

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
        phrases: phrases_of(&person_units, &measured),
        // 渡した言い回しには上限も渡す。 繰り返せとだけ言えば、行きすぎる。
        phrase_ceilings: ceilings_of(
            &phrases_of(&person_units, &measured),
            &person_units,
            &measured,
        ),
        lexicon,
        katas: katas_of(&person_units, &machine_units, &measured, KATAS),
        // 役を入れ替えて、もう 1 度回す。 同じ仕組みで機械の型が出る。
        machine_katas: katas_of(&machine_units, &person_units, &measured, MACHINE_KATAS),
        // 並びで割れた癖を、語彙素で拾い直す。
        machine_gois: gois_of(&machine_units, &person_units, &measured, MACHINE_GOIS),
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
            let n = m.text.matches(p.as_str()).count();
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
fn phrases_of(units: &[Unit], measured: &BTreeMap<String, Measurements>) -> Vec<String> {
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
/// 数えるのは[文節](kakiburi_metrics::word)の数である——形態素の窓ではない。
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

/// 機械の語。基準がよく使い、本人が使わない[語](kakiburi_metrics::word::goi)。
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
                for (g, _, node) in &m.grams {
                    if let Some(i) = katas.iter().position(|k| k.text == *g) {
                        at.entry(*node).or_default().insert(i);
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
            let mut here: BTreeMap<&str, f64> = BTreeMap::new();
            for (g, at, _) in &m.grams {
                here.entry(g.as_str()).or_insert(*at);
            }
            for (g, at) in here {
                let e = out.entry(g.to_owned()).or_insert((0, Vec::new()));
                e.0 += 1;
                e.1.push(at);
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
            let mut here: BTreeMap<&str, usize> = BTreeMap::new();
            for (g, _, _) in &m.grams {
                *here.entry(g.as_str()).or_default() += 1;
            }
            for (g, n) in here {
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
            // [識別子を伏せる](kakiburi_doc::prose::mask_identifiers)のと同じ理由で、
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
    // カセットが持つ辞書で測る。 作ったときと違う割り方をすれば、
    // 比べたものに意味が無い。
    let t = Measurements::of(target, analyzer, &scale.lexicon);

    let vector = |m: &Measurements, name: &str| -> Option<Vec<f64>> {
        let system = System::from_name(name)?;
        let set = &scale.frozen.iter().find(|(n, _)| n == name)?.1;
        set.project(m.parts.get(&system)?)
    };
    let values: Vec<f64> = scale
        .partner_vectors
        .iter()
        .filter_map(|(_, parts)| {
            scale
                .calibration
                .matching_value(&|n| {
                    let a = vector(&t, n)?;
                    let b = parts.iter().find(|(m, _)| m == n)?.1.clone();
                    Some(cosine_delta(&a, &b))
                })
                .ok()
        })
        .collect();
    Measured {
        matching: median(&values),
        humanness: scale.humanness.value(&t.humanness.flat()).ok(),
        // 合算した 1 つの値では直し方を渡せない。「機械の側にある」としか
        // 言えず、どこをどうすればよいかが出てこない。
        //
        // 寄せる向きは較正から読む。 定義に固定すると、素材がその向きを
        // 支えていないカセットで直し方に従うほど人らしさが下がる。
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
        missing_humanness: t.humanness.missing(),
        missing_systems: FOR_VERDICT
            .iter()
            .filter(|s| !t.parts.contains_key(s))
            .map(|s| s.name().to_owned())
            .collect(),
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
    let values: Vec<f64> = scale
        .partner_vectors
        .iter()
        .filter_map(|(_, parts)| {
            scale
                .calibration
                .matching_value(&|n| {
                    let a = vector(t, n)?;
                    let b = parts.iter().find(|(m, _)| m == n)?.1.clone();
                    Some(cosine_delta(&a, &b))
                })
                .ok()
        })
        .collect();
    median(&values)
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

/// 実例を拾う系統。次元が語や記号として読めるものだけである。
///
/// 品詞 bigram の「名詞-助詞」を増やせとは言えない。
pub const EXAMPLE_SYSTEMS: [System; 3] = [System::FunctionWord, System::Comma, System::CharType];

/// 読める系統の次元ごとに、本人の実例を拾っておく。
///
/// 作るのはここだけである。 検めるときに本文へ読みに行く道が無い——
/// カセットは[本文を持たない](../../../docs/spec/200-extract.md#素材を正本にする)。
fn examples_for(
    scale_frozen: &[(String, crate::vocabulary::FrozenSet)],
    partners: &[Sample<'_>],
) -> Vec<(String, String, Vec<String>)> {
    let mut out = Vec::new();
    for system in EXAMPLE_SYSTEMS {
        let name = system.name();
        let Some((_, set)) = scale_frozen.iter().find(|(n, _)| n == name) else {
            continue;
        };
        for dim in set.parts().iter().flat_map(crate::vocabulary::Frozen::dims) {
            let found = examples_of(name, dim, partners);
            if found.is_empty() {
                continue;
            }
            out.push((name.to_owned(), dim.clone(), found));
        }
    }
    out
}

/// 本人がその次元をどう書いているかを、実際の文から拾う。
///
/// 「増やせ」と言うだけでは、どこに置くのかが分からない。 数値と向きだけを
/// 渡された側は、結局その人の文章を自分で読みに行くことになる。
///
/// 拾えないものは無理に作らない。 間隔のような、字面に現れない次元がある。
/// この文章の、その次元を作っている場所。
///
/// 「減らせ」と言うなら、どれを減らすのかを言う。 本人の実例だけを渡しても、
/// 自分の文章のどこを直すのかは分からない。
fn spots_of(system: &str, dim: &str, target: Sample<'_>) -> Vec<String> {
    const WANT: usize = 3;
    if system != "読点の打ち方" || !(dim.ends_with('字') || dim.ends_with("字以上")) {
        return Vec::new();
    }
    let prose = kakiburi_doc::prose::mask_identifiers(&target.document.prose());
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

fn examples_of(system: &str, dim: &str, partners: &[Sample<'_>]) -> Vec<String> {
    const WANT: usize = 3;
    const AROUND: usize = 6;

    // どこにでもある字は実例にならない。 ひらがなや漢字を 1 つ抜き出して
    // 見せても、どこをどう直せばよいかは何も伝わらない。
    if matches!((system, dim), ("文字種", "ひらがな" | "漢字" | "その他")) {
        return Vec::new();
    }

    // 次元が字面に現れるものと、字の種類を指すものを分ける。
    let hit: Box<dyn Fn(char) -> bool> = match (system, dim) {
        ("文字種", "空白") => Box::new(|c: char| c == ' ' || c == '\u{3000}'),
        ("文字種", "約物") => Box::new(|c: char| {
            matches!(
                c,
                '、' | '。' | '「' | '」' | '（' | '）' | '・' | '！' | '？'
            )
        }),
        ("文字種", "カタカナ") => {
            Box::new(|c: char| ('ァ'..='ヶ').contains(&c) || c == 'ー')
        }
        ("文字種", "半角数字") => Box::new(|c: char| c.is_ascii_digit()),
        ("文字種", "半角英字") => Box::new(|c: char| c.is_ascii_alphabetic()),
        _ => {
            // 字面に現れる次元。読点の直前・直後は 1 文字、機能語は語である。
            let needle: String = if system == "読点の打ち方" {
                if dim.ends_with('字') || dim.ends_with("字以上") {
                    // 間隔の次元は、字面に現れない。 その間隔を作っている読点を
                    // まわりごと見せる——「増やせ」だけでは直せない。
                    let mut out: Vec<String> = partners
                        .iter()
                        .flat_map(|p| {
                            let prose = kakiburi_doc::prose::mask_identifiers(&p.document.prose());
                            matching::comma_gaps(&prose)
                        })
                        .filter(|(name, _)| name == dim)
                        .map(|(_, ctx)| ctx)
                        .collect();
                    out.sort();
                    out.dedup();
                    out.truncate(WANT);
                    return out;
                }
                format!("{dim}、")
            } else {
                dim.to_owned()
            };
            return snippets(
                partners,
                &|t: &str| t.match_indices(&needle).map(|(i, _)| i).collect(),
                needle.chars().count(),
                WANT,
                AROUND,
            );
        }
    };
    snippets(
        partners,
        &|t: &str| {
            t.char_indices()
                .filter(|(_, c)| hit(*c))
                .map(|(i, _)| i)
                .collect()
        },
        1,
        WANT,
        AROUND,
    )
}

#[cfg(test)]
mod example_tests {
    use super::*;
    use kakiburi_doc::node::{Kind, Node};

    fn doc(t: &str) -> Document {
        Document::new(vec![Node {
            kind: Kind::Paragraph,
            text: t.to_owned(),
            children: Vec::new(),
            raw_depth: None,
        }])
    }

    #[test]
    fn 空白の実例は打ち方が分かる形で出る() {
        // 「増やせ」と言うだけでは、どこに置くのかが分からない。
        let d = doc("今回は 2024/1/23 に行われた VimConf の話です。");
        let got = examples_of(
            "文字種",
            "空白",
            &[Sample {
                name: "u",
                document: &d,
            }],
        );
        assert!(!got.is_empty(), "{got:?}");
        assert!(got.iter().any(|x| x.contains(' ')), "{got:?}");
    }

    #[test]
    fn どこにでもある字は実例にしない() {
        // ひらがなを 1 つ抜き出して見せても、何も伝わらない。
        let d = doc("これはひらがなばかりの文である。");
        assert!(examples_of(
            "文字種",
            "ひらがな",
            &[Sample {
                name: "u",
                document: &d
            }]
        )
        .is_empty());
    }

    #[test]
    fn 読点の直前の字は読点ごと見せる() {
        let d = doc("そうなので、こうした。ならば、こうする。");
        let got = examples_of(
            "読点の打ち方",
            "で",
            &[Sample {
                name: "u",
                document: &d,
            }],
        );
        assert!(got.iter().any(|x| x.contains("で、")), "{got:?}");
    }

    #[test]
    fn 字面に現れない次元は無理に作らない() {
        // 間隔は字として現れない。
        let d = doc("そうなので、こうした。");
        assert!(examples_of(
            "読点の打ち方",
            "17字",
            &[Sample {
                name: "u",
                document: &d
            }]
        )
        .is_empty());
    }
}

/// 見つけた位置のまわりを切り出す。文字の境で切る。
fn snippets(
    partners: &[Sample<'_>],
    find: &dyn Fn(&str) -> Vec<usize>,
    len: usize,
    want: usize,
    around: usize,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in partners {
        for seg in &p.document.prose() {
            let cs: Vec<char> = seg.text.chars().collect();
            let byte_to_char: BTreeMap<usize, usize> = seg
                .text
                .char_indices()
                .enumerate()
                .map(|(k, (b, _))| (b, k))
                .collect();
            for b in find(&seg.text) {
                let Some(&k) = byte_to_char.get(&b) else {
                    continue;
                };
                let lo = k.saturating_sub(around);
                let hi = (k + len + around).min(cs.len());
                let snip: String = cs[lo..hi].iter().collect();
                let snip = snip.trim().to_owned();
                if snip.chars().count() < 4 || out.contains(&snip) {
                    continue;
                }
                out.push(snip);
                if out.len() >= want {
                    return out;
                }
            }
        }
    }
    out
}

/// 検める 1 本を測った結果。出なかったものは `None` である。
#[derive(Debug, Clone, PartialEq)]
pub struct Measured {
    /// 照合値。相手集合との中央値。
    pub matching: Option<f64>,
    /// 人らしさ値。
    pub humanness: Option<f64>,
    /// 指標ごとの人らしさ値と、人へ寄せる向き。
    ///
    /// 測れなければ空である。 直し方を渡す側が、測れていないことと機械の
    /// 側にあることを取り違えないようにする。
    pub humanness_by_metric: Vec<HumannessByMetric>,
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

    /// 試験用の解析器。UniDic を名乗り、字で切る。
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

    /// 1 単位ぶんの文書。すべての除外を越える長さにする。
    ///
    /// 長さは単位ごとに散らす。 揃えると広がりが 0 になり、長さの範囲の検査が
    /// 「重なり 0」で止まる——両側が同じ範囲に散っている素材でなければ先へ進めない。
    fn document(index: usize, machine: bool) -> Document {
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
                Node::leaf(Kind::Paragraph, format!("{a}は、{b}の{c}を{d}に{a}が。"))
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

    struct Fixture {
        person: Vec<(String, Document)>,
        baseline: Vec<(String, Document)>,
    }

    impl Fixture {
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
        let e = assemble(
            Material {
                person: &Fixture::samples(&m.person),
                baseline: &Fixture::samples(&m.baseline),
                others: &[],
            },
            Some(&Chars),
        )
        .unwrap_err();
        assert!(matches!(e, ScaleError::Split(_)), "{e}");
    }

    #[test]
    fn 解析器が無ければ系統が揃わない() {
        // 一部の系統が形態素を要る。抜いて合算しない。
        let m = Fixture::new(10);
        let e = assemble(
            Material {
                person: &Fixture::samples(&m.person),
                baseline: &Fixture::samples(&m.baseline),
                others: &[],
            },
            None,
        )
        .unwrap_err();
        assert!(matches!(e, ScaleError::Split(_)), "{e}");
    }

    #[test]
    fn 端まで通ると目盛りができる() {
        let m = Fixture::new(10);
        let scale = assemble(
            Material {
                person: &Fixture::samples(&m.person),
                baseline: &Fixture::samples(&m.baseline),
                others: &[],
            },
            Some(&Chars),
        )
        .expect("目盛りができる");
        assert_eq!(scale.frozen.len(), 5, "判定に使う系統が揃う");
        assert_eq!(scale.partners().len(), 5);
        assert_eq!(scale.calibration.systems().len(), 5);
    }

    #[test]
    fn 他人の文書は較正にだけ効き帯には効かない() {
        // 足せる形を決めておく。 決めずに置くと、素材だけ入って判定に効かないと
        // いういちばん質の悪い状態になる——使う側は効いていると思って集め続ける。
        //
        // 較正の違う人の側には効く。 基準だけで学習すると、測っているのは
        // 「その人らしさ」ではなく「この基準との違い」になる。
        //
        // 帯・割り・語彙には効かない。 帯の点の数が本人と基準で釣り合わなくなり、
        // 他人が何人来たかで本人の測り方が変わる。
        let m = Fixture::new(10);
        let extra = Fixture::new(14);
        // 別の場面の他人。**名前が本人・基準と衝突しないようにする。**
        let others: Vec<(String, Document)> = extra.person[10..]
            .iter()
            .map(|(n, d)| (format!("o-{n}"), d.clone()))
            .collect();
        let bare = assemble(
            Material {
                person: &Fixture::samples(&m.person),
                baseline: &Fixture::samples(&m.baseline),
                others: &[],
            },
            Some(&Chars),
        )
        .expect("目盛りができる");
        let with = assemble(
            Material {
                person: &Fixture::samples(&m.person),
                baseline: &Fixture::samples(&m.baseline),
                others: &Fixture::samples(&others),
            },
            Some(&Chars),
        )
        .expect("目盛りができる");

        // 割りと語彙は 1 ミリも動かない。 動けば、相手集合か語彙に
        // 他人が混ざっている。
        assert_eq!(with.selection, bare.selection, "割りが動かない");
        assert_eq!(with.frozen, bare.frozen, "語彙が動かない");

        // 較正は動く。 動かなければ、入れたものが読まれていない。
        assert_ne!(with.calibration, bare.calibration, "照合値の較正は動く");
        assert_ne!(with.humanness, bare.humanness, "人らしさの較正は動く");
    }

    #[test]
    fn 語彙は割る前に全体から固定する() {
        // 側ごとに違う語彙を使えば、側ごとに次元の意味が変わる。
        let m = Fixture::new(10);
        let scale = assemble(
            Material {
                person: &Fixture::samples(&m.person),
                baseline: &Fixture::samples(&m.baseline),
                others: &[],
            },
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
        let m = Fixture::new(10);
        let person = Fixture::samples(&m.person);
        let scale = assemble(
            Material {
                person: &person,
                baseline: &Fixture::samples(&m.baseline),
                others: &[],
            },
            Some(&Chars),
        )
        .unwrap();
        // 相手集合は目盛りが持っている。 本文はもう要らない。
        assert_eq!(scale.partner_vectors.len(), 5);
        let got = measure_against(&scale, person[9], Some(&Chars));
        assert!(got.matching.is_some(), "照合値が出る");
        assert!(got.humanness.is_some(), "人らしさ値が出る");
        assert!(got.missing_systems.is_empty());
    }

    #[test]
    fn 系統が欠ければ照合値を出さない() {
        let m = Fixture::new(10);
        let person = Fixture::samples(&m.person);
        let scale = assemble(
            Material {
                person: &person,
                baseline: &Fixture::samples(&m.baseline),
                others: &[],
            },
            Some(&Chars),
        )
        .unwrap();
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
        assert_eq!(got.missing_systems.len(), 5);
        assert_eq!(got.missing_humanness.len(), 14);
    }
}
