//! 指示できる指標を、足し合わせられる形で返す。
//!
//! 値だけでは、何本かを束ねた単位の値を作れない——割合の平均は割合ではない。
//! だから値の元になった数（分子・分母・下限、変動係数なら元の値の並び）を
//! 値と一緒に返す（[文書ごとの統計値を持つ](../../../docs/spec/200-extract.md#文書ごとの統計値を持つ)）。
//!
//! 下限は数えたあとに掛ける。 下限未満の文書でも数は持つ——束ねれば足りる
//! 文書があり、先に捨てると束ねた単位の値が作れない。
//!
//! 束ねるときは数を足してから下限を掛け直す（[束ね方](Counted::compose)）。
//! 深い見出しと節の長さの変動係数は、連結して測った値と一致しない。見出しの深さも
//! 節の切れ目も文書の中で決まるので、文書ごとに数えたものを足す。
//! [文書の境界を node の境界として残す](../../../docs/spec/200-extract.md#短い文書は束ねる)
//! という束ね方の決まりに合うのは、足し合わせのほうである。

use kakiburi_doc::prose::Segment;
use kakiburi_doc::Document;

use crate::morph::Analyzed;
use crate::{phrase, structure, symbol, word, Measured, Unmeasured};

/// 値の元になった数。
#[derive(Debug, Clone, PartialEq)]
pub enum Parts {
    /// `per × num ÷ den`。分母が下限に届かなければ下限未満である。
    Ratio {
        /// 分子。
        num: usize,
        /// 分母。下限もこれで見る。
        den: usize,
        /// 掛ける数。日本語 1,000 字あたりなら 1000、割合なら 1。
        per: usize,
        /// 分母の下限。
        floor: usize,
        /// 分母が 0 のとき、下限未満ではなく「分母が 0」とするか。
        ///
        /// 1 度も現れないのと、現れたが足りないのを分ける指標がある。
        zero_is_absent: bool,
    },
    /// 変動係数の元の値。本数が下限に届かなければ下限未満である。
    Spread {
        /// 値。並びは数えた順。
        values: Vec<usize>,
        /// 本数の下限。
        floor: usize,
    },
    /// 数そのもの。
    Count(usize),
}

impl Parts {
    /// 値にする。
    #[must_use]
    pub fn measured(&self) -> Measured {
        match self {
            Parts::Ratio {
                num,
                den,
                per,
                floor,
                zero_is_absent,
            } => {
                if *den == 0 {
                    return if *zero_is_absent {
                        Measured::NoDenominator
                    } else {
                        Measured::BelowFloor
                    };
                }
                if den < floor {
                    return Measured::BelowFloor;
                }
                #[allow(clippy::cast_precision_loss)]
                Measured::Value(*per as f64 * *num as f64 / *den as f64)
            }
            Parts::Spread { values, floor } => {
                if values.len() < *floor {
                    return Measured::BelowFloor;
                }
                cv(values)
            }
            #[allow(clippy::cast_precision_loss)]
            Parts::Count(n) => Measured::Value(*n as f64),
        }
    }

    /// 足し合わせる。形が違えば `None`。
    ///
    /// 同じ指標の数なら形は必ず揃う。揃わないのは、違う指標を混ぜたときである。
    fn sum(parts: &[&Parts]) -> Option<Parts> {
        let (first, rest) = parts.split_first()?;
        let mut acc = (*first).clone();
        for p in rest {
            match (&mut acc, p) {
                (
                    Parts::Ratio {
                        num,
                        den,
                        per,
                        floor,
                        zero_is_absent,
                    },
                    Parts::Ratio {
                        num: n,
                        den: d,
                        per: pe,
                        floor: fl,
                        zero_is_absent: z,
                    },
                ) if per == pe && floor == fl && zero_is_absent == z => {
                    *num += n;
                    *den += d;
                }
                (
                    Parts::Spread { values, floor },
                    Parts::Spread {
                        values: v,
                        floor: fl,
                    },
                ) if floor == fl => values.extend(v),
                (Parts::Count(n), Parts::Count(m)) => *n += m,
                _ => return None,
            }
        }
        Some(acc)
    }
}

/// 変動係数。標準偏差 ÷ 平均。
#[allow(clippy::cast_precision_loss)]
fn cv(values: &[usize]) -> Measured {
    if values.is_empty() {
        return Measured::BelowFloor;
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<usize>() as f64 / n;
    if mean <= 0.0 {
        // 平均が 0 では割れない。0 を返さない——測れていない。
        return Measured::BelowFloor;
    }
    let var = values
        .iter()
        .map(|&v| (v as f64 - mean).powi(2))
        .sum::<f64>()
        / n;
    Measured::Value(var.sqrt() / mean)
}

/// 指示できる指標 1 本を測った結果。値と、その元になった数。
///
/// 数を持たないのは、数える前に止まったときだけである（道具が無い、など）。
/// 下限未満は数を持つ——束ねれば届くことがある。
#[derive(Debug, Clone, PartialEq)]
pub struct Counted(Result<Parts, Unmeasured>);

impl Counted {
    /// 数から作る。
    #[must_use]
    pub fn of(parts: Parts) -> Self {
        Self(Ok(parts))
    }

    /// 数える前に止まった。
    #[must_use]
    pub fn unmeasured(why: Unmeasured) -> Self {
        Self(Err(why))
    }

    /// 日本語 1,000 字あたりの数。分母は日本語の文字数、下限は[既定](crate::floor::JAPANESE_CHARS)。
    #[must_use]
    pub fn density(n: usize, japanese: usize) -> Self {
        Self::of(Parts::Ratio {
            num: n,
            den: japanese,
            per: 1000,
            floor: crate::floor::JAPANESE_CHARS,
            zero_is_absent: false,
        })
    }

    /// 割合。分母が `floor` に届かなければ下限未満。
    #[must_use]
    pub fn share(hit: usize, total: usize, floor: usize) -> Self {
        Self::of(Parts::Ratio {
            num: hit,
            den: total,
            per: 1,
            floor,
            zero_is_absent: false,
        })
    }

    /// 変動係数。本数が `floor` に届かなければ下限未満。
    #[must_use]
    pub fn spread(values: Vec<usize>, floor: usize) -> Self {
        Self::of(Parts::Spread { values, floor })
    }

    /// 値。
    #[must_use]
    pub fn measured(&self) -> Measured {
        match &self.0 {
            Ok(p) => p.measured(),
            Err(why) => (*why).into(),
        }
    }

    /// 値があれば返す。
    #[must_use]
    pub fn value(&self) -> Option<f64> {
        self.measured().value()
    }

    /// 測れたか。
    #[must_use]
    pub fn is_measured(&self) -> bool {
        self.measured().is_measured()
    }

    /// 元になった数。数える前に止まっていれば `None`。
    #[must_use]
    pub fn parts(&self) -> Option<&Parts> {
        self.0.as_ref().ok()
    }

    /// 何本かを束ねた単位の値を作る。
    ///
    /// 数を足してから下限を掛け直す。 1 本でも数を持たなければ、束ねた単位も
    /// 数を持たない——理由は、直せる手のいちばん限られたものを返す。
    ///
    /// 1 本だけなら、その 1 本と同じものが返る。
    #[must_use]
    pub fn compose(counted: &[&Counted]) -> Counted {
        let missing = counted.iter().filter_map(|c| c.0.as_ref().err()).min();
        if let Some(why) = missing {
            return Counted::unmeasured(*why);
        }
        let parts: Vec<&Parts> = counted.iter().filter_map(|c| c.parts()).collect();
        match Parts::sum(&parts) {
            Some(p) => Counted::of(p),
            // 束ねるものが無いか、違う指標の数が混ざった。 どちらも値を作れない。
            None => Counted::unmeasured(Unmeasured::NoDenominator),
        }
    }
}

impl PartialEq<Measured> for Counted {
    fn eq(&self, other: &Measured) -> bool {
        self.measured() == *other
    }
}

impl From<Counted> for Measured {
    fn from(c: Counted) -> Self {
        c.measured()
    }
}

/// 指示できる指標を全部測る。名前は定義ファイルの 1 行目と同じ。
///
/// 使う側は一覧を持たない。 ここに置くのは呼び出しの束である。
///
/// 解析器を要るものは、`analyzed` が `None` なら[道具が無い](Unmeasured::ToolMissing)に
/// なる——0 を返さないし、名前も落とさない。落とせば、書き手ごとに軸の数が変わる。
#[must_use]
pub fn measure(doc: &Document, analyzed: Option<&Analyzed>) -> Vec<(String, Counted)> {
    let p = doc.prose();
    let mut out: Vec<(String, Counted)> = fixed(doc, &p)
        .into_iter()
        .map(|(n, m)| (n.to_owned(), m))
        .collect();

    // 1 つの定義が 24 本の軸に展開される。 名前は定義が作る——実装が作れば、
    // 名前が 2 か所に現れる。
    out.push(("語を割る読点".to_owned(), word::splitting_commas(analyzed)));

    // 文末の軸は node の種類ごとに出す。 1 つの定義が種類の数だけ軸を作るので、
    // 種類が増えても指標の側を書き足さなくてよい。
    out.extend(structure::register_rates(&p, analyzed));

    match analyzed.map(word::conjunction_comma) {
        Some(got) => out.extend(got),
        None => out.extend(
            word::conjunction_comma_names()
                .into_iter()
                .map(|n| (n, Counted::unmeasured(Unmeasured::ToolMissing))),
        ),
    }
    out
}

/// 解析器を要らない指標。
fn fixed(doc: &Document, p: &[Segment]) -> Vec<(&'static str, Counted)> {
    vec![
        ("全角括弧", symbol::full_width_paren(p)),
        ("半角括弧", symbol::half_width_paren(p)),
        ("鉤括弧", symbol::corner_bracket(p)),
        ("感嘆符", symbol::exclamation(p)),
        ("疑問符", symbol::question(p)),
        ("三点リーダ", symbol::ellipsis(p)),
        ("三点リーダの字数", symbol::ellipsis_doubled(p)),
        ("中黒", symbol::middle_dot(p)),
        ("波ダッシュ", symbol::wave_dash(p)),
        ("数字の字幅", symbol::digit_width(p)),
        ("感嘆符の字幅", symbol::exclamation_width(p)),
        ("疑問符の字幅", symbol::question_width(p)),
        ("和欧間スペース欠落", symbol::missing_space(p)),
        ("和文間スペース", symbol::wabun_space(p)),
        ("笑い", symbol::laughter(p)),
        ("絵文字", symbol::emoji(p)),
        ("em dash", symbol::em_dash(p)),
        ("見出し", structure::headings(doc)),
        ("深い見出し", structure::deep_headings(doc)),
        ("箇条書き", structure::bullets(doc)),
        ("番号リスト", structure::ordered_lists(doc)),
        ("表", structure::tables(doc)),
        ("引用", structure::quotes(doc)),
        ("補足", structure::notes(doc)),
        ("警告", structure::warnings(doc)),
        ("折りたたみ", structure::details(doc)),
        ("強調", structure::emphasis(doc)),
        ("コードブロック", structure::code_blocks(doc)),
        (
            "1 文だけの段落の割合",
            structure::single_sentence_paragraphs(doc),
        ),
        ("段落あたりの文数", structure::sentences_per_paragraph(doc)),
        ("太字始まりの項目", structure::bold_leading_items(doc)),
        ("段落長の変動係数", structure::paragraph_length_cv(doc)),
        ("箇条書き項目長の変動係数", structure::item_length_cv(doc)),
        ("節の長さの変動係数", structure::section_length_cv(doc)),
        // 手で選んだ語句で数えるもの。形態素解析を要らない。
        ("非断定の密度", phrase::hedging(p)),
        ("対比構文", phrase::contrast(p)),
        ("自己否定の密度", phrase::self_negation(p)),
        ("進行の実況", phrase::narration(p)),
        ("脱線", phrase::digression(p)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use kakiburi_doc::node::{Kind, Node};

    fn density(n: usize, ja: usize) -> Counted {
        Counted::density(n, ja)
    }

    #[test]
    fn 割合は分子と分母から値を作る() {
        assert_eq!(Counted::share(3, 12, 10), Measured::Value(0.25));
        assert_eq!(
            density(2, 4000),
            Measured::Value(1000.0 * 2.0 / 4000.0),
            "1,000 字あたり"
        );
    }

    #[test]
    fn 分母が下限に届かなければ下限未満でも数は持つ() {
        // 束ねれば届くことがある。 先に捨てると束ねた単位の値が作れない。
        let c = density(2, 400);
        assert_eq!(c, Measured::BelowFloor);
        assert!(c.parts().is_some());
    }

    #[test]
    fn 束ねると数を足してから下限を掛け直す() {
        // 400 字と 700 字は、どちらも 1 本では下限を割る。
        let (a, b) = (density(2, 400), density(5, 700));
        let got = Counted::compose(&[&a, &b]);
        assert_eq!(got, Measured::Value(1000.0 * 7.0 / 1100.0));
    }

    #[test]
    fn 割合の平均は割合ではない() {
        let (a, b) = (Counted::share(1, 10, 10), Counted::share(9, 90, 10));
        let got = Counted::compose(&[&a, &b]).value().expect("測れる");
        assert!((got - 0.1).abs() < 1e-12, "{got}");
    }

    #[test]
    fn 束ねた変動係数は値を並べ直して測る() {
        let a = Counted::spread(vec![10, 20, 30], 5);
        let b = Counted::spread(vec![40, 50], 5);
        assert_eq!(a, Measured::BelowFloor, "1 本では本数が足りない");
        let got = Counted::compose(&[&a, &b]);
        assert_eq!(
            got,
            Counted::spread(vec![10, 20, 30, 40, 50], 5).measured(),
            "文書ごとの値を並べた分布で測る"
        );
    }

    #[test]
    fn 数は足す() {
        let got = Counted::compose(&[&Counted::of(Parts::Count(2)), &Counted::of(Parts::Count(3))]);
        assert_eq!(got, Measured::Value(5.0));
    }

    #[test]
    fn 数を持たない文書を束ねると束も数を持たない() {
        // 道具が無いのを、束ねて隠さない。
        let got = Counted::compose(&[
            &density(2, 4000),
            &Counted::unmeasured(Unmeasured::ToolMissing),
        ]);
        assert_eq!(got, Measured::ToolMissing);
        assert!(got.parts().is_none());
    }

    #[test]
    fn 違う指標の数は束ねない() {
        let got = Counted::compose(&[&density(2, 4000), &Counted::share(1, 20, 10)]);
        assert!(!got.is_measured());
    }

    #[test]
    fn 分母が_0_を分ける指標は分ける() {
        let absent = Counted::of(Parts::Ratio {
            num: 0,
            den: 0,
            per: 1,
            floor: 10,
            zero_is_absent: true,
        });
        assert_eq!(absent, Measured::NoDenominator);
        assert_eq!(Counted::share(0, 0, 10), Measured::BelowFloor);
    }

    #[test]
    fn 一覧の値は数から作った値と同じである() {
        // 1 本を束ねたものは、その 1 本を測ったものと一致する。
        let mut nodes = vec![Node::heading(1, "章")];
        for i in 0..12 {
            nodes.push(Node::leaf(
                Kind::Paragraph,
                format!("これは{i}番目の段落です。（補足）を入れます…… 次の文です。"),
            ));
        }
        let doc = Document::new(nodes);
        for (name, c) in measure(&doc, None) {
            let one = Counted::compose(&[&c]);
            assert_eq!(one, c, "{name}");
        }
    }
}
