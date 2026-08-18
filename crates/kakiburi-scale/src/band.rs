//! 帯。<strong>2 つの分布の重なりである。</strong>
//!
//! 天井の分布と床の分布が重なる区間が、そのまま「判定できない帯」になる。
//! <strong>両方の端を見る</strong>——片方だけで切ると、分離したときに通ると通らないが同時に
//! 成立する。

/// 分布の端。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ends {
    /// 下端。
    pub low: f64,
    /// 上端。
    pub high: f64,
}

impl Ends {
    /// 点の並びから端を取る。
    ///
    /// <strong>広がりが 0 なら端が決まらない。</strong> 覆っているのとは違う——覆っているなら端はある。
    #[must_use]
    pub fn of(points: &[f64]) -> Option<Self> {
        if points.is_empty() {
            return None;
        }
        let low = points.iter().copied().fold(f64::INFINITY, f64::min);
        let high = points.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if (high - low).abs() < f64::EPSILON {
            return None;
        }
        Some(Self { low, high })
    }

    /// 広がり。
    #[must_use]
    pub fn spread(self) -> f64 {
        self.high - self.low
    }
}

/// 判定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// 通る。
    Pass,
    /// 通らない。
    Fail,
    /// 判定できない。
    Unknown,
}

/// 帯。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    /// 天井の端。
    pub ceiling: Ends,
    /// 床の端。
    pub floor: Ends,
}

/// 帯が作れない理由。
#[derive(Debug, Clone, PartialEq)]
pub enum BandError {
    /// どちらかの広がりが 0。端が決まらない。
    NoSpread {
        /// どちら側か。
        side: &'static str,
    },
    /// 重なりが大きすぎる。<strong>骨格が通っていない。</strong>
    TooMuchOverlap {
        /// 天井の広がりに対する重なりの割合。
        of_ceiling: f64,
        /// 床の広がりに対する重なりの割合。
        of_floor: f64,
    },
}

impl std::fmt::Display for BandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BandError::NoSpread { side } => {
                write!(f, "{side}の広がりが 0 で、端が決まらない")
            }
            BandError::TooMuchOverlap {
                of_ceiling,
                of_floor,
            } => write!(
                f,
                "重なりが大きすぎる（天井の {:.0}%、床の {:.0}%）。骨格が通っていない",
                of_ceiling * 100.0,
                of_floor * 100.0
            ),
        }
    }
}

impl std::error::Error for BandError {}

/// 重なりが大きすぎると判断する割合。<strong>暫定値である。</strong>
pub const OVERLAP_LIMIT: f64 = 0.5;

impl Band {
    /// 帯を作る。
    ///
    /// <strong>重なりが大きすぎるなら、そこで止める。</strong> 帯を狭めて判定を出すのではない——
    /// 分離していないという事実が出ている。
    pub fn build(ceiling: &[f64], floor: &[f64]) -> Result<Self, BandError> {
        let ceiling = Ends::of(ceiling).ok_or(BandError::NoSpread { side: "天井" })?;
        let floor = Ends::of(floor).ok_or(BandError::NoSpread { side: "床" })?;
        let band = Self { ceiling, floor };
        if let Some(ov) = band.overlap() {
            let of_ceiling = ov / ceiling.spread();
            let of_floor = ov / floor.spread();
            if of_ceiling.max(of_floor) > OVERLAP_LIMIT {
                return Err(BandError::TooMuchOverlap {
                    of_ceiling,
                    of_floor,
                });
            }
        }
        Ok(band)
    }

    /// 分離しているか。天井の下端 > 床の上端。
    #[must_use]
    pub fn separated(self) -> bool {
        self.ceiling.low > self.floor.high
    }

    /// 重なっている区間の幅。分離しているなら `None`。
    ///
    /// <strong>隙間は重なりではない。</strong> 分離したときの隙間を重なりとして扱うと、
    /// うまく分離した場合に「重なりすぎ」で止まる。
    #[must_use]
    pub fn overlap(self) -> Option<f64> {
        if self.separated() {
            return None;
        }
        let lo = self.ceiling.low.max(self.floor.low);
        let hi = self.ceiling.high.min(self.floor.high);
        if hi <= lo {
            return None;
        }
        Some(hi - lo)
    }

    /// 分離しているときの隙間。重なっているなら `None`。
    #[must_use]
    pub fn gap(self) -> Option<f64> {
        if self.separated() {
            Some(self.ceiling.low - self.floor.high)
        } else {
            None
        }
    }

    /// 照合値を判定する。
    ///
    /// <strong>両方の端を見る。</strong> 片方だけで切ると、分離したときに通ると通らないが
    /// 同時に成立する。
    #[must_use]
    pub fn judge(self, value: f64) -> Verdict {
        let above_ceiling_low = value >= self.ceiling.low;
        let above_floor_high = value > self.floor.high;
        let below_floor_high = value <= self.floor.high;
        let below_ceiling_low = value < self.ceiling.low;
        match (
            above_ceiling_low && above_floor_high,
            below_floor_high && below_ceiling_low,
        ) {
            (true, false) => Verdict::Pass,
            (false, true) => Verdict::Fail,
            _ => Verdict::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 広がりが_0_なら端が決まらない() {
        assert_eq!(Ends::of(&[1.0, 1.0, 1.0]), None);
        assert_eq!(Ends::of(&[]), None);
        assert!(Ends::of(&[1.0, 2.0]).is_some());
    }

    #[test]
    fn 分離していれば隙間が出る() {
        let b = Band::build(&[2.0, 3.0], &[0.0, 1.0]).unwrap();
        assert!(b.separated());
        assert_eq!(b.gap(), Some(1.0));
        assert_eq!(b.overlap(), None);
    }

    #[test]
    fn 分離した隙間を重なりとして扱わない() {
        // 扱えば、うまく分離した場合に「重なりすぎ」で止まる。
        let b = Band::build(&[10.0, 20.0], &[0.0, 1.0]).unwrap();
        assert_eq!(b.overlap(), None);
    }

    #[test]
    fn 重なっていれば区間が出る() {
        // 天井 1.0〜3.0、床 0.0〜2.0。重なりは 1.0〜2.0 の 1.0。
        // 天井の広がり 2.0、床の広がり 2.0。どちらも 50% でちょうど限度。
        let b = Band::build(&[1.0, 3.0], &[0.0, 2.0]).unwrap();
        assert!(!b.separated());
        assert_eq!(b.overlap(), Some(1.0));
        assert_eq!(b.gap(), None);
    }

    #[test]
    fn 重なりが大きすぎれば帯を作らない() {
        // 帯を狭めて判定を出すのではない。分離していない事実が出ている。
        let e = Band::build(&[0.0, 2.0], &[0.0, 2.0]).unwrap_err();
        assert!(matches!(e, BandError::TooMuchOverlap { .. }), "{e:?}");
    }

    #[test]
    fn 広がりが_0_なら帯を作らない() {
        let e = Band::build(&[1.0, 1.0], &[0.0, 2.0]).unwrap_err();
        assert!(matches!(e, BandError::NoSpread { side: "天井" }), "{e:?}");
        let e = Band::build(&[1.0, 2.0], &[0.0, 0.0]).unwrap_err();
        assert!(matches!(e, BandError::NoSpread { side: "床" }), "{e:?}");
    }

    #[test]
    fn 分離しているときの判定() {
        // 天井 2.0〜3.0、床 0.0〜1.0。帯は 1.0〜2.0。
        let b = Band::build(&[2.0, 3.0], &[0.0, 1.0]).unwrap();
        assert_eq!(b.judge(2.5), Verdict::Pass);
        assert_eq!(b.judge(2.0), Verdict::Pass, "天井の下端は通る");
        assert_eq!(b.judge(0.5), Verdict::Fail);
        assert_eq!(b.judge(1.0), Verdict::Fail, "床の上端は通らない");
        assert_eq!(b.judge(1.5), Verdict::Unknown, "帯の中");
    }

    #[test]
    fn 分離していても通ると通らないが同時に成立しない() {
        // 片方の端だけで切ると、ここで両方が立つ。
        let b = Band::build(&[2.0, 3.0], &[0.0, 1.0]).unwrap();
        for v in [-1.0, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 4.0] {
            let j = b.judge(v);
            // 3 値のどれか 1 つに必ず落ちる。
            assert!(matches!(
                j,
                Verdict::Pass | Verdict::Fail | Verdict::Unknown
            ));
        }
    }

    #[test]
    fn 重なっているときの判定() {
        // 天井 1.0〜3.0、床 0.0〜2.0。重なりの区間 1.0〜2.0 が判定できない。
        let b = Band::build(&[1.0, 3.0], &[0.0, 2.0]).unwrap();
        assert_eq!(b.judge(2.5), Verdict::Pass, "重なりより上");
        assert_eq!(b.judge(0.5), Verdict::Fail, "重なりより下");
        assert_eq!(b.judge(1.5), Verdict::Unknown, "重なりの中");
    }

    #[test]
    fn 帯の外の値も_3_値に落ちる() {
        let b = Band::build(&[2.0, 3.0], &[0.0, 1.0]).unwrap();
        assert_eq!(b.judge(100.0), Verdict::Pass);
        assert_eq!(b.judge(-100.0), Verdict::Fail);
    }
}
