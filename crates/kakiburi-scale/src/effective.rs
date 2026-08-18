//! どの指標が効くかを決める。
//!
//! 指標の集合は誰にでも当てる。**だが際立つ指標は人によって違う。** ここで決まるのは
//! 選択であって採否ではない——集合から落ちる指標は無い。
//!
//! **判定するのはここだけである。** 検めが自分で計算し直せる形にしておくと、検める
//! 文書を見てから幅や集合を作り直す経路が書けてしまう。

use std::collections::BTreeMap;

/// 幅が狭いと判断する、基準の幅に対する割合。**暫定値である。**
pub const NARROW_RATIO: f64 = 0.5;

/// 端が重なったときに「離れていない」と判断する、その人の幅に対する割合。
/// **暫定値である。**
pub const OVERLAP_RATIO: f64 = 0.5;

/// 指標 1 本の、効くかの判定と根拠。
///
/// **判定だけでなく根拠の値も残す。** 素材が増えたときに、判定が変わったのか値が
/// 変わったのかを見分けられる。
#[derive(Debug, Clone, PartialEq)]
pub struct Effective {
    /// 指標の名前。
    pub name: String,
    /// その人の幅の下端。
    pub low: f64,
    /// その人の幅の上端。
    pub high: f64,
    /// その指標を測れた単位の数。**出現割合の分母である。**
    pub units: usize,
    /// 出現割合。測れた単位のうち、1 回以上出た単位の割合。
    pub rate: f64,
    /// 条件 1。その人の幅が狭い。
    pub narrow: bool,
    /// 条件 2。基準から離れている。
    pub distant: bool,
}

impl Effective {
    /// 条件 1 と 2 の両方を満たすか。
    ///
    /// **条件 3（指示して動くか）はここに入らない。** あちらは直させてみて初めて
    /// 分かるもので、コーパスからは導けない。
    #[must_use]
    pub fn works(&self) -> bool {
        self.narrow && self.distant
    }

    /// 幅。
    #[must_use]
    pub fn spread(&self) -> f64 {
        self.high - self.low
    }
}

/// 単位 1 本ぶんの、指標ごとの値。**測れなかったものは `None`。**
pub type Row = Vec<(String, Option<f64>)>;

/// 指示できる指標について、効くかを判定する。
///
/// `person` と `baseline` は 1 要素が 1 単位ぶんである。
///
/// **両側で測れた指標だけを返す。** 片側しか無ければ比べる相手がいない——
/// 基準の幅が無ければ「狭い」を判定できず、基準の範囲が無ければ「離れている」も
/// 判定できない。
#[must_use]
pub fn judge(person: &[Row], baseline: &[Row]) -> Vec<Effective> {
    let p = gather(person);
    let b = gather(baseline);
    p.into_iter()
        .filter_map(|(name, values)| {
            let theirs = b.get(&name)?;
            let mine = Extent::of(&values)?;
            let base = Extent::of(theirs)?;
            #[allow(clippy::cast_precision_loss)]
            let rate = values.iter().filter(|v| **v > 0.0).count() as f64 / values.len() as f64;
            Some(Effective {
                name,
                low: mine.low,
                high: mine.high,
                units: values.len(),
                rate,
                narrow: mine.spread() <= base.spread() * NARROW_RATIO,
                distant: mine.distant_from(base),
            })
        })
        .collect()
}

/// 指標ごとに、測れた値だけを集める。
fn gather(rows: &[Row]) -> BTreeMap<String, Vec<f64>> {
    let mut out: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for row in rows {
        for (name, v) in row {
            if let Some(v) = v {
                out.entry(name.clone()).or_default().push(*v);
            }
        }
    }
    out.retain(|_, vs| !vs.is_empty());
    out
}

/// 値の並びの端。
#[derive(Debug, Clone, Copy)]
struct Extent {
    low: f64,
    high: f64,
}

impl Extent {
    fn of(values: &[f64]) -> Option<Self> {
        if values.is_empty() {
            return None;
        }
        Some(Self {
            low: values.iter().copied().fold(f64::INFINITY, f64::min),
            high: values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        })
    }

    fn spread(self) -> f64 {
        self.high - self.low
    }

    /// 基準から離れているか。**範囲の交わり方で見る。**
    ///
    /// 素材は少なく、統計的な検定に足りる件数は集まらない。
    ///
    /// | 形 | 判定 |
    /// | --- | --- |
    /// | 交わらない | 離れている |
    /// | 一方が他方を完全に含む | 離れていない。含む側は「その値も取りうる」としか言っていない |
    /// | 端が重なる | 重なりの幅を、その人の幅と比べる。半分を超えるなら離れていない |
    fn distant_from(self, base: Self) -> bool {
        let lo = self.low.max(base.low);
        let hi = self.high.min(base.high);
        if hi < lo {
            return true;
        }
        if (base.low <= self.low && self.high <= base.high)
            || (self.low <= base.low && base.high <= self.high)
        {
            return false;
        }
        let overlap = hi - lo;
        // **幅が 0 なら割れない。** 交わっている以上、その 1 点は基準の中にある。
        if self.spread() <= f64::EPSILON {
            return false;
        }
        overlap / self.spread() <= OVERLAP_RATIO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pairs: &[(&str, Option<f64>)]) -> Row {
        pairs.iter().map(|(n, v)| ((*n).to_owned(), *v)).collect()
    }

    /// 1 指標だけの素材。
    fn side(name: &str, values: &[f64]) -> Vec<Row> {
        values.iter().map(|v| row(&[(name, Some(*v))])).collect()
    }

    #[test]
    fn 幅が基準の半分以下なら狭い() {
        // 基準 0〜10、その人 0〜4。
        let e = judge(&side("x", &[0.0, 4.0]), &side("x", &[0.0, 10.0]));
        assert!(e[0].narrow, "{e:?}");
    }

    #[test]
    fn 幅が基準の半分を超えれば狭くない() {
        let e = judge(&side("x", &[0.0, 6.0]), &side("x", &[0.0, 10.0]));
        assert!(!e[0].narrow, "{e:?}");
    }

    #[test]
    fn 交わらなければ離れている() {
        let e = judge(&side("x", &[10.0, 12.0]), &side("x", &[0.0, 5.0]));
        assert!(e[0].distant, "{e:?}");
    }

    #[test]
    fn 基準が完全に含んでいれば離れていない() {
        // 含む側は「その値も取りうる」としか言っていない。
        let e = judge(&side("x", &[3.0, 4.0]), &side("x", &[0.0, 10.0]));
        assert!(!e[0].distant, "{e:?}");
    }

    #[test]
    fn その人が完全に含んでいても離れていない() {
        let e = judge(&side("x", &[0.0, 10.0]), &side("x", &[3.0, 4.0]));
        assert!(!e[0].distant, "{e:?}");
    }

    #[test]
    fn 端が重なるときは重なりの幅で見る() {
        // その人 0〜10、基準 8〜20。重なりは 2 で、その人の幅の 20%。
        let e = judge(&side("x", &[0.0, 10.0]), &side("x", &[8.0, 20.0]));
        assert!(e[0].distant, "20% なら離れている: {e:?}");
        // その人 0〜10、基準 4〜20。重なりは 6 で 60%。
        let e = judge(&side("x", &[0.0, 10.0]), &side("x", &[4.0, 20.0]));
        assert!(!e[0].distant, "60% なら離れていない: {e:?}");
    }

    #[test]
    fn 出現割合は測れた単位で割る() {
        // 測れなかった単位は分母に入れない——入れると、短い文書の多い書き手ほど
        // 出現割合が下がり、毎回使っている語が「たまにしか使わない」ことになる。
        let person = vec![
            row(&[("x", Some(1.0))]),
            row(&[("x", Some(0.0))]),
            row(&[("x", None)]),
        ];
        let e = judge(&person, &side("x", &[0.0, 10.0]));
        assert_eq!(e[0].units, 2, "測れた 2 本が分母");
        assert!((e[0].rate - 0.5).abs() < 1e-9, "{:?}", e[0].rate);
    }

    #[test]
    fn 片側しか無い指標は判定しない() {
        // 比べる相手がいない。基準の幅が無ければ「狭い」を判定できない。
        let e = judge(&side("x", &[1.0, 2.0]), &side("y", &[0.0, 10.0]));
        assert!(e.is_empty(), "{e:?}");
    }

    #[test]
    fn 条件_1_と_2_の両方でなければ前に出さない() {
        // 狭いが基準に含まれている。
        let e = judge(&side("x", &[3.0, 4.0]), &side("x", &[0.0, 10.0]));
        assert!(e[0].narrow && !e[0].distant);
        assert!(!e[0].works(), "{e:?}");
    }
}
