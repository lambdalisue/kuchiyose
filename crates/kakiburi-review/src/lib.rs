//! 検め。3 段の判定と、3〜4 本の指摘。
//!
//! <strong>目盛りを作る側に依存しない。</strong> 依存すると、検める文書を見てから重みや語彙を
//! 作り直せる経路が開く。目盛りは作り終えた形で渡ってくるものであって、ここで
//! 作るものではない。
//!
//! この境界は `Cargo.toml` が守っている——`kakiburi-scale` を依存に持たない。

pub mod point;
pub mod range;
pub mod verdict;

pub use point::{Point, PointError, Remedies};
pub use range::{appearance_size, Lower, Outside, Range, APPEARANCE_FLOOR};
pub use verdict::{judge, pick_points, Observed, Outcome, Side, Stage, Verdict, MAX_POINTS};

/// 検めた結果。<strong>3 値と指摘を返す。</strong>
#[derive(Debug, Clone, PartialEq)]
pub struct Review {
    /// 判定と、止まった段と理由。
    pub outcome: Outcome,
    /// 指摘。<strong>最大 4 本。</strong>
    pub points: Vec<Point>,
}

/// 検める。
///
/// <strong>指摘は判定と同じ集合から取る。</strong> 別々に定めれば、止めた理由が指摘に出てこない
/// という食い違いが起きる。
#[must_use]
pub fn review(
    humanness: Option<Side>,
    matching: Option<Side>,
    directives: &[Observed],
    remedies: &dyn Remedies,
) -> Review {
    let outcome = judge(humanness, matching, directives);
    // <strong>3 段目まで進んだときだけ指摘を組む。</strong> 前の段で止まったなら、指摘を出しても
    // 受け取った側は逆向きの直しをする。
    let points = if outcome.stage == Stage::Directive {
        pick_points(directives)
            .into_iter()
            .filter_map(|(o, loc)| point::build(o, loc, remedies).ok())
            .collect()
    } else {
        Vec::new()
    };
    Review { outcome, points }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct All;
    impl Remedies for All {
        fn upper(&self, name: &str) -> Option<String> {
            Some(format!("{name}を減らす。"))
        }
        fn lower(&self, name: &str) -> Option<String> {
            Some(format!("{name}を足す。"))
        }
    }

    struct None_;
    impl Remedies for None_ {
        fn upper(&self, _: &str) -> Option<String> {
            None
        }
        fn lower(&self, _: &str) -> Option<String> {
            None
        }
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
    fn 通るときは指摘が無い() {
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let r = review(Some(Side::Human), Some(Side::Human), &d, &All);
        assert_eq!(r.outcome.verdict, Verdict::Pass);
        assert!(r.points.is_empty());
    }

    #[test]
    fn 前の段で止まったら指摘を出さない() {
        // 出せば、受け取った側は照合値を上げようとして人らしさを下げる。
        let d = [observed("全角括弧", 5.0, 0.0, 2.0)];
        let r = review(Some(Side::Machine), Some(Side::Human), &d, &All);
        assert_eq!(r.outcome.stage, Stage::Humanness);
        assert!(r.points.is_empty(), "指摘を出してはいけない");
    }

    #[test]
    fn 照合値で止まっても指摘を出さない() {
        let d = [observed("全角括弧", 5.0, 0.0, 2.0)];
        let r = review(Some(Side::Human), Some(Side::InBand), &d, &All);
        assert_eq!(r.outcome.stage, Stage::Matching);
        assert!(r.points.is_empty());
    }

    #[test]
    fn 三段目まで来たら指摘を組む() {
        let d = [
            observed("全角括弧", 5.0, 0.0, 2.0),
            observed("感嘆符", 10.0, 0.0, 1.0),
        ];
        let r = review(Some(Side::Human), Some(Side::Human), &d, &All);
        assert_eq!(r.outcome.verdict, Verdict::Unknown);
        assert_eq!(r.points.len(), 2);
        // 外れの大きさの降順。感嘆符は 9.0、全角括弧は 1.5。
        assert_eq!(r.points[0].name, "感嘆符");
    }

    #[test]
    fn 指摘は判定と同じ集合から取る() {
        // 止めた理由が指摘に出てこないという食い違いを防ぐ。
        let d = [observed("全角括弧", 5.0, 0.0, 2.0)];
        let r = review(Some(Side::Human), Some(Side::Human), &d, &All);
        assert!(
            r.outcome.reason.contains("全角括弧"),
            "{}",
            r.outcome.reason
        );
        assert_eq!(r.points[0].name, "全角括弧");
    }

    #[test]
    fn 直し方の無い指標は指摘に出ない() {
        // だが判定は止まったままである——言えないもので止めない、の逆側は
        // 「言えるものがある以上、言って次の周へ回す」である。
        let d = [observed("全角括弧", 5.0, 0.0, 2.0)];
        let r = review(Some(Side::Human), Some(Side::Human), &d, &None_);
        assert_eq!(r.outcome.verdict, Verdict::Unknown);
        assert!(r.points.is_empty());
    }

    #[test]
    fn 指摘は_4_本までである() {
        let d: Vec<Observed> = (0..10)
            .map(|i| observed(&format!("m{i:02}"), 5.0, 0.0, 2.0))
            .collect();
        let r = review(Some(Side::Human), Some(Side::Human), &d, &All);
        assert_eq!(r.points.len(), MAX_POINTS);
    }
}
