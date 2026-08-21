//! 3 段の判定。
//!
//! <strong>順に当てる。返す値が決まったら、そこで終わる。</strong>
//!
//! <strong>止まった段を必ず返す。</strong> 言わなければ、受け取った側は照合値を上げようとして
//! 人らしさを下げる、という逆向きの直しをする。

use crate::range::{Lower, Outside, Range};

/// どの段で決まったか。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// 書き方。日本語として成立しているか。
    Writing,
    /// 人らしさ値。
    Humanness,
    /// 照合値。
    Matching,
    /// 指示できる指標。
    Directive,
}

impl Stage {
    /// 名前。返す文に載る。
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Stage::Writing => "書き方",
            Stage::Humanness => "人らしさ値",
            Stage::Matching => "照合値",
            Stage::Directive => "指示できる指標",
        }
    }
}

/// 段ごとの入力。<strong>測れていなければ `None` である。</strong>
///
/// `None` と「0 が出た」は別である——0 は「使わなかった」という値である。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// 人の側。
    Human,
    /// 機械の側。
    Machine,
    /// 帯の中。
    InBand,
}

/// 判定の結果。
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// 3 値。
    pub verdict: Verdict,
    /// どの段で決まったか。
    pub stage: Stage,
    /// なぜそうなったか。<strong>止めた理由を返す。</strong>
    pub reason: String,
}

/// 3 値。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// 通る。
    Pass,
    /// 通らない。
    Fail,
    /// 判定できない。
    Unknown,
}

impl Verdict {
    /// 終了コード。
    ///
    /// <strong>判定できないを 1 と分けるのが要点である。</strong> 一緒にすれば、分からないと
    /// 言えることが呼ぶ側から消える。
    #[must_use]
    pub fn exit_code(self) -> i32 {
        match self {
            Verdict::Pass => 0,
            Verdict::Fail => 1,
            Verdict::Unknown => 2,
        }
    }
}

/// 前に出す指標 1 本の観測。
#[derive(Debug, Clone, PartialEq)]
pub struct Observed {
    /// 指標の名前。
    pub name: String,
    /// 値。<strong>測れていなければ `None`。</strong>
    pub value: Option<f64>,
    /// その人の幅。
    pub range: Range,
    /// 下端の見方。<strong>単位で決まる。</strong>
    pub lower: Lower,
}

/// 検査 1 本の観測。
///
/// 比べる先は書き手ではなく日本語なので、幅ではなく線を持つ。
#[derive(Debug, Clone, PartialEq)]
pub struct Inspected {
    /// 指標の名前。
    pub name: String,
    /// 値。測れていなければ `None`。
    pub value: Option<f64>,
    /// 線。
    pub limit: f64,
    /// 線を超えたと言えるのは上側か。
    pub upper: bool,
    /// 超えたときに何が起きているか。返す文に載る。
    pub broken: String,
}

impl Inspected {
    /// 線を超えているか。測れていなければ超えていない。
    #[must_use]
    pub fn over(&self) -> bool {
        self.value.is_some_and(|v| {
            if self.upper {
                v > self.limit
            } else {
                v < self.limit
            }
        })
    }
}

/// 判定する。
///
/// <strong>1 を先に当てるのは、条件が独立だからである。</strong> 文体がどれだけ寄っても、機械が
/// 書いたと分かる文章は別の条件で落ちる。照合値が天井側にあることは、そこを何も
/// 保証しない。
#[must_use]
pub fn judge(
    inspections: &[Inspected],
    humanness: Option<Side>,
    matching: Option<Side>,
    directives: &[Observed],
) -> Outcome {
    // 0 段目。書き方。
    //
    // 日本語として成立していないものを、文体の似ている似ていないで測らない。
    // **道具が出した指示には、それを満たす壊し方がある**——文字種の空白を
    // 和文のあいだに入れれば、その指標は上がったうえで日本語が壊れる。
    if let Some(i) = inspections.iter().find(|i| i.over()) {
        return Outcome {
            verdict: Verdict::Fail,
            stage: Stage::Writing,
            reason: i.broken.clone(),
        };
    }

    // 1 段目。人らしさ値。
    match humanness {
        None => {
            return Outcome {
                verdict: Verdict::Unknown,
                stage: Stage::Humanness,
                reason: "人らしさ値が測れていない。通過の必要条件を確かめずに先へ進めない".into(),
            }
        }
        Some(Side::Machine) => {
            return Outcome {
                verdict: Verdict::Fail,
                stage: Stage::Humanness,
                reason: "人らしさ値が機械の側にある".into(),
            }
        }
        Some(Side::InBand) => {
            return Outcome {
                verdict: Verdict::Unknown,
                stage: Stage::Humanness,
                reason: "人らしさ値が帯の中にある".into(),
            }
        }
        Some(Side::Human) => {}
    }

    // 2 段目。照合値。
    match matching {
        None => {
            return Outcome {
                verdict: Verdict::Unknown,
                stage: Stage::Matching,
                reason: "照合値が出ていない。系統が欠けている".into(),
            }
        }
        Some(Side::Machine) => {
            return Outcome {
                verdict: Verdict::Fail,
                stage: Stage::Matching,
                reason: "照合値が床の側にある".into(),
            }
        }
        Some(Side::InBand) => {
            return Outcome {
                verdict: Verdict::Unknown,
                stage: Stage::Matching,
                reason: "照合値が帯の中にある".into(),
            }
        }
        Some(Side::Human) => {}
    }

    // 3 段目。指示できる指標。
    //
    // <strong>飛ばすのは個々の指標だけである。</strong> 測れていない指標を幅の中とも外とも
    // 扱わない、という意味であって、段を飛ばすことではない。
    let mut outside: Vec<(&Observed, Outside)> = directives
        .iter()
        .filter_map(|o| {
            let v = o.value?;
            let loc = o.range.locate_by(v, o.lower);
            loc.is_outside().then_some((o, loc))
        })
        .collect();

    if outside.is_empty() {
        return Outcome {
            verdict: Verdict::Pass,
            stage: Stage::Directive,
            reason: "前に出す指標がすべて幅の中にある".into(),
        };
    }

    // <strong>外れの大きさの降順。同じなら指標名の昇順。</strong>
    // 決めておかないと、上位 3〜4 本が実装ごとに変わる。
    outside.sort_by(|a, b| {
        b.1.size()
            .partial_cmp(&a.1.size())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.name.cmp(&b.0.name))
    });

    // <strong>「通らない」ではなく「判定できない」を返す。</strong> 照合値は既に天井側にある——
    // 系統で見るかぎり本人の範囲に入っている。それを覆して「通らない」と言えるだけの
    // 根拠は、細く切った 1 つの指標には無い。<strong>だが黙って通すこともしない。</strong>
    Outcome {
        verdict: Verdict::Unknown,
        stage: Stage::Directive,
        reason: format!(
            "前に出す指標 {} 本が幅の外にある（最も外れているのは {}）",
            outside.len(),
            outside[0].0.name
        ),
    }
}

/// 指摘に出す本数の上限。
///
/// <strong>上限であって、独立な指示の本数ではない。</strong> 1 本直したら別の 1 本も動く。
pub const MAX_POINTS: usize = 4;

/// 指摘を選ぶ。<strong>外れの大きさの降順、同じなら指標名の昇順。</strong>
#[must_use]
pub fn pick_points(directives: &[Observed]) -> Vec<(&Observed, Outside)> {
    let mut outside: Vec<(&Observed, Outside)> = directives
        .iter()
        .filter_map(|o| {
            let v = o.value?;
            let loc = o.range.locate_by(v, o.lower);
            loc.is_outside().then_some((o, loc))
        })
        .collect();
    outside.sort_by(|a, b| {
        b.1.size()
            .partial_cmp(&a.1.size())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.name.cmp(&b.0.name))
    });
    outside.truncate(MAX_POINTS);
    outside
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observed(name: &str, value: Option<f64>, low: f64, high: f64) -> Observed {
        Observed {
            name: name.into(),
            value,
            range: Range {
                low,
                high,
                units: 10,
            },
            lower: Lower::Spread,
        }
    }

    #[test]
    fn 人らしさが機械なら_1_段目で通らない() {
        let o = judge(&[], Some(Side::Machine), Some(Side::Human), &[]);
        assert_eq!(o.verdict, Verdict::Fail);
        assert_eq!(o.stage, Stage::Humanness);
    }

    #[test]
    fn 人らしさが測れなければ判定できない() {
        // 通過の必要条件を確かめずに先へ進めない。
        let o = judge(&[], None, Some(Side::Human), &[]);
        assert_eq!(o.verdict, Verdict::Unknown);
        assert_eq!(o.stage, Stage::Humanness);
    }

    #[test]
    fn 照合値が天井側でも人らしさが落ちれば通らない() {
        // 条件が独立である。文体がどれだけ寄っても、機械が書いたと分かる文章は
        // 別の条件で落ちる。
        let o = judge(&[], Some(Side::Machine), Some(Side::Human), &[]);
        assert_eq!(o.verdict, Verdict::Fail);
        assert_eq!(o.stage, Stage::Humanness, "照合値の段まで進まない");
    }

    #[test]
    fn 系統が欠けたら_2_段目で判定できない() {
        let o = judge(&[], Some(Side::Human), None, &[]);
        assert_eq!(o.verdict, Verdict::Unknown);
        assert_eq!(o.stage, Stage::Matching);
        assert!(o.reason.contains("系統が欠けている"), "{}", o.reason);
    }

    #[test]
    fn 照合値が帯の中なら判定できない() {
        let o = judge(&[], Some(Side::Human), Some(Side::InBand), &[]);
        assert_eq!(o.verdict, Verdict::Unknown);
        assert_eq!(o.stage, Stage::Matching);
    }

    #[test]
    fn 全部が幅の中なら通る() {
        let d = [observed("全角括弧", Some(1.0), 0.0, 2.0)];
        let o = judge(&[], Some(Side::Human), Some(Side::Human), &d);
        assert_eq!(o.verdict, Verdict::Pass);
        assert_eq!(o.stage, Stage::Directive);
    }

    #[test]
    fn 指標が幅の外なら判定できないを返す() {
        // 「通らない」ではない。照合値は既に天井側にある。
        let d = [observed("全角括弧", Some(5.0), 0.0, 2.0)];
        let o = judge(&[], Some(Side::Human), Some(Side::Human), &d);
        assert_eq!(o.verdict, Verdict::Unknown, "通らないではない");
        assert_eq!(o.stage, Stage::Directive);
        assert!(o.reason.contains("全角括弧"), "{}", o.reason);
    }

    #[test]
    fn 測れていない指標は幅の中とも外とも扱わない() {
        // 飛ばすのは個々の指標だけである。段を飛ばすことではない。
        let d = [
            observed("測れない", None, 0.0, 2.0),
            observed("幅の中", Some(1.0), 0.0, 2.0),
        ];
        let o = judge(&[], Some(Side::Human), Some(Side::Human), &d);
        assert_eq!(o.verdict, Verdict::Pass, "測れないものが外れ扱いにならない");
        assert_eq!(o.stage, Stage::Directive, "段は飛ばさない");
    }

    #[test]
    fn 止めた理由を必ず返す() {
        for (h, m) in [
            (Some(Side::Machine), Some(Side::Human)),
            (Some(Side::Human), Some(Side::Machine)),
            (Some(Side::Human), None),
            (None, None),
        ] {
            let o = judge(&[], h, m, &[]);
            assert!(!o.reason.is_empty(), "{o:?}");
        }
    }

    #[test]
    fn 判定できないは終了コードで通らないと分かれる() {
        assert_eq!(Verdict::Pass.exit_code(), 0);
        assert_eq!(Verdict::Fail.exit_code(), 1);
        assert_eq!(Verdict::Unknown.exit_code(), 2);
    }

    #[test]
    fn 指摘は外れの大きさの降順に並ぶ() {
        let d = [
            observed("小さい外れ", Some(2.5), 0.0, 2.0),
            observed("大きい外れ", Some(10.0), 0.0, 2.0),
        ];
        let picks = pick_points(&d);
        assert_eq!(picks[0].0.name, "大きい外れ");
    }

    #[test]
    fn 同じ大きさなら指標名の昇順になる() {
        // 決めておかないと、上位 3〜4 本が実装ごとに変わる。
        let d = [
            observed("絵文字", Some(5.0), 0.0, 2.0),
            observed("感嘆符", Some(5.0), 0.0, 2.0),
            observed("笑い", Some(5.0), 0.0, 2.0),
        ];
        let names: Vec<&str> = pick_points(&d)
            .iter()
            .map(|(o, _)| o.name.as_str())
            .collect();
        // 符号位置の順。五十音順やロケールの照合順は環境に依る。
        assert_eq!(names, vec!["感嘆符", "笑い", "絵文字"]);
    }

    #[test]
    fn 指摘は_4_本までに絞る() {
        let d: Vec<Observed> = (0..10)
            .map(|i| observed(&format!("m{i:02}"), Some(5.0), 0.0, 2.0))
            .collect();
        assert_eq!(pick_points(&d).len(), MAX_POINTS);
    }

    #[test]
    fn 幅_0_の指標が上位に来る() {
        let zero = Observed {
            name: "揺れない指標".into(),
            value: Some(1.0),
            range: Range {
                low: 0.0,
                high: 0.0,
                units: 10,
            },
            lower: Lower::Spread,
        };
        let big = observed("大きく外れた指標", Some(10.0), 0.0, 2.0);
        let d = [big, zero];
        let picks = pick_points(&d);
        assert_eq!(picks[0].0.name, "揺れない指標");
    }

    #[test]
    fn 出てこないことが指摘に出る() {
        // 毎回使う語が丸ごと消えているのに指摘に出ない、ということが起きてはいけない。
        let gone = Observed {
            name: "毎回使う指標".into(),
            value: Some(0.0),
            range: Range {
                low: 0.0,
                high: 5.0,
                units: 10,
            },
            lower: Lower::Appearance { rate: 1.0 },
        };
        let small = observed("少し外れた指標", Some(2.5), 0.0, 2.0);
        let d = [small, gone];
        let picks = pick_points(&d);
        assert_eq!(picks[0].0.name, "毎回使う指標", "上位に来る");
    }
}
