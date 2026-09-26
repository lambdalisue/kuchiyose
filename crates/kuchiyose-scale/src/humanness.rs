//! 人らしさ値。各次元を 1 つの数にする。
//!
//! 照合値と違って距離を取る相手がいない——比べるのは 2 本の文書ではなく、
//! 人が書いたものの集団と機械が書いたものの集団である。だから次元がそのまま尤度比の
//! 単位になる。
//!
//! | | すること |
//! | --- | --- |
//! | 1 | 人らしさの指標を測る |
//! | 2 | 次元ごとに、人の側と機械の側に照らして尤度比に変える |
//! | 3 | 指標ごとに、その指標の次元の対数尤度比を平均する |
//! | 4 | 指標ごとの値を合算して 1 つの人らしさ値にする |
//!
//! 3 段目で平均するのは、当てはめる重みを[指標](Metric::ALL)の数に抑えるためである。
//! 較正に使える単位は少ない。次元ごとの重みを十数点から当てはめれば、どうとでも
//! 決まってしまう——繰り返しだけで次元がいくつもあり、そこが重みを持ち去る。

use kuchiyose_metrics::humanness::Metric;
use kuchiyose_metrics::Measured;

use crate::calibrate::Weights;

/// 人らしさ値が出せない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HumannessError {
    /// 次元が欠けている。
    ///
    /// 欠けた分を抜いて合算しない。 重みはそろっている前提で較正されている。
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

/// 人らしさの較正。1 組だけできる。
#[derive(Debug, Clone, PartialEq)]
pub struct HumannessScale {
    /// 次元の名前。並び順が入力の順を決める。
    dims: Vec<String>,
    /// 次元ごとの、値 → 対数尤度比。
    per_dim: Vec<Weights>,
    /// 指標ごとの対数尤度比の平均 → 1 つの人らしさ値。
    fusion: Weights,
}

/// 向きを持つとみなす傾きの下限。その指標のいちばん大きい傾きに対する割合。
///
/// 暫定値である。 実測では、文字のエントロピーの傾きが語のエントロピーの
/// 7% しかなく、それだけでエントロピーが指示できない指標になっていた。
pub const FAINT: f64 = 0.2;

/// 分けているとみなす、対ごとの並べ替え性能（AUC）の 0.5 からの隔たり。
///
/// 暫定値である。 実測では、異なり語率の AUC が 0.541——本人 28 単位と
/// 機械 21 単位の 588 対で、機械のほうが高い対が半分をわずかに超えるだけだった。
/// 同じ素材で圧縮率は 0.768、語のエントロピーは 0.723 である。
///
/// それでも重みは語のエントロピーの約 11 倍あった。 当てはめは、次元どうしが
/// 相関していると、分けていない次元にも大きな傾きを置く。
pub const SEPARATES: f64 = 0.1;

/// その次元だけで本人と機械を分けているか。対ごとに数える。
///
/// 並べ替え性能で見る——傾きの大きさでは見られない。当てはめが置いた傾きが
/// 大きいことと、その次元が分けていることは別である。
fn separates(column: &[f64], labels: &[u8]) -> bool {
    let human: Vec<f64> = column
        .iter()
        .zip(labels)
        .filter(|(_, l)| **l == 1)
        .map(|(v, _)| *v)
        .collect();
    let machine: Vec<f64> = column
        .iter()
        .zip(labels)
        .filter(|(_, l)| **l == 0)
        .map(|(v, _)| *v)
        .collect();
    if human.is_empty() || machine.is_empty() {
        return true;
    }
    let mut win = 0.0_f64;
    for h in &human {
        for m in &machine {
            if m > h {
                win += 1.0;
            } else if (m - h).abs() < f64::EPSILON {
                win += 0.5;
            }
        }
    }
    #[allow(clippy::cast_precision_loss)]
    let auc = win / (human.len() * machine.len()) as f64;
    (auc - 0.5).abs() >= SEPARATES
}

/// 傾きを 0 にした重み。どんな入力でも 0 を返す。
///
/// 次元を[並び](dims)から外さない——並びは指標の定義から決まり、重みも値も
/// その並びで引く。効かせないことだけを傾きで表す。
fn silenced(w: &Weights) -> Weights {
    Weights::restore(
        0.0,
        vec![0.0; w.slopes().len()],
        w.centers().to_vec(),
        w.scales().to_vec(),
    )
    .expect("傾きを 0 にしただけなので組み立てられる")
}

/// 人らしさの次元の並び。指標の並びから引く。
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
    /// `human` と `machine` は 1 行が 1 単位ぶんの全次元。単位で割る——
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
        // 分けていない次元を合算に効かせない。 当てはめは、本人と機械を
        // 分けていない次元にも大きな傾きを置くことがある——次元どうしが相関して
        // いるためである。そこに乗った重みは、直し方の先頭に来て人を逆へ導く。
        //
        // 仕様は[照合と指示の側に同じ検め](crate::effective)を持っている。
        let per_dim: Vec<Weights> = per_dim
            .into_iter()
            .enumerate()
            .map(|(j, w)| {
                let column: Vec<f64> = rows
                    .iter()
                    .map(|r| r.get(j).copied().unwrap_or(0.0))
                    .collect();
                if separates(&column, &labels) {
                    w
                } else {
                    silenced(&w)
                }
            })
            .collect();
        let fused: Vec<Vec<f64>> = rows.iter().map(|r| per_metric(&per_dim, r)).collect();
        Self {
            dims,
            per_dim,
            fusion: Weights::fit(&fused, &labels),
        }
    }

    /// 合算に効かせていない次元の名前。本人と機械を分けていない次元である。
    ///
    /// 落とした理由は向きではなく、分けていないことである——
    /// [向きが定義と逆](Self::contradicting_dims)でも、分けているなら使う。
    #[must_use]
    pub fn ineffective_dims(&self) -> Vec<String> {
        self.dims
            .iter()
            .zip(&self.per_dim)
            .filter(|(_, w)| w.slopes().iter().all(|s| *s == 0.0))
            .map(|(n, _)| n.clone())
            .collect()
    }

    /// 1 本の人らしさ値。1 次元でも欠けたら出さない。
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

    /// 指標ごとの人らしさ値。どれが機械の側にあるかを名指しできる。
    ///
    /// 合算した 1 つの値では「機械の側にある」としか言えず、直し方を渡せない。
    /// 正が人の側、負が機械の側である。
    ///
    /// 欠けたら出さないのは[合算](Self::value)と同じ——一部の指標だけで
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

    /// 指標ごとの値から合算する。1 指標だけ置き換えられる。
    ///
    /// 効く量を数えるための道具である。 人らしさの直し方は 2 本以上出ることが
    /// あり、互いに正反対を指すことがある——語彙を散らせと言う指標と、
    /// 言い換えるなと言う指標が同時に出る。どちらが勝つかを言わなければ、
    /// 受け取った側は逆を選びうる。 実測でそれが起きた。
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

    /// 較正が定義と逆を学んだ次元の名前。
    ///
    /// 判定には使う。 目盛りは素材から作るものであり、素材が言ったことを
    /// 捨てれば判定が弱くなるだけである。
    ///
    /// 直し方には使えない。 定義は「どちらが機械の側か」を先行研究に基づいて
    /// 名乗っており、較正がその逆を学んだなら、その直し方に従うほど人らしさが
    /// 下がる——道具が自分の指示で自分の判定を悪くする。
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

    /// 指標ごとの、人へ寄せる向き。`true` なら値を上げる。
    ///
    /// 向きは較正から読む。 定義に固定すると、素材がその向きを支えていない
    /// 形代で直し方に従うほど人らしさが下がる。先行研究が言うのは
    /// 「普通はこちら」であって、この人とこの基準でそうなるとはかぎらない。
    ///
    /// 次元の向きが割れている指標は返さない。 どちらへ動かせばよいかを言えない
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
                // 向きを持たない次元は、向きの割れに数えない。 傾きがほぼ 0 の
                // 次元はどちらへ動かしても値を変えないので、それが 1 本あるだけで
                // 指標全体を指示できなくするのは、無い信号に判断を委ねることである。
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

    /// 合算の重みが、どの指標にも均等に開いていないか。
    ///
    /// 開いていたら較正を疑う。 語彙の狭さを見る指標（圧縮率・繰り返し・
    /// 語彙の豊富さ・エントロピー）は同じ現象を別の角度から見ているので、
    /// 較正がそのどれかに偏らせるはずである。句読点の密度は別の現象を見る。
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
    // 0 は合っていない。 どちらへ動かしても値が変わらない次元では、直し方が
    // 効いたかを次の周で確かめられない。
    if slope == 0.0 {
        return false;
    }
    (slope < 0.0) == upper
}

/// 指標ごとに、その指標の次元の対数尤度比を平均する。
///
/// 対数で平均する。 尤度比そのままで平均すると、いちばん大きい 1 次元が全体を
/// 決めてしまい、繰り返しの次元を抑えるという目的が消える。
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

    /// 人の側は繰り返しが多く、ほかは低い。並びは[次元](dims)の順である。
    fn human_row(seed: f64) -> Vec<f64> {
        let mut row = vec![0.30 + seed; 1];
        row.extend(std::iter::repeat_n(0.50 + seed, 8));
        row.push(0.30 + seed);
        row.push(8.0 + seed);
        row.push(6.0 + seed);
        row.push(20.0 + seed);
        row.push(18.0 + seed);
        row
    }

    /// 機械の側は繰り返しが足りず、ほかは高い。
    fn machine_row(seed: f64) -> Vec<f64> {
        let mut row = vec![0.45 + seed; 1];
        row.extend(std::iter::repeat_n(0.10 + seed, 8));
        row.push(0.60 + seed);
        row.push(10.0 + seed);
        row.push(7.5 + seed);
        row.push(28.0 + seed);
        row.push(26.0 + seed);
        row
    }

    fn scale() -> HumannessScale {
        let human: Vec<Vec<f64>> = (0..5).map(|i| human_row(f64::from(i) * 0.01)).collect();
        let machine: Vec<Vec<f64>> = (0..5).map(|i| machine_row(f64::from(i) * 0.01)).collect();
        HumannessScale::fit(&human, &machine)
    }

    #[test]
    fn 向きが割れている指標には直し方を渡さない() {
        // どちらへ動かせばよいかを言えない。 言えないものを指示にしない。
        // 判定には使う——素材が言ったことを捨てれば判定が弱くなるだけである。
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
        assert_eq!(bad.len(), 8, "繰り返しの次元が定義と食い違う: {bad:?}");
        assert!(bad.iter().all(|n| n.contains("gram")), "{bad:?}");
        // 定義と食い違っても、向きが揃っていれば渡せる。 素材が言う向きへ
        // 寄せればよい——繰り返しは減らす側が人になる。
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
    fn 分けない次元は合算に効かせない() {
        // 本人と機械を分けていない次元に、大きな重みが乗ることがある。
        // 実測で当たった——異なり語率は対ごとに機械が高い割合が 0.541（ほぼ偶然）
        // なのに、重みが語のエントロピーの約 11 倍あった。直し方の先頭に来て、
        // 従うと人らしさ値が下がる。
        //
        // 仕様は照合と指示の側に[効くかの判定](crate::effective)を持っている。
        // 人らしさの次元にも同じ検めが要る。
        //
        // 異なり語率（10 番目の次元）だけを、両側で同じ分布に置く。
        let row = |human: bool, i: usize| {
            #[allow(clippy::cast_precision_loss)]
            let seed = i as f64 * 0.01;
            let mut row = if human {
                human_row(seed)
            } else {
                machine_row(seed)
            };
            row[9] = 0.40 + seed;
            row
        };
        let human: Vec<Vec<f64>> = (0..5).map(|i| row(true, i)).collect();
        let machine: Vec<Vec<f64>> = (0..5).map(|i| row(false, i)).collect();
        let s = HumannessScale::fit(&human, &machine);

        assert_eq!(
            s.ineffective_dims(),
            vec!["異なり語率".to_owned()],
            "分けていない次元を名指しできていない"
        );

        // その次元を動かしても値が変わらない。 効かせているなら変わる。
        let mut a = human_row(0.0);
        let mut b = human_row(0.0);
        a[9] = 0.10;
        b[9] = 0.90;
        let (va, vb) = (
            s.value(&named(&a)).expect("測れている"),
            s.value(&named(&b)).expect("測れている"),
        );
        assert!(
            (va - vb).abs() < 1e-9,
            "分けない次元が合算を動かしている: {va} と {vb}"
        );
    }

    #[test]
    fn 分けている次元は落とさない() {
        let s = scale();
        assert!(
            s.ineffective_dims().is_empty(),
            "分けている次元を落としている: {:?}",
            s.ineffective_dims()
        );
    }

    #[test]
    fn 向きが定義どおりならどれも定義の側になる() {
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
    fn 次元は_14_である() {
        assert_eq!(dims().len(), 14);
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

    /// 実測に近い値。次元の尺度が 3 桁ちがう。
    ///
    /// 繰り返しの次元と異なり語率は 0.003〜0.4、語のエントロピーは 7 前後、
    /// 圧縮率は 6 前後である。
    fn wide_row(human: bool, i: usize) -> Vec<f64> {
        // 並びは 圧縮率 1 / 繰り返し 8 / 異なり語率 1 / エントロピー 2 / 句読点 2。
        //
        // 繰り返しと異なり語率は両側が重なる。 実測の広がりをそのまま使い、
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
        // 語のエントロピーだけが分ける。値は 7 前後で、繰り返しの 3 桁上である。
        // 人の側に 1 本だけ機械の側へ食い込む値がある——人が書いた記事が
        // 機械並みに散らないことは実際に起きる。
        //
        // 向きは定義に従う。 エントロピーは上限だけを持つ——機械の側が高く出る。
        // 逆に置けば[向きが割れている](HumannessScale::toward_human)として直し方を
        // 渡せなくなるので、標準化を試すための素材にならない。
        let ent = if human {
            [6.99, 7.02, 7.05, 7.08, 7.91][i]
        } else {
            [7.23, 7.45, 7.60, 7.80, 7.96][i]
        };
        // 句読点の密度は 2 桁の値である。 尺度がさらに 1 桁ちがう次元を混ぜても
        // 合算が支配されないことを、この素材で見る。
        let punct = if human {
            [22.0, 23.5, 24.0, 25.5, 26.0][i]
        } else {
            [27.0, 28.0, 29.0, 30.5, 31.0][i]
        };
        let mut row = vec![if human { 6.35 } else { 6.40 } + i as f64 * 0.02];
        row.extend(std::iter::repeat_n(rep, 8));
        row.push(rich);
        row.push(ent);
        row.push(ent - 2.0);
        row.push(punct);
        row.push(punct - 4.0);
        row
    }

    #[test]
    fn 尺度が_3_桁ちがっても人のほうが大きく出る() {
        // 標準化しないと、値の大きい 1 次元で符号が逆転して合算を支配する。
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
        assert_eq!(names.len(), 14, "人らしさの次元が欠けている");
    }

    #[test]
    fn 重みは指標の数だけである() {
        // 次元ごとの重みを少ない点から当てはめない。繰り返しの次元が重みを持ち去る。
        let s = scale();
        assert_eq!(s.fusion().slopes().len(), Metric::ALL.len());
        assert_eq!(s.per_dim().len(), 14);
    }

    #[test]
    fn 繰り返しの_8_次元は_2_つぶんに畳まれる() {
        // 平均する前と後で、合算の入力の数が変わる。
        // 短いと長いは別の指標なので、繰り返しの次元は 2 つの指標に分かれる。
        let s = scale();
        let row = human_row(0.0);
        assert_eq!(per_metric(s.per_dim(), &row).len(), Metric::ALL.len());
    }

    #[test]
    fn 同じ素材から同じ重みが出る() {
        assert_eq!(scale(), scale());
    }
}
