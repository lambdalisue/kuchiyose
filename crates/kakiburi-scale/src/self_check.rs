//! 本人がいちばん高く出るかを確かめる。
//!
//! 目盛りを組み立てるたびに内側で確かめる（[自分を検査する](../../../docs/spec/300-revise.md#自分を検査する)）。
//! 崩れていれば、その組み合わせでは判定できない。

use crate::Scale;

/// 本人が基準より高く出た対の、通ると言える割合の下限。暫定値である。
///
/// 1.0 を求めない。 それは「1 対でも逆に出たら落とす」ということで、
/// [端で見るのと同じく n で漂う](higher_rate)——素材を足すほど落ちやすくなる。
///
/// 導き直していない。 目盛りが壊れていれば 0.5 付近に落ちるので、そこから
/// 十分に離れた値を置いてある。どこまで緩めてよいかは、複数の書き手で測るまで決まらない。
pub const HIGHER_RATE_FLOOR: f64 = 0.95;

/// 逆に出た 1 対。本人の単位と照合値、基準の単位と照合値。
pub type Inversion = (String, f64, String, f64);

/// 自己検査の結果。
#[derive(Debug, Clone, PartialEq)]
pub struct SelfCheck {
    /// 本人が高く出た対の割合。並んだ対は半分と数える。
    pub rate: f64,
    /// 比べた対の数。
    pub pairs: usize,
    /// 逆に出た対。差の小さい順——いちばん惜しい対から並ぶ。
    pub inverted: Vec<Inversion>,
}

impl SelfCheck {
    /// 本人と基準の照合値から確かめる。どちらかが空なら確かめられない。
    #[must_use]
    pub fn of(mine: &[(String, f64)], theirs: &[(String, f64)]) -> Option<Self> {
        if mine.is_empty() || theirs.is_empty() {
            return None;
        }
        let (rate, inverted) = higher_rate(mine, theirs);
        Some(Self {
            rate,
            pairs: mine.len() * theirs.len(),
            inverted,
        })
    }

    /// 本人がいちばん高く出たと言えるか。
    #[must_use]
    pub fn passed(&self) -> bool {
        self.rate >= HIGHER_RATE_FLOOR
    }
}

/// 自己検査で測る単位か。較正に使った単位を除く。
///
/// 本人の相手集合を測れば、自分との距離を測ることになる。 基準の較正分も
/// 除く——照合値の較正は「基準の較正分 × 相手集合」を違う人の対として合わせて
/// いるので、それを測り直せば、分けるように合わせたものの分け具合を測る
/// ことになる。
#[must_use]
pub fn measured_in_self_check(scale: &Scale, side: Side, name: &str) -> bool {
    let used = match side {
        Side::Person => scale.partners(),
        Side::Baseline => &scale.selection.baseline_partners,
    };
    !used.iter().any(|n| n == name)
}

/// 単位がどちらの側か。2 つのカセットをまたいで同じ名前がありうるので、役で分ける。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// 本人。
    Person,
    /// 基準。
    Baseline,
}

/// 本人が基準より高く出た対の割合。逆に出た対も返す。
///
/// 最小と最大では見ない。 端は n とともに外へ広がるので、素材を足すほど
/// 本人の最小は下がり基準の最大は上がる——目盛りが良くなっても検査が落ちやすくなる。
///
/// 対ごとの比較は漂わない。 全部の対で本人が高ければ 1.0 で、これが
/// 「本人がいちばん高く出る」の言い換えになる。
#[must_use]
pub fn higher_rate(mine: &[(String, f64)], theirs: &[(String, f64)]) -> (f64, Vec<Inversion>) {
    let mut win = 0.0f64;
    let mut inverted = Vec::new();
    for (pn, pv) in mine {
        for (bn, bv) in theirs {
            if pv > bv {
                win += 1.0;
            } else {
                // 並んだ対も逆として数える。 高く出ていないことに変わりはない。
                if pv == bv {
                    win += 0.5;
                }
                inverted.push((pn.clone(), *pv, bn.clone(), *bv));
            }
        }
    }
    // 差の小さい順に並べる。 いちばん惜しい対から見せる。
    inverted.sort_by(|a, b| (b.1 - b.3).total_cmp(&(a.1 - a.3)));
    #[allow(clippy::cast_precision_loss)]
    let n = (mine.len() * theirs.len()) as f64;
    (win / n, inverted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(v: &[f64]) -> Vec<(String, f64)> {
        v.iter()
            .enumerate()
            .map(|(i, x)| (format!("u{i}"), *x))
            .collect()
    }

    #[test]
    fn 全部の対で本人が高ければ_1_である() {
        let (rate, inverted) = higher_rate(&pairs(&[2.0, 3.0]), &pairs(&[0.0, 1.0]));
        assert!((rate - 1.0).abs() < f64::EPSILON, "{rate}");
        assert!(inverted.is_empty());
    }

    #[test]
    fn 逆に出た対を名指しできる() {
        let (rate, inverted) = higher_rate(&pairs(&[1.0, 3.0]), &pairs(&[0.0, 2.0]));
        assert!((rate - 0.75).abs() < f64::EPSILON, "{rate}");
        assert_eq!(inverted.len(), 1);
        assert_eq!(inverted[0].0, "u0");
        assert_eq!(inverted[0].2, "u1");
    }

    #[test]
    fn 並んだ対は高く出たことにしない() {
        // 半分だけ数えるが、逆に出た対としては残す——高く出ていないことに変わりはない。
        let (rate, inverted) = higher_rate(&pairs(&[1.0]), &pairs(&[1.0]));
        assert!((rate - 0.5).abs() < f64::EPSILON, "{rate}");
        assert_eq!(inverted.len(), 1);
    }

    #[test]
    fn 外れた_1_本を足しても割合はほとんど動かない() {
        // これが最小・最大との違いである。 端で見れば、この 1 本だけで
        // 通っていたものが落ちる。
        let mine: Vec<f64> = (0..20).map(|i| 2.0 + f64::from(i)).collect();
        let theirs: Vec<f64> = (0..20).map(|i| -20.0 + f64::from(i)).collect();
        let (before, _) = higher_rate(&pairs(&mine), &pairs(&theirs));
        assert!((before - 1.0).abs() < f64::EPSILON, "{before}");

        let mut theirs = theirs;
        theirs.push(100.0);
        let (after, inverted) = higher_rate(&pairs(&mine), &pairs(&theirs));
        assert_eq!(inverted.len(), 20, "外れた 1 本は全部の対で逆に出る");
        assert!(after > HIGHER_RATE_FLOOR, "{after}");
    }

    #[test]
    fn 片側が空なら確かめられない() {
        assert_eq!(SelfCheck::of(&[], &pairs(&[1.0])), None);
        assert_eq!(SelfCheck::of(&pairs(&[1.0]), &[]), None);
    }

    #[test]
    fn 下限に届けば通る() {
        let ok = SelfCheck::of(&pairs(&[2.0, 3.0]), &pairs(&[0.0, 1.0])).unwrap();
        assert!(ok.passed());
        assert_eq!(ok.pairs, 4);
        let bad = SelfCheck::of(&pairs(&[1.0, 3.0]), &pairs(&[0.0, 2.0])).unwrap();
        assert!(!bad.passed());
    }
}
