//! 基準の文書を、本人と文体が同じで題材の近い分だけに絞る。
//!
//! 基準の形代の文書は全部は使わない（[選び方](../../../docs/spec/200-extract.md#基準の文書は文体が同じで題材の近い分だけを使う)）。
//! 基準が同梱のものでも、人の形代でも、同じ手順を掛ける。
//!
//! 1. 文体を揃える。段落の敬体率で分け、本人の文体の分だけを候補にする。
//! 2. 候補から、本人の素材と題材の近い分を選ぶ。
//!
//! 文体を題材より先に揃えるのは、文体のずれのほうが床を大きく動かすからである。

use std::collections::BTreeSet;
use std::ops::RangeInclusive;

use crate::stats::{DocumentStats, KatashiroStats};

/// 敬体率がこれ以上なら敬体の文書とする。暫定値である。
///
/// 敬体と常体の真ん中に置いた。 本人の記事も池の文書もどちらかに大きく
/// 寄っているので、境目の位置で分け方はほとんど変わらない。
pub const POLITE_SHARE_MIN: f64 = 0.5;

/// 基準から取る本数。暫定値である。
///
/// [下限](crate::split::UNITS_FLOOR)は 10 だが、測れない分が出るので余裕を
/// 持たせる。手作りの基準 21 本で目盛りが作れていたので、そのあたりに置いた。
///
/// 多く取れば題材の遠いものが混ざり、少なく取れば本数が下限を割る。
pub const POOL_TAKE: usize = 24;

/// 文書の文体。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Register {
    /// です・ます。
    Polite,
    /// だ・である。
    Plain,
}

impl Register {
    /// 名前。
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Register::Polite => "敬体",
            Register::Plain => "常体",
        }
    }

    /// 段落の敬体率から分ける。
    ///
    /// 段落だけを見る。 同じ書き手が項目や見出しを常体で書くのは普通で、文書の
    /// 文体を決めているのは段落である。
    #[must_use]
    pub fn of_share(share: f64) -> Self {
        if share >= POLITE_SHARE_MIN {
            Register::Polite
        } else {
            Register::Plain
        }
    }
}

/// 文体ごとの本数。分けられなかった文書は数えない。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegisterCount {
    /// 敬体の本数。
    pub polite: usize,
    /// 常体の本数。
    pub plain: usize,
}

impl RegisterCount {
    /// 段落の敬体率から数える。測れない文書（`None`）は分けない。
    #[must_use]
    pub fn of(shares: impl IntoIterator<Item = Option<f64>>) -> Self {
        let mut out = Self::default();
        for share in shares.into_iter().flatten() {
            match Register::of_share(share) {
                Register::Polite => out.polite += 1,
                Register::Plain => out.plain += 1,
            }
        }
        out
    }

    /// 多い方。同数なら決めない——1 本も分けられなかったときも同数である。
    #[must_use]
    pub fn majority(self) -> Option<Register> {
        match self.polite.cmp(&self.plain) {
            std::cmp::Ordering::Greater => Some(Register::Polite),
            std::cmp::Ordering::Less => Some(Register::Plain),
            std::cmp::Ordering::Equal => None,
        }
    }
}

/// 本人の文体を何から決めたか。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterSource {
    /// 分けられた文書の多い方で決めた。
    Counted,
    /// 本人の形代の申告を採った。数えた結果より申告を優先する。
    Declared,
}

/// 基準を本人の文体で絞った結果。どうなったかは出力に出す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterFilter {
    /// 本人の文体の分だけを候補にした。
    Matched {
        /// 本人の文体。
        register: Register,
        /// 何から決めたか。
        source: RegisterSource,
        /// 本人の文書を分けた本数。
        person: RegisterCount,
        /// 候補の本数。
        candidates: usize,
    },
    /// 本人の文体が決まらなかった。基準を全部候補にした。
    Undecided {
        /// 本人の文書を分けた本数。
        person: RegisterCount,
    },
    /// 基準に本人の文体の文書が無かった。基準を全部候補にした。
    Missing {
        /// 本人の文体。
        register: Register,
        /// 何から決めたか。
        source: RegisterSource,
        /// 本人の文書を分けた本数。
        person: RegisterCount,
    },
}

/// 基準のうち、本人と同じ文体の分。
///
/// 文体を揃えないと、床が本人の文体から外れたところにできる。 常体の基準で
/// 作った床は敬体の機械文より下にあり、本人が敬体なら、敬体の機械文が床と
/// 本人の隙間に落ちて通る——取り置きで 104 本中 8 本がそうして通った。
///
/// 絞れないときは基準を全部使う。 本人の文体が決まらないとき（同数、または
/// 1 本も分けられない）と、基準に本人の文体が無いときである。 絞って 0 本に
/// すれば目盛りが作れないので、揃えられないことを言って作るのは止めない。
///
/// `declared` は本人の形代の文体の申告である。あれば数えた結果より優先する。
pub fn restrict_to_register<T>(
    person: RegisterCount,
    declared: Option<Register>,
    pool: Vec<T>,
    share_of: impl Fn(&T) -> Option<f64>,
) -> (Vec<T>, RegisterFilter) {
    let (register, source) = match (declared, person.majority()) {
        (Some(r), _) => (r, RegisterSource::Declared),
        (None, Some(r)) => (r, RegisterSource::Counted),
        (None, None) => return (pool, RegisterFilter::Undecided { person }),
    };
    let matched = |t: &T| share_of(t).map(Register::of_share) == Some(register);
    if !pool.iter().any(matched) {
        return (
            pool,
            RegisterFilter::Missing {
                register,
                source,
                person,
            },
        );
    }
    let kept: Vec<T> = pool.into_iter().filter(matched).collect();
    let candidates = kept.len();
    (
        kept,
        RegisterFilter::Matched {
            register,
            source,
            person,
            candidates,
        },
    )
}

/// 基準から、本人の題材に近い分を選ぶ。選んだものの添字を昇順で返す。
///
/// 題材を揃えないと、測っているのは題材である。 統制しないで訓練した文体表現は、
/// 題材を揃えたテストで AUC が .79 から .58 へ落ちる
/// （[Wegmann](../../../docs/references/wegmann-2022.md)）。
///
/// 揃えるのは対ごとではなく素材全体である——対で絞ると相手の本数が変わる。
///
/// 近さは 2 文字以上の自立語の重なりで測る。 重なりの多い順に `take` 本を取り、
/// 同点なら並びの順で決める。 候補が `take` 本以下なら、語を集めずに全部使う。
/// 本人の語が 1 つも無いときも全部使う。
pub fn pick_by_topic(
    mine: impl FnOnce() -> BTreeSet<String>,
    pool: usize,
    theirs: impl Fn(usize) -> BTreeSet<String>,
    take: usize,
) -> Vec<usize> {
    let all = || (0..pool).collect();
    if pool <= take {
        return all();
    }
    let mine = mine();
    if mine.is_empty() {
        return all();
    }
    let mut scored: Vec<(usize, usize)> = (0..pool)
        .map(|i| (theirs(i).intersection(&mine).count(), i))
        .collect();
    // 重なりの多い順。同点なら並びの順。 決めておかないと、同じ素材から
    // 違う目盛りができる。
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let mut picked: Vec<usize> = scored.into_iter().take(take).map(|(_, i)| i).collect();
    picked.sort_unstable();
    picked
}

/// 基準の形代から、目盛りに使う文書を選ぶ。
#[derive(Debug)]
pub struct BaselineSelection<'a> {
    /// 文体でどう絞ったか。
    pub register: RegisterFilter,
    /// 選んだ文書。単位名の昇順。
    pub picked: Vec<&'a DocumentStats>,
}

/// 基準の文書を、本人の文体と題材で選ぶ。
///
/// 題材で選んでから、長さで[束ねる](crate::bundle)。 逆にすると、題材の合わない分を
/// 束ねてから捨てることになる。
///
/// `within` を渡すと、題材で選ぶ前に、地の文の字数がその範囲に入る文書だけを
/// 候補にする。 長さの範囲で断られたときの選び直しに使う
/// （[組み立て](crate::assembly::assemble_stats)）。
#[must_use]
pub fn select_baseline<'a>(
    target: &KatashiroStats,
    baseline: &'a KatashiroStats,
    declared: Option<Register>,
    within: Option<RangeInclusive<usize>>,
) -> BaselineSelection<'a> {
    let person = RegisterCount::of(target.documents.iter().map(|d| d.polite_share));
    let pool: Vec<&DocumentStats> = baseline.documents.iter().collect();
    let (mut candidates, register) =
        restrict_to_register(person, declared, pool, |d| d.polite_share);
    if let Some(w) = within {
        candidates.retain(|d| w.contains(&d.chars));
    }
    let picked = pick_by_topic(
        || {
            target
                .documents
                .iter()
                .flat_map(|d| d.content_words.iter().cloned())
                .collect()
        },
        candidates.len(),
        |i| candidates[i].content_words.clone(),
        POOL_TAKE,
    )
    .into_iter()
    .map(|i| candidates[i])
    .collect();
    BaselineSelection { register, picked }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counted(polite: usize, plain: usize) -> RegisterCount {
        RegisterCount { polite, plain }
    }

    /// 名前と段落の敬体率の組。
    fn pool(v: &[(&'static str, Option<f64>)]) -> Vec<(&'static str, Option<f64>)> {
        v.to_vec()
    }

    #[test]
    fn 文体は敬体率で分ける() {
        assert_eq!(Register::of_share(0.94), Register::Polite);
        assert_eq!(
            Register::of_share(POLITE_SHARE_MIN),
            Register::Polite,
            "境目は敬体"
        );
        assert_eq!(Register::of_share(0.1), Register::Plain);
    }

    #[test]
    fn 本人の文体は分けられた文書の多い方で決まる() {
        let count = RegisterCount::of([Some(1.0), Some(0.9), Some(0.0), None, None]);
        assert_eq!(count, counted(2, 1), "測れない分は数えない");
        assert_eq!(count.majority(), Some(Register::Polite));
        assert_eq!(counted(1, 3).majority(), Some(Register::Plain));
    }

    #[test]
    fn 本人の文体は同数でも分けられなくても決まらない() {
        assert_eq!(counted(2, 2).majority(), None);
        assert_eq!(counted(0, 0).majority(), None);
    }

    #[test]
    fn 基準は本人の文体の分だけを候補にする() {
        // 常体の基準で作った床は敬体の機械文の下にある。 本人が敬体なら、
        // 敬体の機械文が床と本人の隙間に落ちて通ってしまう。
        let p = pool(&[
            ("b1", Some(0.0)),
            ("b1-desu", Some(1.0)),
            ("b2", Some(0.0)),
            ("b2-desu", Some(1.0)),
            ("b3", Some(0.0)),
            ("b4", None),
        ]);
        let (kept, filter) = restrict_to_register(counted(2, 1), None, p, |x| x.1);
        let names: Vec<&str> = kept.iter().map(|x| x.0).collect();
        assert_eq!(names, vec!["b1-desu", "b2-desu"]);
        assert_eq!(
            filter,
            RegisterFilter::Matched {
                register: Register::Polite,
                source: RegisterSource::Counted,
                person: counted(2, 1),
                candidates: 2,
            }
        );
    }

    #[test]
    fn 本人の文体が決まらなければ基準を全部候補にする() {
        let p = pool(&[("b1", Some(0.0)), ("b2", Some(1.0)), ("b3", None)]);
        let (kept, filter) = restrict_to_register(counted(1, 1), None, p.clone(), |x| x.1);
        assert_eq!(kept, p, "同数なら絞らない");
        assert_eq!(
            filter,
            RegisterFilter::Undecided {
                person: counted(1, 1)
            }
        );

        let (kept, filter) = restrict_to_register(counted(0, 0), None, p.clone(), |x| x.1);
        assert_eq!(kept, p, "分けられなければ絞らない");
        assert_eq!(
            filter,
            RegisterFilter::Undecided {
                person: counted(0, 0)
            }
        );
    }

    #[test]
    fn 基準に本人の文体が無ければ基準を全部候補にする() {
        // 絞って 0 本になれば目盛りが作れない。 文体を揃えられないことは言って、
        // 作るのは止めない。
        let p = pool(&[("b1", Some(0.0)), ("b2", None)]);
        let (kept, filter) = restrict_to_register(counted(1, 0), None, p.clone(), |x| x.1);
        assert_eq!(kept, p);
        assert_eq!(
            filter,
            RegisterFilter::Missing {
                register: Register::Polite,
                source: RegisterSource::Counted,
                person: counted(1, 0),
            }
        );
    }

    #[test]
    fn 文体の申告は数えた結果より優先する() {
        // 数えれば敬体だが、本人は常体と申告している。
        let p = pool(&[("b1", Some(0.0)), ("b2", Some(1.0))]);
        let (kept, filter) = restrict_to_register(counted(3, 1), Some(Register::Plain), p, |x| x.1);
        assert_eq!(kept.iter().map(|x| x.0).collect::<Vec<_>>(), vec!["b1"]);
        assert!(matches!(
            filter,
            RegisterFilter::Matched {
                register: Register::Plain,
                source: RegisterSource::Declared,
                ..
            }
        ));
    }

    #[test]
    fn 申告があれば数えた文体が決まらなくても絞る() {
        let p = pool(&[("b1", Some(0.0)), ("b2", Some(1.0))]);
        let (kept, _) = restrict_to_register(counted(0, 0), Some(Register::Polite), p, |x| x.1);
        assert_eq!(kept.iter().map(|x| x.0).collect::<Vec<_>>(), vec!["b2"]);
    }

    fn words(v: &[&str]) -> BTreeSet<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn 題材は自立語の重なりの多い順に選ぶ() {
        let theirs = [
            words(&["料理", "包丁"]),
            words(&["東京", "天気", "電車"]),
            words(&["東京", "料理"]),
            words(&["天気"]),
        ];
        let got = pick_by_topic(
            || words(&["東京", "天気", "電車"]),
            4,
            |i| theirs[i].clone(),
            2,
        );
        assert_eq!(got, vec![1, 2], "重なり 3 と 1。同点なら並びの順");
    }

    #[test]
    fn 候補が取る本数以下なら全部使う() {
        let got = pick_by_topic(
            || panic!("語を集めない"),
            POOL_TAKE,
            |_| panic!("語を集めない"),
            POOL_TAKE,
        );
        assert_eq!(got.len(), POOL_TAKE);
    }

    #[test]
    fn 本人の語が無ければ全部使う() {
        let got = pick_by_topic(BTreeSet::new, 30, |_| words(&["東京"]), POOL_TAKE);
        assert_eq!(got.len(), 30);
    }
}
