//! どの対を何に使うかを決める。
//!
//! <strong>較正に使った対から天井と床を作らない。</strong> 較正は 2 つの対集合を分離するように
//! 重みを決める手続きである。同じ対で目盛りを作れば、<strong>分離するように合わせたものの
//! 分離具合を測る</strong>ことになり、いちばん危ない検査がそこで無効になる。
//!
//! <strong>単位は共有する。対は共有しない。</strong>

use crate::split::{Split, Unit};

/// 対。順序を持たない——`(a, b)` と `(b, a)` は同じ対である。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pair {
    /// 名前の小さい側。
    pub left: String,
    /// 名前の大きい側。
    pub right: String,
}

impl Pair {
    /// 作る。<strong>名前で並べ替える</strong>ので、渡す順に依らず同じ対になる。
    #[must_use]
    pub fn new(a: impl Into<String>, b: impl Into<String>) -> Self {
        let (a, b) = (a.into(), b.into());
        if a <= b {
            Self { left: a, right: b }
        } else {
            Self { left: b, right: a }
        }
    }
}

/// 対の割り当て。<strong>ここが唯一の作り手である。</strong>
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pairing {
    /// 較正・同じ人の側。<strong>相手集合の中の対。</strong>
    pub calibration_same: Vec<Pair>,
    /// 較正・違う人の側。<strong>基準の較正分 × 相手集合</strong>と、<strong>他人 × 相手集合</strong>。
    pub calibration_different: Vec<Pair>,
    /// 天井。測る分の本人の単位 1 本 × 相手集合。1 本につき 1 点。
    pub ceiling: Vec<(String, Vec<Pair>)>,
    /// 床。床の点に取った基準 1 本 × 相手集合。1 本につき 1 点。
    pub floor: Vec<(String, Vec<Pair>)>,
}

/// 割り当てを作る。
///
/// <strong>相手集合は較正にも天井にも現れるが、同じ対は 2 度使わない。</strong>
#[must_use]
pub fn pair(person: &Split, baseline: &Split, others: &[Unit]) -> Pairing {
    // 較正・同じ人の側は相手集合の中だけで閉じる。
    let mut calibration_same = Vec::new();
    for i in 0..person.partners.len() {
        for j in i + 1..person.partners.len() {
            calibration_same.push(Pair::new(
                &person.partners[i].name,
                &person.partners[j].name,
            ));
        }
    }
    // 較正・違う人の側は基準の較正分 × 相手集合。
    // 基準の「相手集合」の枠が較正分にあたる。
    let mut calibration_different = Vec::new();
    for b in &baseline.partners {
        for p in &person.partners {
            calibration_different.push(Pair::new(&b.name, &p.name));
        }
    }
    // <strong>他人が手に入るなら、違う人の側に足す。</strong>
    //
    // <strong>基準だけで学習すると、測っているのは「その人らしさ」ではなく
    // 「この基準との違い」になる。</strong> 基準と本人が共有している癖——同じ敬体で書く、
    // といったこと——は、その系統の距離が動かないので重みが 0 に落ちる。
    //
    // 実測でそれが起きた。ですます体を丸ごとである体に変えても照合値は中央 +0.002 しか
    // 動かず、機能語の重みは 0.106（ほかの系統は 0.507〜0.705）だった。
    for o in others {
        for p in &person.partners {
            calibration_different.push(Pair::new(&o.name, &p.name));
        }
    }
    // 天井と床は相手集合の外の単位から出る。
    let against = |name: &str| -> Vec<Pair> {
        person
            .partners
            .iter()
            .map(|p| Pair::new(name, &p.name))
            .collect()
    };
    let ceiling = person
        .points
        .iter()
        .map(|u| (u.name.clone(), against(&u.name)))
        .collect();
    let floor = baseline
        .points
        .iter()
        .map(|u| (u.name.clone(), against(&u.name)))
        .collect();

    Pairing {
        calibration_same,
        calibration_different,
        ceiling,
        floor,
    }
}

impl Pairing {
    /// 較正に使う対の全体。
    #[must_use]
    pub fn calibration_pairs(&self) -> Vec<&Pair> {
        self.calibration_same
            .iter()
            .chain(self.calibration_different.iter())
            .collect()
    }

    /// 目盛りに使う対の全体。
    #[must_use]
    pub fn scale_pairs(&self) -> Vec<&Pair> {
        self.ceiling
            .iter()
            .chain(self.floor.iter())
            .flat_map(|(_, ps)| ps.iter())
            .collect()
    }

    /// <strong>較正と目盛りが対を共有していないか。</strong>
    ///
    /// 共有していれば、分離するように合わせたものの分離具合を測ることになる。
    /// <strong>作り手を 1 つにしたうえで、それでも確かめる</strong>——ここが壊れると、
    /// 骨格の検査そのものが無効になる。
    #[must_use]
    pub fn shares_pairs(&self) -> bool {
        let cal: std::collections::BTreeSet<&Pair> = self.calibration_pairs().into_iter().collect();
        self.scale_pairs().into_iter().any(|p| cal.contains(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::split::{split, Unit};

    fn units(prefix: &str, n: usize) -> Vec<Unit> {
        (0..n)
            .map(|i| Unit {
                name: format!("{prefix}{i:02}"),
                systems_measured: true,
                humanness_measured: true,
            })
            .collect()
    }

    fn pairing() -> Pairing {
        let person = split(&units("p", 10)).unwrap();
        let baseline = split(&units("b", 10)).unwrap();
        pair(&person, &baseline, &[])
    }

    #[test]
    fn 対は順序を持たない() {
        assert_eq!(Pair::new("a", "b"), Pair::new("b", "a"));
    }

    #[test]
    fn 較正は同じ人の対_10_本と違う人の対_25_本になる() {
        let p = pairing();
        assert_eq!(p.calibration_same.len(), 10, "相手集合 5 本の中の対");
        assert_eq!(
            p.calibration_different.len(),
            25,
            "基準 5 本 × 相手集合 5 本"
        );
    }

    #[test]
    fn 天井と床は_1_本につき_1_点になる() {
        let p = pairing();
        assert_eq!(p.ceiling.len(), 5);
        assert_eq!(p.floor.len(), 5);
        for (_, ps) in p.ceiling.iter().chain(p.floor.iter()) {
            assert_eq!(ps.len(), 5, "相手集合 5 本と対にする");
        }
    }

    #[test]
    fn 較正と目盛りは対を共有しない() {
        // ここが壊れると、分離するように合わせたものの分離具合を測ることになる。
        let p = pairing();
        assert!(!p.shares_pairs(), "同じ対を 2 度使ってはいけない");
    }

    #[test]
    fn 基準を割らないと床が汚れる() {
        // 基準を割らずに較正分と床の点を同じにすると、較正に使ったその対で
        // 床を測ることになる。汚れるのが床だけなので帯が狭まり、分離が
        // 実際より良く見える。
        let person = split(&units("p", 10)).unwrap();
        let baseline = split(&units("b", 10)).unwrap();
        let mut bad = baseline.clone();
        bad.points = bad.partners.clone(); // 割らなかった状態
        let p = pair(&person, &bad, &[]);
        assert!(
            p.shares_pairs(),
            "割らなければ共有が起きる——それを検出できる"
        );
    }

    #[test]
    fn 天井は相手集合の外の単位から出る() {
        // 相手集合の中から出せば、較正に使った対で天井を測ることになる。
        let person = split(&units("p", 10)).unwrap();
        let p = pair(&person, &split(&units("b", 10)).unwrap(), &[]);
        let names: Vec<&str> = p.ceiling.iter().map(|(n, _)| n.as_str()).collect();
        let points: Vec<&str> = person.points.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, points, "天井の点は測る分そのもの");
        for n in &names {
            assert!(
                !person.partners.iter().any(|u| u.name == *n),
                "{n} が相手集合に入っている"
            );
        }
    }

    #[test]
    fn 床は基準の外の単位から出る() {
        let baseline = split(&units("b", 10)).unwrap();
        let p = pair(&split(&units("p", 10)).unwrap(), &baseline, &[]);
        let names: Vec<&str> = p.floor.iter().map(|(n, _)| n.as_str()).collect();
        let points: Vec<&str> = baseline.points.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, points, "床の点は床の点そのもの");
        for n in &names {
            assert!(
                !baseline.partners.iter().any(|u| u.name == *n),
                "{n} が較正分に入っている"
            );
        }
    }

    #[test]
    fn 単位は共有するが対は共有しない() {
        // 相手集合は較正にも天井にも現れる。
        let p = pairing();
        let in_cal = p
            .calibration_pairs()
            .iter()
            .any(|q| q.left == "p00" || q.right == "p00");
        let in_scale = p
            .scale_pairs()
            .iter()
            .any(|q| q.left == "p00" || q.right == "p00");
        assert!(in_cal && in_scale, "単位は両方に現れる");
        assert!(!p.shares_pairs(), "だが対は重ならない");
    }
}
