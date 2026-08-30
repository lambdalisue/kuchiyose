//! 幅と、そこからの外れ。
//!
//! <strong>指摘の順位付けがこの値を使う。</strong> 単位の違う指標が同じ尺度で並ぶようにする。

/// その人の幅。<strong>目盛りが作り終えて渡してくるもの。</strong>
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    /// 下端。
    pub low: f64,
    /// 上端。
    pub high: f64,
    /// この幅を作った単位の本数。<strong>幅が 0 のときの外れの大きさに使う。</strong>
    pub units: usize,
}

/// 外れているか。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outside {
    /// 幅の中。
    Inside,
    /// 上に外れている。
    Above {
        /// 幅を 1 とした倍数。
        size: f64,
    },
    /// 下に外れている。
    Below {
        /// 幅を 1 とした倍数。
        size: f64,
    },
}

/// 下端の見方。<strong>単位で決まる。</strong> 指標ごとに決めない。
///
/// | 単位 | どちらか |
/// | --- | --- |
/// | 密度・個数・その現象の出現に対する割合 | [`Lower::Appearance`] |
/// | 無次元・常に値を持つ割合 | [`Lower::Spread`] |
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Lower {
    /// 幅の下端をそのまま使う。
    Spread,
    /// <strong>使った割合で見る。</strong>
    ///
    /// 0 を取りうる指標では、下端を「最小値を下回る」で判定してはいけない——一度でも
    /// 使わなかった文書があれば最小値は 0 になり、それを下回ることはないので
    /// <strong>下限の判定が永久に効かなくなる。</strong>
    Appearance {
        /// 本人の単位のうち、この指標が現れた文書の割合。
        rate: f64,
    },
}

/// 出現割合がこれ以上なら、出てこないことを外れとする。
///
/// <strong>毎回使っているときだけである。</strong> 8 割で切ると、<strong>残りの 2 割はその人自身である</strong>
/// ——出てこない文書をその人が現に書いているのに、それを「あなたらしくない」と言うことに
/// なる。
///
/// 実測で数えた。前に出す指標 3 本の出現割合は 0.90 / 0.84 / 0.84 で、<strong>どれか 1 本が
/// 誤って止める見込みは 36%</strong> になる。カセットから抜いた本人の記事 8 本のうち
/// <strong>3 本が実際にこれで止まっていた。</strong>
///
/// <strong>1.0 にしても、この規則が拾いたいものは拾える。</strong> 元の意図は
/// [毎回使う語が丸ごと消えている](appearance_size)ことを見逃さないことであって、
/// たまに使わない語を数えることではない。
pub const APPEARANCE_FLOOR: f64 = 1.0;

/// 出現割合を幅と同じ単位に直す。
///
/// > p / (1 − p)
///
/// <strong>混ぜて並べれば、出てこないことは永久に上位に来ない</strong>——前者は 2.5 や 4.0 に
/// なり、後者は 0.8 から 1.0 にしかならない。毎回使う語が丸ごと消えているのに指摘に
/// 出ない、ということが起きる。
///
/// <strong>p は 1.0 になりうるので頭を抑える。</strong> 単位が n 本なら上限を n /(n + 1) とする——
/// 抑えなければ無限大に飛び、比べる相手がいなくなる。
#[must_use]
pub fn appearance_size(rate: f64, units: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let cap = units as f64 / (units as f64 + 1.0);
    let p = rate.min(cap).clamp(0.0, 1.0);
    if p >= 1.0 {
        // 抑えたうえでなお 1.0 なら、単位が 0 本である。
        return 0.0;
    }
    p / (1.0 - p)
}

impl Range {
    /// 幅の大きさ。
    #[must_use]
    pub fn spread(self) -> f64 {
        self.high - self.low
    }

    /// 下端の見方を選んで判定する。
    ///
    /// <strong>密度・個数の下端は幅で見ない。</strong> 上端はどちらでも幅で見る——多すぎる側は
    /// 0 の問題を持たない。
    #[must_use]
    pub fn locate_by(self, value: f64, lower: Lower) -> Outside {
        match lower {
            Lower::Spread => self.locate(value),
            Lower::Appearance { rate } => {
                if value > self.high {
                    return self.locate(value);
                }
                // <strong>出てこないことだけを外れとする。</strong> 少ないことは外れではない——
                // 幅の下端が 0 なら、下回る値が存在しない。
                if value == 0.0 && rate >= APPEARANCE_FLOOR {
                    return Outside::Below {
                        size: appearance_size(rate, self.units),
                    };
                }
                Outside::Inside
            }
        }
    }

    /// 値がどこにあるか。
    ///
    /// <strong>外れの大きさ ＝ 幅の端からはみ出した量 ÷ 幅の大きさ。</strong>
    ///
    /// <strong>幅が 0 のときは、この式が使えない。</strong> 全部の単位で同じ値だった指標である。
    /// 0 を分母にせず、大きさを<strong>単位の本数</strong>とする——その人が一度も揺れなかった点が
    /// 動いている以上、弱い信号ではない。
    #[must_use]
    pub fn locate(self, value: f64) -> Outside {
        let spread = self.spread();
        #[allow(clippy::cast_precision_loss)]
        let zero_spread_size = self.units as f64;
        if value > self.high {
            let over = value - self.high;
            return Outside::Above {
                size: if spread > 0.0 {
                    over / spread
                } else {
                    zero_spread_size
                },
            };
        }
        if value < self.low {
            let under = self.low - value;
            return Outside::Below {
                size: if spread > 0.0 {
                    under / spread
                } else {
                    zero_spread_size
                },
            };
        }
        Outside::Inside
    }
}

impl Outside {
    /// 外れているか。
    #[must_use]
    pub fn is_outside(self) -> bool {
        !matches!(self, Outside::Inside)
    }

    /// 外れの大きさ。中なら 0。
    #[must_use]
    pub fn size(self) -> f64 {
        match self {
            Outside::Inside => 0.0,
            Outside::Above { size } | Outside::Below { size } => size,
        }
    }

    /// どちらへ直すか。
    #[must_use]
    pub fn direction(self) -> Option<&'static str> {
        match self {
            Outside::Inside => None,
            Outside::Above { .. } => Some("減らす"),
            Outside::Below { .. } => Some("増やす"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(low: f64, high: f64) -> Range {
        Range {
            low,
            high,
            units: 10,
        }
    }

    #[test]
    fn 密度の下端は幅で見ない() {
        // 一度でも使わなかった文書があれば最小値は 0 になり、それを下回ることは
        // ないので、<strong>下限の判定が永久に効かなくなる。</strong>
        let r = range(0.0, 5.0);
        assert_eq!(r.locate(0.0), Outside::Inside, "幅では永久に効かない");
        assert_eq!(
            r.locate_by(0.0, Lower::Appearance { rate: 1.0 }),
            Outside::Below {
                size: appearance_size(1.0, 10)
            },
            "出現割合なら外れになる"
        );
    }

    #[test]
    fn 出現割合が低ければ出てこなくても外れではない() {
        let r = range(0.0, 5.0);
        assert_eq!(
            r.locate_by(0.0, Lower::Appearance { rate: 0.5 }),
            Outside::Inside
        );
    }

    #[test]
    fn 少ないことは外れではない() {
        // 幅の下端が 0 なら、下回る値が存在しない。
        let r = range(0.0, 5.0);
        assert_eq!(
            r.locate_by(0.1, Lower::Appearance { rate: 1.0 }),
            Outside::Inside
        );
    }

    #[test]
    fn 上端はどちらの見方でも幅で見る() {
        // 多すぎる側は 0 の問題を持たない。
        let r = range(0.0, 5.0);
        assert_eq!(
            r.locate_by(10.0, Lower::Appearance { rate: 1.0 }),
            Outside::Above { size: 1.0 }
        );
    }

    #[test]
    fn 出現割合は幅と同じ単位に直す() {
        // 0.8 では 4.0 になる。<strong>幅 4 つ分の外れと同じ重さで並ぶ。</strong>
        assert!((appearance_size(0.8, 100) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn 出現割合の頭を抑える() {
        // 抑えなければ無限大に飛び、比べる相手がいなくなる。
        let size = appearance_size(1.0, 10);
        assert!(size.is_finite(), "{size}");
        // 10 本なら上限は 10/11 なので 10.0 になる。
        assert!((size - 10.0).abs() < 1e-9, "{size}");
    }

    #[test]
    fn 単位が_0_本なら大きさを持たない() {
        assert_eq!(appearance_size(1.0, 0), 0.0);
    }

    #[test]
    fn 外れの大きさは幅を_1_とした倍数である() {
        // 幅 10〜20 の指標で 45 なら、はみ出し 25 ÷ 幅 10 = 2.5。
        let r = range(10.0, 20.0);
        assert_eq!(r.locate(45.0), Outside::Above { size: 2.5 });
    }

    #[test]
    fn 単位の違う指標が同じ尺度で並ぶ() {
        // 幅 0.1〜0.2 で 0.45 も、幅 10〜20 で 45 も、同じ 2.5 になる。
        let small = range(0.1, 0.2);
        let large = range(10.0, 20.0);
        assert!((small.locate(0.45).size() - large.locate(45.0).size()).abs() < 1e-9);
    }

    #[test]
    fn 幅の中なら外れていない() {
        let r = range(10.0, 20.0);
        assert_eq!(r.locate(15.0), Outside::Inside);
        assert_eq!(r.locate(10.0), Outside::Inside, "端は中である");
        assert_eq!(r.locate(20.0), Outside::Inside, "端は中である");
    }

    #[test]
    fn 下に外れれば増やす向きになる() {
        let r = range(10.0, 20.0);
        let o = r.locate(5.0);
        assert_eq!(o, Outside::Below { size: 0.5 });
        assert_eq!(o.direction(), Some("増やす"));
    }

    #[test]
    fn 上に外れれば減らす向きになる() {
        let r = range(10.0, 20.0);
        assert_eq!(r.locate(30.0).direction(), Some("減らす"));
    }

    #[test]
    fn 幅が_0_なら単位の本数を大きさにする() {
        // 0 を分母にしない。一度も揺れなかった点が動いている以上、
        // 弱い信号ではない——上位に来る側である。
        let r = Range {
            low: 0.0,
            high: 0.0,
            units: 10,
        };
        assert_eq!(r.locate(1.0), Outside::Above { size: 10.0 });
        assert_eq!(r.locate(-1.0), Outside::Below { size: 10.0 });
        assert_eq!(r.locate(0.0), Outside::Inside);
    }

    #[test]
    fn 幅が_0_の外れは普通の外れより上位に来る() {
        let zero = Range {
            low: 0.0,
            high: 0.0,
            units: 10,
        };
        let normal = range(10.0, 20.0);
        assert!(zero.locate(1.0).size() > normal.locate(45.0).size());
    }
}
