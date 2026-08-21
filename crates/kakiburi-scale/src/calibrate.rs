//! 尤度比への較正と、合算。
//!
//! <strong>距離 1 つを尤度比 1 つに変える手続きと、尤度比を合算する手続きの両方に、
//! ロジスティック回帰を使う。</strong>
//!
//! <strong>正則化を入れる。</strong> 下限ちょうどの素材では同一人の対が 10 本しかない。
//! 正則化が無ければ完全に分離してしまい、尤度比が無限大に飛ぶ。

/// 正則化の強さ。<strong>暫定値である。</strong>
pub const LAMBDA: f64 = 0.1;

/// 標本の事前オッズの対数。<strong>切片から引く。</strong>
///
/// 素のロジスティック回帰が返すのは<strong>事後</strong>オッズの対数であって、尤度比ではない。
/// 同じ人の対と違う人の対は同じ数だけ作れない——[相手集合の割り](../../../docs/spec/200-extract.md#較正に使う-2-つの対集合)から
/// できるのは同一人 10 対・別人 25 対である。標本の作り方で決まったこの偏りが、
/// そのまま切片に乗っている。
///
/// <strong>クラス重みで釣り合わせる道は採らない。</strong> 正則化を掛けたまま重みを変えると傾きまで
/// 動くので、切片だけを直す手順とは別の答えになりうる。
///
/// 片側が 0 なら当てはめ自体が成り立たないので、補正しない。
fn prior_log_odds(labels: &[u8]) -> f64 {
    let positives = labels.iter().filter(|&&y| y == 1).count();
    let negatives = labels.len() - positives;
    if positives == 0 || negatives == 0 {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    ((positives as f64) / (negatives as f64)).ln()
}

/// 当てはめた重み。<strong>1 組だけできる。</strong> 検めはそれをそのまま使う。
///
/// <strong>作り方は[`Weights::fit`]だけである。</strong> 検める 1 本から作り直す道を持たない。
#[derive(Debug, Clone, PartialEq)]
pub struct Weights {
    intercept: f64,
    slopes: Vec<f64>,
    /// 次元ごとの平均。<strong>当てはめる前に引く。</strong>
    centers: Vec<f64>,
    /// 次元ごとの標準偏差。<strong>当てはめる前に割る。</strong>
    ///
    /// 広がりが 0 の次元は 1 を入れる——割れないうえ、定数の列は信号を持たない。
    scales: Vec<f64>,
}

/// 次元ごとの平均と標準偏差を取る。<strong>母標準偏差である。</strong>
fn moments(rows: &[Vec<f64>], p: usize) -> (Vec<f64>, Vec<f64>) {
    #[allow(clippy::cast_precision_loss)]
    let n = rows.len() as f64;
    let mut centers = vec![0.0; p];
    let mut scales = vec![1.0; p];
    if n == 0.0 {
        return (centers, scales);
    }
    for j in 0..p {
        let mean = rows.iter().map(|r| r[j]).sum::<f64>() / n;
        let var = rows.iter().map(|r| (r[j] - mean).powi(2)).sum::<f64>() / n;
        centers[j] = mean;
        // <strong>広がりが 0 なら 1 のままにする。</strong> 割れないし、定数の列は分けない。
        let sd = var.sqrt();
        if sd > f64::EPSILON {
            scales[j] = sd;
        }
    }
    (centers, scales)
}

impl Weights {
    /// 当てはめる。
    ///
    /// `rows` は 1 行が 1 対、`labels` は 1（同じ人）/ 0（違う人）。
    ///
    /// 仮定は<strong>対数尤度比が入力の 1 次式で書ける</strong>ことである。<strong>仮定が無いのではなく、
    /// 置く場所が違う</strong>——残差を見て確かめる。
    ///
    /// 目的関数は<strong>標本ごとの負の対数尤度の平均 + `λ` × 傾きの二乗和</strong>で、これを
    /// 最小化する。和ではなく平均にするのは `λ` の意味を標本数から切り離すため
    /// である——和にすると、素材が増えるだけで正則化が相対的に弱くなる。
    ///
    /// 当てはめたあと、<strong>切片から[標本の事前オッズ](prior_log_odds)を引く</strong>。引かなければ
    /// 返るのは事後オッズであって尤度比ではない。<strong>これは近似である</strong>——傾きを L2 で
    /// 縮めている以上、切片を直しても厳密な対数尤度比にはならない。
    ///
    /// # 当てはめる前に標準化する
    ///
    /// <strong>歩幅も罰則も、次元の尺度に依る。</strong> 標準化しなければ、次元の大きさがそのまま
    /// 当てはめの結果を決める。
    ///
    /// | 何が起きるか | |
    /// | --- | --- |
    /// | 値が大きい次元 | <strong>歩幅が安定条件を超えて収束しない。</strong> 歩幅を変えると答えが変わり、<strong>符号まで逆に出る</strong> |
    /// | 値が小さい次元 | 罰則がデータの勾配を上回り、<strong>傾きが 0 付近に潰れる</strong> |
    ///
    /// 人らしさの 12 次元は 0.003 から 7.6 まで 3 桁ちがう値をそのまま渡すので、
    /// 両方が同時に起きる——<strong>符号の逆転した 1 次元が合算を支配し、天井と床が
    /// 同じ 1 点に潰れる</strong>。
    ///
    /// <strong>平均と標準偏差は固定して持つ。</strong> 検めが取り直せば、違う軸に載った値を
    /// 同じ重みで測ることになる（[語彙](../../../docs/spec/200-extract.md#語彙は先に決めて固定する)と
    /// 同じ理由である）。
    #[must_use]
    pub fn fit(rows: &[Vec<f64>], labels: &[u8]) -> Self {
        let p = rows.first().map_or(0, Vec::len);
        #[allow(clippy::cast_precision_loss)]
        let n = rows.len() as f64;
        if n == 0.0 {
            return Self {
                intercept: 0.0,
                slopes: vec![0.0; p],
                centers: vec![0.0; p],
                scales: vec![1.0; p],
            };
        }
        let (centers, scales) = moments(rows, p);
        let z: Vec<Vec<f64>> = rows
            .iter()
            .map(|r| {
                r.iter()
                    .enumerate()
                    .map(|(j, x)| (x - centers[j]) / scales[j])
                    .collect()
            })
            .collect();

        let mut w = vec![0.0; p + 1];
        let lr = 0.5;
        for _ in 0..4000 {
            let mut g = vec![0.0; p + 1];
            for (row, &y) in z.iter().zip(labels) {
                let s = w[0] + row.iter().zip(&w[1..]).map(|(x, b)| x * b).sum::<f64>();
                let s = 1.0 / (1.0 + (-s.clamp(-30.0, 30.0)).exp());
                let e = s - f64::from(y);
                g[0] += e;
                for (j, x) in row.iter().enumerate() {
                    g[j + 1] += e * x;
                }
            }
            w[0] -= lr * g[0] / n;
            for j in 1..=p {
                // L2。切片には掛けない。
                w[j] -= lr * (g[j] / n + LAMBDA * w[j]);
            }
        }
        Self {
            intercept: w[0] - prior_log_odds(labels),
            slopes: w[1..].to_vec(),
            centers,
            scales,
        }
    }

    /// 対数尤度比を返す。<strong>大きいほど同じ人らしい。</strong>
    ///
    /// 当てはめたときと<strong>同じ平均と標準偏差</strong>で標準化してから当てる。取り直せば、
    /// 違う軸に載った値を同じ重みで測ることになる。
    #[must_use]
    pub fn log_lr(&self, row: &[f64]) -> f64 {
        self.intercept
            + row
                .iter()
                .zip(&self.slopes)
                .enumerate()
                .map(|(j, (x, b))| {
                    let c = self.centers.get(j).copied().unwrap_or(0.0);
                    let s = self.scales.get(j).copied().unwrap_or(1.0);
                    (x - c) / s * b
                })
                .sum::<f64>()
    }

    /// 固定した平均。<strong>検めはこれをそのまま使う。</strong>
    #[must_use]
    pub fn centers(&self) -> &[f64] {
        &self.centers
    }

    /// 固定した標準偏差。
    #[must_use]
    pub fn scales(&self) -> &[f64] {
        &self.scales
    }

    /// 傾き。指紋に入る。
    #[must_use]
    pub fn slopes(&self) -> &[f64] {
        &self.slopes
    }

    /// 負の傾きを 0 にする。
    ///
    /// <strong>向きが論理で決まっている当てはめに使う。</strong> 入力がすでに「大きいほど
    /// そちら側」に揃っているなら、負の重みは成り立ちようのないことを言っている。
    ///
    /// <strong>反転させない。</strong> 反転は素材が言っていないことを言うことになる。
    pub fn clamp_non_negative(&mut self) {
        for w in &mut self.slopes {
            if *w < 0.0 {
                *w = 0.0;
            }
        }
    }

    /// 切片。
    #[must_use]
    pub fn intercept(&self) -> f64 {
        self.intercept
    }

    /// 保存した派生物から組み立てる。
    ///
    /// <strong>当てはめ直しではない。</strong>[`Weights::fit`]が全体から作ったものを読み戻す道で
    /// ある——検める 1 本からは、渡す素材が無いので呼べない。
    ///
    /// <strong>平均と標準偏差も読み戻す。</strong> 落とせば、検めが標準化なしの値に標準化ずみの
    /// 重みを当てることになり、<strong>値だけが静かに変わる</strong>。
    ///
    /// <strong>だから欠けていたら組み立てない。</strong> 長さが揃わないものを受けると、
    /// [`Self::log_lr`]が足りない次元を「平均 0・標準偏差 1」で埋める——
    /// <strong>次元ごとに標準化が外れた値が、エラーにならずに出る。</strong> 仕様が名指しで
    /// 禁じている経路そのものである
    /// （[平均と標準偏差は固定して持つ](../../../docs/spec/200-extract.md#平均と標準偏差は固定して持つ)）。
    ///
    /// <strong>標準偏差 0 も断る。</strong> 割れば無限大か NaN になり、そこから先の比較が
    /// すべて壊れる。<strong>広がりが 0 の次元は 1 として持つ</strong>のが仕様なので、
    /// 保存されたものに 0 が入っていること自体が壊れている印である。
    #[must_use]
    pub fn restore(
        intercept: f64,
        slopes: Vec<f64>,
        centers: Vec<f64>,
        scales: Vec<f64>,
    ) -> Option<Self> {
        if slopes.len() != centers.len() || slopes.len() != scales.len() {
            return None;
        }
        if !intercept.is_finite() {
            return None;
        }
        if !slopes.iter().chain(&centers).all(|v| v.is_finite()) {
            return None;
        }
        if !scales.iter().all(|v| v.is_finite() && *v > 0.0) {
            return None;
        }
        Some(Self {
            intercept,
            slopes,
            centers,
            scales,
        })
    }
}

/// 系統ごとの較正と、合算の重み。
#[derive(Debug, Clone, PartialEq)]
pub struct Calibration {
    /// 系統の名前。<strong>並び順が入力の順を決める。</strong>
    systems: Vec<String>,
    /// 系統ごとの、距離 → 対数尤度比。
    per_system: Vec<Weights>,
    /// 対数尤度比の並び → 1 つの照合値。
    fusion: Weights,
}

/// 照合値が出せない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchError {
    /// 系統が欠けている。
    ///
    /// <strong>欠けた分を抜いて合算しない。</strong> 重みはそろっている前提で較正されている。
    /// 1 つ抜けば残りの重みの意味が変わり、同じ目盛りに載らない値が出る。しかも
    /// <strong>欠けた系統ほどその文書で特徴的だった可能性がある。</strong>
    MissingSystems {
        /// 欠けた系統の名前。
        names: Vec<String>,
    },
}

impl std::fmt::Display for MatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchError::MissingSystems { names } => {
                write!(f, "系統が欠けている: {}", names.join("、"))
            }
        }
    }
}

impl std::error::Error for MatchError {}

impl Calibration {
    /// 較正する。
    ///
    /// `distances` は 1 行が 1 対、列が系統の順。`labels` は 1 / 0。
    #[must_use]
    pub fn fit(systems: &[String], distances: &[Vec<f64>], labels: &[u8]) -> Self {
        let per_system: Vec<Weights> = (0..systems.len())
            .map(|j| {
                let rows: Vec<Vec<f64>> = distances.iter().map(|r| vec![r[j]]).collect();
                Weights::fit(&rows, labels)
            })
            .collect();
        // 合算は、系統ごとの対数尤度比を入力にしてもう一度当てはめる。
        let fused_rows: Vec<Vec<f64>> = distances
            .iter()
            .map(|r| {
                per_system
                    .iter()
                    .enumerate()
                    .map(|(j, w)| w.log_lr(&[r[j]]))
                    .collect()
            })
            .collect();
        // <strong>合算の重みは負にならない。</strong> 系統ごとの対数尤度比は、すでに「本人に
        // 近いほど大きい」向きに揃っている——負の重みは<strong>「本人に近いほど本人らしく
        // ない」</strong>という、成り立ちようのないことを言う。
        //
        // <strong>これは素材の発見ではなく、当てはめの揺れである。</strong> 人らしさの向きは
        // 先行研究の主張なので素材が覆しうるが、こちらは<strong>距離の定義から決まる</strong>。
        // 実測では 5 系統のうち機能語だけが負に出た。
        //
        // <strong>反転させず 0 にする。</strong> 反転は素材が言っていないことを言うことになる。
        let mut fusion = Weights::fit(&fused_rows, labels);
        fusion.clamp_non_negative();
        Self {
            systems: systems.to_vec(),
            per_system,
            fusion,
        }
    }

    /// 1 対の照合値。
    ///
    /// `distances` は系統の名前から距離を引ける形。<strong>1 つでも欠けたら出さない。</strong>
    pub fn matching_value(
        &self,
        distances: &dyn Fn(&str) -> Option<f64>,
    ) -> Result<f64, MatchError> {
        let mut missing = Vec::new();
        let mut llrs = Vec::with_capacity(self.systems.len());
        for (name, w) in self.systems.iter().zip(&self.per_system) {
            match distances(name) {
                Some(d) => llrs.push(w.log_lr(&[d])),
                None => missing.push(name.clone()),
            }
        }
        if !missing.is_empty() {
            return Err(MatchError::MissingSystems { names: missing });
        }
        Ok(self.fusion.log_lr(&llrs))
    }

    /// 系統ごとの重み。指紋に入る。
    #[must_use]
    pub fn per_system(&self) -> &[Weights] {
        &self.per_system
    }

    /// 合算の重み。
    #[must_use]
    pub fn fusion(&self) -> &Weights {
        &self.fusion
    }

    /// 系統の名前と並び。
    #[must_use]
    pub fn systems(&self) -> &[String] {
        &self.systems
    }

    /// 保存した派生物から組み立てる。
    ///
    /// <strong>系統の数と重みの数が合わなければ組み立てない。</strong> 合わないまま合算すれば、
    /// 系統を 1 つずれた重みで掛ける。
    #[must_use]
    pub fn restore(
        systems: Vec<String>,
        per_system: Vec<Weights>,
        fusion: Weights,
    ) -> Option<Self> {
        // <strong>系統が 0 本の較正は組み立てない。</strong> 数が揃っていることだけを見ると、
        // 空どうしが揃っているとして通る——そのとき照合値は入力を 1 つも要求せず、
        // <strong>合算の切片をいつも返す。</strong> 防ごうとした「定数を返す判定器」が、
        // ここだけ残る。
        if systems.is_empty() || systems.len() != per_system.len() {
            return None;
        }
        // <strong>本数だけでは足りない。</strong> 傾きが 0 本の重みは、どんな入力でも切片だけを
        // 返す——<strong>壊れた目盛りが「いつも同じ値」を出す判定器として正常に動く。</strong>
        //
        // 系統ごとは距離 1 つを受けるので 1 次元、合算は系統の数だけ受ける。
        if per_system.iter().any(|w| w.slopes().len() != 1) {
            return None;
        }
        if fusion.slopes().len() != systems.len() {
            return None;
        }
        Some(Self {
            systems,
            per_system,
            fusion,
        })
    }
}

/// 中央値。
///
/// <strong>平均にしない。</strong> たまたま似ていない 1 本が全体を引き下げる。
#[must_use]
pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("s{i}")).collect()
    }

    /// 傾きが `n` 本の重み。
    fn weights_of(n: usize) -> Weights {
        Weights::restore(0.0, vec![1.0; n], vec![0.0; n], vec![1.0; n]).expect("組める")
    }

    #[test]
    fn 傾きの本数が合わなければ組み立てない() {
        // <strong>本数だけでは足りない。</strong> 傾きが 0 本の重みは、どんな入力でも切片だけを
        // 返す——<strong>壊れた目盛りが「いつも同じ値」を出す判定器として正常に動く。</strong>
        let systems = names(3);
        // 系統ごとは距離 1 つを受けるので 1 次元、合算は系統の数だけ受ける。
        assert!(
            Calibration::restore(systems.clone(), vec![weights_of(1); 3], weights_of(3)).is_some()
        );
        assert!(
            Calibration::restore(systems.clone(), vec![weights_of(0); 3], weights_of(3)).is_none(),
            "系統ごとの傾きが空でも通っている"
        );
        assert!(
            Calibration::restore(systems.clone(), vec![weights_of(1); 3], weights_of(0)).is_none(),
            "合算の傾きが空でも通っている"
        );
        assert!(
            Calibration::restore(systems, vec![weights_of(1); 3], weights_of(2)).is_none(),
            "合算の幅が系統の数と違っても通っている"
        );
        // <strong>空どうしは「揃っている」ので、数を見るだけでは通ってしまう。</strong>
        // そのとき照合値は入力を 1 つも要求せず、合算の切片をいつも返す。
        assert!(
            Calibration::restore(vec![], vec![], weights_of(0)).is_none(),
            "系統 0 本の較正が組み立っている"
        );
    }

    /// 1 列を行の形にする。
    fn column(xs: &[f64]) -> Vec<Vec<f64>> {
        xs.iter().map(|&x| vec![x]).collect()
    }

    /// 人 5 本・機械 5 本。
    const HALF: [u8; 10] = [1, 1, 1, 1, 1, 0, 0, 0, 0, 0];

    #[test]
    fn 値が大きい次元でも符号が正しい() {
        // 語のエントロピーのような 7 前後の次元。人のほうが高い。
        // <strong>標準化しないと歩幅が安定条件を超え、符号が逆に出る</strong>——機械の側に
        // 1 本だけ食い込む値があると、そこで裏返った。
        let rows = column(&[7.23, 7.45, 7.60, 7.80, 7.96, 6.99, 7.02, 7.05, 7.08, 7.91]);
        let w = Weights::fit(&rows, &HALF);
        assert!(
            w.slopes()[0] > 0.0,
            "人のほうが高いので正が正しい: {:?}",
            w.slopes()
        );
    }

    #[test]
    fn 値が小さい次元の傾きが潰れない() {
        // 圧縮率のような 0.01 前後の次元。<strong>標準化しないと罰則がデータの勾配を
        // 上回り、傾きが 0 付近に潰れる。</strong>
        let rows = column(&[
            0.0043, 0.0060, 0.0080, 0.0110, 0.0144, 0.0170, 0.0190, 0.0210, 0.0230, 0.0257,
        ]);
        let w = Weights::fit(&rows, &HALF);
        assert!(
            w.slopes()[0] < -0.5,
            "人のほうが低いので負で、しかも潰れていない: {:?}",
            w.slopes()
        );
    }

    #[test]
    fn 尺度が_3_桁ちがっても同じ強さで効く() {
        // 同じ分離をする 2 つの次元は、同じ大きさの傾きを持つはずである。
        // <strong>標準化しなければ、大きい側だけが合算を支配する。</strong>
        let big = [7.0, 7.1, 7.2, 7.3, 7.4, 6.0, 6.1, 6.2, 6.3, 6.4];
        let small: Vec<f64> = big.iter().map(|x| x / 1000.0).collect();
        let rows: Vec<Vec<f64>> = big.iter().zip(&small).map(|(&a, &b)| vec![a, b]).collect();
        let w = Weights::fit(&rows, &HALF);
        let (a, b) = (w.slopes()[0].abs(), w.slopes()[1].abs());
        assert!(
            (a - b).abs() / a.max(b) < 1e-6,
            "同じ分離なら同じ強さで効く: {a} vs {b}"
        );
    }

    #[test]
    fn 尺度を変えても答えが変わらない() {
        // 単位を変えただけで判定が動いてはいけない。
        let xs = [7.23, 7.45, 7.60, 7.80, 7.96, 6.99, 7.02, 7.05, 7.08, 7.91];
        let a = Weights::fit(&column(&xs), &HALF);
        let scaled: Vec<f64> = xs.iter().map(|x| x * 1000.0).collect();
        let b = Weights::fit(&column(&scaled), &HALF);
        for (x, y) in xs.iter().zip(&scaled) {
            let (p, q) = (a.log_lr(&[*x]), b.log_lr(&[*y]));
            assert!((p - q).abs() < 1e-9, "{p} vs {q}");
        }
    }

    #[test]
    fn 広がりが_0_の次元は分けない() {
        // 定数の列は信号を持たない。割れもしない。
        let rows: Vec<Vec<f64>> = (0..10).map(|_| vec![3.0]).collect();
        let w = Weights::fit(&rows, &HALF);
        assert!(w.slopes()[0].abs() < 1e-9, "{:?}", w.slopes());
        assert!(w.log_lr(&[3.0]).is_finite());
    }

    #[test]
    fn 距離が小さいほど尤度比が大きくなる() {
        // 同じ人の対は距離が小さい。
        let rows: Vec<Vec<f64>> = vec![
            vec![0.1],
            vec![0.2],
            vec![0.15],
            vec![0.8],
            vec![0.9],
            vec![0.85],
        ];
        let labels = vec![1, 1, 1, 0, 0, 0];
        let w = Weights::fit(&rows, &labels);
        assert!(
            w.log_lr(&[0.1]) > w.log_lr(&[0.9]),
            "近いほうが同じ人らしい: {} vs {}",
            w.log_lr(&[0.1]),
            w.log_lr(&[0.9])
        );
    }

    #[test]
    fn 正則化が無限大を防ぐ() {
        // 完全に分離した素材でも重みが有限に留まる。
        let rows: Vec<Vec<f64>> = vec![vec![0.0], vec![0.0], vec![1.0], vec![1.0]];
        let labels = vec![1, 1, 0, 0];
        let w = Weights::fit(&rows, &labels);
        assert!(w.slopes()[0].is_finite());
        assert!(w.slopes()[0].abs() < 100.0, "{}", w.slopes()[0]);
    }

    #[test]
    fn 系統が_1_つでも欠けたら照合値を出さない() {
        // 欠けた分を抜いて合算しない。残りの重みの意味が変わる。
        let sys = names(3);
        let distances = vec![
            vec![0.1, 0.2, 0.1],
            vec![0.15, 0.25, 0.15],
            vec![0.8, 0.7, 0.9],
            vec![0.85, 0.75, 0.95],
        ];
        let c = Calibration::fit(&sys, &distances, &[1, 1, 0, 0]);
        let e = c
            .matching_value(&|n| if n == "s1" { None } else { Some(0.1) })
            .unwrap_err();
        assert!(
            matches!(&e, MatchError::MissingSystems { names } if names == &["s1".to_string()]),
            "{e:?}"
        );
    }

    #[test]
    fn 欠けた系統の名前を添える() {
        let sys = names(3);
        let distances = vec![vec![0.1, 0.2, 0.1], vec![0.8, 0.7, 0.9]];
        let c = Calibration::fit(&sys, &distances, &[1, 0]);
        let e = c.matching_value(&|n| if n == "s0" { Some(0.1) } else { None });
        let MatchError::MissingSystems { names } = e.unwrap_err();
        assert_eq!(names, vec!["s1".to_string(), "s2".to_string()]);
    }

    #[test]
    fn そろえば照合値が出る() {
        let sys = names(2);
        let distances = vec![
            vec![0.1, 0.1],
            vec![0.15, 0.12],
            vec![0.8, 0.9],
            vec![0.85, 0.88],
        ];
        let c = Calibration::fit(&sys, &distances, &[1, 1, 0, 0]);
        let near = c.matching_value(&|_| Some(0.1)).unwrap();
        let far = c.matching_value(&|_| Some(0.9)).unwrap();
        assert!(near > far, "近いほうが大きい: {near} vs {far}");
    }

    #[test]
    fn 中央値は外れた_1_本に動かされない() {
        // 平均だと引き下げられる。
        let with_outlier = [1.0, 1.0, 1.0, 1.0, -100.0];
        assert_eq!(median(&with_outlier), Some(1.0));
    }

    #[test]
    fn 中央値は偶数個でも出る() {
        assert_eq!(median(&[1.0, 3.0]), Some(2.0));
        assert_eq!(median(&[]), None);
    }

    #[test]
    fn 重みは_1_組だけできる() {
        // 同じ素材から 2 度当てはめても同じ重みが出る。検めはそれを使う。
        let sys = names(2);
        let d = vec![vec![0.1, 0.1], vec![0.8, 0.9]];
        let a = Calibration::fit(&sys, &d, &[1, 0]);
        let b = Calibration::fit(&sys, &d, &[1, 0]);
        assert_eq!(a, b);
    }
}
