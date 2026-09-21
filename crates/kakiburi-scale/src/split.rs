//! 単位を 2 つに割る。
//!
//! <strong>交差検証ではなく固定の 2 分割にする。</strong> 交差検証は束ごとに違う重みを作るので、
//! 新しい文書をどの重みで採点するかが決まらない。<strong>1 組しか作らない。</strong>

/// 単位。名前と、判定に使うものが測れたかどうか。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    /// 単位の名前。<strong>ファイルの名前ではない</strong>——束ねた単位では食い違う。
    pub name: String,
    /// 判定に使う系統が全部測れたか。
    pub systems_measured: bool,
    /// 人らしさの指標が全部測れたか。
    ///
    /// <strong>忘れると人らしさ側だけが痩せる。</strong> 人らしさの除外は系統の除外より厳しい
    /// ので、系統は測れるのに人らしさは測れない単位が普通に出る。
    pub humanness_measured: bool,
}

impl Unit {
    /// 帯に使える単位か。<strong>両方測れていなければ入れない。</strong>
    #[must_use]
    pub fn usable(&self) -> bool {
        self.systems_measured && self.humanness_measured
    }
}

/// 片側に要る本数。
pub const PER_SIDE: usize = 5;

/// 1 つの側に要る単位の下限。相手集合 5 ＋ 測る分 5。
pub const UNITS_FLOOR: usize = PER_SIDE * 2;

/// 割った結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    /// 相手集合。<strong>あらゆる照合の相手。</strong> 天井も床も検めも、必ずこれと対にする。
    pub partners: Vec<Unit>,
    /// 測る分。天井の点になる。
    pub points: Vec<Unit>,
}

/// 割れない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitError {
    /// 条件を満たした単位が下限に届かない。
    NotEnough {
        /// 使える単位の数。
        usable: usize,
        /// 要る数。
        need: usize,
    },
}

impl std::fmt::Display for SplitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SplitError::NotEnough { usable, need } => {
                write!(f, "測れた単位が {usable} 本で、{need} 本に届かない")
            }
        }
    }
}

impl std::error::Error for SplitError {}

/// 並びから、目標の本数どおりに<strong>配る</strong>。
///
/// <strong>先頭から順に切ってはいけない。</strong> 割りが素材を足した順とそのまま揃う。単位名には
/// 媒体・年代・題材が入るので、<strong>「ある時期 対 別の時期」や「題材の合う基準 対 合わない
/// 基準」で割れた帯</strong>ができる。値は出るしエラーにもならないので、気付けない。
///
/// だから 1 本ずつ、<strong>取り分がいちばん余っている群へ配る。</strong> どの群も並び全体から
/// まんべんなく取るので、名前に相関する性質が片側へ寄らない。同じ余りなら群の順で決める。
fn deal(sorted: &[Unit], targets: [usize; 3]) -> [Vec<Unit>; 3] {
    let total: usize = targets.iter().sum();
    #[allow(clippy::cast_precision_loss)]
    let share = targets.map(|t| t as f64 / total as f64);
    let mut credit = [0.0_f64; 3];
    let mut out: [Vec<Unit>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for u in sorted.iter().take(total) {
        for g in 0..3 {
            credit[g] += share[g];
        }
        let mut pick = None;
        for g in 0..3 {
            if out[g].len() >= targets[g] {
                continue;
            }
            match pick {
                Some(best) if credit[g] <= credit[best] => {}
                _ => pick = Some(g),
            }
        }
        if let Some(g) = pick {
            out[g].push(u.clone());
            credit[g] -= 1.0;
        }
    }
    out
}

/// 単位を割る。
///
/// <strong>取れるのは、判定に使うものをすべて測れた単位だけである。</strong> 除外に掛かった単位を
/// 入れれば、それを含む対から照合値が出ず、相手の本数が黙って 5 を割る。
///
/// <strong>相手集合は 5 ちょうど取る。</strong> あらゆる照合の相手なので、増やせば照合値の意味が
/// 単位数で変わる。
///
/// <strong>帯の点は、相手集合を除いた残りの半分を取る。</strong> 端を
/// [分位](crate::band::Ends::trimmed_low)で取るようになったので、点が多いほうが端は
/// 安定する。<strong>半分で止めるのは較正のためである</strong>——帯の点に使った単位は較正から
/// 外れるので、全部を点にすると較正に渡す単位が無くなる。
///
/// <strong>どの群も、単位名の昇順の全体から[配る](deal)。</strong> 先頭から切ると、割りが素材を
/// 足した順と揃ってしまう。
pub fn split(units: &[Unit]) -> Result<Split, SplitError> {
    let mut usable: Vec<Unit> = units.iter().filter(|u| u.usable()).cloned().collect();
    // 単位名の昇順。ファイル名ではない。
    usable.sort_by(|a, b| a.name.cmp(&b.name));
    if usable.len() < UNITS_FLOOR {
        return Err(SplitError::NotEnough {
            usable: usable.len(),
            need: UNITS_FLOOR,
        });
    }
    let n_points = PER_SIDE.max((usable.len() - PER_SIDE) / 2);
    let n_rest = usable.len() - PER_SIDE - n_points;
    let [partners, points, _rest] = deal(&usable, [PER_SIDE, n_points, n_rest]);
    Ok(Split { partners, points })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(name: &str) -> Unit {
        Unit {
            name: name.into(),
            systems_measured: true,
            humanness_measured: true,
        }
    }

    fn units(n: usize) -> Vec<Unit> {
        (0..n).map(|i| unit(&format!("u{i:02}"))).collect()
    }

    /// 単位名の末尾の番号。並びのどこから取られたかを見るため。
    fn at(u: &Unit) -> usize {
        u.name[1..].parse().expect("番号が付いている")
    }

    #[test]
    fn 昇順に並べて交互に配る() {
        // 下限ちょうどなら 1 本ずつ交互になる。
        let s = split(&units(10)).unwrap();
        let names: Vec<&str> = s.partners.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, vec!["u00", "u02", "u04", "u06", "u08"]);
        let names: Vec<&str> = s.points.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, vec!["u01", "u03", "u05", "u07", "u09"]);
    }

    #[test]
    fn 先頭から切らない() {
        // 切ると、割りが素材を足した順とそのまま揃う。単位名には媒体・年代・題材が
        // 入るので、「ある時期 対 別の時期」で割れた帯ができる。
        let s = split(&units(30)).unwrap();
        let names: Vec<&str> = s.partners.iter().map(|u| u.name.as_str()).collect();
        assert_ne!(
            names,
            vec!["u00", "u01", "u02", "u03", "u04"],
            "先頭 5 本をそのまま取っている"
        );
    }

    #[test]
    fn どの群も並び全体から取る() {
        let s = split(&units(30)).unwrap();
        for (label, us) in [("相手集合", &s.partners), ("帯の点", &s.points)] {
            let early = us.iter().filter(|u| at(u) < 15).count();
            let late = us.len() - early;
            assert!(
                early > 0 && late > 0,
                "{label}が前半 {early} 本・後半 {late} 本に偏っている"
            );
        }
    }

    #[test]
    fn 渡した順に依らない() {
        let mut shuffled = units(10);
        shuffled.reverse();
        assert_eq!(split(&shuffled).unwrap(), split(&units(10)).unwrap());
    }

    #[test]
    fn 下限を割れば割らない() {
        let e = split(&units(9)).unwrap_err();
        assert!(
            matches!(
                e,
                SplitError::NotEnough {
                    usable: 9,
                    need: 10
                }
            ),
            "{e:?}"
        );
    }

    #[test]
    fn 下限ちょうどならどちらも_5() {
        let s = split(&units(10)).unwrap();
        assert_eq!(s.partners.len(), 5);
        assert_eq!(s.points.len(), 5);
    }

    #[test]
    fn 帯の点は使える単位が増えれば増える() {
        // 端を分位で取るので、点が多いほうが端は安定する。5 本で引いた端は、
        // その 5 本が動くだけで動く。
        let s = split(&units(30)).unwrap();
        assert_eq!(s.partners.len(), 5, "相手集合は 5 のまま");
        assert_eq!(s.points.len(), 12, "相手集合を除いた残りの半分");
    }

    #[test]
    fn 較正に渡す単位を残す() {
        // 帯の点に使った単位は較正から外れる。全部を点にすると較正がゼロになる。
        let n = 30;
        let s = split(&units(n)).unwrap();
        let left = n - s.partners.len() - s.points.len();
        assert!(
            left >= s.points.len(),
            "較正に残る {left} 本が帯の点 {} 本を下回らない",
            s.points.len()
        );
    }

    #[test]
    fn 相手集合は帯の点と重ならない() {
        let s = split(&units(30)).unwrap();
        for p in &s.points {
            assert!(
                !s.partners.iter().any(|q| q.name == p.name),
                "{} が両方に入っている",
                p.name
            );
        }
    }

    #[test]
    fn 系統が測れない単位は入れない() {
        let mut us = units(10);
        us[0].systems_measured = false;
        let e = split(&us).unwrap_err();
        assert!(
            matches!(e, SplitError::NotEnough { usable: 9, .. }),
            "{e:?}"
        );
    }

    #[test]
    fn 人らしさが測れない単位も入れない() {
        // 忘れると人らしさ側だけが痩せる。
        let mut us = units(10);
        us[3].humanness_measured = false;
        let e = split(&us).unwrap_err();
        assert!(
            matches!(e, SplitError::NotEnough { usable: 9, .. }),
            "{e:?}"
        );
    }

    #[test]
    fn 測れない単位を飛ばして詰める() {
        let mut us = units(12);
        us[0].systems_measured = false;
        us[1].humanness_measured = false;
        let s = split(&us).unwrap();
        for g in [&s.partners, &s.points] {
            for u in g {
                assert!(at(u) >= 2, "測れない {} が入っている", u.name);
            }
        }
        assert_eq!(s.partners.len() + s.points.len(), 10, "残り 10 本を割る");
    }
}
