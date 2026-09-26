//! 良くなった版だけを採る（[仕様](../../../docs/spec/400-write.md#良くなった版だけを採る)）。
//!
//! 同じ目盛りで検めた 2 つの結果を、決めた順に比べ、最初に差が付いたところで決める。
//! 差が付かなければ前の版を残す。 同じ出来なら、元の文章に近いほうが内容を保って
//! いる見込みが高い。
//!
//! ここに置くのは、判定と段と知らせの意味を知っているのがこのクレートだからである。
//! 組み立て層に置けば、判定の順序が 2 か所に書かれる。

use std::cmp::Ordering;

use crate::verdict::{outside_in_order, Outcome, Stage, Verdict};
use crate::Observations;

/// 止まった段の量。段ごとに見るものが違う。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Amount {
    /// 0 段目。線を超えた検査の本数。少ないほうが良い。
    Writing {
        /// 線を超えた検査の本数。
        over: usize,
    },
    /// 1 段目。基準との距離。大きいほうが良い。
    Humanness {
        /// 基準との距離。
        distance: f64,
    },
    /// 2 段目。照合値。大きいほうが良い。
    Matching {
        /// 照合値。
        value: f64,
    },
    /// 3 段目。許す大きさを超えて幅の外にある前に出す指標。本数が少なく、
    /// 同じなら外れの大きさの合計が小さいほうが良い。
    Directive {
        /// 幅の外にある本数。
        outside: usize,
        /// 外れの大きさの合計。
        excess: f64,
    },
}

/// 比べるために版ごとに取り出したもの。
#[derive(Debug, Clone, PartialEq)]
pub struct Standing {
    /// 判定。
    pub verdict: Verdict,
    /// 止まった段。
    pub stage: Stage,
    /// 止まった段の量。
    pub amount: Amount,
    /// 本人の上限を超えた言い回しの本数。
    pub overused: usize,
    /// 草稿に出ている基準の型と基準の語の出現回数の合計。
    pub baseline_times: usize,
}

/// 1 つの版。測れなかった版は、ほかの鍵を比べずに捨てる。
#[derive(Debug, Clone, PartialEq)]
pub enum Version {
    /// 測れなかった。理由を持つ。
    ///
    /// 直させた結果として測れなくなったのは、内容を削ったか壊したかである。
    Unmeasurable(String),
    /// 測れた。
    Measured(Standing),
}

/// 段の量の材料のうち、[検めの入力](Observations)が持たない生の値。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Values {
    /// 基準との距離。測れなければ `None`。
    pub humanness: Option<f64>,
    /// 照合値。系統を埋めて出したならその値。測れなければ `None`。
    pub matching: Option<f64>,
}

/// 検めた結果から、比べるものを取り出す。
///
/// 値は丸めない。`review` が出す生の値で比べる。
#[must_use]
pub fn standing(o: &Observations<'_>, values: Values, outcome: &Outcome) -> Version {
    if let Some(what) = o.too_short {
        return Version::Unmeasurable(format!("短すぎて測れない（{what}）"));
    }
    let unmeasured = || {
        Version::Unmeasurable(format!(
            "系統が欠けて{}の段が測れない",
            outcome.stage.name()
        ))
    };
    let amount = match outcome.stage {
        Stage::Writing => Amount::Writing {
            over: o.inspections.iter().filter(|i| i.over()).count(),
        },
        Stage::Humanness => match values.humanness {
            Some(distance) => Amount::Humanness { distance },
            None => return unmeasured(),
        },
        Stage::Matching => match values.matching {
            Some(value) => Amount::Matching { value },
            None => return unmeasured(),
        },
        Stage::Directive => {
            let outside = outside_in_order(o.directives);
            Amount::Directive {
                outside: outside.len(),
                excess: outside.iter().map(|(_, loc)| loc.size()).sum(),
            }
        }
    };
    let baseline_times = o
        .machine_katas
        .iter()
        .filter(|k| k.used)
        .map(|k| k.times)
        .chain(o.machine_gois.iter().map(|g| g.times))
        .sum();
    Version::Measured(Standing {
        verdict: outcome.verdict,
        stage: outcome.stage,
        amount,
        overused: crate::over_used(o.phrases).len(),
        baseline_times,
    })
}

fn verdict_rank(v: Verdict) -> u8 {
    match v {
        Verdict::Fail => 0,
        Verdict::Unknown => 1,
        Verdict::Pass => 2,
    }
}

fn stage_rank(s: Stage) -> u8 {
    match s {
        Stage::Writing => 0,
        Stage::Humanness => 1,
        Stage::Matching => 2,
        Stage::Directive => 3,
    }
}

/// 大きいほうが良い値を比べる。比べられない値（NaN）は差が付かないとする。
fn larger(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

/// 小さいほうが良い値を比べる。
fn smaller(a: f64, b: f64) -> Ordering {
    larger(b, a)
}

/// 同じ段で止まった 2 つの量を比べる。段が違えばここへは来ない。
fn amount(a: Amount, b: Amount) -> Ordering {
    match (a, b) {
        (Amount::Writing { over: x }, Amount::Writing { over: y }) => y.cmp(&x),
        (Amount::Humanness { distance: x }, Amount::Humanness { distance: y })
        | (Amount::Matching { value: x }, Amount::Matching { value: y }) => larger(x, y),
        (
            Amount::Directive {
                outside: n,
                excess: x,
            },
            Amount::Directive {
                outside: m,
                excess: y,
            },
        ) => m.cmp(&n).then_with(|| smaller(x, y)),
        _ => Ordering::Equal,
    }
}

/// `a` が `b` より良ければ `Greater`。最初に差が付いた鍵で決める。
#[must_use]
pub fn compare(a: &Standing, b: &Standing) -> Ordering {
    verdict_rank(a.verdict)
        .cmp(&verdict_rank(b.verdict))
        .then_with(|| stage_rank(a.stage).cmp(&stage_rank(b.stage)))
        .then_with(|| {
            if a.stage == b.stage {
                amount(a.amount, b.amount)
            } else {
                Ordering::Equal
            }
        })
        .then_with(|| b.overused.cmp(&a.overused))
        .then_with(|| b.baseline_times.cmp(&a.baseline_times))
}

/// 直した版を採るか。それまでに採った版より良くなったときだけ採る。
///
/// 差が付かなければ採らない。 測れなかった版は、ほかの鍵を比べずに捨てる。
#[must_use]
pub fn adopt(candidate: &Version, kept: &Standing) -> bool {
    match candidate {
        Version::Unmeasurable(_) => false,
        Version::Measured(s) => compare(s, kept) == Ordering::Greater,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::range::{Lower, Range};
    use crate::verdict::{Inspected, Observed};
    use crate::{Goi, Kata};

    fn at(verdict: Verdict, stage: Stage, amount: Amount) -> Standing {
        Standing {
            verdict,
            stage,
            amount,
            overused: 0,
            baseline_times: 0,
        }
    }

    fn humanness(d: f64) -> Amount {
        Amount::Humanness { distance: d }
    }

    #[test]
    fn 判定は通るが判定できないより判定できないが通らないより良い() {
        let pass = at(
            Verdict::Pass,
            Stage::Directive,
            Amount::Directive {
                outside: 0,
                excess: 0.0,
            },
        );
        let unknown = at(
            Verdict::Unknown,
            Stage::Directive,
            Amount::Directive {
                outside: 1,
                excess: 0.5,
            },
        );
        let fail = at(
            Verdict::Fail,
            Stage::Matching,
            Amount::Matching { value: 9.0 },
        );
        assert_eq!(compare(&pass, &unknown), Ordering::Greater);
        assert_eq!(compare(&unknown, &fail), Ordering::Greater);
        assert_eq!(compare(&fail, &pass), Ordering::Less);
    }

    #[test]
    fn 判定が同じなら後ろの段で止まったほうが良い() {
        // 量は見ない。 段が違えば量の意味が違う。
        let later = at(
            Verdict::Fail,
            Stage::Matching,
            Amount::Matching { value: -5.0 },
        );
        let earlier = at(Verdict::Fail, Stage::Humanness, humanness(100.0));
        let first = at(Verdict::Fail, Stage::Writing, Amount::Writing { over: 0 });
        assert_eq!(compare(&later, &earlier), Ordering::Greater);
        assert_eq!(compare(&earlier, &first), Ordering::Greater);
        let directive = at(
            Verdict::Unknown,
            Stage::Directive,
            Amount::Directive {
                outside: 3,
                excess: 9.0,
            },
        );
        let matching = at(
            Verdict::Unknown,
            Stage::Matching,
            Amount::Matching { value: 9.0 },
        );
        assert_eq!(compare(&directive, &matching), Ordering::Greater);
    }

    #[test]
    fn 検査で止まったなら線を超えた本数が少ないほうが良い() {
        let one = at(Verdict::Fail, Stage::Writing, Amount::Writing { over: 1 });
        let two = at(Verdict::Fail, Stage::Writing, Amount::Writing { over: 2 });
        assert_eq!(compare(&one, &two), Ordering::Greater);
    }

    #[test]
    fn 基準との距離で止まったなら距離が大きいほうが良い() {
        let far = at(Verdict::Fail, Stage::Humanness, humanness(-0.1));
        let near = at(Verdict::Fail, Stage::Humanness, humanness(-0.2));
        assert_eq!(compare(&far, &near), Ordering::Greater);
    }

    #[test]
    fn 照合値で止まったなら照合値が大きいほうが良い() {
        let high = at(
            Verdict::Unknown,
            Stage::Matching,
            Amount::Matching { value: 1.2 },
        );
        let low = at(
            Verdict::Unknown,
            Stage::Matching,
            Amount::Matching { value: 1.1 },
        );
        assert_eq!(compare(&high, &low), Ordering::Greater);
    }

    #[test]
    fn 指示できる指標で止まったなら外れた本数が少なく同じなら外れの合計が小さいほうが良い() {
        let fewer = at(
            Verdict::Unknown,
            Stage::Directive,
            Amount::Directive {
                outside: 1,
                excess: 5.0,
            },
        );
        let more = at(
            Verdict::Unknown,
            Stage::Directive,
            Amount::Directive {
                outside: 2,
                excess: 0.5,
            },
        );
        assert_eq!(compare(&fewer, &more), Ordering::Greater);
        let smaller = at(
            Verdict::Unknown,
            Stage::Directive,
            Amount::Directive {
                outside: 2,
                excess: 0.4,
            },
        );
        assert_eq!(compare(&smaller, &more), Ordering::Greater);
    }

    #[test]
    fn 量が同じなら上限を超えた言い回しの本数が少ないほうが良い() {
        let mut a = at(Verdict::Fail, Stage::Humanness, humanness(0.5));
        let mut b = a.clone();
        a.overused = 1;
        b.overused = 2;
        b.baseline_times = 0;
        a.baseline_times = 9;
        assert_eq!(
            compare(&a, &b),
            Ordering::Greater,
            "基準の言い回しより先に見る"
        );
    }

    #[test]
    fn 上限も同じなら基準の型と語の出現回数の合計が小さいほうが良い() {
        let mut a = at(Verdict::Fail, Stage::Humanness, humanness(0.5));
        let mut b = a.clone();
        a.baseline_times = 3;
        b.baseline_times = 4;
        assert_eq!(compare(&a, &b), Ordering::Greater);
    }

    #[test]
    fn どれも同じなら採らない() {
        // 同じ出来なら、元の文章に近いほうが内容を保っている見込みが高い。
        let kept = at(Verdict::Fail, Stage::Humanness, humanness(0.5));
        assert_eq!(compare(&kept, &kept), Ordering::Equal);
        assert!(!adopt(&Version::Measured(kept.clone()), &kept));
    }

    #[test]
    fn 良くなった版は採り悪くなった版は採らない() {
        let kept = at(Verdict::Fail, Stage::Humanness, humanness(0.5));
        let better = at(Verdict::Fail, Stage::Humanness, humanness(0.6));
        let worse = at(Verdict::Fail, Stage::Humanness, humanness(0.4));
        assert!(adopt(&Version::Measured(better), &kept));
        assert!(!adopt(&Version::Measured(worse), &kept));
    }

    #[test]
    fn 測れなかった版はほかの鍵を見ずに捨てる() {
        let kept = at(Verdict::Fail, Stage::Writing, Amount::Writing { over: 3 });
        assert!(!adopt(
            &Version::Unmeasurable("短すぎて測れない".into()),
            &kept
        ));
    }

    fn observations<'a>(
        inspections: &'a [Inspected],
        directives: &'a [Observed],
        too_short: Option<&'a str>,
    ) -> Observations<'a> {
        Observations {
            inspections,
            humanness: Some(crate::Side::Human),
            matching: Some(crate::Side::Human),
            matching_substituted: None,
            too_short,
            directives,
            directives_muted: false,
            habits: &[],
            humanness_by_metric: &[],
            diverging: &[],
            katas: &[],
            machine_katas: &[],
            machine_gois: &[],
            phrases: &[],
            first_person: &[],
            draft_first_person: &[],
            opening: &[],
            draft_opening: None,
        }
    }

    fn outcome(verdict: Verdict, stage: Stage) -> Outcome {
        Outcome {
            verdict,
            stage,
            reason: String::new(),
        }
    }

    #[test]
    fn 短すぎる版は測れない() {
        let o = observations(&[], &[], Some("延べ 10 語 / 下限 729 語"));
        let v = standing(
            &o,
            Values::default(),
            &outcome(Verdict::Fail, Stage::Writing),
        );
        assert!(
            matches!(v, Version::Unmeasurable(ref why) if why.contains("短すぎて")),
            "{v:?}"
        );
    }

    #[test]
    fn 止まった段の値が出ていなければ測れない() {
        let o = observations(&[], &[], None);
        for stage in [Stage::Humanness, Stage::Matching] {
            let v = standing(&o, Values::default(), &outcome(Verdict::Unknown, stage));
            assert!(matches!(v, Version::Unmeasurable(_)), "{stage:?}: {v:?}");
        }
    }

    #[test]
    fn 段の量は止まった段から取る() {
        let inspections = [Inspected {
            name: "和文間スペース".into(),
            value: Some(3.0),
            limit: 1.0,
            upper: true,
            broken: String::new(),
        }];
        let o = observations(&inspections, &[], None);
        let v = standing(
            &o,
            Values::default(),
            &outcome(Verdict::Fail, Stage::Writing),
        );
        assert!(
            matches!(
                v,
                Version::Measured(Standing {
                    amount: Amount::Writing { over: 1 },
                    ..
                })
            ),
            "{v:?}"
        );

        let o = observations(&[], &[], None);
        let values = Values {
            humanness: Some(0.7),
            matching: Some(1.3),
        };
        let v = standing(&o, values, &outcome(Verdict::Fail, Stage::Matching));
        assert!(
            matches!(v, Version::Measured(Standing { amount: Amount::Matching { value }, .. }) if (value - 1.3).abs() < 1e-12),
            "{v:?}"
        );
    }

    #[test]
    fn 指示できる指標の量は許す大きさを超えて外にある本数と外れの合計である() {
        let observed = |name: &str, value: f64| Observed {
            name: name.into(),
            value: Some(value),
            range: Range {
                low: 1.0,
                high: 2.0,
                units: 10,
            },
            lower: Lower::Spread,
            direct: false,
        };
        let directives = [observed("a", 1.5), observed("b", 5.0), observed("c", 9.0)];
        let o = observations(&[], &directives, None);
        let v = standing(
            &o,
            Values::default(),
            &outcome(Verdict::Unknown, Stage::Directive),
        );
        let Version::Measured(Standing {
            amount: Amount::Directive { outside, excess },
            ..
        }) = v
        else {
            panic!("{v:?}");
        };
        assert_eq!(outside, 2);
        assert!(excess > 0.0);
    }

    #[test]
    fn 知らせの量は使いすぎと基準の言い回しの回数から取る() {
        let kata = |text: &str, used: bool, times: usize, density: f64, ceiling: f64| Kata {
            text: text.into(),
            rate: 0.5,
            at: 0.5,
            used,
            spots: Vec::new(),
            times,
            density,
            ceiling,
            base: 0.0,
        };
        let phrases = [
            kata("と思います。", true, 5, 4.0, 1.0),
            kata("ですね。", true, 1, 0.5, 1.0),
        ];
        let machine_katas = [
            kata("のではなく、", true, 2, 0.0, 0.0),
            kata("と言えます。", false, 0, 0.0, 0.0),
        ];
        let machine_gois = [Goi {
            text: "地味".into(),
            rate: 0.2,
            base: 0.0,
            times: 3,
            theirs: Vec::new(),
        }];
        let mut o = observations(&[], &[], None);
        o.phrases = &phrases;
        o.machine_katas = &machine_katas;
        o.machine_gois = &machine_gois;
        let values = Values {
            humanness: Some(0.1),
            matching: None,
        };
        let Version::Measured(s) = standing(&o, values, &outcome(Verdict::Fail, Stage::Humanness))
        else {
            panic!("測れる");
        };
        assert_eq!(s.overused, 1);
        assert_eq!(s.baseline_times, 5);
    }
}
