//! 指摘。観測・直し方・普段が 3 つ揃って初めて直せる。
//!
//! これは軸の要求そのものである。「ここが、こちらへ、これだけ」の 3 つに、それぞれ
//! 観測・直し方・普段が対応する。

use crate::range::{Lower, Outside};
use crate::verdict::Observed;

/// 普段の言い方。判定に使った見方と揃える。
///
/// 下端を[出現割合](Lower::Appearance)で見た指標を「どこからどこまで」で言えば、
/// 文面がそれ自体で矛盾する——出てこなかったことを外れとしたのに、その指標の
/// 幅の下端は 0 なので「0 になっている。普段は 0 から N の範囲」と読める。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Usual {
    /// 幅で見た。どこからどこまでで書いてきたか。
    Spread {
        /// 下端。
        low: f64,
        /// 上端。
        high: f64,
    },
    /// 使った割合で見た。何割の単位で使ってきたか。
    Appearance {
        /// 現れた単位の割合。
        rate: f64,
        /// 多いときの値。足す先の見当が付かないと直せない。
        high: f64,
    },
}

impl Usual {
    /// 散文にする。
    fn prose(self) -> String {
        match self {
            Usual::Spread { low, high } => {
                format!("普段は{low:.3}から{high:.3}の範囲で書いている。")
            }
            Usual::Appearance { rate, high } => format!(
                "普段は{:.0}%の単位で使っており、多いときで{high:.3}になる。",
                rate * 100.0
            ),
        }
    }
}

/// 1 本の指摘。
#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    /// 指標の名前。
    pub name: String,
    /// 観測。 この文章ではどうだったか。
    pub observed: f64,
    /// 普段。 その人はこれまでどう書いてきたか。
    ///
    /// 判定に使った見方のまま持つ。
    pub usual: Usual,
    /// 直し方。 どちらへ、どうすればよいか。
    ///
    /// 指標の定義から取る。 指摘を出す側で書くと、指標を足したときに直し方の
    /// 無い指摘が出る。
    pub remedy: String,
    /// 外れの大きさ。幅を 1 とした倍数。
    pub size: f64,
}

/// 指摘が作れない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointError {
    /// 指標の定義に直し方が無い。
    ///
    /// 名前と数値だけでは直せない。 3 つ揃わないものを指摘として出さない。
    NoRemedy {
        /// どの指標か。
        name: String,
        /// どちらの向きが無いか。
        direction: &'static str,
    },
}

impl std::fmt::Display for PointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PointError::NoRemedy { name, direction } => {
                write!(f, "{name}: {direction}側の直し方が定義に無い")
            }
        }
    }
}

impl std::error::Error for PointError {}

/// 指標の定義が持つ直し方。
pub trait Remedies {
    /// 上に外れたときの直し方。
    fn upper(&self, name: &str) -> Option<String>;
    /// 下に外れたときの直し方。
    fn lower(&self, name: &str) -> Option<String>;
}

/// 指摘を組む。
///
/// 直し方が無ければ作らない。 名前と数値だけ渡しても直せない。
pub fn build(
    observed: &Observed,
    outside: Outside,
    remedies: &dyn Remedies,
) -> Result<Point, PointError> {
    let value = observed.value.expect("外れている以上、測れている");
    // 下に外れたときだけ見方が分かれる。 上に外れたなら、どちらの指標でも
    // 幅の上端を超えたという同じ事実である。
    let usual = match (outside, observed.lower) {
        (Outside::Below { .. }, Lower::Appearance { rate }) => Usual::Appearance {
            rate,
            high: observed.range.high,
        },
        _ => Usual::Spread {
            low: observed.range.low,
            high: observed.range.high,
        },
    };
    let (remedy, direction) = match outside {
        Outside::Above { .. } => (remedies.upper(&observed.name), "上"),
        Outside::Below { .. } => (remedies.lower(&observed.name), "下"),
        Outside::Inside => {
            return Err(PointError::NoRemedy {
                name: observed.name.clone(),
                direction: "中",
            })
        }
    };
    let remedy = remedy.ok_or_else(|| PointError::NoRemedy {
        name: observed.name.clone(),
        direction,
    })?;
    Ok(Point {
        name: observed.name.clone(),
        observed: value,
        usual,
        remedy,
        size: outside.size(),
    })
}

impl Point {
    /// 散文にする。
    ///
    /// 表で返さない。 構造化した形を求めると成績が落ちる。数値は落とさない——
    /// 散文にするのは言い方であって、根拠の値ではない。
    #[must_use]
    pub fn prose(&self) -> String {
        format!(
            "{}が{:.3}になっている。{}{}",
            self.name,
            self.observed,
            self.usual.prose(),
            self.remedy
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::range::{Lower, Range};
    use std::collections::BTreeMap;

    struct Table {
        upper: BTreeMap<String, String>,
        lower: BTreeMap<String, String>,
    }

    impl Remedies for Table {
        fn upper(&self, name: &str) -> Option<String> {
            self.upper.get(name).cloned()
        }
        fn lower(&self, name: &str) -> Option<String> {
            self.lower.get(name).cloned()
        }
    }

    fn table() -> Table {
        let mut upper = BTreeMap::new();
        upper.insert("全角括弧".to_owned(), "全角括弧を減らす。".to_owned());
        let mut lower = BTreeMap::new();
        lower.insert("全角括弧".to_owned(), "全角括弧を足す。".to_owned());
        Table { upper, lower }
    }

    fn observed(name: &str, value: f64, low: f64, high: f64) -> Observed {
        Observed {
            name: name.into(),
            value: Some(value),
            range: Range {
                low,
                high,
                units: 10,
            },
            lower: Lower::Spread,
            direct: false,
        }
    }

    #[test]
    fn 観測と普段と直し方が_3_つ揃う() {
        let o = observed("全角括弧", 5.0, 0.0, 2.0);
        let p = build(&o, o.range.locate(5.0), &table()).unwrap();
        assert_eq!(p.observed, 5.0);
        assert_eq!(
            p.usual,
            Usual::Spread {
                low: 0.0,
                high: 2.0
            }
        );
        assert_eq!(p.remedy, "全角括弧を減らす。");
    }

    #[test]
    fn 向きに応じた直し方を取る() {
        let o = observed("全角括弧", -1.0, 0.0, 2.0);
        let p = build(&o, o.range.locate(-1.0), &table()).unwrap();
        assert_eq!(p.remedy, "全角括弧を足す。");
    }

    #[test]
    fn 直し方が無ければ指摘を作らない() {
        // 名前と数値だけでは直せない。
        let o = observed("直し方の無い指標", 5.0, 0.0, 2.0);
        let e = build(&o, o.range.locate(5.0), &table()).unwrap_err();
        assert!(matches!(e, PointError::NoRemedy { .. }), "{e:?}");
    }

    #[test]
    fn 片側しか無ければその側で作れない() {
        // 片方しか書けないなら、それは両側ではない。
        let mut t = table();
        t.lower.clear();
        let o = observed("全角括弧", -1.0, 0.0, 2.0);
        assert!(build(&o, o.range.locate(-1.0), &t).is_err());
        let o = observed("全角括弧", 5.0, 0.0, 2.0);
        assert!(build(&o, o.range.locate(5.0), &t).is_ok(), "上は作れる");
    }

    #[test]
    fn 散文に数値が残る() {
        // 散文にするのは言い方であって、根拠の値ではない。
        let o = observed("全角括弧", 5.0, 0.0, 2.0);
        let p = build(&o, o.range.locate(5.0), &table()).unwrap();
        let s = p.prose();
        assert!(s.contains("5.000"), "{s}");
        assert!(s.contains("0.000"), "{s}");
        assert!(s.contains("2.000"), "{s}");
        assert!(s.contains("減らす"), "{s}");
    }

    #[test]
    fn 出現割合で見た指標は割合で普段を言う() {
        // **幅で言えば文面がそれ自体で矛盾する。** 下端が 0 なので「0 で、普段は
        // 0 から 2 の範囲」となり、受け取った側は範囲の中だと判断する。
        // **毎回使っているときだけ外れとする**（[線](crate::APPEARANCE_FLOOR)）。
        let mut o = observed("全角括弧", 0.0, 0.0, 2.0);
        o.lower = Lower::Appearance { rate: 1.0 };
        let p = build(&o, o.range.locate_by(0.0, o.lower), &table()).unwrap();
        assert_eq!(
            p.usual,
            Usual::Appearance {
                rate: 1.0,
                high: 2.0
            }
        );
        let s = p.prose();
        assert!(s.contains("100%"), "{s}");
        assert!(!s.contains("0.000から"), "{s}");
        assert!(s.contains("2.000"), "多いときの値は残す: {s}");
    }

    #[test]
    fn 出現割合の指標でも上に外れれば幅で言う() {
        // 多すぎる側は 0 の問題を持たないので、どちらの指標でも同じ事実である。
        let mut o = observed("全角括弧", 5.0, 0.0, 2.0);
        o.lower = Lower::Appearance { rate: 0.9 };
        let p = build(&o, o.range.locate_by(5.0, o.lower), &table()).unwrap();
        assert_eq!(
            p.usual,
            Usual::Spread {
                low: 0.0,
                high: 2.0
            }
        );
    }

    #[test]
    fn 幅の中では指摘にならない() {
        let o = observed("全角括弧", 1.0, 0.0, 2.0);
        assert!(build(&o, o.range.locate(1.0), &table()).is_err());
    }
}
