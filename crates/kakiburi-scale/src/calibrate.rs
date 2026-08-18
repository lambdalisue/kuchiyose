//! 尤度比への較正と、合算。
//!
//! <strong>距離 1 つを尤度比 1 つに変える手続きと、尤度比を合算する手続きの両方に、
//! ロジスティック回帰を使う。</strong>
//!
//! <strong>正則化を入れる。</strong> 下限ちょうどの素材では同一人の対が 10 本しかない。
//! 正則化が無ければ完全に分離してしまい、尤度比が無限大に飛ぶ。

/// 正則化の強さ。<strong>暫定値である。</strong>
pub const LAMBDA: f64 = 0.1;

/// 当てはめた重み。<strong>1 組だけできる。</strong> 検めはそれをそのまま使う。
///
/// <strong>作り方は[`Weights::fit`]だけである。</strong> 検める 1 本から作り直す道を持たない。
#[derive(Debug, Clone, PartialEq)]
pub struct Weights {
    intercept: f64,
    slopes: Vec<f64>,
}

impl Weights {
    /// 当てはめる。
    ///
    /// `rows` は 1 行が 1 対、`labels` は 1（同じ人）/ 0（違う人）。
    ///
    /// 仮定は<strong>対数尤度比が入力の 1 次式で書ける</strong>ことである。<strong>仮定が無いのではなく、
    /// 置く場所が違う</strong>——残差を見て確かめる。
    #[must_use]
    pub fn fit(rows: &[Vec<f64>], labels: &[u8]) -> Self {
        let p = rows.first().map_or(0, Vec::len);
        let mut w = vec![0.0; p + 1];
        #[allow(clippy::cast_precision_loss)]
        let n = rows.len() as f64;
        if n == 0.0 {
            return Self {
                intercept: 0.0,
                slopes: vec![0.0; p],
            };
        }
        let lr = 0.5;
        for _ in 0..4000 {
            let mut g = vec![0.0; p + 1];
            for (row, &y) in rows.iter().zip(labels) {
                let z = w[0] + row.iter().zip(&w[1..]).map(|(x, b)| x * b).sum::<f64>();
                let s = 1.0 / (1.0 + (-z.clamp(-30.0, 30.0)).exp());
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
            intercept: w[0],
            slopes: w[1..].to_vec(),
        }
    }

    /// 対数尤度比を返す。<strong>大きいほど同じ人らしい。</strong>
    #[must_use]
    pub fn log_lr(&self, row: &[f64]) -> f64 {
        self.intercept
            + row
                .iter()
                .zip(&self.slopes)
                .map(|(x, b)| x * b)
                .sum::<f64>()
    }

    /// 傾き。指紋に入る。
    #[must_use]
    pub fn slopes(&self) -> &[f64] {
        &self.slopes
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
    #[must_use]
    pub fn restore(intercept: f64, slopes: Vec<f64>) -> Self {
        Self { intercept, slopes }
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
        Self {
            systems: systems.to_vec(),
            per_system,
            fusion: Weights::fit(&fused_rows, labels),
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
        if systems.len() != per_system.len() {
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
