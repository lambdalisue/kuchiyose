//! 3 段の判定。
//!
//! 順に当てる。返す値が決まったら、そこで終わる。
//!
//! 止まった段を必ず返す。 言わなければ、受け取った側は照合値を上げようとして
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
            Stage::Humanness => "基準との距離",
            Stage::Matching => "照合値",
            Stage::Directive => "指示できる指標",
        }
    }
}

/// 段ごとの入力。測れていなければ `None` である。
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
    /// なぜそうなったか。止めた理由を返す。
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
    /// 判定できないを 1 と分けるのが要点である。 一緒にすれば、分からないと
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
    /// 値。測れていなければ `None`。
    pub value: Option<f64>,
    /// その人の幅。
    pub range: Range,
    /// 下端の見方。単位で決まる。
    pub lower: Lower,
    /// 切り口そのものを調べた研究があるか。定義ファイルの `直接。` の行から。
    ///
    /// 指摘の並びの 2 番目の鍵である。 外れの大きさが並んだとき、裏付けの
    /// 濃い指標から先に出す。
    pub direct: bool,
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
/// 1 を先に当てるのは、条件が独立だからである。 文体がどれだけ寄っても、機械が
/// 書いたと分かる文章は別の条件で落ちる。照合値が天井側にあることは、そこを何も
/// 保証しない。
#[must_use]
pub fn judge(
    inspections: &[Inspected],
    humanness: Option<Side>,
    matching: Option<Side>,
    directives: &[Observed],
) -> Outcome {
    judge_with(
        inspections,
        humanness,
        matching,
        Notes::default(),
        directives,
    )
}

/// 値の出どころについて、判定に添えるもの。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Notes<'a> {
    /// 照合値を代わりの値で出したなら、欠けていた系統。
    ///
    /// 代わりの値は機械の側へ寄せて置いてある。 その照合値が人の側に出たときだけ
    /// 先の段へ進み、床の側や帯の中なら判定できないで止める——通らないとは
    /// 言わない。 置いた値のせいでそこに出たのかもしれない。
    pub substituted: Option<&'a str>,
    /// 素材の下限に届かない短さ。`地の文の日本語 812 字 / 下限 1,000 字` の形。
    ///
    /// 値が出なかった段の理由に使う。 言わなければ、短いだけの文書が
    /// 目盛りの壊れと同じ顔で止まる。 測れた段は値で決める——短さは
    /// 測れなかった理由であって、測れた値を覆す理由ではない。
    pub too_short: Option<&'a str>,
    /// 効く指標はあったが、本人が全部を無効にしたために前に出す指標が空になった。
    ///
    /// 理由をそう言う。 「効く指標が無い」と言えば、書き手の性質の話に見えてしまう
    /// （[前に出す指標が無ければ判定できない](../../../docs/spec/300-revise.md#前に出す指標が無ければ判定できない)）。
    pub directives_muted: bool,
}

/// 判定する。値の出どころを `notes` に添える。
#[must_use]
pub fn judge_with(
    inspections: &[Inspected],
    humanness: Option<Side>,
    matching: Option<Side>,
    notes: Notes<'_>,
    directives: &[Observed],
) -> Outcome {
    let short = |stage: Stage| {
        notes.too_short.map(|what| Outcome {
            verdict: Verdict::Unknown,
            stage,
            reason: format!("短すぎて測れない（{what}）"),
        })
    };
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
    if let (None, Some(o)) = (humanness, short(Stage::Humanness)) {
        return o;
    }
    match humanness {
        None => return Outcome {
            verdict: Verdict::Unknown,
            stage: Stage::Humanness,
            reason:
                "基準の書きぶりが残っているかを測れていない。読みづらさの元を確かめずに先へ進めない"
                    .into(),
        },
        Some(Side::Machine) => {
            return Outcome {
                verdict: Verdict::Fail,
                stage: Stage::Humanness,
                reason: "基準の書きぶりが残っている".into(),
            }
        }
        Some(Side::InBand) => {
            return Outcome {
                verdict: Verdict::Unknown,
                stage: Stage::Humanness,
                reason: "基準の書きぶりが残っているかを決められない".into(),
            }
        }
        Some(Side::Human) => {}
    }

    // 2 段目。照合値。
    if let (Some(system), Some(Side::Machine | Side::InBand)) = (notes.substituted, matching) {
        return Outcome {
            verdict: Verdict::Unknown,
            stage: Stage::Matching,
            reason: format!(
                "{system} が測れていない。機械の側へ寄せた代わりの値で出した照合値は人の側に届かない"
            ),
        };
    }
    if let (None, Some(o)) = (matching, short(Stage::Matching)) {
        return o;
    }
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
    // 集合が空なら通さない。 空の集合は「すべて幅の中」を必ず満たすので、
    // その人らしさを 1 本も確かめていないのに通るが返る——この書き手では、
    // 効く指標があるという仮説そのものが確かめられなかったのである。
    if directives.is_empty() && notes.directives_muted {
        return Outcome {
            verdict: Verdict::Unknown,
            stage: Stage::Directive,
            reason:
                "効く指標を本人のカセットで全部無効にしたため、その人らしさを指標で確かめられない"
                    .into(),
        };
    }
    if directives.is_empty() {
        return Outcome {
            verdict: Verdict::Unknown,
            stage: Stage::Directive,
            reason: "この書き手には効く指標が無い。その人らしさを指標で確かめられない".into(),
        };
    }

    // 飛ばすのは個々の指標だけである。 測れていない指標を幅の中とも外とも
    // 扱わない、という意味であって、段を飛ばすことではない。
    let outside = outside_in_order(directives);

    if outside.is_empty() {
        return Outcome {
            verdict: Verdict::Pass,
            stage: Stage::Directive,
            reason: "前に出す指標がすべて幅の中にある".into(),
        };
    }

    // 「通らない」ではなく「判定できない」を返す。 照合値は既に天井側にある——
    // 系統で見るかぎり本人の範囲に入っている。それを覆して「通らない」と言えるだけの
    // 根拠は、細く切った 1 つの指標には無い。だが黙って通すこともしない。
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
/// 上限であって、独立な指示の本数ではない。 1 本直したら別の 1 本も動く。
pub const MAX_POINTS: usize = 4;

/// 指摘を選ぶ。並びは[判定が挙げる順](outside_in_order)と同じである。
#[must_use]
pub fn pick_points(directives: &[Observed]) -> Vec<(&Observed, Outside)> {
    let mut outside = outside_in_order(directives);
    outside.truncate(MAX_POINTS);
    outside
}

/// 幅の外にある指標を、[指摘の並び](../../../docs/spec/300-revise.md#指摘は-3-本か-4-本に絞る)の順に並べる。
///
/// 外れの大きさの降順、同じなら `直接` を名乗る指標が先、それでも同じなら
/// 指標名の符号位置の昇順。 決めておかないと、上位 3〜4 本が実装ごとに変わる。
///
/// 判定と指摘で並べ方を 2 つ持たない。 持てば、判定が「最も外れている」と
/// 名指しした指標と、指摘の先頭が食い違いうる。
///
/// 外れの大きさが[幅を作った本数から見込む取りこぼし](Range::tolerance)の内に
/// ある指標は、判定にも指摘にも出さない。 片方だけで落とせば、止めた理由に
/// 無い指標が指摘に並ぶ。
fn outside_in_order(directives: &[Observed]) -> Vec<(&Observed, Outside)> {
    let mut outside: Vec<(&Observed, Outside)> = directives
        .iter()
        .filter_map(|o| {
            let v = o.value?;
            let loc = o.range.locate_by(v, o.lower);
            (loc.is_outside() && loc.size() > o.range.tolerance()).then_some((o, loc))
        })
        .collect();
    outside.sort_by(|a, b| {
        b.1.size()
            .partial_cmp(&a.1.size())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.0.direct.cmp(&a.0.direct))
            .then_with(|| a.0.name.cmp(&b.0.name))
    });
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
            direct: false,
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
        // 読みづらさの元を確かめずに先へ進めない。
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

    fn substituted(system: &str) -> Notes<'_> {
        Notes {
            substituted: Some(system),
            ..Notes::default()
        }
    }

    fn too_short(what: &str) -> Notes<'_> {
        Notes {
            too_short: Some(what),
            ..Notes::default()
        }
    }

    #[test]
    fn 前に出す指標を全部無効にしたなら無効にしたためと言う() {
        // 「効く指標が無い」と言えば、書き手の性質の話に見えてしまう。
        let muted = Notes {
            directives_muted: true,
            ..Notes::default()
        };
        let o = judge_with(&[], Some(Side::Human), Some(Side::Human), muted, &[]);
        assert_eq!(o.verdict, Verdict::Unknown);
        assert_eq!(o.stage, Stage::Directive);
        assert!(o.reason.contains("無効にした"), "{}", o.reason);

        let none = judge(&[], Some(Side::Human), Some(Side::Human), &[]);
        assert!(none.reason.contains("効く指標が無い"), "{}", none.reason);
    }

    #[test]
    fn 短くて人らしさが測れなければ短すぎると言う() {
        // 目盛りが壊れたと読まれないように、長さのせいだと言う。
        let o = judge_with(
            &[],
            None,
            None,
            too_short("地の文の日本語 812 字 / 下限 1,000 字"),
            &[],
        );
        assert_eq!(o.verdict, Verdict::Unknown);
        assert_eq!(o.stage, Stage::Humanness);
        assert_eq!(
            o.reason,
            "短すぎて測れない（地の文の日本語 812 字 / 下限 1,000 字）"
        );
    }

    #[test]
    fn 短くて照合値が出なければ短すぎると言う() {
        // 人らしさは字数より緩い下限で測れることがある。
        let o = judge_with(
            &[],
            Some(Side::Human),
            None,
            too_short("地の文の日本語 900 字 / 下限 1,000 字"),
            &[],
        );
        assert_eq!(o.verdict, Verdict::Unknown);
        assert_eq!(o.stage, Stage::Matching);
        assert!(o.reason.starts_with("短すぎて測れない"), "{}", o.reason);
    }

    #[test]
    fn 短くても測れた段は値で決める() {
        // 短さは測れなかった理由であって、測れた値を覆す理由ではない。
        let o = judge_with(
            &[],
            Some(Side::Machine),
            None,
            too_short("地の文の日本語 900 字 / 下限 1,000 字"),
            &[],
        );
        assert_eq!(o.verdict, Verdict::Fail);
        assert_eq!(o.stage, Stage::Humanness);
    }

    #[test]
    fn 代わりの値の照合値が床の側でも通らないとは言わない() {
        // 機械の側へ寄せて置いた値である。 床の側に出たのは置いた値のせいでありうる。
        for side in [Side::Machine, Side::InBand] {
            let o = judge_with(
                &[],
                Some(Side::Human),
                Some(side),
                substituted("読点の打ち方"),
                &[],
            );
            assert_eq!(o.verdict, Verdict::Unknown, "{side:?}");
            assert_eq!(o.stage, Stage::Matching);
            assert!(o.reason.contains("読点の打ち方"), "{}", o.reason);
            assert!(o.reason.contains("代わりの値"), "{}", o.reason);
        }
    }

    #[test]
    fn 代わりの値の照合値が人の側なら先の段へ進む() {
        let d = [observed("全角括弧", Some(1.0), 0.0, 2.0)];
        let o = judge_with(
            &[],
            Some(Side::Human),
            Some(Side::Human),
            substituted("読点の打ち方"),
            &d,
        );
        assert_eq!(o.verdict, Verdict::Pass);
        assert_eq!(o.stage, Stage::Directive);
    }

    #[test]
    fn 代わりの値を置いていなければ床の側は通らない() {
        let o = judge_with(
            &[],
            Some(Side::Human),
            Some(Side::Machine),
            Notes::default(),
            &[],
        );
        assert_eq!(o.verdict, Verdict::Fail);
        assert_eq!(o, judge(&[], Some(Side::Human), Some(Side::Machine), &[]));
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
    fn 前に出す指標が_1_本も無ければ判定できない() {
        // 空の集合は「すべて幅の中」を満たしてしまう。 通すと、その人らしさを
        // 1 本も確かめていないのに通るが返る。
        let o = judge(&[], Some(Side::Human), Some(Side::Human), &[]);
        assert_eq!(o.verdict, Verdict::Unknown, "空の集合で通さない");
        assert_eq!(o.stage, Stage::Directive);
        assert!(o.reason.contains("効く指標"), "{}", o.reason);
    }

    #[test]
    fn 測れていない指標しか無くても通る() {
        // 空と、全部が測れていないのは別である。 集合はあり、飛ばすのは
        // 個々の指標だけである。
        let d = [observed("測れない", None, 0.0, 2.0)];
        let o = judge(&[], Some(Side::Human), Some(Side::Human), &d);
        assert_eq!(o.verdict, Verdict::Pass);
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
    fn 同じ大きさなら直接を名乗る指標が先に来る() {
        // 裏付けの濃い指標から先に出す。 名前の順はその次である。
        let mut direct = observed("絵文字", Some(5.0), 0.0, 2.0);
        direct.direct = true;
        let d = [observed("感嘆符", Some(5.0), 0.0, 2.0), direct];
        let names: Vec<&str> = pick_points(&d)
            .iter()
            .map(|(o, _)| o.name.as_str())
            .collect();
        assert_eq!(names, vec!["絵文字", "感嘆符"]);
        // 判定が名指しする先頭も同じ並びから取る。
        let o = judge(&[], Some(Side::Human), Some(Side::Human), &d);
        assert!(o.reason.contains("絵文字"), "{}", o.reason);
    }

    #[test]
    fn 直接でも外れが小さければ先に来ない() {
        // 1 番目の鍵は外れの大きさである。
        let mut direct = observed("絵文字", Some(2.5), 0.0, 2.0);
        direct.direct = true;
        let d = [direct, observed("感嘆符", Some(5.0), 0.0, 2.0)];
        assert_eq!(pick_points(&d)[0].0.name, "感嘆符");
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
            direct: false,
        };
        let big = observed("大きく外れた指標", Some(10.0), 0.0, 2.0);
        let d = [big, zero];
        let picks = pick_points(&d);
        assert_eq!(picks[0].0.name, "揺れない指標");
    }

    fn with_units(name: &str, value: f64, low: f64, high: f64, units: usize) -> Observed {
        let mut o = observed(name, Some(value), low, high);
        o.range.units = units;
        o
    }

    #[test]
    fn 標本の端の誤差に収まる外れは判定にも指摘にも数えない() {
        // 10 本で作った幅 0〜9 は、端ごとに平均 9 ÷ 9 = 1 だけ本人の広がりを
        // 取りこぼす。 9.5 は外れの大きさ 0.056 で、1/9 に収まる。
        let d = [with_units("全角括弧", 9.5, 0.0, 9.0, 10)];
        let o = judge(&[], Some(Side::Human), Some(Side::Human), &d);
        assert_eq!(o.verdict, Verdict::Pass);
        assert!(pick_points(&d).is_empty(), "判定と指摘は同じ集合から取る");
    }

    #[test]
    fn 標本の端の誤差を越える外れは数える() {
        // 1/9 ≈ 0.111 を越える 0.2。
        let d = [with_units("全角括弧", 10.8, 0.0, 9.0, 10)];
        let o = judge(&[], Some(Side::Human), Some(Side::Human), &d);
        assert_eq!(o.verdict, Verdict::Unknown);
        assert_eq!(pick_points(&d).len(), 1);
    }

    #[test]
    fn 誤差の内の指標は外れの本数に入らない() {
        let d = [
            with_units("誤差の内", 9.5, 0.0, 9.0, 10),
            with_units("大きい外れ", 20.0, 0.0, 9.0, 10),
        ];
        let o = judge(&[], Some(Side::Human), Some(Side::Human), &d);
        assert!(o.reason.contains("1 本"), "{}", o.reason);
        let names: Vec<&str> = pick_points(&d)
            .iter()
            .map(|(o, _)| o.name.as_str())
            .collect();
        assert_eq!(names, vec!["大きい外れ"]);
    }

    #[test]
    fn 単位が少ないほど誤差の内は広い() {
        // 2 本なら端ごとに幅 1 つ分を取りこぼしうる。
        let within = [with_units("全角括弧", 3.5, 0.0, 2.0, 2)];
        let beyond = [with_units("全角括弧", 4.5, 0.0, 2.0, 2)];
        assert!(pick_points(&within).is_empty(), "大きさ 0.75");
        assert_eq!(pick_points(&beyond).len(), 1, "大きさ 1.25");
    }

    #[test]
    fn 幅_0_の外れは誤差の内に入らない() {
        // 大きさは単位の本数で、誤差の内 1/(n−1) を必ず越える。
        for units in [1, 2, 10] {
            let d = [with_units("揺れない指標", 1.0, 0.0, 0.0, units)];
            assert_eq!(pick_points(&d).len(), 1, "{units} 本");
        }
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
            direct: false,
        };
        let small = observed("少し外れた指標", Some(2.5), 0.0, 2.0);
        let d = [small, gone];
        let picks = pick_points(&d);
        assert_eq!(picks[0].0.name, "毎回使う指標", "上位に来る");
    }
}
