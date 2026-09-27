//! 良くなった版だけを採る（[仕様](../../../docs/spec/400-write.md#良くなった版だけを採る)）。
//!
//! 同じ目盛りで検めた 2 つの結果を、決めた順に比べ、最初に差が付いたところで決める。
//! 差が付かなければ前の版を残す。 同じ出来なら、元の文章に近いほうが内容を保って
//! いる見込みが高い。
//!
//! 比べる 2 つは対等ではない。 片方は直した版、もう片方はそれまでに採った版である。
//! 使いすぎを増やしたかは、直した版の側だけを止める。
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
    /// 本人の上限を超えた言い回しと、この長さで使える回数を超えた分。文字列の順。
    pub overused: Vec<(String, usize)>,
    /// 草稿に出ている基準の型と基準の語の出現回数の合計。
    pub baseline_times: usize,
}

impl Standing {
    /// 使いすぎの本数と、超えた回数の合計。
    #[must_use]
    pub fn overuse(&self) -> Overuse {
        Overuse {
            phrases: self.overused.len(),
            times: self.overused.iter().map(|(_, n)| n).sum(),
        }
    }
}

/// 使いすぎの大きさ。本数が少なく、同じなら超えた回数の合計が少ないほうが良い。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Overuse {
    /// 本人の上限を超えた言い回しの本数。
    pub phrases: usize,
    /// この長さで使える回数を超えた分の合計。
    pub times: usize,
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
        overused: crate::overuse(o.phrases),
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

/// 差が付いた鍵と、そこでの 2 つの版の値。鍵は比べる順に並ぶ。
///
/// `before` は比べられる側（それまでに採った版）、`after` は比べる側（直した版）である。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Key {
    /// 判定。
    Verdict {
        /// 比べられる側。
        before: Verdict,
        /// 比べる側。
        after: Verdict,
    },
    /// 止まった段。
    Stage {
        /// 比べられる側。
        before: Stage,
        /// 比べる側。
        after: Stage,
    },
    /// 直した版が使いすぎを増やした。 増やしていなければ差は付かない。
    OveruseGrew {
        /// 比べられる側。
        before: Overuse,
        /// 比べる側。
        after: Overuse,
    },
    /// 止まった段の量。段は同じである。
    Amount {
        /// 比べられる側。
        before: Amount,
        /// 比べる側。
        after: Amount,
    },
    /// 使いすぎの大きさ。
    Overused {
        /// 比べられる側。
        before: Overuse,
        /// 比べる側。
        after: Overuse,
    },
    /// 基準の型と基準の語の出現回数の合計。
    BaselineTimes {
        /// 比べられる側。
        before: usize,
        /// 比べる側。
        after: usize,
    },
}

/// 最初に差が付いた鍵と、`a` が `b` より良ければ `Greater`。どこにも差が付かなければ `None`。
///
/// `a` は直した版、`b` はそれまでに採った版である。 使いすぎを増やしたかは `a` の側だけを
/// 止めるので、`a` と `b` を入れ替えても逆の答えになるとは限らない。
///
/// 比べる順はここにだけ書く。 [`compare`] もここを通す。 採らなかった理由を言う側が
/// 別に順を持てば、採否と理由が食い違う。
///
/// 使いすぎを増やしたかは、止まった段の量より先に見る。 基準との距離は同じ言い回しを
/// 重ねれば上がり、文末を揃えるのがいちばん安い。 量を先に見れば、本人に寄らずに
/// 距離だけを上げた版を採る。 判定と段より後に見るのは、判定に使わない知らせで、
/// 検めの答えが良くなった版を捨てないためである。
#[must_use]
pub fn decisive(a: &Standing, b: &Standing) -> Option<(Key, Ordering)> {
    let (grew, kept) = (a.overuse(), b.overuse());
    let keys = [
        (
            Key::Verdict {
                before: b.verdict,
                after: a.verdict,
            },
            verdict_rank(a.verdict).cmp(&verdict_rank(b.verdict)),
        ),
        (
            Key::Stage {
                before: b.stage,
                after: a.stage,
            },
            stage_rank(a.stage).cmp(&stage_rank(b.stage)),
        ),
        (
            Key::OveruseGrew {
                before: kept,
                after: grew,
            },
            // 片側だけを見る。 減らしたことは、ここでは差にしない。 使いすぎを減らす直しは
            // 基準との距離を下げることが多く、ここで採れば距離を下げる直しを採る。
            if grew > kept {
                Ordering::Less
            } else {
                Ordering::Equal
            },
        ),
        (
            Key::Amount {
                before: b.amount,
                after: a.amount,
            },
            if a.stage == b.stage {
                amount(a.amount, b.amount)
            } else {
                Ordering::Equal
            },
        ),
        (
            Key::Overused {
                before: kept,
                after: grew,
            },
            kept.cmp(&grew),
        ),
        (
            Key::BaselineTimes {
                before: b.baseline_times,
                after: a.baseline_times,
            },
            b.baseline_times.cmp(&a.baseline_times),
        ),
    ];
    keys.into_iter().find(|(_, o)| *o != Ordering::Equal)
}

/// 直した版 `a` が、それまでに採った版 `b` より良ければ `Greater`。最初に差が付いた鍵で決める。
#[must_use]
pub fn compare(a: &Standing, b: &Standing) -> Ordering {
    decisive(a, b).map_or(Ordering::Equal, |(_, o)| o)
}

fn verdict_name(v: Verdict) -> &'static str {
    match v {
        Verdict::Pass => "通る",
        Verdict::Fail => "通らない",
        Verdict::Unknown => "判定できない",
    }
}

/// 直した版を採らなかった理由を 1 文で言う。直させるプロンプトに載せる。
///
/// 値は `review` の散文と同じく小数 3 桁で書く。 比べたのは丸めない値なので、
/// 丸めた 2 つが同じに見えることはある。
#[must_use]
pub fn rejection(candidate: &Version, kept: &Standing) -> String {
    let s = match candidate {
        Version::Unmeasurable(why) => return format!("測れなかった（{why}）"),
        Version::Measured(s) => s,
    };
    let Some((key, _)) = decisive(s, kept) else {
        return "どの比べ方でも差が付かなかった。差が付かなければ前の版を残す".to_owned();
    };
    let moved = |more_is_better: bool, before: f64, after: f64, what: &str| {
        let dir = if after < before {
            "下がった"
        } else {
            "上がった"
        };
        let good = if more_is_better {
            "大きい"
        } else {
            "小さい"
        };
        format!("{what}が {before:.3} から {after:.3} に{dir}。{good}ほうが良い")
    };
    let changed = |before: usize, after: usize, what: &str, unit: &str| {
        let dir = if after < before {
            "減った"
        } else {
            "増えた"
        };
        format!("{what}が {before} {unit}から {after} {unit}に{dir}")
    };
    let count = |before: usize, after: usize, what: &str, unit: &str| {
        format!("{}。少ないほうが良い", changed(before, after, what, unit))
    };
    // 本数が同じなら、超えた回数で言う。 差が付いたのがそこだからである。
    let overuse = |before: Overuse, after: Overuse| {
        if after.phrases == before.phrases {
            changed(before.times, after.times, "本人の上限を超えた回数", "回")
        } else {
            changed(
                before.phrases,
                after.phrases,
                "本人の上限を超えた言い回し",
                "本",
            )
        }
    };
    match key {
        Key::Verdict { before, after } => format!(
            "判定が「{}」から「{}」に下がった",
            verdict_name(before),
            verdict_name(after)
        ),
        Key::Stage { before, after } => format!(
            "止まった段が「{}」から手前の「{}」に戻った",
            before.name(),
            after.name()
        ),
        Key::Amount { before, after } => match (before, after) {
            (Amount::Writing { over: b }, Amount::Writing { over: a }) => {
                count(b, a, "線を超えた検査", "本")
            }
            (Amount::Humanness { distance: b }, Amount::Humanness { distance: a }) => {
                moved(true, b, a, "基準との距離")
            }
            (Amount::Matching { value: b }, Amount::Matching { value: a }) => {
                moved(true, b, a, "照合値")
            }
            (
                Amount::Directive {
                    outside: bo,
                    excess: be,
                },
                Amount::Directive {
                    outside: ao,
                    excess: ae,
                },
            ) => {
                if bo == ao {
                    moved(false, be, ae, "幅の外にある前に出す指標の外れの合計")
                } else {
                    count(bo, ao, "幅の外にある前に出す指標", "本")
                }
            }
            _ => unreachable!("段が同じときだけ量で差が付く"),
        },
        Key::OveruseGrew { before, after } => {
            let which = grown(s, kept);
            let which = if which.is_empty() {
                String::new()
            } else {
                format!("（{which}）")
            };
            format!(
                "{}{which}。使いすぎを増やした版は、止まった段の量が良くなっても採らない",
                overuse(before, after)
            )
        }
        Key::Overused { before, after } => format!(
            "止まった段の量では差が付かず、{}。少ないほうが良い",
            overuse(before, after)
        ),
        Key::BaselineTimes { before, after } => format!(
            "止まった段の量では差が付かず、{}",
            count(before, after, "基準の型と基準の語の出現回数", "回")
        ),
    }
}

/// 直した版で、使いすぎを増やした言い回し。新たに超えたか、超えた分が増えたもの。文字列の順。
fn grown(candidate: &Standing, kept: &Standing) -> String {
    candidate
        .overused
        .iter()
        .filter(|(text, n)| {
            kept.overused
                .iter()
                .find(|(k, _)| k == text)
                .is_none_or(|(_, m)| n > m)
        })
        .map(|(text, _)| format!("「{text}」"))
        .collect::<Vec<_>>()
        .join("")
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
            overused: Vec::new(),
            baseline_times: 0,
        }
    }

    fn humanness(d: f64) -> Amount {
        Amount::Humanness { distance: d }
    }

    fn overused(v: &[(&str, usize)]) -> Vec<(String, usize)> {
        v.iter().map(|(t, n)| ((*t).to_owned(), *n)).collect()
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
        a.overused = overused(&[("しています。", 1)]);
        b.overused = overused(&[("しています。", 1), ("になります。", 1)]);
        b.baseline_times = 0;
        a.baseline_times = 9;
        assert_eq!(
            compare(&a, &b),
            Ordering::Greater,
            "基準の言い回しより先に見る"
        );
    }

    #[test]
    fn 本数も同じなら上限を超えた回数の合計が少ないほうが良い() {
        let mut a = at(Verdict::Fail, Stage::Humanness, humanness(0.5));
        let mut b = a.clone();
        a.overused = overused(&[("しています。", 1)]);
        b.overused = overused(&[("しています。", 3)]);
        assert_eq!(compare(&a, &b), Ordering::Greater);
    }

    #[test]
    fn 基準との距離が上がっても使いすぎの言い回しを増やした版は採らない() {
        let kept = at(Verdict::Fail, Stage::Humanness, humanness(-0.961));
        let mut ending = at(Verdict::Fail, Stage::Humanness, humanness(-0.948));
        ending.overused = overused(&[("しています。", 2)]);
        assert!(!adopt(&Version::Measured(ending.clone()), &kept));
        assert_eq!(
            rejection(&Version::Measured(ending), &kept),
            "本人の上限を超えた言い回しが 0 本から 1 本に増えた（「しています。」）。\
             使いすぎを増やした版は、止まった段の量が良くなっても採らない"
        );
    }

    #[test]
    fn 基準との距離が上がっても上限を超えた回数を増やした版は採らない() {
        let mut kept = at(Verdict::Fail, Stage::Humanness, humanness(-0.961));
        kept.overused = overused(&[("しています。", 1)]);
        let mut more = at(Verdict::Fail, Stage::Humanness, humanness(-0.9));
        more.overused = overused(&[("しています。", 4)]);
        assert!(!adopt(&Version::Measured(more.clone()), &kept));
        assert_eq!(
            rejection(&Version::Measured(more), &kept),
            "本人の上限を超えた回数が 1 回から 4 回に増えた（「しています。」）。\
             使いすぎを増やした版は、止まった段の量が良くなっても採らない"
        );
    }

    #[test]
    fn 使いすぎを増やしていなければ基準との距離が上がった版を採る() {
        let mut kept = at(Verdict::Fail, Stage::Humanness, humanness(-0.961));
        kept.overused = overused(&[("しています。", 2)]);
        let mut same = at(Verdict::Fail, Stage::Humanness, humanness(-0.948));
        same.overused = overused(&[("しています。", 2)]);
        assert!(adopt(&Version::Measured(same), &kept));
    }

    #[test]
    fn 使いすぎを減らしただけでは基準との距離が下がった版を採らない() {
        // 片側だけを見る。 使いすぎを減らす直しは距離を下げることが多い。
        let mut kept = at(Verdict::Fail, Stage::Humanness, humanness(-0.948));
        kept.overused = overused(&[("しています。", 2)]);
        let fewer = at(Verdict::Fail, Stage::Humanness, humanness(-0.992));
        assert!(!adopt(&Version::Measured(fewer.clone()), &kept));
        assert!(rejection(&Version::Measured(fewer), &kept).starts_with("基準との距離が"));
    }

    #[test]
    fn 後ろの段に進んだ版と判定が上がった版は使いすぎを増やしても採る() {
        // 判定と段は検めの答えそのものである。 判定に使わない知らせで捨てない。
        let kept = at(Verdict::Fail, Stage::Humanness, humanness(-0.1));
        let mut later = at(
            Verdict::Fail,
            Stage::Matching,
            Amount::Matching { value: -0.5 },
        );
        later.overused = overused(&[("しています。", 3)]);
        assert!(adopt(&Version::Measured(later), &kept));
        let mut passed = at(
            Verdict::Pass,
            Stage::Directive,
            Amount::Directive {
                outside: 0,
                excess: 0.0,
            },
        );
        passed.overused = overused(&[("しています。", 3)]);
        assert!(adopt(&Version::Measured(passed), &kept));
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

    #[test]
    fn 採らなかった理由は差が付いた鍵とその前後の値を言う() {
        let kept = at(Verdict::Fail, Stage::Humanness, humanness(-0.957));
        let worse = at(Verdict::Fail, Stage::Humanness, humanness(-0.997_2));
        assert_eq!(
            rejection(&Version::Measured(worse), &kept),
            "基準との距離が -0.957 から -0.997 に下がった。大きいほうが良い"
        );
    }

    #[test]
    fn 採らなかった理由は比べる順の最初の鍵で言う() {
        let kept = at(
            Verdict::Unknown,
            Stage::Matching,
            Amount::Matching { value: 1.0 },
        );
        let earlier = at(Verdict::Unknown, Stage::Humanness, humanness(9.0));
        assert_eq!(
            rejection(&Version::Measured(earlier), &kept),
            "止まった段が「照合値」から手前の「基準との距離」に戻った"
        );
        let failed = at(
            Verdict::Fail,
            Stage::Matching,
            Amount::Matching { value: 9.0 },
        );
        assert_eq!(
            rejection(&Version::Measured(failed), &kept),
            "判定が「判定できない」から「通らない」に下がった"
        );
    }

    #[test]
    fn 量で差が付かなければ知らせの鍵で言う() {
        let kept = at(Verdict::Fail, Stage::Humanness, humanness(0.5));
        let mut fewer = kept.clone();
        fewer.overused = overused(&[("しています。", 1)]);
        let mut kept_over = kept.clone();
        kept_over.overused = overused(&[("しています。", 1), ("になります。", 1)]);
        assert_eq!(
            decisive(&kept_over, &fewer).map(|(k, _)| k),
            Some(Key::OveruseGrew {
                before: Overuse {
                    phrases: 1,
                    times: 1
                },
                after: Overuse {
                    phrases: 2,
                    times: 2
                },
            }),
            "増やしたほうは量より先に止まる"
        );
        assert_eq!(
            decisive(&fewer, &kept_over),
            Some((
                Key::Overused {
                    before: Overuse {
                        phrases: 2,
                        times: 2
                    },
                    after: Overuse {
                        phrases: 1,
                        times: 1
                    },
                },
                Ordering::Greater
            )),
            "減らしたほうは量で差が付かなかったときに良い"
        );
        let mut times = kept.clone();
        times.baseline_times = 3;
        assert!(rejection(&Version::Measured(times), &kept)
            .contains("基準の型と基準の語の出現回数が 0 回から 3 回に増えた"));
    }

    #[test]
    fn 指示できる指標は本数が同じなら外れの合計で言う() {
        let d = |outside, excess| {
            at(
                Verdict::Unknown,
                Stage::Directive,
                Amount::Directive { outside, excess },
            )
        };
        assert_eq!(
            rejection(&Version::Measured(d(2, 0.1)), &d(1, 0.5)),
            "幅の外にある前に出す指標が 1 本から 2 本に増えた。少ないほうが良い"
        );
        assert_eq!(
            rejection(&Version::Measured(d(1, 0.7)), &d(1, 0.5)),
            "幅の外にある前に出す指標の外れの合計が 0.500 から 0.700 に上がった。小さいほうが良い"
        );
    }

    #[test]
    fn 差が付かなかった版と測れなかった版もそう言う() {
        let kept = at(Verdict::Fail, Stage::Humanness, humanness(0.5));
        assert!(rejection(&Version::Measured(kept.clone()), &kept).contains("差が付かなかった"));
        assert_eq!(
            rejection(&Version::Unmeasurable("短すぎて測れない".into()), &kept),
            "測れなかった（短すぎて測れない）"
        );
    }

    #[test]
    fn 差が付いた鍵は比べ方と同じ答えを返す() {
        let kept = at(Verdict::Fail, Stage::Humanness, humanness(0.5));
        let better = at(Verdict::Fail, Stage::Humanness, humanness(0.6));
        assert_eq!(
            decisive(&better, &kept),
            Some((
                Key::Amount {
                    before: humanness(0.5),
                    after: humanness(0.6)
                },
                Ordering::Greater
            ))
        );
        assert_eq!(decisive(&kept, &kept), None);
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
        assert_eq!(
            s.overused,
            vec![("と思います。".to_owned(), 4)],
            "1,250 字で上限 1.0 回なら 1 回まで"
        );
        assert_eq!(s.baseline_times, 5);
    }
}
