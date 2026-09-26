//! 目盛り。語彙・重み・天井・床・帯。
//!
//! 検めがここに手を伸ばせないようにする。 検める文書を見てから重みや語彙を
//! 作り直せてしまう経路を作らない——[`Scale`]は作り終えた形しか公開しない。

pub mod assemble;
pub mod band;
pub mod calibrate;
pub mod effective;
pub mod humanness;
pub mod pairing;
pub mod split;
pub mod vocabulary;

pub use assemble::{
    assemble, diverging, inspect, measure_against, Divergence, HumannessByMetric, Measured, Report,
    Sample, Substituted,
};
pub use band::{Band, BandError, Ends, Verdict};
pub use calibrate::{Calibration, MatchError, Weights};
pub use effective::{Basis, Effective};
pub use humanness::{HumannessError, HumannessScale};
pub use pairing::{Pair, Pairing};
pub use split::{Split, SplitError, Unit};
pub use vocabulary::{cosine_delta, Counts, Frozen, FrozenSet};

/// 標本の長さの範囲が揃っているかを認める割合。暫定値である。
pub const LENGTH_OVERLAP_FLOOR: f64 = 0.5;

/// 帯で止まったときの内訳。
///
/// どちらの帯か、点がどこに来たかを添える。 割合だけでは、素材を足すべきか
/// 系統を見直すべきかが分からない——そして照合の帯と人らしさの帯は原因が違う。
#[derive(Debug, Clone, PartialEq)]
pub struct BandStop {
    /// 何が起きたか。
    pub source: BandError,
    /// どちらの帯か。
    pub which: &'static str,
    /// 天井の点。1 単位につき 1 点。
    pub ceiling: Vec<f64>,
    /// 床の点。
    pub floor: Vec<f64>,
    /// ここまでに作れた帯。
    ///
    /// 作れたものを言う。「止まった」だけでは、どこまで通ったのかが分からない——
    /// 照合の帯が作れていて人らしさで止まったのなら、直すべきは素材の側である。
    pub built: Option<Band>,
}

/// 目盛りが作れない理由。
#[derive(Debug, Clone, PartialEq)]
pub enum ScaleError {
    /// 単位が足りない。
    Split(SplitError),
    /// 帯が作れない。中身は別に持つ——理由のいちばん大きいものが型の大きさを決める。
    Band(Box<BandStop>),
    /// 標本の長さの範囲が揃っていない。
    ///
    /// 指標ごとではなく、その場面まるごと止める。 素材の取り方の問題なので、
    /// 指標を選び直して直るものではない。
    LengthRange {
        /// 本人の範囲に対する重なりの割合。
        of_person: f64,
        /// 基準の範囲に対する重なりの割合。
        of_baseline: f64,
    },
    /// 較正と目盛りが対を共有している。
    ///
    /// ここが壊れると、分離するように合わせたものの分離具合を測ることになる。
    SharedPairs,
}

impl std::fmt::Display for ScaleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScaleError::Split(e) => write!(f, "{e}"),
            ScaleError::Band(stop) => {
                let BandStop {
                    source,
                    which,
                    ceiling,
                    floor,
                    built,
                } = stop.as_ref();
                let show = |v: &[f64]| {
                    v.iter()
                        .map(|x| format!("{x:.2}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                };
                write!(
                    f,
                    "{which}の帯: {source}\n  天井の点: {}\n  床の点: {}",
                    show(ceiling),
                    show(floor)
                )?;
                if let Some(b) = built {
                    write!(
                        f,
                        "\n  ここまでは通った——照合値の帯は作れた（天井 {:.2}〜{:.2} / 床 {:.2}〜{:.2}）",
                        b.ceiling.low, b.ceiling.high, b.floor.low, b.floor.high
                    )?;
                }
                Ok(())
            }
            ScaleError::LengthRange {
                of_person,
                of_baseline,
            } => write!(
                f,
                "長さの範囲が半分も重ならない（本人の {:.0}%、基準の {:.0}%）",
                of_person * 100.0,
                of_baseline * 100.0
            ),
            ScaleError::SharedPairs => {
                write!(f, "較正と目盛りが対を共有している")
            }
        }
    }
}

impl std::error::Error for ScaleError {}

/// 長さの範囲が揃っているかを見る。
///
/// 見る前に確かめる。 揃っていなければ、離れていても重なっても、どちらの結果も
/// 信じられない——長い文書だけを書いた人と短い文書ばかりの基準を比べれば、長さと
/// 連動する指標はすべて離れて見える。
///
/// 範囲は最小と最大で測る。だから 1 単位が全体を止めうる。 分位で測れば外れ値に
/// 強くなるが、10 単位では分位も信じられない。
pub fn length_range_ok(person: &[usize], baseline: &[usize]) -> Result<(), ScaleError> {
    let span = |v: &[usize]| -> Option<(f64, f64)> {
        let lo = *v.iter().min()?;
        let hi = *v.iter().max()?;
        #[allow(clippy::cast_precision_loss)]
        Some((lo as f64, hi as f64))
    };
    let Some((plo, phi)) = span(person) else {
        return Err(ScaleError::LengthRange {
            of_person: 0.0,
            of_baseline: 0.0,
        });
    };
    let Some((blo, bhi)) = span(baseline) else {
        return Err(ScaleError::LengthRange {
            of_person: 0.0,
            of_baseline: 0.0,
        });
    };
    let overlap = (phi.min(bhi) - plo.max(blo)).max(0.0);
    let pr = (phi - plo).max(1.0);
    let br = (bhi - blo).max(1.0);
    let of_person = overlap / pr;
    let of_baseline = overlap / br;
    if of_person < LENGTH_OVERLAP_FLOOR || of_baseline < LENGTH_OVERLAP_FLOOR {
        return Err(ScaleError::LengthRange {
            of_person,
            of_baseline,
        });
    }
    Ok(())
}

/// 目盛りを決める較正の設定と閾値。指紋に入れる材料である。
///
/// 平文で返す。 ハッシュにすると、合わないときにどれが変わったかを言えない。
///
/// 同じ素材・同じ道具でも、ここが変われば別の目盛りができる。 入れなければ、
/// 閾値を動かしたあとも古いカセットが同じ指紋を名乗り、黙って使われる。
#[must_use]
pub fn settings() -> Vec<(&'static str, String)> {
    let list = |v: &[usize]| {
        v.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    vec![
        ("scale::calibrate::LAMBDA", calibrate::LAMBDA.to_string()),
        (
            "scale::calibrate::LEARNING_RATE",
            calibrate::LEARNING_RATE.to_string(),
        ),
        (
            "scale::calibrate::ITERATIONS",
            calibrate::ITERATIONS.to_string(),
        ),
        (
            "scale::calibrate::SUBSTITUTE_SIGMA",
            calibrate::SUBSTITUTE_SIGMA.to_string(),
        ),
        ("scale::band::CEILING_TRIM", band::CEILING_TRIM.to_string()),
        ("scale::band::FLOOR_TRIM", band::FLOOR_TRIM.to_string()),
        (
            "scale::band::OVERLAP_LIMIT",
            band::OVERLAP_LIMIT.to_string(),
        ),
        ("scale::band::GAP_MARGIN", band::GAP_MARGIN.to_string()),
        (
            "scale::humanness::SEPARATES",
            humanness::SEPARATES.to_string(),
        ),
        ("scale::humanness::FAINT", humanness::FAINT.to_string()),
        (
            "scale::effective::NARROW_RATIO",
            effective::NARROW_RATIO.to_string(),
        ),
        (
            "scale::effective::OVERLAP_RATIO",
            effective::OVERLAP_RATIO.to_string(),
        ),
        (
            "scale::effective::APPEARANCE_CONSISTENT",
            effective::APPEARANCE_CONSISTENT.to_string(),
        ),
        (
            "scale::effective::APPEARANCE_GAP",
            effective::APPEARANCE_GAP.to_string(),
        ),
        (
            "scale::vocabulary::MIN_SD_RATIO",
            vocabulary::MIN_SD_RATIO.to_string(),
        ),
        ("scale::split::PER_SIDE", split::PER_SIDE.to_string()),
        (
            "scale::LENGTH_OVERLAP_FLOOR",
            LENGTH_OVERLAP_FLOOR.to_string(),
        ),
        ("scale::assemble::PHRASES", assemble::PHRASES.to_string()),
        ("scale::assemble::KATA_N", list(&assemble::KATA_N)),
        (
            "scale::assemble::KATA_PERSON_MIN",
            assemble::KATA_PERSON_MIN.to_string(),
        ),
        (
            "scale::assemble::KATA_BASE_MAX",
            assemble::KATA_BASE_MAX.to_string(),
        ),
        ("scale::assemble::KATAS", assemble::KATAS.to_string()),
        (
            "scale::assemble::MACHINE_KATAS",
            assemble::MACHINE_KATAS.to_string(),
        ),
        (
            "scale::assemble::MACHINE_GOIS",
            assemble::MACHINE_GOIS.to_string(),
        ),
        (
            "scale::assemble::KATA_TIGHT",
            assemble::KATA_TIGHT.to_string(),
        ),
        (
            "scale::assemble::KATA_LONG",
            assemble::KATA_LONG.to_string(),
        ),
        (
            "scale::assemble::FRAME_KEEP",
            assemble::FRAME_KEEP.to_string(),
        ),
        (
            "scale::assemble::GOI_THEIRS",
            assemble::GOI_THEIRS.to_string(),
        ),
    ]
}

/// 相手集合の 1 単位ぶんの、系統ごとの部分ベクトル。
///
/// 単位の名前と、系統の名前から投影し終えた値への並びである。
pub type PartnerVector = (String, Vec<(String, Vec<f64>)>);

/// 作り終えた目盛り。
///
/// 検めはこれを受け取る。 中の語彙も重みも読めるが、作り直す道は無い——
/// [`Frozen::fit`]も[`Calibration::fit`]も全体を要求するので、検める 1 本から呼べない。
#[derive(Debug, Clone, PartialEq)]
pub struct Scale {
    /// 系統ごとの固定した語彙と z 得点。系統は部分ベクトルを複数持ちうる。
    pub frozen: Vec<(String, FrozenSet)>,
    /// 較正と合算の重み。
    pub calibration: Calibration,
    /// 照合値の帯。
    pub band: Band,
    /// 単位をどう割ったか。検めは相手集合と対にする。
    pub selection: Selection,
    /// 相手集合の本文から拾った実例。系統・次元ごとに数本。
    ///
    /// 「増やせ」と言うだけでは、どこに置くのかが分からない。 数値と向きだけを
    /// 渡された側は、結局その人の文章を自分で読みに行くことになる——
    /// カセットは本文を持たないので、読みに行く先が無い。
    ///
    /// どの次元を訊かれるかは検めるまで決まらない。 だから
    /// [読める系統](EXAMPLE_SYSTEMS)の次元を全部ここで拾っておく。
    pub examples: Vec<(String, String, Vec<String>)>,
    /// 相手集合の、系統ごとの部分ベクトル。本文の代わりである。
    ///
    /// 照合値は相手集合との中央値なので、検めるには相手の側の値が要る。
    /// カセットは本文を持たない（[素材を正本にする](../../../docs/spec/200-extract.md#素材を正本にする)）
    /// ので、投影し終えたベクトルだけを持つ——ここから本文は戻らないし、
    /// 戻す必要も無い。
    ///
    /// 外側が単位で、内側が系統である。並びは[固定した語彙](Self::frozen)に従う。
    pub partner_vectors: Vec<PartnerVector>,
    /// 人らしさの較正。
    pub humanness: HumannessScale,
    /// 人らしさ値の帯。天井にあたるのが人の側、床にあたるのが機械の側。
    pub humanness_band: Band,
    /// 人らしさの指標ごとの、本人の代表値。効く量を数える相手である。
    ///
    /// 直し方が 2 本以上出て正反対を指すことがある。 どちらが勝つかを
    /// 言わなければ、受け取った側は逆を選びうる。
    pub humanness_target: Vec<(String, f64)>,
    /// その人が現に繰り返している言い回し。多くの単位で再来した順。
    ///
    /// 直し方が「その人が現に繰り返している言い回しを繰り返す」と言うなら、その
    /// 言い回しを渡さなければ直せない。 数値と向きだけでは、受け取った側は自分で
    /// でっち上げた定型句を挿し込むことになる。
    pub phrases: Vec<String>,
    /// [渡した言い回し](Self::phrases)ごとの、本人が 1 本の中で使う上限。
    ///
    /// 日本語 1,000 字あたりの、本人の単位での最大である。
    ///
    /// 「繰り返せ」と言うなら、どこまで繰り返してよいかも言う。 言わなければ、
    /// 受け取った側は本人の何倍も入れる——実測で、本人が 42 本で 25 回しか使わない
    /// `ことができます` を、直した 1 本に 10 回入れた。
    /// 日本語は壊れないので検査では止まらない。 ここで測るしかない。
    pub phrase_ceilings: Vec<(String, f64)>,
    /// コーパスから見つけた語。辞書に無い語が割れるのを直す。
    ///
    /// 素材から作るものなので、素材が変われば変わる——
    /// 検めるときも同じ辞書で割らなければ、比べたものに意味が無い。
    pub lexicon: kakiburi_metrics::lexicon::Lexicon,
    /// その人の型。**コーパスから見つけた語の並び。**
    ///
    /// 手で並べた定型ではない——道具は書き手を選ばないので、特定の言い回しを実装に
    /// 持たない（[取り出し](assemble::Kata)）。
    pub katas: Vec<assemble::Kata>,
    /// 機械の型。 基準がよく使い、本人がほとんど使わない語の並び。
    ///
    /// [その人の型](Self::katas)と同じ仕組みを、役を入れ替えて回したものである。
    /// 片側だけでは足りない——本人の型が入っていないことは言えても、
    /// 機械の言い回しが残っていることが言えない。
    ///
    /// 実測で、本人が 50 単位中 1 度も使わない「地味に〜」を、基準は 21 単位中 3 本で
    /// 使っていた。元の草稿にそのまま残り、判定は通っていた。
    pub machine_katas: Vec<assemble::Kata>,
    /// 機械の語。 基準がよく使い、本人が使わない語彙素。
    ///
    /// [機械の型](Self::machine_katas)が表層の並びで取りこぼすものを拾う——
    /// 同じ癖が語形ごとに割れると、どの綴りも床を割る。実測で、基準の池 44 本のうち
    /// `地味` は 9 本に出るのに `地味に` という綴りは 2 本にしかなく、
    /// 並びとしては一度も拾えなかった。
    pub machine_gois: Vec<assemble::Goi>,
    /// 本人の一人称と、それが現れた単位の割合。多い順。
    ///
    /// [型](Self::katas)にも[語](Self::machine_gois)にも載らない。 型は表層の
    /// 並びを枠の数だけ採るので、実測で `僕は` は割合 0.35 で 7 番目に落ちた。
    /// 語は 形容詞・形状詞・副詞 しか見ないので代名詞が入らない。
    ///
    /// 一人称だけ別に持つ理由は、[閉じた集合](kakiburi_metrics::word::FIRST_PERSON)
    /// だからである。 開いた品詞では「本人が使わない」は題材でそうなるが、
    /// 一人称はどの題材でも必ずどれかを選ぶので、選ばれなかったことが癖になる。
    ///
    /// 実測で、本人 49 本のうち `僕` が 28 本（57%）、`私` が 5 本（10%）。
    /// 基準 44 本に `僕` は 1 度も出てこない。
    pub first_person: Vec<(String, f64)>,
    /// 本人の[書き出しの node の種類](kakiburi_doc::Document::opening)と、その割合。多い順。
    ///
    /// [見出しの密度](assemble::Kata)では言えない。 密度は 1,000 字あたりの
    /// 本数なので、見出しが 1 番目にあっても 3 番目にあっても同じ値になる。
    /// [型](Self::katas)の位置でも言えない——あれは語の位置で、挨拶の前に
    /// 見出しを 1 本挟んでも 0.000 が 0.02 になるだけである。
    ///
    /// 実測で、本人 49 本のうち 46 本（93%）が段落で始まり、基準 44 本では
    /// 31 本（70%）が見出しで始まる。
    pub opening: Vec<(String, f64)>,
}

impl Scale {
    /// 相手集合の単位の名前。あらゆる照合の相手である。
    #[must_use]
    pub fn partners(&self) -> &[String] {
        &self.selection.person_partners
    }

    /// [取り置いたベクトル](Self::partner_vectors)が目盛りと噛み合っているか。
    ///
    /// 読めることと、噛み合っていることは別である。 次元の数がずれていても
    /// [距離](crate::vocabulary::cosine_delta)は短いほうまでで計算されるので、
    /// エラーにならずに違う照合値が出る——目盛りがあるのに壊れている
    /// という、いちばん見えにくい形になる。
    ///
    /// 欠けた系統も同じである。 相手ごと落ちるだけで、残りの相手で
    /// 中央値が出てしまう。
    #[must_use]
    pub fn partner_vectors_ok(&self) -> bool {
        if self.partner_vectors.len() != self.selection.person_partners.len() {
            return false;
        }
        for ((unit, parts), want) in self
            .partner_vectors
            .iter()
            .zip(&self.selection.person_partners)
        {
            if unit != want || parts.len() != self.frozen.len() {
                return false;
            }
            for ((name, v), (fname, set)) in parts.iter().zip(&self.frozen) {
                let dims: usize = set.parts().iter().map(crate::vocabulary::Frozen::len).sum();
                if name != fname || v.len() != dims || v.iter().any(|x| !x.is_finite()) {
                    return false;
                }
            }
        }
        true
    }
}

/// 単位をどう割ったか。名前だけを持つ。
///
/// 正本ではない。[単位名の昇順](../../../docs/spec/200-extract.md#相手集合を-1-つ決める)
/// から決定的に導けるので、持つのは覗くためである。
///
/// 覗けないと、割りが偏っていることに気付けない。 昇順で取るので、単位名に
/// 年や媒体が入っていれば、相手集合と測る分がその境目で分かれる——値は出るし、
/// エラーにもならない。 帯は「本人 対 本人」ではなく「ある時期 対 別の時期」に
/// なっているのに、出力からは区別が付かない。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Selection {
    /// 本人の相手集合。
    pub person_partners: Vec<String>,
    /// 本人の測る分。天井の点になる。
    pub person_points: Vec<String>,
    /// 基準の較正分。
    pub baseline_partners: Vec<String>,
    /// 基準の床の点。
    pub baseline_points: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 長さの範囲が揃っていれば通る() {
        let person = vec![1000, 1500, 2000, 2500, 3000];
        let baseline = vec![1200, 1800, 2400, 2800];
        assert!(length_range_ok(&person, &baseline).is_ok());
    }

    #[test]
    fn 短い基準では判定を出さない() {
        // 長さと連動する指標がすべて離れて見える。
        let person = vec![1000, 3000, 5000, 12000];
        let baseline = vec![1200, 1300, 1400];
        let e = length_range_ok(&person, &baseline).unwrap_err();
        assert!(matches!(e, ScaleError::LengthRange { .. }), "{e:?}");
    }

    #[test]
    fn 外れ値_1_本が全体を止めうる() {
        // 分位で測れば外れ値に強くなるが、10 単位では分位も信じられない。
        let baseline: Vec<usize> = (0..10).map(|i| 1200 + i * 200).collect();
        let tight: Vec<usize> = (0..10).map(|i| 1200 + i * 200).collect();
        assert!(length_range_ok(&tight, &baseline).is_ok());

        let mut with_outlier = tight.clone();
        with_outlier.push(12000);
        let e = length_range_ok(&with_outlier, &baseline).unwrap_err();
        assert!(
            matches!(e, ScaleError::LengthRange { .. }),
            "1 本で止まる: {e:?}"
        );
    }

    #[test]
    fn 空なら通さない() {
        assert!(length_range_ok(&[], &[1000]).is_err());
        assert!(length_range_ok(&[1000], &[]).is_err());
    }

    #[test]
    fn 較正の設定と閾値を平文で返す() {
        // どれが変わったかを指紋の差として名指せるようにする。
        let s = settings();
        for (name, want) in [
            ("scale::calibrate::LAMBDA", calibrate::LAMBDA.to_string()),
            ("scale::band::GAP_MARGIN", band::GAP_MARGIN.to_string()),
            (
                "scale::calibrate::SUBSTITUTE_SIGMA",
                calibrate::SUBSTITUTE_SIGMA.to_string(),
            ),
            (
                "scale::humanness::SEPARATES",
                humanness::SEPARATES.to_string(),
            ),
        ] {
            assert!(
                s.contains(&(name, want.clone())),
                "{name}={want} が無い: {s:?}"
            );
        }
        let mut names: Vec<&str> = s.iter().map(|(n, _)| *n).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), s.len(), "名前が重なれば片方が消える");
    }

    #[test]
    fn 理由は読める形で出る() {
        let e = length_range_ok(&[1000, 12000], &[1200, 1300]).unwrap_err();
        let s = e.to_string();
        assert!(s.contains("長さの範囲"), "{s}");
    }
}
