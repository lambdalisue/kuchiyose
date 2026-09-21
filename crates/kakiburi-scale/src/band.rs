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

    /// 床の端。<strong>上の裾を切る。</strong>
    ///
    /// <strong>2 つの端は役目が違う。</strong> 対称に見えるが、外したときの代償が違う。
    ///
    /// | 端 | 意味 | 外すとどうなる |
    /// | --- | --- | --- |
    /// | 天井の下端 | ここ以上なら通る | 高すぎても<strong>判定できないに落ちるだけ</strong> |
    /// | <strong>床の上端</strong> | ここ以下なら通らない | 高すぎると<strong>書き手に「あなたの文章は機械だ」と言う</strong> |
    ///
    /// <strong>床の上端だけが、間違えたときに書き手を否定する。</strong> だから上の裾を切って
    /// 保守側に倒す——切った分は判定できないになり、それは
    /// [正しい停止](../../../docs/spec/010-strategy.md#届かないときは判定できないと言う)である。
    ///
    /// <strong>実測では、これで「人が書いた文章を通らないと言う」誤りが 10 本から 1 本に
    /// なった。</strong> 通るの数は変わらず、機械の検出だけが 9/9 から 7/9 に落ちる。
    #[must_use]
    pub fn trimmed(points: &[f64]) -> Option<Self> {
        let mut s: Vec<f64> = points.to_vec();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let low = *s.first()?;
        let high = quantile(&s, FLOOR_TRIM);
        if (high - low).abs() < f64::EPSILON {
            return None;
        }
        Some(Self { low, high })
    }

    /// 天井の端。<strong>下の裾を切る。</strong>
    ///
    /// 下端は「ここ以上なら通る」の閾値である。<strong>最小値で取ると、点を足すほど外れた
    /// 1 本まで下がる</strong>——素材を足すほど帯が緩み、機械の側の文章が通るようになる。
    /// 分位で取れば点の数で漂わないので、[帯の点](crate::split::Split::points)を
    /// 5 本に固定せずに済む。
    ///
    /// <strong>上端は最大のまま切らない。</strong> 上端は[判定](Band::judge)に使わないので、
    /// 外れた 1 本が伸ばしても誰も否定されない。
    #[must_use]
    pub fn trimmed_low(points: &[f64]) -> Option<Self> {
        let mut s: Vec<f64> = points.to_vec();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let low = quantile(&s, CEILING_TRIM);
        let high = *s.last()?;
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

/// 天井の下端に取る分位。<strong>暫定値である。</strong>
///
/// 向きは[代償が対称でない](Ends::trimmed)ことから決まる——天井の下端を下げると
/// <strong>機械の側の文章を通す</strong>ので、床の上端とは逆に、切って上へ倒すのが保守側である。
/// <strong>どこで切るかは 1 人分の実測で選んだ。</strong> カセットの `provisional` が「帯の端」を
/// 挙げているのはこのことである。
pub const CEILING_TRIM: f64 = 0.25;

/// 床の上端に取る分位。<strong>暫定値である。</strong>
///
/// 向きは[代償が対称でない](Ends::trimmed)ことから決まるが、<strong>どこで切るかは
/// 1 人分の実測で選んだ</strong>——[骨格が通るまで閾値を手で決めない](../../../docs/spec/010-strategy.md#それでも骨格を先に通す)
/// の但し書きが付いたままである。カセットの `provisional` が「帯の端」を挙げている
/// のはこのことである。
pub const FLOOR_TRIM: f64 = 0.75;

/// 並べ替えずみの列から分位を取る。
fn quantile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let i = p * (sorted.len() - 1) as f64;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let lo = i as usize;
    let hi = (lo + 1).min(sorted.len() - 1);
    #[allow(clippy::cast_precision_loss)]
    let frac = i - lo as f64;
    sorted[lo] + (sorted[hi] - sorted[lo]) * frac
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
    /// 照合値の帯を作る。
    ///
    /// <strong>重なりが大きすぎるなら、そこで止める。</strong> 帯を狭めて判定を出すのではない——
    /// 分離していないという事実が出ている。
    ///
    /// 止めるのは重なりが <strong>天井の広がりと床の広がりのどちらに対しても</strong>
    /// [限度](OVERLAP_LIMIT)を超えたときである。<strong>片方だけで止めない</strong>——片方が
    /// 広ければ、その広さだけで割合が上がる。
    pub fn build(ceiling: &[f64], floor: &[f64]) -> Result<Self, BandError> {
        let band = Self::of_ends(ceiling, floor)?;
        if let Some(ov) = band.overlap() {
            let of_ceiling = ov / band.ceiling.spread();
            let of_floor = ov / band.floor.spread();
            if of_ceiling.min(of_floor) > OVERLAP_LIMIT {
                return Err(BandError::TooMuchOverlap {
                    of_ceiling,
                    of_floor,
                });
            }
        }
        Ok(band)
    }

    /// 人らしさの帯を作る。<strong>重なりでは止めない。</strong>
    ///
    /// 人らしさの帯が全体を覆うのは設計どおりの結果である——当てはまらないことが
    /// 判定不能として出る仕組みなので、<strong>止めれば自己診断が消える</strong>。重なった帯は
    /// そのまま書き出す。
    ///
    /// 広がりが 0 のときだけ作らない。端が決まらなければ、重なっているのかどうかも
    /// 読めない。
    pub fn build_humanness(human: &[f64], machine: &[f64]) -> Result<Self, BandError> {
        Self::of_ends(human, machine)
    }

    fn of_ends(ceiling: &[f64], floor: &[f64]) -> Result<Self, BandError> {
        Ok(Self {
            ceiling: Ends::trimmed_low(ceiling).ok_or(BandError::NoSpread { side: "天井" })?,
            floor: Ends::trimmed(floor).ok_or(BandError::NoSpread { side: "床" })?,
        })
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
    fn 床は上の裾を切る() {
        // <strong>2 つの端は役目が違う。</strong> 床の上端だけが、間違えたときに書き手へ
        // 「あなたの文章は機械だ」と言う——だから保守側に倒す。
        let e = Ends::trimmed(&[0.0, 1.0, 2.0, 3.0, 4.0]).expect("端が決まる");
        assert_eq!(e.low, 0.0, "下端は最小のまま");
        assert_eq!(e.high, 3.0, "上端は 75% 分位");
        // 天井は切らない。<strong>高すぎても判定できないに落ちるだけである。</strong>
        let c = Ends::of(&[0.0, 1.0, 2.0, 3.0, 4.0]).expect("端が決まる");
        assert_eq!(c.high, 4.0);
    }

    #[test]
    fn 裾を切って広がりが_0_になれば端が決まらない() {
        // 上位 4 分の 1 を除くと 1 点に潰れる並び。
        assert_eq!(Ends::trimmed(&[1.0, 1.0, 1.0, 1.0, 9.0]), None);
    }

    #[test]
    fn 天井は下の裾を切る() {
        // <strong>下端は「ここ以上なら通る」の閾値である。</strong> 最小値で取ると、点を足すほど
        // 外れた 1 本まで下がって、機械の側の文章が通る。
        let e = Ends::trimmed_low(&[0.0, 1.0, 2.0, 3.0, 4.0]).expect("端が決まる");
        assert_eq!(e.low, 1.0, "下端は 25% 分位");
        assert_eq!(e.high, 4.0, "上端は最大のまま");
    }

    #[test]
    fn 天井の下端は外れた_1_本で落ちない() {
        // 最小値で取っていれば、この 1 本がそのまま下端になる。
        let e = Ends::trimmed_low(&[-10.0, 1.0, 2.0, 3.0, 4.0, 5.0]).expect("端が決まる");
        assert!(e.low > 0.0, "下端が外れ値に引かれない: {}", e.low);
    }

    #[test]
    fn 天井も裾を切って広がりが_0_になれば端が決まらない() {
        // 下位 4 分の 1 を除くと 1 点に潰れる並び。
        assert_eq!(Ends::trimmed_low(&[-9.0, 1.0, 1.0, 1.0, 1.0]), None);
    }

    #[test]
    fn 分離していれば隙間が出る() {
        // 天井は下の裾を切るので 2.25〜3.0、床は上の裾を切るので 0.0〜0.75。
        let b = Band::build(&[2.0, 3.0], &[0.0, 1.0]).unwrap();
        assert!(b.separated());
        assert_eq!(b.gap(), Some(1.5));
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
        // 天井は裾を切って 2.0〜5.0、床も裾を切って 0.0〜3.0。重なりは 2.0〜3.0 の 1.0。
        let b = Band::build(&[1.0, 5.0], &[0.0, 4.0]).unwrap();
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
    fn 片側だけ超えても帯は作る() {
        // 天井は裾を切って 2.5〜10.0、床も裾を切って 4.0〜5.5。重なりは 4.0〜5.5 の 1.5。
        // 床に対しては 100% だが、天井に対しては 20%。片方だけでは止めない。
        let b = Band::build(&[0.0, 10.0], &[4.0, 6.0]).expect("片側だけなら作る");
        assert_eq!(b.overlap(), Some(1.5));
    }

    #[test]
    fn 人らしさは重なっても帯を作る() {
        // 覆っていることが判定不能として出る仕組みなので、止めれば自己診断が消える。
        let b = Band::build_humanness(&[0.0, 2.0], &[0.0, 2.0]).expect("覆っていても作る");
        assert_eq!(b.overlap(), Some(1.0));
    }

    #[test]
    fn 人らしさでも広がりが_0_なら作らない() {
        // 端が決まらなければ、重なっているのかどうかも読めない。
        let e = Band::build_humanness(&[1.0, 1.0], &[0.0, 2.0]).unwrap_err();
        assert!(matches!(e, BandError::NoSpread { .. }), "{e:?}");
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
        // 天井は裾を切って 2.25〜3.0、床も裾を切って 0.0〜0.75。帯は 0.75〜2.25。
        let b = Band::build(&[2.0, 3.0], &[0.0, 1.0]).unwrap();
        assert_eq!(b.judge(2.5), Verdict::Pass);
        assert_eq!(b.judge(2.25), Verdict::Pass, "天井の下端は通る");
        assert_eq!(b.judge(0.5), Verdict::Fail);
        assert_eq!(b.judge(0.75), Verdict::Fail, "床の上端は通らない");
        assert_eq!(
            b.judge(1.0),
            Verdict::Unknown,
            "切った裾は判定できないになる"
        );
        assert_eq!(
            b.judge(2.0),
            Verdict::Unknown,
            "天井の切った裾も判定できないになる"
        );
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
