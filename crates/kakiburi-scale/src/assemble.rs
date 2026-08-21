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
    /// その単位の中で再来した言い回し。
    recurring: Vec<String>,
    /// その単位が繰り返しすぎている短い言い回し。
    overused: Vec<String>,
    /// その単位で一度しか出てこない語。<strong>語を散らしている当のものである。</strong>
    once_only: Vec<String>,
    /// 人らしさの 12 次元。
    humanness: Humanness,
    /// 地の文の日本語の文字数。長さの範囲に使う。
    chars: usize,
}

impl Measurements {
    fn of(sample: Sample<'_>, analyzer: Option<&dyn Analyzer>) -> Self {
        // <strong>識別子を伏せてから測る。</strong> `denops.vim` のような半角英字の連なりは
        // <strong>書き手が選んだ書きぶりではなく題材が決めるもの</strong>で、そのまま入れると
        // <strong>同じ人が別の題材で書いた文章を「その人らしくない」と言う</strong>。
        //
        // <strong>掛けるのはここだけである。</strong> 指示できる指標は和欧間スペースや半角英字
        // そのものを測るので、生の文から測り続ける。
        let prose = kakiburi_doc::prose::mask_identifiers(&sample.document.prose());
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
            // <strong>長いほうだけを取る。</strong> 短い言い回しは誰でも繰り返すので、
            // 渡しても癖にならない。
            recurring: kakiburi_metrics::humanness::recurring(
                analyzed.as_ref(),
                &kakiburi_metrics::humanness::LONG_N,
            ),
            // <strong>「減らせ」と言うなら、どれを減らすのかを言う。</strong>
            overused: kakiburi_metrics::humanness::overused(
                analyzed.as_ref(),
                &kakiburi_metrics::humanness::SHORT_N,
                OVERUSED,
            ),
            // <strong>「散らすな」と言うなら、どれが散らしているのかを言う。</strong>
            once_only: kakiburi_metrics::humanness::once_only(analyzed.as_ref(), ONCE_ONLY),
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

/// 目盛りを作る材料。
///
/// <strong>場面を跨げる素材を、跨げない素材と同じ配列に入れない。</strong> 入れれば、
/// 相手集合にも天井にも他人が混ざる道が開く——[場面ごとに閉じる](../../../docs/spec/010-strategy.md#場面ごとに閉じる)
/// が、呼ぶ側の注意だけで守られることになる。
///
/// <strong>欄で分けたうえで、[`assemble`]は [`others`](Self::others) を人らしさの較正にしか
/// 渡さない。</strong> 型が全部を守るわけではないが、跨ぐ道が 1 本に絞られる。
#[derive(Debug, Clone, Copy)]
pub struct Material<'a> {
    /// <strong>1 つの場面の</strong>本人の単位。
    pub person: &'a [Sample<'a>],
    /// <strong>同じ場面の</strong>基準の単位。
    pub baseline: &'a [Sample<'a>],
    /// 他人の文書。<strong>場面を跨いでよい唯一の素材である。</strong>
    ///
    /// [人らしさの較正の人の側](../../../docs/spec/200-extract.md#人らしさの境目は同じ材料から出る)
    /// にだけ足す。<strong>帯の側には足さない</strong>——足せば、帯の点の数が本人と基準で
    /// 釣り合わなくなる。
    pub others: &'a [Sample<'a>],
}

/// 組み立てる。
///
/// <strong>作れないことは失敗ではない。</strong> 素材が足りなければ目盛りを作らず、検めが
/// 判定できないを返す——それが正しい振る舞いである。
pub fn assemble(m: Material<'_>, analyzer: Option<&dyn Analyzer>) -> Result<Scale, ScaleError> {
    let (person, baseline) = (m.person, m.baseline);
    let measured: BTreeMap<String, Measurements> = person
        .iter()
        .chain(baseline.iter())
        .chain(m.others.iter())
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
    // <strong>他人の文書は較正の側にだけ足す。</strong> 人らしさの人の側は「誰の文章でも人が
    // 書いたものは人の側に落ちる」ので、素材が足りなければ混ぜてよい——
    // <strong>場面は人と機械の別を跨がない。</strong>
    //
    // <strong>較正には、帯に使う分を除いた全部を渡す。</strong>
    //
    // 帯の端を各側 5 点に固定したのは、<strong>最小・最大が n とともに外へ広がる</strong>から
    // である。<strong>回帰は漂わない</strong>ので、同じ縛りを較正に掛ける理由が無い——
    // 10 単位で 4 つの重みを当てはめると、標本外で当たらない。
    //
    // <strong>帯の点は除く。</strong> 較正に使った単位で帯を作れば、分離するように合わせた
    // ものの分離具合を見ることになる。
    let for_calibration = |all: &[Unit], points: &[Unit]| -> Vec<Unit> {
        all.iter()
            .filter(|u| u.usable() && !points.iter().any(|p| p.name == u.name))
            .cloned()
            .collect()
    };
    let human_units = for_calibration(&person_units, &person_split.points);
    let machine_units = for_calibration(&baseline_units, &baseline_split.points);

    // <strong>測れなかったものは落とす。</strong> 12 次元が揃わない行を混ぜれば、列の数が
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
    // <strong>重なりでは止めない。</strong> 人らしさの帯が全体を覆うのは設計どおりの結果で、
    // 当てはまらないことが判定不能として出る。止めれば自己診断が消える。
    let humanness_band = Band::build_humanness(&human, &machine).map_err(|source| {
        ScaleError::Band(Box::new(crate::BandStop {
            source,
            which: "人らしさ値",
            ceiling: human.clone(),
            floor: machine.clone(),
            // <strong>照合の帯は作れている。</strong> 止まったのは人らしさの側である。
            built: Some(band),
        }))
    })?;

    // <strong>本人の代表値を先に作る。</strong> 効く量を数える相手である。
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

    Ok(Scale {
        frozen,
        calibration,
        band,
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
        // <strong>作るのはここだけである。</strong> 検めが作り直せる形にしておくと、検める文書を
        // 見てから言い回しを選び直す経路が書ける。
        humanness_target,
        phrases: phrases_of(&person_units, &measured),
    })
}

/// 何本の単位で再来したかで、言い回しを並べる。
///
/// <strong>1 本にしか出ない言い回しは、その文書の題材が作ったものである。</strong> 2 本以上で
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
    // <strong>多い順。同じなら短い順、次に文字の順。</strong> 決めておかないと並びが実装で変わる。
    out.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.chars().count().cmp(&b.1.chars().count()))
            .then_with(|| a.1.cmp(&b.1))
    });
    out.truncate(PHRASES);
    out.into_iter().map(|(_, g)| g).collect()
}

/// 残す言い回しの数。<strong>暫定値である。</strong>
pub const PHRASES: usize = 20;

/// 名指しする「繰り返しすぎ」の数。<strong>暫定値である。</strong>
pub const OVERUSED: usize = 3;

/// 名指しする「一度きりの語」の数。<strong>暫定値である。</strong>
pub const ONCE_ONLY: usize = 8;

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
        // <strong>合算した 1 つの値では直し方を渡せない。</strong>「機械の側にある」としか
        // 言えず、どこをどうすればよいかが出てこない。
        //
        // <strong>寄せる向きは較正から読む。</strong> 定義に固定すると、素材がその向きを
        // 支えていないカセットで<strong>直し方に従うほど人らしさが下がる</strong>。
        //
        // <strong>次元の向きが割れている指標は渡さない。</strong> どちらへ動かせばよいかを
        // 言えないものを指示にしない。
        humanness_by_metric: {
            let toward = scale.humanness.toward_human();
            let all = scale
                .humanness
                .by_metric(&t.humanness.flat())
                .unwrap_or_default();
            let values: Vec<f64> = all.iter().map(|(_, v)| *v).collect();
            // <strong>いまの合算。</strong> 差を取る相手である。
            let now = scale.humanness.fuse(&values, None);
            all.iter()
                .enumerate()
                .filter_map(|(j, (n, v))| {
                    let (_, up) = toward.iter().find(|(m, _)| m == n)?;
                    // <strong>その指標だけを本人の代表値へ置いて、合算し直す。</strong>
                    //
                    // <strong>直し方は 2 本以上出て、互いに正反対を指すことがある</strong>
                    // ——語彙を散らせと言う指標と、言い換えるなと言う指標が同時に
                    // 出る。<strong>どちらが勝つかを言わなければ、受け取った側は逆を選ぶ。</strong>
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
                        // <strong>減らす側では、この文章が繰り返しすぎているものを名指す。</strong>
                        // 増やす側で本人の言い回しを渡すのと表裏である。
                        overused: if *up { Vec::new() } else { t.overused.clone() },
                        // <strong>散らすなと言う側でだけ渡す。</strong> 散らせと言う側に
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
/// <strong>並びの位置で意味を持たせない。</strong> 3 つ組で渡すと、受け取る側が位置の意味を
/// コメントで補うことになり、<strong>順番を入れ替えたときに型が何も言わない。</strong>
#[derive(Debug, Clone, PartialEq)]
pub struct HumannessByMetric {
    /// 指標の名前。
    pub name: String,
    /// その指標だけで見た人らしさ値。<strong>正が人の側、負が機械の側。</strong>
    pub value: f64,
    /// 人へ寄せる向き。<strong>`true` なら値を上げる。</strong>
    pub raise: bool,
    /// この文章が繰り返しすぎている言い回し。<strong>減らす側でだけ意味を持つ。</strong>
    pub overused: Vec<String>,
    /// この文章で一度しか出てこない語。<strong>語を散らしている当のものである。</strong>
    pub once_only: Vec<String>,
    /// 本人の代表値へ置いたときに、人らしさ値が動く量。
    ///
    /// <strong>正反対を指す直し方が同時に出ることがある。</strong> どちらが勝つかは、
    /// 動く量でしか言えない。
    pub effect: f64,
    /// 本人の代表値。
    ///
    /// <strong>0 と比べてはいけない。</strong> 0 は人と機械の境目であって、その人の
    /// ところではない——<strong>4 指標とも境目より人の側にいるのに、合算では機械の側</strong>
    /// ということが実際に起きる。
    pub target: f64,
}

/// 系統の 1 次元ぶんの隔たり。
///
/// <strong>系統そのものは指示にならない</strong>——「342 次元目を増やせ」は言葉にならない。
/// <strong>だが次元が語として読める系統なら、その 1 次元は指示になる</strong>——「あなたは
/// 『〜のだ』をよく使うが、この草稿には出てこない」は直せる。
#[derive(Debug, Clone, PartialEq)]
pub struct Divergence {
    /// どの系統か。
    pub system: String,
    /// どの次元か。<strong>語・記号・字種など、読める形である。</strong>
    pub dim: String,
    /// この文章の、標準化した値。
    pub mine: f64,
    /// 相手集合の、標準化した値の中央値。
    pub theirs: f64,
    /// その系統の、合算での重み。
    pub weight: f64,
    /// この次元を本人の値に置いたときに照合値が動く量。<strong>正なら近づく。</strong>
    pub effect: f64,
    /// 本人がその次元をどう書いているかの実例。
    ///
    /// <strong>「増やせ」と言うだけでは、どこに置くのかが分からない。</strong> 実測では、
    /// 「空白を増やす」という指示を受けた側が<strong>本人の記事を自分で覗いて</strong>
    /// 打ち方を調べることになった。
    pub examples: Vec<String>,
}

impl Divergence {
    /// 隔たりの大きさ。
    #[must_use]
    pub fn size(&self) -> f64 {
        (self.mine - self.theirs).abs()
    }

    /// この 1 次元を本人の値に置いたとき、照合値が実際に動く量。
    ///
    /// <strong>当て推量で並べてはいけない。</strong> 隔たりの大きさでも、隔たり × 重みでも、
    /// <strong>実際に動く量とは一致しない</strong>——次元の数が系統ごとに違うので、10 次元しか
    /// ない系統の 1 本は 500 次元の系統の 1 本よりずっと大きく効く。実測では、
    /// <strong>いちばん効く直し（文字種）が一度も上位に出てこなかった。</strong>
    ///
    /// <strong>だから数える。</strong> その次元だけを本人の代表値に置き換えて照合値を出し直し、
    /// 差を取る。<strong>これは見込みではなく、そのまま効く量である。</strong>
    #[must_use]
    pub fn effect(&self) -> f64 {
        self.effect
    }

    /// 増やす側か。<strong>相手のほうが大きいなら増やす。</strong>
    #[must_use]
    pub fn raise(&self) -> bool {
        self.theirs > self.mine
    }
}

/// 照合値を出し直す。<strong>1 次元だけ置き換えられる。</strong>
///
/// `swap` に `(系統, 次元, 値)` を渡すと、その 1 次元だけを差し替えて測る。
/// <strong>効く量を数えるための道具である</strong>——見込みではなく、そのまま動く量が出る。
fn matching_of(
    scale: &Scale,
    t: &Measurements,
    ps: &[Measurements],
    swap: Option<(System, usize, f64)>,
) -> Option<f64> {
    let vector = |m: &Measurements, name: &str, mine: bool| -> Option<Vec<f64>> {
        let system = System::from_name(name)?;
        let set = &scale.frozen.iter().find(|(n, _)| n == name)?.1;
        let parts = m.parts.get(&system)?;
        let Some((s, j, x)) = swap.filter(|_| mine) else {
            return set.project(parts);
        };
        if s != system {
            return set.project(parts);
        }
        // <strong>投影した値を書き換えない。数え上げの側で動かす。</strong>
        //
        // 系統の次元はどれも相対頻度なので、<strong>1 つを減らせば残りの割合が上がる</strong>
        // ——読点を 1 つ外せば、外さなかった読点の取り分が増える。<strong>投影した値を
        // 直接書き換えると、実際には書けない直しを見積もることになる。</strong>
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
    let values: Vec<f64> = ps
        .iter()
        .filter_map(|p| {
            scale
                .calibration
                .matching_value(&|n| {
                    let a = vector(t, n, true)?;
                    let b = vector(p, n, false)?;
                    Some(cosine_delta(&a, &b))
                })
                .ok()
        })
        .collect();
    median(&values)
}

/// 相手集合からいちばん離れている次元を挙げる。
///
/// <strong>照合値は 1 つの数なので、どこが違うのかを言えない。</strong> 帯の中で止まったときに
/// 何も出さなければ、受け取った側は動きようがない。
///
/// <strong>読める系統だけを見る。</strong> `systems` に渡すのは、次元が語や記号として読める
/// ものだけである——品詞 bigram の「名詞-助詞」を増やせとは言えない。
///
/// <strong>相手は中央値で代表する。</strong> 平均だと 1 本の外れ値が代表を引っ張る。
#[must_use]
pub fn diverging(
    scale: &Scale,
    target: Sample<'_>,
    partners: &[Sample<'_>],
    analyzer: Option<&dyn Analyzer>,
    systems: &[System],
    top: usize,
) -> Vec<Divergence> {
    let t = Measurements::of(target, analyzer);
    let ps: Vec<Measurements> = partners
        .iter()
        .map(|s| Measurements::of(*s, analyzer))
        .collect();

    let mut out: Vec<Divergence> = Vec::new();
    for system in systems {
        let name = system.name();
        let Some((_, set)) = scale.frozen.iter().find(|(n, _)| n == name) else {
            continue;
        };
        // <strong>重みを引く。</strong> 重みの小さい系統をいくら直しても照合値は動かない。
        let weight = scale
            .calibration
            .systems()
            .iter()
            .position(|n| n == name)
            .and_then(|j| scale.calibration.fusion().slopes().get(j).copied())
            .unwrap_or(0.0);
        // <strong>重みが 0 の系統は挙げない。</strong> 直しても動かないものを指示にしない。
        if weight <= 0.0 {
            continue;
        }
        let Some(mine) = t.parts.get(system).and_then(|c| set.project(c)) else {
            continue;
        };
        let theirs: Vec<Vec<f64>> = ps
            .iter()
            .filter_map(|p| p.parts.get(system).and_then(|c| set.project(c)))
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
        // <strong>いまの照合値を、この 1 本の相手ごとに控えておく。</strong> 差を取る相手である。
        let now = matching_of(scale, &t, &ps, None);
        for (j, dim) in dims.iter().enumerate() {
            let Some(&m) = mine.get(j) else { continue };
            let mut col: Vec<f64> = theirs.iter().filter_map(|v| v.get(j).copied()).collect();
            if col.is_empty() {
                continue;
            }
            col.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let theirs_med = col[col.len() / 2];
            // <strong>その次元だけを本人の代表値に置いて、照合値を出し直す。</strong>
            let fixed = matching_of(scale, &t, &ps, Some((*system, j, theirs_med)));
            let (Some(a), Some(b)) = (now, fixed) else {
                continue;
            };
            out.push(Divergence {
                system: name.to_owned(),
                dim: (*dim).to_owned(),
                mine: m,
                theirs: theirs_med,
                weight,
                effect: b - a,
                examples: Vec::new(),
            });
        }
    }
    // <strong>動く見込みの大きい順。同じなら系統・次元の名前順。</strong> 決めておかないと、
    // 上位が実装ごとに変わる。
    // <strong>直しても近づかない次元は挙げない。</strong> 効かない指示を出さない。
    out.retain(|d| d.effect > 0.0);
    out.sort_by(|a, b| {
        b.effect()
            .partial_cmp(&a.effect())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.system.cmp(&b.system))
            .then_with(|| a.dim.cmp(&b.dim))
    });
    out.truncate(top);
    // <strong>渡すぶんだけ実例を探す。</strong> 全次元で探すと、使われない実例のために
    // 相手集合を何度も読み直すことになる。
    for d in &mut out {
        d.examples = examples_of(&d.system, &d.dim, partners);
    }
    out
}

/// 本人がその次元をどう書いているかを、実際の文から拾う。
///
/// <strong>「増やせ」と言うだけでは、どこに置くのかが分からない。</strong> 数値と向きだけを
/// 渡された側は、結局その人の文章を自分で読みに行くことになる。
///
/// <strong>拾えないものは無理に作らない。</strong> 間隔のような、字面に現れない次元がある。
fn examples_of(system: &str, dim: &str, partners: &[Sample<'_>]) -> Vec<String> {
    const WANT: usize = 3;
    const AROUND: usize = 6;

    // <strong>どこにでもある字は実例にならない。</strong> ひらがなや漢字を 1 つ抜き出して
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
            // 字面に現れる次元。<strong>読点の直前・直後は 1 文字、機能語は語である。</strong>
            let needle: String = if system == "読点の打ち方" {
                if dim.ends_with('字') || dim.ends_with("字以上") {
                    return Vec::new();
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
        // <strong>「増やせ」と言うだけでは、どこに置くのかが分からない。</strong>
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

/// 見つけた位置のまわりを切り出す。<strong>文字の境で切る。</strong>
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

/// 検める 1 本を測った結果。<strong>出なかったものは `None` である。</strong>
#[derive(Debug, Clone, PartialEq)]
pub struct Measured {
    /// 照合値。<strong>相手集合との中央値。</strong>
    pub matching: Option<f64>,
    /// 人らしさ値。
    pub humanness: Option<f64>,
    /// 指標ごとの人らしさ値と、<strong>人へ寄せる向き</strong>。
    ///
    /// <strong>測れなければ空である。</strong> 直し方を渡す側が、測れていないことと機械の
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
                // 機能語を 5 つ含める。<strong>対象の形態素の下限を越えるためである</strong>——
                // 越えなければ機能語が測れず、判定に使う 5 系統が揃わない。
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
    fn 素材が足りなければ目盛りを作らない() {
        // 作れないことは失敗ではない。<strong>判定できないが返る。</strong>
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
        // 5 系統のうち 2 つが形態素を要る。<strong>抜いて合算しない。</strong>
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
        assert_eq!(scale.frozen.len(), 5, "判定に使う 5 系統");
        assert_eq!(scale.partners().len(), 5);
        assert_eq!(scale.calibration.systems().len(), 5);
    }

    #[test]
    fn 他人の文書は人らしさの較正にだけ効く() {
        // <strong>足せる形を決めておく。</strong> 決めずに置くと、素材だけ入って判定に効かないと
        // いういちばん質の悪い状態になる——使う側は効いていると思って集め続ける。
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

        // <strong>照合の側は 1 ミリも動かない。</strong> 動けば、相手集合か語彙か天井に
        // 他人が混ざっている。
        assert_eq!(with.selection, bare.selection, "割りが動かない");
        assert_eq!(with.frozen, bare.frozen, "語彙が動かない");
        assert_eq!(with.band, bare.band, "照合値の帯が動かない");
        assert_eq!(with.calibration, bare.calibration, "照合値の較正が動かない");

        // <strong>人らしさの較正だけが動く。</strong> 動かなければ、入れたものが読まれていない。
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
        // 相手集合の 5 本を名前で引く。
        let partners: Vec<Sample<'_>> = person
            .iter()
            .filter(|s| scale.partners().iter().any(|n| n == s.name))
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
        let partners: Vec<Sample<'_>> = person
            .iter()
            .filter(|s| scale.partners().iter().any(|n| n == s.name))
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
