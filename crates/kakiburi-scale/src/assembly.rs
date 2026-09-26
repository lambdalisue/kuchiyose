//! 2 つのカセットの統計値から目盛りを組み立てる。
//!
//! 目盛りは保存しない。比べるたびに、本人のカセットと基準のカセットからその場で
//! 組み立てる（[目盛りを作る](../../../docs/spec/200-extract.md#目盛りを作る)）。
//! どの基準のカセットにも同じ手順を掛ける。
//!
//! 1. 基準の文書を、本人の文体と題材で[選ぶ](crate::select)。
//! 2. 本人の長さに合わせて、基準の統計値を[束ねる](crate::bundle)。
//! 3. 相手集合と点に割り、語彙を固定し、較正し、帯を作る（[組み立て](crate::assemble)）。
//! 4. 効く指標を決め、本人がいちばん高く出るかを[確かめる](crate::self_check)。
//!
//! 入口の欄は 2 つである。本人の統計値と、基準の統計値。草稿を受け取る欄は無い。
//! 本人の欄の統計値だけが相手集合・天井・幅・型の本人の側になり、基準の欄の
//! 統計値だけが較正の違う人の側・床・基準の型の側になる。

use crate::assemble::{build, matching_of_unit, Measurements, Report, Units};
use crate::bundle::{bundle_plans, measurable_length, try_plans, Attempt};
use crate::effective::{self, Effective, Row};
use crate::select::{select_baseline, Register, RegisterFilter};
use crate::self_check::{measured_in_self_check, SelfCheck, Side};
use crate::stats::{bundle_name, compose, compose_directives, CassetteStats, DocumentStats};
use crate::{Scale, ScaleError};

/// 本人のカセットの調整のうち、組み立てに効くもの。
///
/// 無効にした指標や言い回しは組み立てに効かない。検めで外す。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tuning {
    /// 申告した文体。あれば、数えた本人の文体より優先する。
    pub register: Option<Register>,
}

/// 組み立てた目盛りと、それと一緒に決まるもの。
#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    /// 目盛り。
    pub scale: Scale,
    /// 指示できる指標ごとの、効くかの判定。
    pub effective: Vec<Effective>,
    /// 本人がいちばん高く出るか。照合値を出せる単位が片側に 1 本も無ければ `None`。
    pub self_check: Option<SelfCheck>,
}

/// 目盛りを作らずに止まった。止まっても失敗ではない。
#[derive(Debug, Clone, PartialEq)]
pub struct Stopped {
    /// 止まった理由。
    pub error: ScaleError,
    /// 本人の単位ごとの、測れたかの内訳。
    pub person: Vec<Report>,
    /// 基準の単位ごとの、測れたかの内訳。最後に試した束ね方の単位である。
    pub baseline: Vec<Report>,
}

/// 組み立ての結果。どの基準の文書をどう使ったかも持つ。
#[derive(Debug, Clone, PartialEq)]
pub struct Assembly {
    /// 基準を文体でどう絞ったか。
    pub register: RegisterFilter,
    /// 題材で選んだ基準の文書の名前。単位名の昇順。
    pub picked: Vec<String>,
    /// 最後に試した束ね方。束ねた単位ごとに、中の文書の名前を束の中の並びで持つ。
    pub bundles: Vec<Vec<String>>,
    /// 試した束ね方の数。長さで断られたときだけ次の案を試す。
    pub tried: usize,
    /// 目盛りか、止まった理由。
    pub outcome: Result<Built, Stopped>,
}

/// 基準の単位を組み立ての中で指す名前。
///
/// 2 つのカセットをまたいで同じ名前があってもよい。単位は役と名前の組で
/// 決まる。組み立ては名前で値を引くので、基準の側だけ役を名前に入れて重ならない
/// ようにし、組み立てが終わったら外す。
const BASELINE_ROLE: char = '\u{0}';

/// 試した束ね方と、その結果。
type Tried<'a> = (Vec<Vec<&'a DocumentStats>>, Result<Scale, ScaleError>);

fn baseline_key(name: &str) -> String {
    format!("{BASELINE_ROLE}{name}")
}

fn baseline_name(key: &str) -> String {
    key.strip_prefix(BASELINE_ROLE).unwrap_or(key).to_owned()
}

/// 2 つのカセットの統計値から目盛りを組み立てる。
///
/// `by_appearance` は、指示できる指標のうち、効くかを使った割合で見るものを
/// 言う。定義ファイルが決めるので、呼ぶ側が渡す。
#[must_use]
pub fn assemble_stats(
    target: &CassetteStats,
    baseline: &CassetteStats,
    tuning: Tuning,
    by_appearance: &dyn Fn(&str) -> bool,
) -> Assembly {
    let selection = select_baseline(target, baseline, tuning.register);
    let picked = selection.picked;

    // 本人の最も長い記事は、測れそうな記事だけで取る。
    let person_lengths: Vec<usize> = target
        .documents
        .iter()
        .filter_map(|d| measurable_length(d.chars, d.commas))
        .collect();
    let pool_lengths: Vec<usize> = picked.iter().map(|d| d.chars).collect();
    let plans = bundle_plans(&person_lengths, &pool_lengths);

    let mut tried = 0usize;
    let mut last: Option<Tried<'_>> = None;
    let attempted: Result<Attempt, std::convert::Infallible> = try_plans(plans, |_, plan| {
        tried += 1;
        let groups: Vec<Vec<&DocumentStats>> = plan
            .iter()
            .map(|g| g.iter().map(|&i| picked[i]).collect())
            .collect();
        let built = build(Units {
            person: person_units(target),
            baseline: groups
                .iter()
                .map(|g| (baseline_key(&bundle_name(g)), compose(g)))
                .collect(),
            others: Vec::new(),
            lexicon: target.lexicon.clone(),
        });
        let how = match &built {
            Ok(_) => Attempt::Built,
            Err(ScaleError::LengthRange { .. }) => Attempt::LengthRange,
            Err(_) => Attempt::Stopped,
        };
        last = Some((groups, built));
        Ok(how)
    });
    if let Err(never) = attempted {
        match never {}
    }

    let (groups, built) = last.unwrap_or_else(|| {
        (
            Vec::new(),
            Err(ScaleError::Split(crate::SplitError::NotEnough {
                usable: 0,
                need: crate::split::UNITS_FLOOR,
            })),
        )
    });
    let outcome = match built {
        Ok(scale) => Ok(finish(scale, target, &groups, by_appearance)),
        Err(error) => Err(Stopped {
            error,
            person: target
                .documents
                .iter()
                .map(|d| compose(&[d]).report(&d.name))
                .collect(),
            baseline: groups
                .iter()
                .map(|g| compose(g).report(&bundle_name(g)))
                .collect(),
        }),
    };
    Assembly {
        register: selection.register,
        picked: picked.iter().map(|d| d.name.clone()).collect(),
        bundles: groups
            .iter()
            .map(|g| g.iter().map(|d| d.name.clone()).collect())
            .collect(),
        tried,
        outcome,
    }
}

/// 本人の単位。本人の文書は束ねない。
fn person_units(target: &CassetteStats) -> Vec<(String, Measurements)> {
    target
        .documents
        .iter()
        .map(|d| (d.name.clone(), compose(&[d])))
        .collect()
}

/// 組み立てた目盛りに、効く指標と自己検査を添える。
fn finish(
    mut scale: Scale,
    target: &CassetteStats,
    groups: &[Vec<&DocumentStats>],
    by_appearance: &dyn Fn(&str) -> bool,
) -> Built {
    for names in [
        &mut scale.selection.baseline_partners,
        &mut scale.selection.baseline_points,
    ] {
        for n in names.iter_mut() {
            *n = baseline_name(n);
        }
    }

    // 相手集合の 5 本ではなく、その役の全単位から取る。 帯に使わない単位も値と幅には使う。
    let row = |docs: &[&DocumentStats]| -> Row {
        compose_directives(docs)
            .into_iter()
            .map(|(name, c)| (name, c.value()))
            .collect()
    };
    let person_rows: Vec<Row> = target.documents.iter().map(|d| row(&[d])).collect();
    let baseline_rows: Vec<Row> = groups.iter().map(|g| row(g)).collect();
    let effective = effective::judge(&person_rows, &baseline_rows, by_appearance);

    let side = |role: Side, units: Vec<(String, Measurements)>| -> Vec<(String, f64)> {
        units
            .into_iter()
            .filter(|(n, _)| measured_in_self_check(&scale, role, n))
            .filter_map(|(n, m)| matching_of_unit(&scale, &m).map(|v| (n, v)))
            .collect()
    };
    let mine = side(Side::Person, person_units(target));
    let theirs = side(
        Side::Baseline,
        groups
            .iter()
            .map(|g| (bundle_name(g), compose(g)))
            .collect(),
    );
    let self_check = SelfCheck::of(&mine, &theirs);
    Built {
        scale,
        effective,
        self_check,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kakiburi_doc::Document;
    use kakiburi_metrics::directive;
    use kakiburi_metrics::lexicon::Lexicon;
    use kakiburi_metrics::morph::Analyzed;

    use crate::assemble::{assemble, lexicon_of, measure_against, Material, Sample};
    use crate::select::{RegisterSource, POOL_TAKE};
    use crate::testing::{document, Chars, Fixture};

    fn samples(pairs: &[(String, Document)]) -> Vec<Sample<'_>> {
        Fixture::samples(pairs)
    }

    fn stats(pairs: &[(String, Document)]) -> CassetteStats {
        CassetteStats::measure(&samples(pairs), Some(&Chars)).expect("測れる")
    }

    fn stats_with(pairs: &[(String, Document)], lexicon: &Lexicon) -> CassetteStats {
        CassetteStats::measure_with(&samples(pairs), Some(&Chars), lexicon.clone()).expect("測れる")
    }

    /// 効くかを使った割合で見る指標。両方の見方を通す。
    fn by_appearance(name: &str) -> bool {
        name.contains("見出し") || name.contains("括弧")
    }

    fn built(a: Assembly) -> Built {
        match a.outcome {
            Ok(b) => b,
            Err(s) => panic!("目盛りができない: {}", s.error),
        }
    }

    #[test]
    fn 本人のまとめ方で測った統計値からは本文から組み立てた目盛りと同じ目盛りができる() {
        // 統計値の経路が正しいことの確かめである。 語のまとめ方を本人のもので揃え、
        // 束ねない素材なら、本文から組み立てた目盛りとビットまで一致する。
        for n in [10, 14] {
            let m = Fixture::new(n);
            let (person, baseline) = (samples(&m.person), samples(&m.baseline));
            let old = assemble(
                Material {
                    person: &person,
                    baseline: &baseline,
                    others: &[],
                },
                Some(&Chars),
            )
            .expect("目盛りができる");

            let lexicon = lexicon_of(&person, Some(&Chars));
            let got = assemble_stats(
                &stats(&m.person),
                &stats_with(&m.baseline, &lexicon),
                Tuning::default(),
                &by_appearance,
            );
            assert!(
                got.bundles.iter().all(|g| g.len() == 1),
                "束ねない素材である"
            );
            let b = built(got);
            assert_eq!(b.scale, old, "目盛り（{n} 本）");

            // 効くかの判定は、本文から測った指示できる指標の値と同じものから出る。
            let rows = |ss: &[Sample<'_>]| -> Vec<Row> {
                ss.iter()
                    .map(|s| {
                        let plain =
                            Analyzed::with_lexicon(&s.document.prose(), &Chars, &lexicon).ok();
                        directive::measure(s.document, plain.as_ref())
                            .into_iter()
                            .map(|(name, c)| (name, c.value()))
                            .collect()
                    })
                    .collect()
            };
            assert_eq!(
                b.effective,
                effective::judge(&rows(&person), &rows(&baseline), &by_appearance),
                "効く指標（{n} 本）"
            );

            // 自己検査は、本文から検めたときの照合値と同じ値で確かめる。
            let side = |ss: &[Sample<'_>], role: Side| -> Vec<(String, f64)> {
                ss.iter()
                    .filter(|s| measured_in_self_check(&old, role, s.name))
                    .filter_map(|s| {
                        measure_against(&old, *s, Some(&Chars))
                            .matching
                            .map(|v| (s.name.to_owned(), v))
                    })
                    .collect()
            };
            assert_eq!(
                b.self_check,
                SelfCheck::of(
                    &side(&person, Side::Person),
                    &side(&baseline, Side::Baseline)
                ),
                "自己検査（{n} 本）"
            );
        }
    }

    #[test]
    fn 自己検査は組み立てるたびに走る() {
        let m = Fixture::new(10);
        let b = built(assemble_stats(
            &stats(&m.person),
            &stats(&m.baseline),
            Tuning::default(),
            &by_appearance,
        ));
        let check = b.self_check.expect("確かめられる");
        assert!(check.passed(), "{check:?}");
        assert!(check.pairs > 0);
    }

    #[test]
    fn 草稿は本人のカセットのまとめ方で測る() {
        // 基準のまとめ方は基準の文書を測ったときに使ったきりである。
        let m = Fixture::new(10);
        let (target, baseline) = (stats(&m.person), stats(&m.baseline));
        assert_ne!(target.lexicon, baseline.lexicon, "カセットごとに見つける");
        let b = built(assemble_stats(
            &target,
            &baseline,
            Tuning::default(),
            &by_appearance,
        ));
        assert_eq!(b.scale.lexicon, target.lexicon);
    }

    #[test]
    fn 同じ名前が本人と基準をまたいでもよい() {
        // 単位は役と名前の組で指す。 役は引数の位置で決まる。
        let m = Fixture::new(10);
        let renamed: Vec<(String, Document)> = m
            .baseline
            .iter()
            .enumerate()
            .map(|(i, (_, d))| (format!("p{i:02}"), d.clone()))
            .collect();
        let with_own = built(assemble_stats(
            &stats(&m.person),
            &stats(&m.baseline),
            Tuning::default(),
            &by_appearance,
        ));
        let with_same = built(assemble_stats(
            &stats(&m.person),
            &stats(&renamed),
            Tuning::default(),
            &by_appearance,
        ));
        assert_eq!(
            with_same.scale.selection.baseline_partners,
            vec!["p00", "p02", "p04", "p06", "p08"],
            "名前は役を外して出す"
        );
        assert_eq!(
            with_same.scale.band, with_own.scale.band,
            "値は名前に依らない"
        );
        assert_eq!(with_same.self_check, with_own.self_check);
    }

    #[test]
    fn 基準のカセットを替えても本人の側の割りは動かない() {
        // 本人の欄の統計値だけが相手集合と測る分になる。 基準を替えれば床と較正は動く。
        let m = Fixture::new(12);
        let other: Vec<(String, Document)> = (10..22)
            .map(|i| (format!("c{i:02}"), document(i % 12, true)))
            .rev()
            .collect();
        let a = built(assemble_stats(
            &stats(&m.person[..10]),
            &stats(&m.baseline),
            Tuning::default(),
            &by_appearance,
        ));
        let other_person = stats(&m.person[..10]);
        let b = built(assemble_stats(
            &other_person,
            &stats(&other[..11]),
            Tuning::default(),
            &by_appearance,
        ));
        assert_eq!(
            a.scale.selection.person_partners,
            b.scale.selection.person_partners
        );
        assert_eq!(
            a.scale.selection.person_points,
            b.scale.selection.person_points
        );
        assert_ne!(a.scale.calibration, b.scale.calibration, "較正は動く");
        assert_ne!(
            a.scale.selection.baseline_partners, b.scale.selection.baseline_partners,
            "基準の側の割りは動く"
        );
    }

    #[test]
    fn 短い基準は束ねて本人の長さに届かせる() {
        // 本人の最も長い記事が基準の最も長い文書より長ければ、短いほうから積む。
        let person: Vec<(String, Document)> = (0..10)
            .map(|i| (format!("p{i:02}"), document(i * 3, false)))
            .collect();
        let baseline: Vec<(String, Document)> = (0..18)
            .map(|i| (format!("b{i:02}"), document(i % 9, true)))
            .collect();
        let a = assemble_stats(
            &stats(&person),
            &stats(&baseline),
            Tuning::default(),
            &by_appearance,
        );
        let bundled: Vec<&Vec<String>> = a.bundles.iter().filter(|g| g.len() > 1).collect();
        assert!(!bundled.is_empty(), "{:?}", a.bundles);
        let used: usize = a.bundles.iter().map(Vec::len).sum();
        assert_eq!(used, baseline.len(), "基準を余さず使う");
        if let Ok(b) = &a.outcome {
            let names: Vec<&String> = b
                .scale
                .selection
                .baseline_partners
                .iter()
                .chain(&b.scale.selection.baseline_points)
                .collect();
            assert!(
                names
                    .iter()
                    .all(|n| a.bundles.iter().any(|g| g.join("+") == **n)),
                "束ねた単位の名前は中の名前を + で繋いだもの: {names:?}"
            );
        }
    }

    #[test]
    fn どの基準も題材で選ぶ() {
        // 同梱の基準でも人のカセットでも同じ手順を掛ける。
        let m = Fixture::new(10);
        let many: Vec<(String, Document)> = (0..POOL_TAKE + 6)
            .map(|i| (format!("b{i:02}"), document(i % 10, true)))
            .collect();
        let words = |v: &[&str]| {
            v.iter()
                .map(|w| (*w).to_owned())
                .collect::<std::collections::BTreeSet<_>>()
        };
        let mut target = stats(&m.person);
        for d in &mut target.documents {
            d.content_words = words(&["東京", "天気"]);
        }
        let mut pool = stats(&many);
        for (i, d) in pool.documents.iter_mut().enumerate() {
            // 後ろの 6 本だけが本人と題材を共有しない。
            d.content_words = if i < POOL_TAKE {
                words(&["東京", "料理"])
            } else {
                words(&["料理"])
            };
        }
        let a = assemble_stats(&target, &pool, Tuning::default(), &by_appearance);
        let want: Vec<String> = many[..POOL_TAKE].iter().map(|(n, _)| n.clone()).collect();
        assert_eq!(a.picked, want, "重なりの多い順に {POOL_TAKE} 本");
    }

    #[test]
    fn 文体の申告は基準の選び方に効く() {
        // 作り物の文は敬体でも常体でも終わらないので、数えた文体は決まらない。
        let m = Fixture::new(10);
        let counted = assemble_stats(
            &stats(&m.person),
            &stats(&m.baseline),
            Tuning::default(),
            &by_appearance,
        );
        assert!(matches!(counted.register, RegisterFilter::Undecided { .. }));
        let declared = assemble_stats(
            &stats(&m.person),
            &stats(&m.baseline),
            Tuning {
                register: Some(Register::Polite),
            },
            &by_appearance,
        );
        assert!(matches!(
            declared.register,
            RegisterFilter::Missing {
                register: Register::Polite,
                source: RegisterSource::Declared,
                ..
            }
        ));
        assert_eq!(
            declared.picked, counted.picked,
            "基準に敬体が無ければ全部使う"
        );
    }

    #[test]
    fn 素材が足りなければ止まり単位ごとの内訳を返す() {
        // 作れないことは失敗ではない。 どの単位のどこで止まったかを言う。
        let m = Fixture::new(10);
        let a = assemble_stats(
            &stats(&m.person[..4]),
            &stats(&m.baseline),
            Tuning::default(),
            &by_appearance,
        );
        let stopped = a.outcome.expect_err("止まる");
        assert!(
            matches!(stopped.error, ScaleError::Split(_)),
            "{}",
            stopped.error
        );
        assert_eq!(stopped.person.len(), 4);
        assert_eq!(stopped.baseline.len(), m.baseline.len());
        assert_eq!(a.tried, 1, "長さ以外で止まったら束ね直さない");
    }
}
