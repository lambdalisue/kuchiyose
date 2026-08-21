//! 人らしさ値。<strong>12 次元を 1 つの数にする。</strong>
//!
//! 照合値と違って<strong>距離を取る相手がいない</strong>——比べるのは 2 本の文書ではなく、
//! 人が書いたものの集団と機械が書いたものの集団である。<strong>だから次元がそのまま尤度比の
//! 単位になる。</strong>
//!
//! | | すること |
//! | --- | --- |
//! | 1 | 12 次元を測る |
//! | 2 | <strong>次元ごとに</strong>、人の側と機械の側に照らして尤度比に変える |
//! | 3 | <strong>指標ごとに</strong>、その指標の次元の対数尤度比を平均する |
//! | 4 | 5 つを合算して 1 つの人らしさ値にする |
//!
//! <strong>3 段目で平均するのは、当てはめる重みを 4 つに抑えるためである。</strong> 較正に使える単位は
//! 10 本しかない。12 の重みを 10 点から当てはめれば、どうとでも決まってしまう——
//! <strong>繰り返しだけで 8 次元あり、そこが重みを持ち去る。</strong>

use kakiburi_metrics::humanness::Metric;
use kakiburi_metrics::Measured;

use crate::calibrate::Weights;

/// 人らしさ値が出せない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HumannessError {
    /// 次元が欠けている。
    ///
    /// <strong>欠けた分を抜いて合算しない。</strong> 重みはそろっている前提で較正されている。
    MissingDims {
        /// 欠けた次元の名前。
        names: Vec<String>,
    },
}

impl std::fmt::Display for HumannessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HumannessError::MissingDims { names } => {
                write!(f, "次元が測れていない: {}", names.join("、"))
            }
        }
    }
}

impl std::error::Error for HumannessError {}

/// 人らしさの較正。<strong>1 組だけできる。</strong>
#[derive(Debug, Clone, PartialEq)]
pub struct HumannessScale {
    /// 次元の名前。<strong>並び順が入力の順を決める。</strong>
    dims: Vec<String>,
    /// 次元ごとの、値 → 対数尤度比。
    per_dim: Vec<Weights>,
    /// 指標ごとの対数尤度比の平均 → 1 つの人らしさ値。
    fusion: Weights,
}

/// 向きを持つとみなす傾きの下限。<strong>その指標のいちばん大きい傾きに対する割合。</strong>
///
/// <strong>暫定値である。</strong> 実測では、文字のエントロピーの傾きが語のエントロピーの
/// 7% しかなく、それだけでエントロピーが指示できない指標になっていた。
pub const FAINT: f64 = 0.2;

/// 12 次元の並び。<strong>指標の並びから引く。</strong>
#[must_use]
pub fn dims() -> Vec<String> {
    Metric::ALL.into_iter().flat_map(Metric::dims).collect()
}

/// 次元が何番目の指標に属するか。
#[must_use]
fn owners() -> Vec<usize> {
    Metric::ALL
        .iter()
        .enumerate()
        .flat_map(|(i, m)| std::iter::repeat_n(i, m.dims().len()))
        .collect()
}

impl HumannessScale {
    /// 較正する。
    ///
    /// `human` と `machine` は 1 行が 1 単位ぶんの 12 次元。<strong>単位で割る</strong>——
    /// 人らしさは 1 本ごとに出る値で、対を作らないからである。
    #[must_use]
    pub fn fit(human: &[Vec<f64>], machine: &[Vec<f64>]) -> Self {
        let dims = dims();
        let mut rows: Vec<Vec<f64>> = Vec::new();
        let mut labels: Vec<u8> = Vec::new();
        for r in human {
            rows.push(r.clone());
            labels.push(1);
        }
        for r in machine {
            rows.push(r.clone());
            labels.push(0);
        }
        let per_dim: Vec<Weights> = (0..dims.len())
            .map(|j| {
                let column: Vec<Vec<f64>> = rows
                    .iter()
                    .map(|r| vec![r.get(j).copied().unwrap_or(0.0)])
                    .collect();
                Weights::fit(&column, &labels)
            })
            .collect();
        let fused: Vec<Vec<f64>> = rows.iter().map(|r| per_metric(&per_dim, r)).collect();
        Self {
            dims,
            per_dim,
            fusion: Weights::fit(&fused, &labels),
        }
    }

    /// 1 本の人らしさ値。<strong>1 次元でも欠けたら出さない。</strong>
    pub fn value(&self, measured: &[(String, Measured)]) -> Result<f64, HumannessError> {
        let mut missing = Vec::new();
        let mut row = Vec::with_capacity(self.dims.len());
        for name in &self.dims {
            match measured
                .iter()
                .find(|(n, _)| n == name)
                .and_then(|(_, m)| m.value())
            {
                Some(v) => row.push(v),
                None => missing.push(name.clone()),
            }
        }
        if !missing.is_empty() {
            return Err(HumannessError::MissingDims { names: missing });
        }
        Ok(self.fusion.log_lr(&per_metric(&self.per_dim, &row)))
    }

    /// 指標ごとの人らしさ値。<strong>どれが機械の側にあるかを名指しできる。</strong>
    ///
    /// 合算した 1 つの値では「機械の側にある」としか言えず、直し方を渡せない。
    /// <strong>正が人の側、負が機械の側である。</strong>
    ///
    /// <strong>欠けたら出さない</strong>のは[合算](Self::value)と同じ——一部の指標だけで
    /// 直し方を出せば、測れていない指標が見落とされる。
    pub fn by_metric(
        &self,
        measured: &[(String, Measured)],
    ) -> Result<Vec<(&'static str, f64)>, HumannessError> {
        let mut missing = Vec::new();
        let mut row = Vec::with_capacity(self.dims.len());
        for name in &self.dims {
            match measured
                .iter()
                .find(|(n, _)| n == name)
                .and_then(|(_, m)| m.value())
            {
                Some(v) => row.push(v),
                None => missing.push(name.clone()),
            }
        }
        if !missing.is_empty() {
            return Err(HumannessError::MissingDims { names: missing });
        }
        Ok(Metric::ALL
            .into_iter()
            .map(Metric::name)
            .zip(per_metric(&self.per_dim, &row))
            .collect())
    }

    /// 指標ごとの値から合算する。<strong>1 指標だけ置き換えられる。</strong>
    ///
    /// <strong>効く量を数えるための道具である。</strong> 人らしさの直し方は 2 本以上出ることが
    /// あり、<strong>互いに正反対を指すことがある</strong>——語彙を散らせと言う指標と、
    /// 言い換えるなと言う指標が同時に出る。<strong>どちらが勝つかを言わなければ、
    /// 受け取った側は逆を選びうる。</strong> 実測でそれが起きた。
    #[must_use]
    pub fn fuse(&self, values: &[f64], swap: Option<(usize, f64)>) -> f64 {
        let mut v = values.to_vec();
        if let Some((j, x)) = swap {
            if let Some(slot) = v.get_mut(j) {
                *slot = x;
            }
        }
        self.fusion.log_lr(&v)
    }

    /// 次元の名前と並び。
    #[must_use]
    pub fn dims(&self) -> &[String] {
        &self.dims
    }

    /// 次元ごとの重み。指紋に入る。
    #[must_use]
    pub fn per_dim(&self) -> &[Weights] {
        &self.per_dim
    }

    /// 合算の重み。
    #[must_use]
    pub fn fusion(&self) -> &Weights {
        &self.fusion
    }

    /// 保存した派生物から組み立てる。
    #[must_use]
    pub fn restore(per_dim: Vec<Weights>, fusion: Weights) -> Option<Self> {
        let dims = dims();
        if per_dim.len() != dims.len() {
            return None;
        }
        // <strong>本数だけでは足りない。</strong> 傾きが 0 本の重みは、どんな入力でも切片だけを
        // 返す——<strong>壊れた目盛りが「いつも同じ値」を出す判定器として正常に動く。</strong>
        //
        // 次元ごとは値 1 つを受けるので 1 次元、合算は指標の数だけ受ける。
        if per_dim.iter().any(|w| w.slopes().len() != 1) {
            return None;
        }
        if fusion.slopes().len() != Metric::ALL.len() {
            return None;
        }
        Some(Self {
            dims,
            per_dim,
            fusion,
        })
    }

    /// 較正が定義と逆を学んだ次元の名前。
    ///
    /// <strong>判定には使う。</strong> 目盛りは素材から作るものであり、素材が言ったことを
    /// 捨てれば判定が弱くなるだけである。
    ///
    /// <strong>直し方には使えない。</strong> 定義は「どちらが機械の側か」を先行研究に基づいて
    /// 名乗っており、較正がその逆を学んだなら、<strong>その直し方に従うほど人らしさが
    /// 下がる</strong>——道具が自分の指示で自分の判定を悪くする。
    #[must_use]
    pub fn contradicting_dims(&self) -> Vec<String> {
        let owners = owners();
        self.dims
            .iter()
            .enumerate()
            .zip(&self.per_dim)
            .filter(|((j, _), w)| !agrees(&owners, *j, w))
            .map(|((_, n), _)| n.clone())
            .collect()
    }

    /// 指標ごとの、<strong>人へ寄せる向き</strong>。`true` なら値を上げる。
    ///
    /// <strong>向きは較正から読む。</strong> 定義に固定すると、素材がその向きを支えていない
    /// カセットで<strong>直し方に従うほど人らしさが下がる</strong>。先行研究が言うのは
    /// 「普通はこちら」であって、この人とこの基準でそうなるとはかぎらない。
    ///
    /// <strong>次元の向きが割れている指標は返さない。</strong> どちらへ動かせばよいかを言えない
    /// ものを指示にしない。
    #[must_use]
    pub fn toward_human(&self) -> Vec<(&'static str, bool)> {
        let owners = owners();
        Metric::ALL
            .into_iter()
            .enumerate()
            .filter_map(|(i, m)| {
                let slopes: Vec<f64> = self
                    .per_dim
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| owners.get(*j) == Some(&i))
                    .map(|(_, w)| w.slopes().first().copied().unwrap_or(0.0))
                    .collect();
                // <strong>向きを持たない次元は、向きの割れに数えない。</strong> 傾きがほぼ 0 の
                // 次元はどちらへ動かしても値を変えないので、<strong>それが 1 本あるだけで
                // 指標全体を指示できなくするのは、無い信号に判断を委ねること</strong>である。
                let max = slopes.iter().fold(0.0f64, |a, b| a.max(b.abs()));
                if max == 0.0 {
                    return None;
                }
                let mut signs = slopes
                    .iter()
                    .filter(|x| x.abs() / max >= FAINT)
                    .map(|x| *x > 0.0)
                    .peekable();
                let first = *signs.peek()?;
                signs.all(|x| x == first).then_some((m.name(), first))
            })
            .collect()
    }

    /// 5 つに均等に開いていないか。
    ///
    /// <strong>開いていたら較正を疑う。</strong> 5 つは同じ現象を別の角度から見ているので、
    /// 較正が偏らせるはずである。
    #[must_use]
    pub fn evenly_spread(&self) -> bool {
        let w = self.fusion.slopes();
        let max = w.iter().fold(0.0f64, |a, b| a.max(b.abs()));
        if max == 0.0 {
            return true;
        }
        w.iter().all(|x| x.abs() / max > 0.8)
    }
}

/// その次元の傾きが、定義の名乗る向きと合っているか。
///
/// 定義が「機械の側が高い」と言うなら、人へ寄るほど値は小さい——傾きは負である。
fn agrees(owners: &[usize], j: usize, w: &Weights) -> bool {
    let Some(upper) = owners
        .get(j)
        .and_then(|&i| Metric::ALL.get(i))
        .map(|m| m.upper_bound())
    else {
        return false;
    };
    let slope = w.slopes().first().copied().unwrap_or(0.0);
    // <strong>0 は合っていない。</strong> どちらへ動かしても値が変わらない次元では、直し方が
    // 効いたかを次の周で確かめられない。
    if slope == 0.0 {
        return false;
    }
    (slope < 0.0) == upper
}

/// 指標ごとに、その指標の次元の<strong>対数尤度比を平均する</strong>。
///
/// <strong>対数で平均する。</strong> 尤度比そのままで平均すると、いちばん大きい 1 次元が全体を
/// 決めてしまい、繰り返しの 8 次元を抑えるという目的が消える。
fn per_metric(per_dim: &[Weights], row: &[f64]) -> Vec<f64> {
    let owners = owners();
    let mut sums = vec![0.0; Metric::ALL.len()];
    let mut counts = vec![0usize; Metric::ALL.len()];
    for (j, w) in per_dim.iter().enumerate() {
        let Some(&owner) = owners.get(j) else {
            continue;
        };
        let x = row.get(j).copied().unwrap_or(0.0);
        sums[owner] += w.log_lr(&[x]);
        counts[owner] += 1;
    }
    sums.iter()
        .zip(&counts)
        .map(|(s, &c)| {
            if c == 0 {
                0.0
            } else {
                #[allow(clippy::cast_precision_loss)]
                let n = c as f64;
                s / n
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(row: &[f64]) -> Vec<(String, Measured)> {
        dims()
            .into_iter()
            .zip(row)
            .map(|(n, v)| (n, Measured::Value(*v)))
            .collect()
    }

    /// 人の側は繰り返しが多く、ほかの 3 つが低い。
    fn human_row(seed: f64) -> Vec<f64> {
        let mut row = vec![0.30 + seed; 1];
        row.extend(std::iter::repeat_n(0.50 + seed, 8));
        row.push(0.30 + seed);
        row.push(8.0 + seed);
        row.push(6.0 + seed);
        row
    }

    /// 機械の側は繰り返しが足りず、ほかの 3 つが高い。
    fn machine_row(seed: f64) -> Vec<f64> {
        let mut row = vec![0.45 + seed; 1];
        row.extend(std::iter::repeat_n(0.10 + seed, 8));
        row.push(0.60 + seed);
        row.push(10.0 + seed);
        row.push(7.5 + seed);
        row
    }

    fn scale() -> HumannessScale {
        let human: Vec<Vec<f64>> = (0..5).map(|i| human_row(f64::from(i) * 0.01)).collect();
        let machine: Vec<Vec<f64>> = (0..5).map(|i| machine_row(f64::from(i) * 0.01)).collect();
        HumannessScale::fit(&human, &machine)
    }

    #[test]
    fn 向きが割れている指標には直し方を渡さない() {
        // <strong>どちらへ動かせばよいかを言えない。</strong> 言えないものを指示にしない。
        // <strong>判定には使う</strong>——素材が言ったことを捨てれば判定が弱くなるだけである。
        //
        // 繰り返しだけを逆に置く——機械の側が多く繰り返す素材である。
        let flip = |human: bool, i: usize| {
            let seed = f64::from(i as u32) * 0.01;
            // ほかの 3 指標は定義どおりのまま、繰り返しだけを入れ替える。
            let mut row = if human {
                human_row(seed)
            } else {
                machine_row(seed)
            };
            let rep = if human { 0.10 } else { 0.50 } + seed;
            for x in row.iter_mut().take(9).skip(1) {
                *x = rep;
            }
            row
        };
        let human: Vec<Vec<f64>> = (0..5).map(|i| flip(true, i)).collect();
        let machine: Vec<Vec<f64>> = (0..5).map(|i| flip(false, i)).collect();
        let s = HumannessScale::fit(&human, &machine);
        let bad = s.contradicting_dims();
        assert_eq!(bad.len(), 8, "繰り返しの 8 次元が定義と食い違う: {bad:?}");
        assert!(bad.iter().all(|n| n.contains("gram")), "{bad:?}");
        // <strong>定義と食い違っても、向きが揃っていれば渡せる。</strong> 素材が言う向きへ
        // 寄せればよい——繰り返しは<strong>減らす</strong>側が人になる。
        let toward = s.toward_human();
        assert_eq!(toward.len(), Metric::ALL.len(), "{toward:?}");
        for m in ["短い繰り返し", "長い繰り返し", "圧縮率"] {
            assert!(
                !toward.iter().find(|(n, _)| *n == m).unwrap().1,
                "{m}: {toward:?}"
            );
        }
    }

    #[test]
    fn 向きが定義どおりなら_4_つとも定義の側になる() {
        let s = scale();
        assert!(
            s.contradicting_dims().is_empty(),
            "{:?}",
            s.contradicting_dims()
        );
        let toward = s.toward_human();
        assert_eq!(toward.len(), Metric::ALL.len());
        // 繰り返しは増やす側、ほかは減らす側が人である。
        for m in ["短い繰り返し", "長い繰り返し"] {
            assert!(toward.iter().find(|(n, _)| *n == m).unwrap().1, "{m}");
        }
        assert!(!toward.iter().find(|(m, _)| *m == "圧縮率").unwrap().1);
    }

    #[test]
    fn 次元は_12_である() {
        assert_eq!(dims().len(), 12);
    }

    #[test]
    fn 次元は必ずどれかの指標に属する() {
        // 属さない次元があれば、平均から静かに落ちる。
        assert_eq!(owners().len(), dims().len());
        assert!(owners().iter().all(|&i| i < Metric::ALL.len()));
    }

    #[test]
    fn 人の側は機械の側より大きく出る() {
        let s = scale();
        let h = s.value(&named(&human_row(0.005))).unwrap();
        let m = s.value(&named(&machine_row(0.005))).unwrap();
        assert!(h > m, "人のほうが大きい: {h} vs {m}");
    }

    /// 実測に近い値。<strong>次元の尺度が 3 桁ちがう。</strong>
    ///
    /// 繰り返しの 8 次元と異なり語率は 0.003〜0.4、語のエントロピーは 7 前後、
    /// 圧縮率は 6 前後である。
    fn wide_row(human: bool, i: usize) -> Vec<f64> {
        // 並びは 圧縮率 1 / 繰り返し 8 / 異なり語率 1 / エントロピー 2。
        //
        // <strong>繰り返しと異なり語率は両側が重なる。</strong> 実測の広がりをそのまま使い、
        // 向きだけ定義に合わせる——繰り返しは人が多い側、異なり語率は機械が高い側。
        let rep = if human {
            [0.0048, 0.0090, 0.0150, 0.0210, 0.0257][i]
        } else {
            [0.0043, 0.0060, 0.0080, 0.0110, 0.0144][i]
        };
        let rich = if human {
            [0.250, 0.270, 0.300, 0.320, 0.340][i]
        } else {
            [0.259, 0.280, 0.310, 0.340, 0.362][i]
        };
        // <strong>語のエントロピーだけが分ける。値は 7 前後で、繰り返しの 3 桁上である。</strong>
        // 人の側に 1 本だけ機械の側へ食い込む値がある——人が書いた記事が
        // 機械並みに散らないことは実際に起きる。
        //
        // <strong>向きは定義に従う。</strong> エントロピーは上限だけを持つ——機械の側が高く出る。
        // 逆に置けば[向きが割れている](HumannessScale::toward_human)として直し方を
        // 渡せなくなるので、標準化を試すための素材にならない。
        let ent = if human {
            [6.99, 7.02, 7.05, 7.08, 7.91][i]
        } else {
            [7.23, 7.45, 7.60, 7.80, 7.96][i]
        };
        let mut row = vec![if human { 6.35 } else { 6.40 } + i as f64 * 0.02];
        row.extend(std::iter::repeat_n(rep, 8));
        row.push(rich);
        row.push(ent);
        row.push(ent - 2.0);
        row
    }

    #[test]
    fn 尺度が_3_桁ちがっても人のほうが大きく出る() {
        // <strong>標準化しないと、値の大きい 1 次元で符号が逆転して合算を支配する。</strong>
        // 実測では人らしさ値がどの文書でも同じ 1 点に潰れ、しかも向きが逆だった
        // ——天井と床が重なるので、素材を何に替えても止まる。
        let human: Vec<Vec<f64>> = (0..5).map(|i| wide_row(true, i)).collect();
        let machine: Vec<Vec<f64>> = (0..5).map(|i| wide_row(false, i)).collect();
        let s = HumannessScale::fit(&human, &machine);

        let hs: Vec<f64> = human.iter().map(|r| s.value(&named(r)).unwrap()).collect();
        let ms: Vec<f64> = machine
            .iter()
            .map(|r| s.value(&named(r)).unwrap())
            .collect();
        let lo = hs.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = ms.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(lo > hi, "人の側が丸ごと上に来る: 人 {hs:?} / 機械 {ms:?}");
    }

    #[test]
    fn 人らしさ値が_1_点に潰れない() {
        // 潰れれば天井と床の広がりが 0 になり、帯が作れない。
        let human: Vec<Vec<f64>> = (0..5).map(|i| wide_row(true, i)).collect();
        let machine: Vec<Vec<f64>> = (0..5).map(|i| wide_row(false, i)).collect();
        let s = HumannessScale::fit(&human, &machine);
        let hs: Vec<f64> = human.iter().map(|r| s.value(&named(r)).unwrap()).collect();
        let spread = hs.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            - hs.iter().copied().fold(f64::INFINITY, f64::min);
        assert!(spread > 1e-3, "広がりが 0 では端が決まらない: {hs:?}");
    }

    #[test]
    fn 次元が_1_つでも欠けたら出さない() {
        // 欠けた分を抜いて合算しない。
        let s = scale();
        let mut row = named(&human_row(0.0));
        row[3].1 = Measured::BelowFloor;
        let e = s.value(&row).unwrap_err();
        let HumannessError::MissingDims { names } = e;
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn 次元が無ければ名前を添える() {
        let s = scale();
        let e = s.value(&[]).unwrap_err();
        let HumannessError::MissingDims { names } = e;
        assert_eq!(names.len(), 12, "12 次元とも欠けている");
    }

    #[test]
    fn 重みは指標の数だけである() {
        // 12 の重みを 10 点から当てはめない。繰り返しの 8 次元が重みを持ち去る。
        let s = scale();
        assert_eq!(s.fusion().slopes().len(), Metric::ALL.len());
        assert_eq!(s.per_dim().len(), 12);
    }

    #[test]
    fn 繰り返しの_8_次元は_2_つぶんに畳まれる() {
        // 平均する前と後で、合算の入力の数が変わる。
        // <strong>短いと長いは別の指標</strong>なので、8 次元は 2 つになる。
        let s = scale();
        let row = human_row(0.0);
        assert_eq!(per_metric(s.per_dim(), &row).len(), Metric::ALL.len());
    }

    #[test]
    fn 読み戻しは次元の数が合わなければ組み立てない() {
        let s = scale();
        assert!(HumannessScale::restore(s.per_dim().to_vec(), s.fusion().clone()).is_some());
        assert!(HumannessScale::restore(vec![], s.fusion().clone()).is_none());

        // <strong>本数だけでは足りない。</strong> 傾きが 0 本の重みは、どんな入力でも切片だけを
        // 返す——壊れた目盛りが「いつも同じ値」を出す判定器として正常に動く。
        let empty = Weights::restore(0.0, vec![], vec![], vec![]).expect("組める");
        assert!(
            HumannessScale::restore(vec![empty.clone(); dims().len()], s.fusion().clone())
                .is_none(),
            "次元ごとの傾きが空でも通っている"
        );
        assert!(
            HumannessScale::restore(s.per_dim().to_vec(), empty).is_none(),
            "合算の傾きが空でも通っている"
        );
    }

    #[test]
    fn 同じ素材から同じ重みが出る() {
        assert_eq!(scale(), scale());
    }
}
