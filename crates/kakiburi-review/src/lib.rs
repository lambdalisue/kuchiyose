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
pub use verdict::{
    judge, pick_points, Inspected, Observed, Outcome, Side, Stage, Verdict, MAX_POINTS,
};

/// 検めた結果。<strong>3 値と指摘を返す。</strong>
#[derive(Debug, Clone, PartialEq)]
pub struct Review {
    /// 判定と、止まった段と理由。
    pub outcome: Outcome,
    /// 指摘。<strong>最大 4 本。</strong>
    pub points: Vec<Point>,
    /// 照合の直し方。<strong>書きぶりの枠を奪わない。</strong>
    ///
    /// <strong>系統そのものは指示にならない</strong>——「何番目かの次元を増やせ」は言葉にならない。
    /// <strong>だが次元が語として読める系統なら、その 1 次元は指示になる。</strong>
    pub matching: Vec<String>,
    /// 人らしさの直し方。<strong>書きぶりの枠を奪わない。</strong>
    ///
    /// 3〜4 本という上限は書きぶりの側の話であり、人らしさはそこに数えない
    /// （[指標](../../../docs/spec/100-metrics.md#指標の種類)）。
    /// <strong>枠を奪い合わせると、機械臭さを消す指示と、その人へ寄せる指示が、席を
    /// 取り合う。</strong>
    pub humanness: Vec<String>,
    /// 本人の癖から外れているところ。<strong>書きぶりの枠を奪わない。</strong>
    ///
    /// <strong>判定には使わない。</strong> 実測で、この軸まで判定に入れるとカセットから抜いた
    /// 本人の記事 16 本のうち幅の外に出るものが 2 本から 6 本に増えた。
    pub habits: Vec<Point>,
    /// 使われていない型。<strong>書きぶりの枠を奪わない。</strong>
    ///
    /// <strong>分布では言えないものがある。</strong> その人が繰り返し使う語の並びは、頻度の
    /// ベクトルに均されて消える——[照合値](Self::matching)がどれだけ寄っても、
    /// <strong>その人の型が 1 つも出てこない文章</strong>はありうる。
    ///
    /// <strong>判定には使わない。</strong> 実測で、本人の記事 50 本のうち 5 本が型を 1 つも
    /// 使っていなかった。止める材料にはできない。
    pub katas: Vec<String>,
}

/// その人の型 1 つ。
#[derive(Debug, Clone, PartialEq)]
pub struct Kata {
    /// 語の並び。<strong>穴あきなら、間を `〜` で見せた形。</strong>
    pub text: String,
    /// 本人の単位のうち、これが現れた割合。
    pub rate: f64,
    /// 文書の中での位置の中央。<strong>0 に近ければ書き出しの型である。</strong>
    pub at: f64,
    /// この文章に現れているか。
    pub used: bool,
}

/// 使われていない型の渡し方。
///
/// <strong>どこで使うかまで言う。</strong> 位置が偏っている型は、そこで使うから型なのであって、
/// どこかに混ぜればよいものではない。
fn kata_remedies(katas: &[Kata]) -> Vec<String> {
    let mut missing: Vec<&Kata> = katas.iter().filter(|k| !k.used).collect();
    missing.sort_by(|a, b| {
        b.rate
            .partial_cmp(&a.rate)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.text.cmp(&b.text))
    });
    missing
        .into_iter()
        .take(MAX_POINTS)
        .map(|k| {
            let at = if k.at < 0.2 {
                "書き出しで"
            } else if k.at > 0.8 {
                "結びで"
            } else {
                ""
            };
            format!(
                "本人は{at}「{}」と書く（{:.0}% の記事で使っている）。この文章には出てこない。",
                k.text,
                k.rate * 100.0
            )
        })
        .collect()
}

/// 照合の、次元ごとの観測。
///
/// <strong>照合値は 1 つの数なので、どこが違うのかを言えない。</strong> 帯の中で止まったときに
/// 何も出さなければ、受け取った側は動きようがない。
#[derive(Debug, Clone, PartialEq)]
pub struct MatchingObserved {
    /// どの系統か。
    pub system: String,
    /// どの次元か。<strong>語・記号・字種など、読める形である。</strong>
    pub dim: String,
    /// この文章の、標準化した値。
    pub mine: f64,
    /// 相手集合の代表値。
    pub theirs: f64,
    /// 直したときに照合値が動く量。<strong>見込みではなく、そのまま動く量である。</strong>
    pub effect: f64,
    /// 本人がその次元をどう書いているかの実例。
    ///
    /// <strong>「増やせ」と言うだけでは、どこに置くのかが分からない。</strong>
    pub examples: Vec<String>,
    /// <strong>この文章の、直す場所。</strong>
    ///
    /// <strong>「減らせ」と言うなら、どれを減らすのかを言う。</strong> 本人の実例だけでは、
    /// <strong>自分の文章のどこを直すのかが分からない</strong>——実測で、受け取った側が
    /// 道具の外で数え直すことになった。
    pub spots: Vec<String>,
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
    /// その人が現に繰り返している言い回し。<strong>この指標に添えるものだけ。</strong>
    ///
    /// <strong>数値と向きだけでは直せない。</strong>「その人が繰り返している言い回しを
    /// 繰り返す」と言うなら、その言い回しを渡さなければ、受け取った側は自分で
    /// でっち上げた定型句を挿し込むことになる。
    pub phrases: Vec<String>,
    /// この文章が繰り返しすぎている言い回し。<strong>減らす側でだけ意味を持つ。</strong>
    ///
    /// <strong>「減らせ」と言うなら、どれを減らすのかを言わなければ直せない。</strong>
    pub overused: Vec<String>,
    /// この文章で一度しか出てこない語。
    ///
    /// <strong>「語を散らすな」と言うなら、どれが散らしているのかを言わなければ直せない。</strong>
    /// 実測では、この指示を受けた側が<strong>散らす方向へ直してしまった</strong>。
    pub once_only: Vec<String>,
    /// 直したときに人らしさ値が動く量。
    ///
    /// <strong>正反対を指す直し方が同時に出ることがある。</strong> 語彙を散らせと言う指標と、
    /// 言い換えるなと言う指標が同時に出たとき、<strong>どちらが勝つかは動く量でしか
    /// 言えない</strong>——言わなければ、受け取った側は逆を選ぶ。
    pub effect: f64,
    /// 本人の代表値。<strong>比べる相手はここである。</strong>
    ///
    /// <strong>0 と比べてはいけない。</strong> 0 は人と機械の境目であって、その人のところ
    /// ではない——<strong>どの指標も境目より人の側にいるのに、合算では機械の側</strong>という
    /// ことが実際に起きる。
    pub target: f64,
}

/// 検めに渡す観測ぜんぶ。
///
/// <strong>ばらばらに渡さない。</strong> 段の数だけ引数が増えると、呼ぶ側が並びを間違えても
/// 型が合ってしまう。
pub struct Observations<'a> {
    /// [検査](Inspected)の観測。<strong>0 段目。</strong>
    pub inspections: &'a [Inspected],
    /// 人らしさ値がどちら側か。<strong>1 段目。</strong>
    pub humanness: Option<Side>,
    /// 照合値がどちら側か。<strong>2 段目。</strong>
    pub matching: Option<Side>,
    /// 前に出す指標の観測。<strong>3 段目。</strong>
    pub directives: &'a [Observed],
    /// 一貫しているだけの指標の観測。<strong>判定には使わない。</strong>
    ///
    /// [条件 2](kakiburi_scale::effective::Effective::narrow_only)で捨てられた軸である
    /// ——基準と本人が一致していても、草稿がそこから外れることはある。
    pub habits: &'a [Observed],
    /// 指標ごとの人らしさ値。<strong>1 段目の直し方を組む。</strong>
    pub humanness_by_metric: &'a [HumannessObserved],
    /// 相手集合から離れている次元。<strong>2 段目の直し方を組む。</strong>
    pub diverging: &'a [MatchingObserved],
    /// その人の型と、この文章で使われているか。
    pub katas: &'a [Kata],
}

/// 検める。
///
/// <strong>指摘は判定と同じ集合から取る。</strong> 別々に定めれば、止めた理由が指摘に出てこない
/// という食い違いが起きる。
#[must_use]
pub fn review(o: &Observations<'_>, remedies: &dyn Remedies) -> Review {
    let (humanness_by_metric, diverging, directives) =
        (o.humanness_by_metric, o.diverging, o.directives);
    let outcome = judge(o.inspections, o.humanness, o.matching, directives);
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
    //
    // <strong>帯の中で止まったときも渡す。</strong> 機械の側に落ちたときだけ出すと、
    // <strong>帯の中から人の側へ出る道が示されない</strong>——通らないよりも判定できないの
    // ほうが多いので、そちらで黙るほうが害が大きい。
    let humanness = if outcome.stage == Stage::Humanness {
        humanness_remedies(humanness_by_metric, remedies)
    } else {
        Vec::new()
    };
    // <strong>2 段目で止まったときにも渡す。</strong> ここで何も出さなければ、その人へ寄せる
    // という目的そのものに対して、道具が黙ることになる。
    let matching = if outcome.stage == Stage::Matching {
        matching_remedies(diverging)
    } else {
        Vec::new()
    };
    // <strong>2 段目と 3 段目で渡す。</strong> 型はその人へ寄せるためのものなので、
    // 人らしさの段で出せば、受け取った側は 2 つの目的を同時に追うことになる。
    //
    // <strong>通ったときにも渡す。</strong> 分布が寄っていても型が 1 つも出てこないことは
    // ありうる——そこで黙れば、道具は「通った」としか言わないまま
    // <strong>その人らしくない文章を返す。</strong>
    let katas = if matches!(outcome.stage, Stage::Matching | Stage::Directive) {
        kata_remedies(o.katas)
    } else {
        Vec::new()
    };
    // <strong>型と同じ扱いである。</strong> その人へ寄せるためのものなので、人らしさの段では出さない。
    let habits = if matches!(outcome.stage, Stage::Matching | Stage::Directive) {
        pick_points(o.habits)
            .into_iter()
            .filter_map(|(x, loc)| point::build(x, loc, remedies).ok())
            .collect()
    } else {
        Vec::new()
    };
    Review {
        outcome,
        points,
        matching,
        humanness,
        habits,
        katas,
    }
}

/// 相手集合から離れている次元を、離れている順に言う。
///
/// <strong>直し方は定義から取らない。</strong> 系統の次元は指標ではないので定義ファイルを
/// 持たない——<strong>観測がそのまま指示になる</strong>「本人より多い／少ない」を言う。
fn matching_remedies(observed: &[MatchingObserved]) -> Vec<String> {
    observed
        .iter()
        .take(MAX_POINTS)
        .map(|o| {
            let way = if o.theirs > o.mine {
                "少ない"
            } else {
                "多い"
            };
            // <strong>「本人と同じ書き方で」を付ける。</strong> 数だけ言うと、数を満たす壊し方が
            // 選ばれる——文字種の空白を上げよと言われた側が、和文のあいだに空白を
            // 入れたことが実際に起きた（[0 段目](verdict::Stage::Writing)）。
            let fix = if o.theirs > o.mine {
                "本人と同じ書き方で増やす"
            } else {
                "減らす"
            };
            let mut line = format!(
                "{}の「{}」が本人より{way}（この文章 {:.3} / 本人 {:.3}）。{fix}と照合値が {:+.3} 動く。",
                o.system, o.dim, o.mine, o.theirs, o.effect
            );
            if !o.examples.is_empty() {
                // <strong>本人がどう書いているかを見せる。</strong> 見せなければ、受け取った側は
                // その人の文章を自分で読みに行くことになる。
                line.push_str(&format!("本人はこう書いている: 「{}」。", o.examples.join("」「")));
            }
            if !o.spots.is_empty() {
                // <strong>この文章のどこかを見せる。</strong> 本人の実例だけでは、自分の文章の
                // どこを直すのかが分からない。
                line.push_str(&format!("この文章のここ: 「{}」。", o.spots.join("」「")));
            }
            line
        })
        .collect()
}

/// 機械の側に落ちている指標の直し方を、落ちている順に並べる。
///
/// <strong>向きは観測が持っている。</strong> 較正から読んだもので、定義の名乗る向きとは限らない
/// ——同じ型で書かせた生成文は、先行研究の言う「機械の側」に来ないことがある。
fn humanness_remedies(observed: &[HumannessObserved], remedies: &dyn Remedies) -> Vec<String> {
    // <strong>本人へ寄せると合算が上がる指標だけを渡す。</strong>
    //
    // <strong>0 と比べない。</strong> 0 は人と機械の境目であって、その人のところではない。
    // <strong>従うと悪くなる直し方も渡さない</strong>——指標どうしが正反対を向くことがあり、
    // 渡せば受け取った側は自分の手で判定を悪くする。
    let mut low: Vec<&HumannessObserved> = observed.iter().filter(|o| o.effect > 0.0).collect();
    // <strong>効く量の大きい順。</strong> 落ちている深さで並べると、<strong>正反対を指す 2 本の
    // どちらが勝つかを言えない</strong>——実測で、受け取った側が逆を選んだ。
    low.sort_by(|a, b| {
        b.effect
            .partial_cmp(&a.effect)
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
            let way = if o.target > o.value { "足りない" } else { "多い" };
            let mut line = format!(
                "{}が本人より{way}（この文章 {:.3} / 本人 {:.3}）。{remedy}人らしさ値が {:+.3} 動く。",
                o.name, o.value, o.target, o.effect
            );
            if !o.phrases.is_empty() {
                // <strong>本人が現に繰り返している言い回しを添える。</strong> 添えなければ、
                // 受け取った側は自分ででっち上げた定型句を挿し込む。
                line.push_str(&format!(
                    " 本人が繰り返しているのは「{}」。",
                    o.phrases.join("」「")
                ));
            }
            if !o.once_only.is_empty() {
                // <strong>どれが散らしているのかを言う。</strong> 言わなければ、受け取った側は
                // 言い換えを別の言い換えに置き換えることになる。
                line.push_str(&format!(
                    " 一度しか出てこない語: 「{}」。",
                    o.once_only.join("」「")
                ));
            }
            if !o.overused.is_empty() {
                // <strong>どれを減らすのかを言う。</strong> 言わなければ、受け取った側は
                // 手当たり次第に言い換えることになる。
                line.push_str(&format!(
                    " この文章が繰り返しているのは「{}」。",
                    o.overused.join("」「")
                ));
            }
            Some(line)
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
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &[],
                katas: &[],
            },
            &All,
        );
        assert_eq!(r.outcome.verdict, Verdict::Pass);
        assert!(r.points.is_empty());
    }

    fn human(name: &str, value: f64) -> HumannessObserved {
        HumannessObserved {
            name: name.into(),
            value,
            raise: true,
            phrases: Vec::new(),
            overused: Vec::new(),
            once_only: Vec::new(),
            effect: 0.1,
            target: 1.0,
        }
    }

    fn diverge(system: &str, dim: &str, mine: f64, theirs: f64) -> MatchingObserved {
        MatchingObserved {
            system: system.into(),
            dim: dim.into(),
            mine,
            theirs,
            effect: 0.05,
            examples: Vec::new(),
            spots: Vec::new(),
        }
    }

    #[test]
    fn 照合で止まったら離れている次元を渡す() {
        // <strong>ここで何も出さなければ、その人へ寄せるという目的そのものに対して
        // 道具が黙る。</strong>
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let v = [diverge("機能語", "一方", 18.6, -0.2)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::InBand),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &v,
                katas: &[],
            },
            &All,
        );
        assert_eq!(r.outcome.stage, Stage::Matching);
        assert!(r.points.is_empty(), "書きぶりの枠は奪わない");
        assert_eq!(r.matching.len(), 1, "{:?}", r.matching);
        assert!(r.matching[0].contains("一方"), "{:?}", r.matching);
        assert!(r.matching[0].contains("多い"), "{:?}", r.matching);
        assert!(r.matching[0].contains("減らす"), "{:?}", r.matching);
    }

    #[test]
    fn 本人のほうが多ければ増やすと言う() {
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let v = [diverge("機能語", "のだ", -1.2, 2.4)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::InBand),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &v,
                katas: &[],
            },
            &All,
        );
        assert!(r.matching[0].contains("少ない"), "{:?}", r.matching);
        assert!(r.matching[0].contains("増やす"), "{:?}", r.matching);
    }

    #[test]
    fn 照合の直し方も枠に上限がある() {
        // 数え上げても直せない。<strong>書きぶりと同じ上限を掛ける。</strong>
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let v: Vec<MatchingObserved> = (0..10)
            .map(|i| diverge("機能語", &format!("語{i}"), f64::from(i), 0.0))
            .collect();
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::InBand),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &v,
                katas: &[],
            },
            &All,
        );
        assert_eq!(r.matching.len(), MAX_POINTS, "{:?}", r.matching);
    }

    #[test]
    fn 照合を通ったら照合の直し方は出さない() {
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let v = [diverge("機能語", "一方", 18.6, -0.2)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &v,
                katas: &[],
            },
            &All,
        );
        assert!(r.matching.is_empty(), "{:?}", r.matching);
    }

    #[test]
    fn 人らしさで止まったら人らしさの直し方を渡す() {
        // **ここで何も出さなければ、機械の書いた草稿は止められるだけで直せない。**
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let mut ok = human("繰り返し", -1.2);
        ok.effect = 0.7;
        let mut done = human("圧縮率", 0.4);
        done.effect = 0.0;
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[ok, done],
                diverging: &[],
                katas: &[],
            },
            &All,
        );
        assert_eq!(r.outcome.stage, Stage::Humanness);
        assert!(r.points.is_empty(), "書きぶりの枠は奪わない");
        assert_eq!(r.humanness.len(), 1, "効く量があるのは 1 本");
        assert!(r.humanness[0].contains("繰り返し"), "{:?}", r.humanness);
        assert!(r.humanness[0].contains("足す"), "{:?}", r.humanness);
    }

    #[test]
    fn 減らす側では繰り返しすぎているものを名指す() {
        // <strong>「減らせ」と言うなら、どれを減らすのかを言わなければ直せない。</strong>
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let mut h = human("短い繰り返し", -0.2);
        h.raise = false;
        h.overused = vec!["ます。".into(), "ています".into()];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[h],
                diverging: &[],
                katas: &[],
            },
            &All,
        );
        assert!(r.humanness[0].contains("ます。"), "{:?}", r.humanness);
    }

    #[test]
    fn 言い回しを持つなら添える() {
        // <strong>数値と向きだけでは直せない。</strong> 受け取った側が自分ででっち上げた
        // 定型句を挿し込むことになる。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let mut h = human("長い繰り返し", -0.4);
        h.phrases = vec!["と思っています".into(), "しています。".into()];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[h],
                diverging: &[],
                katas: &[],
            },
            &All,
        );
        assert!(
            r.humanness[0].contains("と思っています"),
            "{:?}",
            r.humanness
        );
        assert!(r.humanness[0].contains("しています。"), "{:?}", r.humanness);
    }

    #[test]
    fn 従うと悪くなる直し方は渡さない() {
        // <strong>指標どうしが正反対を向くことがある。</strong> 渡せば、受け取った側は
        // 自分の手で判定を悪くする。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let mut bad = human("語彙の豊富さ", -0.06);
        bad.effect = -0.95;
        let mut good = human("圧縮率", -0.78);
        good.effect = 0.99;
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[bad, good],
                diverging: &[],
                katas: &[],
            },
            &All,
        );
        assert_eq!(r.humanness.len(), 1, "{:?}", r.humanness);
        assert!(r.humanness[0].contains("圧縮率"), "{:?}", r.humanness);
    }

    #[test]
    fn 人らしさの直し方は効く量の順に並ぶ() {
        // <strong>落ちている深さで並べてはいけない。</strong> 正反対を指す 2 本のどちらが
        // 勝つかを言えず、受け取った側が逆を選ぶ。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let mut deep = human("繰り返し", -1.5);
        deep.effect = 0.05;
        let mut shallow = human("圧縮率", -0.3);
        shallow.effect = 0.90;
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[deep, shallow],
                diverging: &[],
                katas: &[],
            },
            &All,
        );
        assert!(
            r.humanness[0].contains("圧縮率"),
            "深いほうではない: {:?}",
            r.humanness
        );
        assert!(r.humanness[1].contains("繰り返し"), "{:?}", r.humanness);
    }

    #[test]
    fn 本人に届いている指標は直させない() {
        // 直しても合算は上がらない。<strong>効く量が 0 以下なら渡さない。</strong>
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let mut a = human("繰り返し", 0.8);
        a.effect = 0.0;
        let mut b = human("圧縮率", 0.4);
        b.effect = -0.2;
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[a, b],
                diverging: &[],
                katas: &[],
            },
            &All,
        );
        assert!(r.humanness.is_empty(), "{:?}", r.humanness);
    }

    #[test]
    fn 帯の中で止まっても人らしさの直し方を渡す() {
        // <strong>帯の中から人の側へ出る道を示さなければ、受け取った側は動けない。</strong>
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let h = [human("繰り返し", -1.2)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::InBand),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &h,
                diverging: &[],
                katas: &[],
            },
            &All,
        );
        assert_eq!(r.outcome.stage, Stage::Humanness);
        assert_eq!(r.outcome.verdict, Verdict::Unknown);
        assert_eq!(r.humanness.len(), 1, "{:?}", r.humanness);
    }

    #[test]
    fn 人らしさが測れないときは直し方を出さない() {
        // **測れていないことと、機械の側にあることは違う。** 混ぜれば、短いだけの
        // 文章に「繰り返しを足せ」と言うことになる。
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: None,
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &[],
                katas: &[],
            },
            &All,
        );
        assert_eq!(r.outcome.verdict, Verdict::Unknown);
        assert!(r.humanness.is_empty());
    }

    #[test]
    fn 通るときは人らしさの直し方も無い() {
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let h = [human("繰り返し", -1.2)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &h,
                diverging: &[],
                katas: &[],
            },
            &All,
        );
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
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[down],
                diverging: &[],
                katas: &[],
            },
            &All,
        );
        assert_eq!(r.humanness.len(), 1, "{:?}", r.humanness);
        assert!(r.humanness[0].contains("減らす"), "{:?}", r.humanness);

        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[human("圧縮率", -0.9)],
                diverging: &[],
                katas: &[],
            },
            &All,
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
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &h,
                diverging: &[],
                katas: &[],
            },
            &UpperOnly,
        );
        assert_eq!(r.humanness.len(), 1, "{:?}", r.humanness);
        assert!(r.humanness[0].contains("減らす"), "{:?}", r.humanness);
    }

    #[test]
    fn 直し方が定義に無ければ出さない() {
        let d = [observed("全角括弧", 1.0, 0.0, 2.0)];
        let h = [human("繰り返し", -1.2)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &h,
                diverging: &[],
                katas: &[],
            },
            &None_,
        );
        assert!(r.humanness.is_empty(), "{:?}", r.humanness);
    }

    #[test]
    fn 前の段で止まったら指摘を出さない() {
        // 出せば、受け取った側は照合値を上げようとして人らしさを下げる。
        let d = [observed("全角括弧", 5.0, 0.0, 2.0)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Machine),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &[],
                katas: &[],
            },
            &All,
        );
        assert_eq!(r.outcome.stage, Stage::Humanness);
        assert!(r.points.is_empty(), "指摘を出してはいけない");
    }

    #[test]
    fn 照合値で止まっても指摘を出さない() {
        let d = [observed("全角括弧", 5.0, 0.0, 2.0)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::InBand),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &[],
                katas: &[],
            },
            &All,
        );
        assert_eq!(r.outcome.stage, Stage::Matching);
        assert!(r.points.is_empty());
    }

    #[test]
    fn 三段目まで来たら指摘を組む() {
        let d = [
            observed("全角括弧", 5.0, 0.0, 2.0),
            observed("感嘆符", 10.0, 0.0, 1.0),
        ];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &[],
                katas: &[],
            },
            &All,
        );
        assert_eq!(r.outcome.verdict, Verdict::Unknown);
        assert_eq!(r.points.len(), 2);
        // 外れの大きさの降順。感嘆符は 9.0、全角括弧は 1.5。
        assert_eq!(r.points[0].name, "感嘆符");
    }

    #[test]
    fn 指摘は判定と同じ集合から取る() {
        // 止めた理由が指摘に出てこないという食い違いを防ぐ。
        let d = [observed("全角括弧", 5.0, 0.0, 2.0)];
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &[],
                katas: &[],
            },
            &All,
        );
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
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &[],
                katas: &[],
            },
            &None_,
        );
        assert_eq!(r.outcome.verdict, Verdict::Unknown);
        assert!(r.points.is_empty());
    }

    #[test]
    fn 指摘は_4_本までである() {
        let d: Vec<Observed> = (0..10)
            .map(|i| observed(&format!("m{i:02}"), 5.0, 0.0, 2.0))
            .collect();
        let r = review(
            &Observations {
                inspections: &[],
                humanness: Some(Side::Human),
                matching: Some(Side::Human),
                directives: &d,
                habits: &[],
                humanness_by_metric: &[],
                diverging: &[],
                katas: &[],
            },
            &All,
        );
        assert_eq!(r.points.len(), MAX_POINTS);
    }

    fn kata(text: &str, rate: f64, at: f64, used: bool) -> Kata {
        Kata {
            text: text.into(),
            rate,
            at,
            used,
        }
    }

    #[test]
    fn 使われている型は渡さない() {
        let got = kata_remedies(&[kata("となります。", 0.3, 0.5, true)]);
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn 使われていない型は割合とともに渡す() {
        let got = kata_remedies(&[kata("となります。", 0.3, 0.5, false)]);
        assert_eq!(got.len(), 1);
        assert!(got[0].contains("となります。"), "{}", got[0]);
        assert!(got[0].contains("30%"), "{}", got[0]);
    }

    #[test]
    fn 位置の偏った型はどこで使うかまで言う() {
        // **そこで使うから型なのであって、どこかに混ぜればよいものではない。**
        let head = kata_remedies(&[kata("ご無沙汰しております", 0.26, 0.03, false)]);
        assert!(head[0].contains("書き出しで"), "{}", head[0]);
        let tail = kata_remedies(&[kata("以上です。", 0.26, 0.95, false)]);
        assert!(tail[0].contains("結びで"), "{}", tail[0]);
        let mid = kata_remedies(&[kata("となります。", 0.3, 0.5, false)]);
        assert!(!mid[0].contains("書き出し"), "{}", mid[0]);
    }

    #[test]
    fn 型は広く使う順に渡す() {
        let got = kata_remedies(&[
            kata("たまに書く", 0.26, 0.5, false),
            kata("よく書く", 0.42, 0.5, false),
        ]);
        assert!(got[0].contains("よく書く"), "{}", got[0]);
    }
}
