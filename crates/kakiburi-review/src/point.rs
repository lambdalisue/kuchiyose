//! 指摘。<strong>観測・直し方・普段が 3 つ揃って初めて直せる。</strong>
//!
//! これは軸の要求そのものである。「ここが、こちらへ、これだけ」の 3 つに、それぞれ
//! 観測・直し方・普段が対応する。

use crate::range::Outside;
use crate::verdict::Observed;

/// 1 本の指摘。
#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    /// 指標の名前。
    pub name: String,
    /// <strong>観測。</strong> この文章ではどうだったか。
    pub observed: f64,
    /// <strong>普段。</strong> その人はどこからどこまでで書いてきたか。
    pub usual: (f64, f64),
    /// <strong>直し方。</strong> どちらへ、どうすればよいか。
    ///
    /// <strong>指標の定義から取る。</strong> 指摘を出す側で書くと、指標を足したときに直し方の
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
    /// <strong>名前と数値だけでは直せない。</strong> 3 つ揃わないものを指摘として出さない。
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
/// <strong>直し方が無ければ作らない。</strong> 名前と数値だけ渡しても直せない。
pub fn build(
    observed: &Observed,
    outside: Outside,
    remedies: &dyn Remedies,
) -> Result<Point, PointError> {
    let value = observed.value.expect("外れている以上、測れている");
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
        usual: (observed.range.low, observed.range.high),
        remedy,
        size: outside.size(),
    })
}

impl Point {
    /// 散文にする。
    ///
    /// <strong>表で返さない。</strong> 構造化した形を求めると成績が落ちる。<strong>数値は落とさない</strong>——
    /// 散文にするのは言い方であって、根拠の値ではない。
    #[must_use]
    pub fn prose(&self) -> String {
        format!(
            "{}が{:.3}になっている。普段は{:.3}から{:.3}の範囲で書いている。{}",
            self.name, self.observed, self.usual.0, self.usual.1, self.remedy
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
        }
    }

    #[test]
    fn 観測と普段と直し方が_3_つ揃う() {
        let o = observed("全角括弧", 5.0, 0.0, 2.0);
        let p = build(&o, o.range.locate(5.0), &table()).unwrap();
        assert_eq!(p.observed, 5.0);
        assert_eq!(p.usual, (0.0, 2.0));
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
    fn 幅の中では指摘にならない() {
        let o = observed("全角括弧", 1.0, 0.0, 2.0);
        assert!(build(&o, o.range.locate(1.0), &table()).is_err());
    }
}
