//! 目盛り。語彙・重み・天井・床・帯。
//!
//! <strong>検めがここに手を伸ばせないようにする。</strong> 検める文書を見てから重みや語彙を
//! 作り直せてしまう経路を作らない——[`Scale`]は作り終えた形しか公開しない。

pub mod assemble;
pub mod band;
pub mod calibrate;
pub mod effective;
pub mod humanness;
pub mod pairing;
pub mod split;
pub mod vocabulary;

pub use assemble::{assemble, inspect, measure_against, Measured, Report, Sample};
pub use band::{Band, BandError, Ends, Verdict};
pub use calibrate::{Calibration, MatchError, Weights};
pub use effective::Effective;
pub use humanness::{HumannessError, HumannessScale};
pub use pairing::{Pair, Pairing};
pub use split::{Split, SplitError, Unit};
pub use vocabulary::{cosine_delta, Counts, Frozen, FrozenSet};

/// 標本の長さの範囲が揃っているかを認める割合。<strong>暫定値である。</strong>
pub const LENGTH_OVERLAP_FLOOR: f64 = 0.5;

/// 帯で止まったときの内訳。
///
/// <strong>どちらの帯か、点がどこに来たかを添える。</strong> 割合だけでは、素材を足すべきか
/// 系統を見直すべきかが分からない——そして<strong>照合の帯と人らしさの帯は原因が違う。</strong>
#[derive(Debug, Clone, PartialEq)]
pub struct BandStop {
    /// 何が起きたか。
    pub source: BandError,
    /// どちらの帯か。
    pub which: &'static str,
    /// 天井の点。<strong>1 単位につき 1 点。</strong>
    pub ceiling: Vec<f64>,
    /// 床の点。
    pub floor: Vec<f64>,
    /// ここまでに作れた帯。
    ///
    /// <strong>作れたものを言う。</strong>「止まった」だけでは、どこまで通ったのかが分からない——
    /// 照合の帯が作れていて人らしさで止まったのなら、直すべきは素材の側である。
    pub built: Option<Band>,
}

/// 目盛りが作れない理由。
#[derive(Debug, Clone, PartialEq)]
pub enum ScaleError {
    /// 単位が足りない。
    Split(SplitError),
    /// 帯が作れない。<strong>中身は別に持つ</strong>——理由のいちばん大きいものが型の大きさを決める。
    Band(Box<BandStop>),
    /// 標本の長さの範囲が揃っていない。
    ///
    /// <strong>指標ごとではなく、その場面まるごと止める。</strong> 素材の取り方の問題なので、
    /// 指標を選び直して直るものではない。
    LengthRange {
        /// 本人の範囲に対する重なりの割合。
        of_person: f64,
        /// 基準の範囲に対する重なりの割合。
        of_baseline: f64,
    },
    /// 較正と目盛りが対を共有している。
    ///
    /// <strong>ここが壊れると、分離するように合わせたものの分離具合を測ることになる。</strong>
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
/// <strong>見る前に確かめる。</strong> 揃っていなければ、離れていても重なっても、どちらの結果も
/// 信じられない——長い文書だけを書いた人と短い文書ばかりの基準を比べれば、長さと
/// 連動する指標はすべて離れて見える。
///
/// <strong>範囲は最小と最大で測る。だから 1 単位が全体を止めうる。</strong> 分位で測れば外れ値に
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

/// 作り終えた目盛り。
///
/// <strong>検めはこれを受け取る。</strong> 中の語彙も重みも読めるが、<strong>作り直す道は無い</strong>——
/// [`Frozen::fit`]も[`Calibration::fit`]も全体を要求するので、検める 1 本から呼べない。
#[derive(Debug, Clone, PartialEq)]
pub struct Scale {
    /// 系統ごとの固定した語彙と z 得点。<strong>系統は部分ベクトルを複数持ちうる。</strong>
    pub frozen: Vec<(String, FrozenSet)>,
    /// 較正と合算の重み。
    pub calibration: Calibration,
    /// 照合値の帯。
    pub band: Band,
    /// 単位をどう割ったか。<strong>検めは相手集合と対にする。</strong>
    pub selection: Selection,
    /// 人らしさの較正。
    pub humanness: HumannessScale,
    /// 人らしさ値の帯。<strong>天井にあたるのが人の側、床にあたるのが機械の側。</strong>
    pub humanness_band: Band,
}

impl Scale {
    /// 相手集合の単位の名前。<strong>あらゆる照合の相手である。</strong>
    #[must_use]
    pub fn partners(&self) -> &[String] {
        &self.selection.person_partners
    }
}

/// 単位をどう割ったか。<strong>名前だけを持つ。</strong>
///
/// <strong>正本ではない。</strong>[単位名の昇順](../../../docs/spec/200-extract.md#相手集合を-1-つ決める)
/// から決定的に導けるので、持つのは<strong>覗くため</strong>である。
///
/// <strong>覗けないと、割りが偏っていることに気付けない。</strong> 昇順で取るので、単位名に
/// 年や媒体が入っていれば、相手集合と測る分がその境目で分かれる——<strong>値は出るし、
/// エラーにもならない。</strong> 帯は「本人 対 本人」ではなく「ある時期 対 別の時期」に
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
    fn 理由は読める形で出る() {
        let e = length_range_ok(&[1000, 12000], &[1200, 1300]).unwrap_err();
        let s = e.to_string();
        assert!(s.contains("長さの範囲"), "{s}");
    }
}
