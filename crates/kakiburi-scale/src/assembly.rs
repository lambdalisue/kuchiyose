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

use std::collections::{BTreeMap, BTreeSet};
use std::ops::RangeInclusive;

use crate::assemble::{build, matching_of_unit, Measurements, Report, Units};
use crate::bundle::{bundle_plans, measurable_length, try_plans, Attempt};
use crate::effective::{self, Effective, Row};
use crate::select::{select_baseline, Register, RegisterFilter};
use crate::self_check::{measured_in_self_check, SelfCheck, Side};
use crate::stats::{
    bundle_key, bundle_name, compose, compose_directives, display_name, CassetteStats,
    DocumentStats,
};
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
    /// 本人の[言い回しの上限](CassetteStats::phrase_ceilings)。
    ///
    /// 草稿が繰り返している言い回しに、本人の上限を添えるのに使う。
    pub phrase_ceilings: BTreeMap<String, f64>,
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
    /// 長さの範囲で断られて、基準の文書を選び直したときの長さの窓。地の文の字数。
    ///
    /// 窓は本人の測れそうな記事の最短から最長までである。選び直さなければ `None`。
    pub length_window: Option<RangeInclusive<usize>>,
    /// 最後に試した束ね方。束ねた単位ごとに、中の文書の名前を束の中の並びで持つ。
    pub bundles: Vec<Vec<String>>,
    /// 試した束ね方の数。長さで断られたときだけ次の案を試す。選び直す前の分も数える。
    pub tried: usize,
    /// 目盛りか、止まった理由。
    pub outcome: Result<Built, Stopped>,
}

/// 基準の単位を組み立ての中で指す名前。
///
/// 2 つのカセットをまたいで同じ名前があってもよい。単位は役と名前の組で
/// 決まる。組み立ては名前で値を引くので、基準の側だけ役を名前に入れて重ならない
/// ようにし、組み立てが終わったら外す。
///
/// 本人の側の名前がこの前置で始まれば、前置した基準の名前と重なりうる。
/// 重なりは[組み立て](crate::assemble)が `ScaleError::DuplicateName` で断るので、
/// 値が黙って置き換わることはない。 型で役を持たせる手もあるが、組み立ての中は
/// 名前を 1 つの文字列として引き回しているので、変える範囲が大きい。
const BASELINE_ROLE: char = '\u{0}';

/// 試した束ね方と、その結果。
type Tried<'a> = (Vec<Vec<&'a DocumentStats>>, Result<Scale, ScaleError>);

fn baseline_key(name: &str) -> String {
    format!("{BASELINE_ROLE}{name}")
}

/// 組み立ての中の基準の名前を、見せる名前にする。役の前置を外し、束の繋ぎを `+` にする。
fn baseline_name(key: &str) -> String {
    display_name(key.strip_prefix(BASELINE_ROLE).unwrap_or(key))
}

/// 2 つのカセットの統計値から目盛りを組み立てる。
///
/// `by_appearance` は、指示できる指標のうち、効くかを使った割合で見るものを
/// 言う。定義ファイルが決めるので、呼ぶ側が渡す。
///
/// どの束ね方も長さの範囲で断られたら、基準の文書を本人の長さの窓の中から
/// 選び直して、もう一度試す。 題材の重なりは語彙の多い長い文書ほど大きく出るので、
/// 題材で選んだ基準は長いほうへ寄る。 束ねれば長くなる一方なので、本人の記事が
/// 選んだ基準より短いと、束ね方をいくら替えても届かない。 窓に入る文書が単位の
/// 下限に届かなければ選び直さない——作れないことに変わりはなく、最初に断られた
/// 理由のほうが素材の足りなさを正しく言う。
#[must_use]
pub fn assemble_stats(
    target: &CassetteStats,
    baseline: &CassetteStats,
    tuning: Tuning,
    by_appearance: &dyn Fn(&str) -> bool,
) -> Assembly {
    // 本人の最も長い記事は、測れそうな記事だけで取る。
    let person_lengths: Vec<usize> = target
        .documents
        .iter()
        .filter_map(|d| measurable_length(d.chars, d.commas))
        .collect();

    let first = select_baseline(target, baseline, tuning.register, None);
    // 読み戻す口でも断るが、組み立ては渡された統計値を信じない。 束ねてからでは、
    // 同じ名前の 2 本が 1 つの束の中に入ったとき重なりが見えない。
    for side in [target, baseline] {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        if let Some(d) = side.documents.iter().find(|d| !seen.insert(&d.name)) {
            return Assembly {
                register: first.register,
                picked: Vec::new(),
                length_window: None,
                bundles: Vec::new(),
                tried: 0,
                outcome: Err(Stopped {
                    error: ScaleError::DuplicateName(d.name.clone()),
                    person: Vec::new(),
                    baseline: Vec::new(),
                }),
            };
        }
    }
    let mut round = try_selection(target, &person_lengths, first.picked);
    let mut register = first.register;
    let mut length_window = None;
    if matches!(round.built, Err(ScaleError::LengthRange { .. })) {
        if let Some(window) = window_of(&person_lengths) {
            let again = select_baseline(target, baseline, tuning.register, Some(window.clone()));
            if again.picked.len() >= crate::split::UNITS_FLOOR {
                let tried = round.tried;
                round = try_selection(target, &person_lengths, again.picked);
                round.tried += tried;
                register = again.register;
                length_window = Some(window);
            }
        }
    }

    let Round {
        picked,
        groups,
        built,
        tried,
    } = round;
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
        register,
        picked: picked.iter().map(|d| d.name.clone()).collect(),
        length_window,
        bundles: groups
            .iter()
            .map(|g| g.iter().map(|d| d.name.clone()).collect())
            .collect(),
        tried,
        outcome,
    }
}

/// 本人の測れそうな記事の長さの範囲。1 本も無ければ `None`。
fn window_of(person_lengths: &[usize]) -> Option<RangeInclusive<usize>> {
    Some(*person_lengths.iter().min()?..=*person_lengths.iter().max()?)
}

/// 選んだ基準の文書で、束ね方を順に試した結果。
struct Round<'a> {
    picked: Vec<&'a DocumentStats>,
    /// 最後に試した束ね方。
    groups: Vec<Vec<&'a DocumentStats>>,
    built: Result<Scale, ScaleError>,
    tried: usize,
}

fn try_selection<'a>(
    target: &CassetteStats,
    person_lengths: &[usize],
    picked: Vec<&'a DocumentStats>,
) -> Round<'a> {
    let pool_lengths: Vec<usize> = picked.iter().map(|d| d.chars).collect();
    let plans = bundle_plans(person_lengths, &pool_lengths);

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
                .map(|g| (baseline_key(&bundle_key(g)), compose(g)))
                .collect(),
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
    Round {
        picked,
        groups,
        built,
        tried,
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
    // 自己検査は組み立ての中の名前で引く。 見せる名前では、束と同じ名前の文書が重なる。
    let side = |role: Side, units: Vec<(String, Measurements)>| -> Vec<(String, f64)> {
        units
            .into_iter()
            .filter(|(n, _)| measured_in_self_check(&scale, role, n))
            .filter_map(|(n, m)| matching_of_unit(&scale, &m).map(|v| (n, v)))
            .collect()
    };
    let mine = side(Side::Person, person_units(target));
    let theirs: Vec<(String, f64)> = side(
        Side::Baseline,
        groups
            .iter()
            .map(|g| (baseline_key(&bundle_key(g)), compose(g)))
            .collect(),
    )
    .into_iter()
    .map(|(n, v)| (baseline_name(&n), v))
    .collect();
    let self_check = SelfCheck::of(&mine, &theirs);

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

    Built {
        scale,
        effective,
        self_check,
        phrase_ceilings: target.phrase_ceilings(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kakiburi_doc::Document;
    use kakiburi_metrics::directive;
    use kakiburi_metrics::lexicon::Lexicon;
    use kakiburi_metrics::morph::Analyzed;

    use crate::assemble::{lexicon_of, measure_against, Sample};
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
    fn 本人のまとめ方で測った統計値からは束ねずに組み立てた目盛りと同じ目盛りができる() {
        // 統計値の経路が正しいことの確かめである。 語のまとめ方を本人のもので揃え、
        // 束ねない素材なら、選ぶ段と束ねる段を通しても中身はビットまで一致する。
        // 効くかの判定と自己検査は、本文から直接測った値と照らす。
        for n in [10, 14] {
            let m = Fixture::new(n);
            let (person, baseline) = (samples(&m.person), samples(&m.baseline));
            let old = m.built(Some(&Chars)).expect("目盛りができる");

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
    fn 役を分ける前置と同じ名前が本人の側にあれば止まる() {
        // 名前で値を引くので、重なれば片方の値がもう片方で黙って置き換わる。
        // 読み戻す口でも断るが、組み立ては渡された統計値を信じない。
        let m = Fixture::new(10);
        let mut target = stats(&m.person);
        for (d, (b, _)) in target.documents.iter_mut().zip(&m.baseline) {
            d.name = baseline_key(b);
        }
        let a = assemble_stats(
            &target,
            &stats(&m.baseline),
            Tuning::default(),
            &by_appearance,
        );
        let stopped = a.outcome.expect_err("止まる");
        assert!(
            matches!(stopped.error, ScaleError::DuplicateName(_)),
            "{}",
            stopped.error
        );
    }

    #[test]
    fn 本人の側に同じ名前が_2_つあれば止まる() {
        let m = Fixture::new(10);
        let mut target = stats(&m.person);
        target.documents[1].name = target.documents[0].name.clone();
        let a = assemble_stats(
            &target,
            &stats(&m.baseline),
            Tuning::default(),
            &by_appearance,
        );
        let stopped = a.outcome.expect_err("止まる");
        assert!(
            matches!(stopped.error, ScaleError::DuplicateName(_)),
            "{}",
            stopped.error
        );
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
        assert_eq!(a.length_window, None, "長い側は束ねて届かせる");
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
    fn 束の名前と同じ名前の文書が基準にあっても重ならない() {
        // `a` と `b` の束を `a+b` で引けば、`a+b` という 1 本と重なって止まる。
        let person: Vec<(String, Document)> = (0..10)
            .map(|i| (format!("p{i:02}"), document(i * 3, false)))
            .collect();
        let baseline: Vec<(String, Document)> = (0..18)
            .map(|i| (format!("b{i:02}"), document(i % 9, true)))
            .collect();
        let target = stats(&person);
        let mut pool = stats(&baseline);
        let before = assemble_stats(&target, &pool, Tuning::default(), &by_appearance);
        let bundle = before
            .bundles
            .iter()
            .find(|g| g.len() > 1)
            .expect("束がある")
            .clone();
        let single = before
            .bundles
            .iter()
            .find(|g| g.len() == 1)
            .expect("束ねない 1 本がある")[0]
            .clone();
        let joined = bundle.join("+");
        // 並びを変えないよう、統計値の上で名前だけ替える。
        pool.documents
            .iter_mut()
            .find(|d| d.name == single)
            .expect("ある")
            .name
            .clone_from(&joined);
        let after = assemble_stats(&target, &pool, Tuning::default(), &by_appearance);
        assert!(after.bundles.contains(&bundle), "{:?}", after.bundles);
        assert!(
            after.bundles.contains(&vec![joined.clone()]),
            "{:?}",
            after.bundles
        );
        if let Err(stopped) = &after.outcome {
            assert!(
                !matches!(stopped.error, ScaleError::DuplicateName(_)),
                "{}",
                stopped.error
            );
        }
        assert_eq!(
            before.outcome.is_ok(),
            after.outcome.is_ok(),
            "名前を替えただけで結果は変わらない"
        );
    }

    #[test]
    fn 基準の側に同じ名前が_2_つあれば束ねる前に止まる() {
        // 同じ名前の 2 本が 1 つの束に入れば、束ねた後では重なりが見えない。
        let m = Fixture::new(10);
        let mut pool = stats(&m.baseline);
        pool.documents[1].name = pool.documents[0].name.clone();
        let a = assemble_stats(&stats(&m.person), &pool, Tuning::default(), &by_appearance);
        let stopped = a.outcome.expect_err("止まる");
        assert!(
            matches!(stopped.error, ScaleError::DuplicateName(_)),
            "{}",
            stopped.error
        );
        assert_eq!(a.tried, 0, "束ね方を試さない");
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
    fn 長さで断られなければ選び直さない() {
        // 窓で選び直すのは長さで断られたときだけである。 断られない本人には、
        // これまでと同じ基準の文書を使う。
        let m = Fixture::new(10);
        let (target, baseline) = (stats(&m.person), stats(&m.baseline));
        let a = assemble_stats(&target, &baseline, Tuning::default(), &by_appearance);
        assert!(a.outcome.is_ok());
        assert_eq!(a.length_window, None);
        let before: Vec<String> = select_baseline(&target, &baseline, None, None)
            .picked
            .iter()
            .map(|d| d.name.clone())
            .collect();
        assert_eq!(a.picked, before);
    }

    #[test]
    fn 本人より短い基準を題材で選べないときは本人の長さの窓の中から選び直す() {
        // 題材の重なりは語彙の多い長い文書ほど大きく出る。 本人の記事が短いと、
        // 題材で選んだ基準が本人の最も長い記事より長くなり、束ねても長くなる
        // 一方なので、どの束ね方も長さの範囲で断られる。
        let person: Vec<(String, Document)> = (0..10)
            .map(|i| (format!("p{i:02}"), document(i, false)))
            .collect();
        let long: Vec<(String, Document)> = (0..POOL_TAKE)
            .map(|i| (format!("l{i:02}"), document(30 + i, true)))
            .collect();
        let short: Vec<(String, Document)> = (0..12)
            .map(|i| (format!("s{i:02}"), document(i % 10, true)))
            .collect();
        let pool_docs: Vec<(String, Document)> = long.iter().chain(&short).cloned().collect();
        let words = |v: &[&str]| {
            v.iter()
                .map(|w| (*w).to_owned())
                .collect::<std::collections::BTreeSet<_>>()
        };
        let mut target = stats(&person);
        for d in &mut target.documents {
            d.content_words = words(&["東京", "天気"]);
        }
        let mut pool = stats(&pool_docs);
        for d in &mut pool.documents {
            d.content_words = if d.name.starts_with('l') {
                words(&["東京", "天気"])
            } else {
                words(&["東京"])
            };
        }
        let first = select_baseline(&target, &pool, None, None);
        assert!(
            first.picked.iter().all(|d| d.name.starts_with('l')),
            "題材では長い文書が選ばれる"
        );

        let a = assemble_stats(&target, &pool, Tuning::default(), &by_appearance);
        let lengths: Vec<usize> = target
            .documents
            .iter()
            .filter_map(|d| measurable_length(d.chars, d.commas))
            .collect();
        let (lo, hi) = (
            *lengths.iter().min().expect("測れる"),
            *lengths.iter().max().expect("測れる"),
        );
        assert_eq!(a.length_window, Some(lo..=hi));
        let want: Vec<String> = short.iter().map(|(n, _)| n.clone()).collect();
        assert_eq!(a.picked, want, "窓に入る文書だけから選ぶ");
        assert!(a.tried > 1, "最初の選び方の束ね方も数える");
        if let Err(s) = &a.outcome {
            panic!("目盛りができない: {}", s.error);
        }
    }

    #[test]
    fn 窓に入る基準が下限に届かなければ最初の選び方の結果を返す() {
        // 窓の中が単位の下限より少なければ、選び直しても目盛りは作れない。
        // 長さで断られたことをそのまま言う。
        let person: Vec<(String, Document)> = (0..10)
            .map(|i| (format!("p{i:02}"), document(i, false)))
            .collect();
        let pool_docs: Vec<(String, Document)> = (0..12)
            .map(|i| (format!("l{i:02}"), document(30 + i, true)))
            .collect();
        let a = assemble_stats(
            &stats(&person),
            &stats(&pool_docs),
            Tuning::default(),
            &by_appearance,
        );
        assert_eq!(a.length_window, None);
        let stopped = a.outcome.expect_err("止まる");
        assert!(
            matches!(stopped.error, ScaleError::LengthRange { .. }),
            "{}",
            stopped.error
        );
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
