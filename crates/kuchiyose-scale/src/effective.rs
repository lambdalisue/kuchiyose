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

/// 使った割合で見る指標で、一貫していると判断する割合。暫定値である。
///
/// いつも使うか、ほとんど使わないかなら一貫している。 両側に同じ隔たりを置く
/// （[狭さの判定](../../../docs/spec/200-extract.md#下端を出現割合で見る指標は出現割合で狭さを見る)）。
///
/// [下端の判定](../../../docs/spec/200-extract.md#下限は使った割合で見る)は 1.0
/// （すべての単位）で切るので、こちらのほうが緩い。 前に出すかを決めるだけなら、
/// 間違えたときの害は「言わなくてよいことを言った」で済む。
pub const APPEARANCE_CONSISTENT: f64 = 0.8;

/// 使った割合で見る指標で、基準から離れていると判断する差。暫定値である。
pub const APPEARANCE_GAP: f64 = 0.4;

/// 判定の根拠。どちらの見方で判定したかと、比べた基準の値。
///
/// 結果だけでは検算できない。 本人の側は幅も出現割合も残っているが、比べた
/// 相手が残っていなければ、判定が変わったときに「基準が変わったのか、閾値を
/// 変えたのか」を言えない。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Basis {
    /// 幅で見た。基準の幅の端。
    Spread {
        /// 基準の下端。
        low: f64,
        /// 基準の上端。
        high: f64,
    },
    /// 使った割合で見た。基準の出現割合。
    Appearance {
        /// 基準の出現割合。
        rate: f64,
    },
}

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
    /// 条件 1・2 の根拠。
    pub basis: Basis,
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

    /// 一貫しているだけか。条件 1 は満たすが、条件 2 は満たさない。
    ///
    /// 条件 2 は「基準と本人が違うか」で軸を選ぶ。 だが基準と本人が一致して
    /// いる軸でも、草稿がそこから外れることはある——捨てられている軸なので、
    /// 外れても何も言われない。
    ///
    /// 実例。ある書き手は箇条書きの 85% を常体で書き、基準も常体で書く。だから
    /// 条件 2 が落とす。箇条書きを敬体で書いた草稿は、本人の幅の外にいるのに
    /// 何も言われない。
    ///
    /// 判定には使わない。指摘にだけ出す。 実測で、この軸まで判定に入れると
    /// 形代から抜いた本人の記事 16 本のうち幅の外に出るものが 2 本から 6 本に
    /// 増えた——止めれば害だが、言うだけなら払える。
    #[must_use]
    pub fn narrow_only(&self) -> bool {
        self.narrow && !self.distant
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
pub fn judge(
    person: &[Row],
    baseline: &[Row],
    by_appearance: &dyn Fn(&str) -> bool,
) -> Vec<Effective> {
    let p = gather(person);
    let b = gather(baseline);
    p.into_iter()
        .filter_map(|(name, values)| {
            let theirs = b.get(&name)?;
            let mine = Extent::of(&values)?;
            let base = Extent::of(theirs)?;
            let rate = appeared(&values);
            // 下端を使った割合で見る指標は、効くかも使った割合で見る。
            //
            // 密度や個数では 0 が「使わなかった」を意味するので、素の幅は
            // 0 から最大までに広がる。90% の記事で使っている指標でも、残りの
            // 10% の 0 が幅を 0 まで引き下げ、「一貫していない」と判定される
            // ——仕様が下端のためにこの規則を置いたのに、効くかの判定がそれを
            // 見ていなかった。
            //
            // 実測では、この 1 か所だけで効く指標が 0 本から 3 本になる。
            let (narrow, distant, basis) = if by_appearance(&name) {
                let theirs_rate = appeared(theirs);
                (
                    // いつも使うか、ほとんど使わないかなら、一貫している。
                    rate >= APPEARANCE_CONSISTENT || rate <= 1.0 - APPEARANCE_CONSISTENT,
                    (rate - theirs_rate).abs() >= APPEARANCE_GAP,
                    Basis::Appearance { rate: theirs_rate },
                )
            } else {
                (
                    mine.spread() <= base.spread() * NARROW_RATIO,
                    mine.distant_from(base),
                    Basis::Spread {
                        low: base.low,
                        high: base.high,
                    },
                )
            };
            Some(Effective {
                name,
                low: mine.low,
                high: mine.high,
                units: values.len(),
                rate,
                narrow,
                distant,
                basis,
            })
        })
        .collect()
}

/// 値が出た単位の割合。測れなかった単位は入っていない。
fn appeared(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let n = values.len() as f64;
    #[allow(clippy::cast_precision_loss)]
    let hit = values.iter().filter(|v| **v > 0.0).count() as f64;
    hit / n
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
        let e = judge(&side("x", &[0.0, 4.0]), &side("x", &[0.0, 10.0]), &|_| {
            false
        });
        assert!(e[0].narrow, "{e:?}");
    }

    #[test]
    fn 幅が基準の半分を超えれば狭くない() {
        let e = judge(&side("x", &[0.0, 6.0]), &side("x", &[0.0, 10.0]), &|_| {
            false
        });
        assert!(!e[0].narrow, "{e:?}");
    }

    #[test]
    fn 交わらなければ離れている() {
        let e = judge(&side("x", &[10.0, 12.0]), &side("x", &[0.0, 5.0]), &|_| {
            false
        });
        assert!(e[0].distant, "{e:?}");
    }

    #[test]
    fn 基準が完全に含んでいれば離れていない() {
        // 含む側は「その値も取りうる」としか言っていない。
        let e = judge(&side("x", &[3.0, 4.0]), &side("x", &[0.0, 10.0]), &|_| {
            false
        });
        assert!(!e[0].distant, "{e:?}");
    }

    #[test]
    fn その人が完全に含んでいても離れていない() {
        let e = judge(&side("x", &[0.0, 10.0]), &side("x", &[3.0, 4.0]), &|_| {
            false
        });
        assert!(!e[0].distant, "{e:?}");
    }

    #[test]
    fn 端が重なるときは重なりの幅で見る() {
        // その人 0〜10、基準 8〜20。重なりは 2 で、その人の幅の 20%。
        let e = judge(&side("x", &[0.0, 10.0]), &side("x", &[8.0, 20.0]), &|_| {
            false
        });
        assert!(e[0].distant, "20% なら離れている: {e:?}");
        // その人 0〜10、基準 4〜20。重なりは 6 で 60%。
        let e = judge(&side("x", &[0.0, 10.0]), &side("x", &[4.0, 20.0]), &|_| {
            false
        });
        assert!(!e[0].distant, "60% なら離れていない: {e:?}");
    }

    #[test]
    fn 使った割合で見る指標は_0_で幅が広がっても一貫している() {
        // ここが 0 本の正体だった。 密度では 0 が「使わなかった」を意味するので、
        // 90% の記事で使っていても、残り 10% の 0 が幅を 0 まで引き下げる。
        // 素の幅で見ると「一貫していない」になり、いちばん指示しやすい指標が落ちる。
        //
        // 本人 9/10 で使う、基準 2/10。
        let mine = side("x", &[0.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0]);
        let theirs = side("x", &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 3.0, 3.0]);

        let by_width = judge(&mine, &theirs, &|_| false);
        assert!(!by_width[0].narrow, "素の幅では一貫していないとされる");

        let by_rate = judge(&mine, &theirs, &|_| true);
        assert!(by_rate[0].narrow, "使った割合なら一貫している");
        assert!(by_rate[0].distant, "基準は 2 割しか使わない");
        assert!(by_rate[0].works());
    }

    #[test]
    fn 使った割合が中途半端なら一貫していない() {
        // 半分の記事でだけ使う指標は、指示する先が無い。
        let mine = side("x", &[0.0, 0.0, 0.0, 0.0, 0.0, 5.0, 5.0, 5.0, 5.0, 5.0]);
        let theirs = side("x", &[0.0; 10]);
        let e = judge(&mine, &theirs, &|_| true);
        assert!(!e[0].narrow, "5 割では一貫していない: {e:?}");
    }

    #[test]
    fn 使った割合が同じなら離れていない() {
        // どちらも毎回使うなら、その指標では分かれない。
        let both = side("x", &[5.0; 10]);
        let e = judge(&both, &both, &|_| true);
        assert!(e[0].narrow, "毎回使うので一貫している");
        assert!(!e[0].distant, "基準も毎回使う: {e:?}");
    }

    #[test]
    fn ほとんど使わないことも一貫である() {
        // 「あなたは絵文字をまず使わない」も指示になる。
        let mine = side("x", &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0]);
        let theirs = side("x", &[2.0; 10]);
        let e = judge(&mine, &theirs, &|_| true);
        assert!(e[0].narrow && e[0].distant, "{e:?}");
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
        let e = judge(&person, &side("x", &[0.0, 10.0]), &|_| false);
        assert_eq!(e[0].units, 2, "測れた 2 本が分母");
        assert!((e[0].rate - 0.5).abs() < 1e-9, "{:?}", e[0].rate);
    }

    #[test]
    fn 根拠には比べた基準の値が残る() {
        // 結果だけでは、判定が変わったときに基準が変わったのかを言えない。
        let e = judge(&side("x", &[0.0, 4.0]), &side("x", &[1.0, 10.0]), &|_| {
            false
        });
        assert_eq!(
            e[0].basis,
            Basis::Spread {
                low: 1.0,
                high: 10.0
            }
        );

        let mine = side("x", &[5.0; 10]);
        let theirs = side("x", &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 3.0, 3.0]);
        let e = judge(&mine, &theirs, &|_| true);
        assert_eq!(e[0].basis, Basis::Appearance { rate: 0.2 });
    }

    #[test]
    fn 片側しか無い指標は判定しない() {
        // 比べる相手がいない。基準の幅が無ければ「狭い」を判定できない。
        let e = judge(&side("x", &[1.0, 2.0]), &side("y", &[0.0, 10.0]), &|_| {
            false
        });
        assert!(e.is_empty(), "{e:?}");
    }

    #[test]
    fn 条件_1_と_2_の両方でなければ前に出さない() {
        // 狭いが基準に含まれている。
        let e = judge(&side("x", &[3.0, 4.0]), &side("x", &[0.0, 10.0]), &|_| {
            false
        });
        assert!(e[0].narrow && !e[0].distant);
        assert!(!e[0].works(), "{e:?}");
    }
}
