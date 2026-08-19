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
    /// 人らしさの直し方。<strong>書きぶりの枠を奪わない。</strong>
    ///
    /// 3〜4 本という上限は書きぶりの側の話であり、人らしさはそこに数えない
    /// （[指標](../../../docs/spec/100-metrics.md#指標には-3-種類ある)）。
    /// <strong>枠を奪い合わせると、機械臭さを消す指示と、その人へ寄せる指示が、席を
    /// 取り合う。</strong>
    pub humanness: Vec<String>,
}

/// 人らしさの、指標ごとの観測。<strong>正が人の側、負が機械の側。</strong>
///
/// <strong>合算した 1 つの値では直し方を渡せない。</strong>「機械の側にある」としか言えず、
/// どこをどうすればよいかが出てこない。
#[derive(Debug, Clone, PartialEq)]
pub struct HumannessObserved {
    /// 指標の名前。
    pub name: String,
    /// その指標だけで見た人らしさ値。
    pub value: f64,
    /// <strong>人へ寄せる向き。</strong> `true` なら値を上げる。
    ///
    /// <strong>較正が決める。</strong> 定義に固定すると、素材がその向きを支えていない
    /// カセットで直し方に従うほど人らしさが下がる。
    pub raise: bool,
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
    humanness_by_metric: &[HumannessObserved],
) -> Review {
    let outcome = judge(humanness, matching, directives);
    // <strong>3 段目まで進んだときだけ書きぶりの指摘を組む。</strong> 前の段で止まったなら、
    // 出しても受け取った側は逆向きの直しをする。
    let points = if outcome.stage == Stage::Directive {
        pick_points(directives)
            .into_iter()
            .filter_map(|(o, loc)| point::build(o, loc, remedies).ok())
            .collect()
    } else {
        Vec::new()
    };
    // <strong>1 段目で止まったときにこそ、人らしさの直し方を渡す。</strong> ここで何も出さな
    // ければ、機械の書いた草稿は<strong>止められるだけで直せない</strong>——道具が使われる
    // 場面で出力が空になる。
    let humanness = if outcome.stage == Stage::Humanness && outcome.verdict == Verdict::Fail {
        humanness_remedies(humanness_by_metric, remedies)
    } else {
        Vec::new()
    };
    Review {
        outcome,
        points,
        humanness,
    }
}

/// 機械の側に落ちている指標の直し方を、落ちている順に並べる。
///
/// <strong>向きは観測が持っている。</strong> 較正から読んだもので、定義の名乗る向きとは限らない
/// ——同じ型で書かせた生成文は、先行研究の言う「機械の側」に来ないことがある。
fn humanness_remedies(observed: &[HumannessObserved], remedies: &dyn Remedies) -> Vec<String> {
    let mut low: Vec<&HumannessObserved> = observed.iter().filter(|o| o.value < 0.0).collect();
    low.sort_by(|a, b| {
        a.value
            .partial_cmp(&b.value)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });
    low.iter()
        .filter_map(|o| {
            // <strong>観測が持っている向きで選ぶ。</strong> 上げるなら「足りない側の直し方」、
            // 下げるなら「多すぎる側の直し方」——[指摘](point::build)が外れた向きで
            // 選ぶのと同じ規則である。
            //
            // <strong>下と決め打ってはいけない。</strong> 人らしさの指標は両側の直し方を持つので、
            // 決め打つと必ず「足す」側が返り、<strong>較正が「減らせ」と言った場面でも
            // 「増やせ」と指示する</strong>——直し方に従うほど人らしさが下がる。
            let remedy = if o.raise {
                remedies.lower(&o.name).or_else(|| remedies.upper(&o.name))
            } else {
                remedies.upper(&o.name).or_else(|| remedies.lower(&o.name))
            }?;
            Some(format!(
                "{}が機械の側にある（{:.3}）。{remedy}",
                o.name, o.value
            ))
        })
        .collect()
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
        let r = review(Some(Side::Human), Some(Side::Human), &d, &All, &[]);
        assert_eq!(r.outcome.verdict, Verdict::Pass);
        assert!(r.points.is_empty());
    }

    fn human(name: &str, value: f64) -> HumannessObserved {
        HumannessObserved {
            name: name.into(),
            value,
            raise: true,
        }
    }

    #[test]
    fn 人らしさで止まったら人らしさの直し方を渡す() {
        // **ここで何も出さなければ、機械の書いた草稿は止められるだけで直せない。**
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let h = [human("繰り返し", -1.2), human("圧縮率", 0.4)];
        let r = review(Some(Side::Machine), Some(Side::Human), &d, &All, &h);
        assert_eq!(r.outcome.stage, Stage::Humanness);
        assert!(r.points.is_empty(), "書きぶりの枠は奪わない");
        assert_eq!(r.humanness.len(), 1, "機械の側に落ちているのは 1 本");
        assert!(r.humanness[0].contains("繰り返し"), "{:?}", r.humanness);
        assert!(r.humanness[0].contains("足す"), "{:?}", r.humanness);
    }

    #[test]
    fn 人らしさの直し方は落ちている順に並ぶ() {
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let h = [human("圧縮率", -0.3), human("繰り返し", -1.5)];
        let r = review(Some(Side::Machine), Some(Side::Human), &d, &All, &h);
        assert!(r.humanness[0].contains("繰り返し"), "{:?}", r.humanness);
        assert!(r.humanness[1].contains("圧縮率"), "{:?}", r.humanness);
    }

    #[test]
    fn 人の側にある指標は直させない() {
        // 直せば人らしさが下がる。**寄せる向きは 1 つしかない。**
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let h = [human("繰り返し", 0.8), human("圧縮率", 0.4)];
        let r = review(Some(Side::Machine), Some(Side::Human), &d, &All, &h);
        assert!(r.humanness.is_empty(), "{:?}", r.humanness);
    }

    #[test]
    fn 人らしさが測れないときは直し方を出さない() {
        // **測れていないことと、機械の側にあることは違う。** 混ぜれば、短いだけの
        // 文章に「繰り返しを足せ」と言うことになる。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let r = review(None, Some(Side::Human), &d, &All, &[]);
        assert_eq!(r.outcome.verdict, Verdict::Unknown);
        assert!(r.humanness.is_empty());
    }

    #[test]
    fn 通るときは人らしさの直し方も無い() {
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let h = [human("繰り返し", -1.2)];
        let r = review(Some(Side::Human), Some(Side::Human), &d, &All, &h);
        assert!(r.humanness.is_empty(), "{:?}", r.humanness);
    }

    #[test]
    fn 寄せる向きは観測が持つ() {
        // <strong>人らしさの指標は両側の直し方を持つ。</strong> 下と決め打てば必ず「足す」側が
        // 返り、<strong>較正が「減らせ」と言った場面でも「増やせ」と指示する</strong>——直し方に
        // 従うほど人らしさが下がる。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];

        let mut down = human("圧縮率", -0.9);
        down.raise = false;
        let r = review(Some(Side::Machine), Some(Side::Human), &d, &All, &[down]);
        assert_eq!(r.humanness.len(), 1, "{:?}", r.humanness);
        assert!(r.humanness[0].contains("減らす"), "{:?}", r.humanness);

        let r = review(
            Some(Side::Machine),
            Some(Side::Human),
            &d,
            &All,
            &[human("圧縮率", -0.9)],
        );
        assert!(r.humanness[0].contains("足す"), "{:?}", r.humanness);
    }

    #[test]
    fn 片側しか無い指標でも直し方を出す() {
        // 向きが「上げる」でも、定義が下側の文面を持たないなら上側で出す。
        // <strong>3 つ揃わないものを出さない</strong>より、<strong>持っている側で出す</strong>ほうが直せる。
        struct UpperOnly;
        impl Remedies for UpperOnly {
            fn upper(&self, name: &str) -> Option<String> {
                Some(format!("{name}を減らす。"))
            }
            fn lower(&self, _: &str) -> Option<String> {
                None
            }
        }
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let h = [human("圧縮率", -0.9)];
        let r = review(Some(Side::Machine), Some(Side::Human), &d, &UpperOnly, &h);
        assert_eq!(r.humanness.len(), 1, "{:?}", r.humanness);
        assert!(r.humanness[0].contains("減らす"), "{:?}", r.humanness);
    }

    #[test]
    fn 直し方が定義に無ければ出さない() {
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let h = [human("繰り返し", -1.2)];
        let r = review(Some(Side::Machine), Some(Side::Human), &d, &None_, &h);
        assert!(r.humanness.is_empty(), "{:?}", r.humanness);
    }

    #[test]
    fn 前の段で止まったら指摘を出さない() {
        // 出せば、受け取った側は照合値を上げようとして人らしさを下げる。
        let d = [observed("全角括弧", 5.0, 0.0, 2.0)];
        let r = review(Some(Side::Machine), Some(Side::Human), &d, &All, &[]);
        assert_eq!(r.outcome.stage, Stage::Humanness);
        assert!(r.points.is_empty(), "指摘を出してはいけない");
    }

    #[test]
    fn 照合値で止まっても指摘を出さない() {
        let d = [observed("全角括弧", 5.0, 0.0, 2.0)];
        let r = review(Some(Side::Human), Some(Side::InBand), &d, &All, &[]);
        assert_eq!(r.outcome.stage, Stage::Matching);
        assert!(r.points.is_empty());
    }

    #[test]
    fn 三段目まで来たら指摘を組む() {
        let d = [
            observed("全角括弧", 5.0, 0.0, 2.0),
            observed("感嘆符", 10.0, 0.0, 1.0),
        ];
        let r = review(Some(Side::Human), Some(Side::Human), &d, &All, &[]);
        assert_eq!(r.outcome.verdict, Verdict::Unknown);
        assert_eq!(r.points.len(), 2);
        // 外れの大きさの降順。感嘆符は 9.0、全角括弧は 1.5。
        assert_eq!(r.points[0].name, "感嘆符");
    }

    #[test]
    fn 指摘は判定と同じ集合から取る() {
        // 止めた理由が指摘に出てこないという食い違いを防ぐ。
        let d = [observed("全角括弧", 5.0, 0.0, 2.0)];
        let r = review(Some(Side::Human), Some(Side::Human), &d, &All, &[]);
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
        let r = review(Some(Side::Human), Some(Side::Human), &d, &None_, &[]);
        assert_eq!(r.outcome.verdict, Verdict::Unknown);
        assert!(r.points.is_empty());
    }

    #[test]
    fn 指摘は_4_本までである() {
        let d: Vec<Observed> = (0..10)
            .map(|i| observed(&format!("m{i:02}"), 5.0, 0.0, 2.0))
            .collect();
        let r = review(Some(Side::Human), Some(Side::Human), &d, &All, &[]);
        assert_eq!(r.points.len(), MAX_POINTS);
    }
}
