//! 語彙と z 得点。先に決めて固定する。
//!
//! 検めが選び直せば、比べたものに意味が無い——違う軸のベクトルどうしの距離になる。
//! だから型で `Frozen` にして、作り直す道を検め側に渡さない。

use std::collections::BTreeMap;

/// 数え上げ。識別子ごとの生の回数。
pub type Counts = BTreeMap<String, usize>;

/// 固定した語彙と z 得点。派生物として保存する。
///
/// 作り方はここにしかない。[`Frozen::fit`]は全体を要求するので、検める 1 本から
/// 作り直せない。
#[derive(Debug, Clone, PartialEq)]
pub struct Frozen {
    /// 次元の並び。頻度の降順、同順位は識別子の昇順。
    dims: Vec<String>,
    /// 次元ごとの平均。
    mean: Vec<f64>,
    /// 次元ごとの標準偏差。
    sd: Vec<f64>,
}

impl Frozen {
    /// 全体から 1 度だけ作る。
    ///
    /// 割る前に行う。 相手集合と測る分に分ける前に、全体から選ぶ——側ごとに違う
    /// 語彙を使えば、側ごとに次元の意味が変わる。
    ///
    /// `limit` が `None` なら次元を絞らない（文字種・品詞 bigram・語の文体値のように
    /// 次元が固定の系統）。
    #[must_use]
    pub fn fit(all: &[Counts], limit: Option<usize>) -> Self {
        Self::fit_against(all, all, limit)
    }

    /// 次元は `all` から選び、平均と標準偏差は `reference` から取る。
    ///
    /// 本人を含む素材で標準化してはいけない。 本人の単位が過半を占めるなら、
    /// 平均は本人のところに来る——本人の記事は原点に置かれ、z 得点に残るのは
    /// 1 本ごとの雑音だけになる。雑音どうしの向きは揃わないので、
    /// 同じ人が書いた 2 本がほぼ直交する。
    ///
    /// 実測で、本人どうしの機能語の距離は 0.997 だった——コサインにして 0.003 である。
    /// 次元を 300 から 20 まで削っても 0.90 までしか下がらなかった。
    ///
    /// 基準を物差しにする。 そうすれば本人の単位はどれも「ふつうの書きぶりから
    /// どちらへ外れているか」を表し、その向きが揃う。
    pub fn fit_against(all: &[Counts], reference: &[Counts], limit: Option<usize>) -> Self {
        let dims = pick_dims(all, limit);
        let rows: Vec<Vec<f64>> = reference.iter().map(|c| relative(c, &dims)).collect();
        let d = dims.len();
        let mut mean = vec![0.0; d];
        let mut sd = vec![0.0; d];
        #[allow(clippy::cast_precision_loss)]
        let n = rows.len() as f64;
        if n > 0.0 {
            for j in 0..d {
                let m = rows.iter().map(|r| r[j]).sum::<f64>() / n;
                mean[j] = m;
                sd[j] = (rows.iter().map(|r| (r[j] - m).powi(2)).sum::<f64>() / n).sqrt();
            }
        }
        floor_sd(&mut sd);
        Self { dims, mean, sd }
    }

    /// 次元の数。
    #[must_use]
    pub fn len(&self) -> usize {
        self.dims.len()
    }

    /// 空か。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.dims.is_empty()
    }

    /// 次元の並び。指紋に入る。
    #[must_use]
    pub fn dims(&self) -> &[String] {
        &self.dims
    }

    /// 次元ごとの平均。指紋に入る。
    #[must_use]
    pub fn mean(&self) -> &[f64] {
        &self.mean
    }

    /// 次元ごとの標準偏差。指紋に入る。
    #[must_use]
    pub fn sd(&self) -> &[f64] {
        &self.sd
    }

    /// 数え上げを、固定した軸に投影する。
    ///
    /// 語彙も平均も標準偏差も作り直さない。 検めが呼ぶのはこれだけである。
    #[must_use]
    pub fn project(&self, counts: &Counts) -> Vec<f64> {
        let raw = relative(counts, &self.dims);
        (0..self.dims.len())
            .map(|j| {
                if self.sd[j] > 0.0 {
                    (raw[j] - self.mean[j]) / self.sd[j]
                } else {
                    0.0
                }
            })
            .collect()
    }
}

impl Frozen {
    /// その次元の相対頻度が `want` になるよう、数え上げを動かす。
    ///
    /// 1 次元だけを動かす直しは、実際には書けない。 系統の次元はどれも
    /// 相対頻度なので、1 つを減らせば残りの割合が上がる——読点を 1 つ外せば、
    /// 外さなかった読点の取り分が増える。
    ///
    /// だから数え上げの側で動かす。 投影し直せば、正規化が自然に起きて
    /// ほかの次元も動く。それが実際に起きることである。
    #[must_use]
    pub fn shifted(&self, counts: &Counts, dim: usize, want: f64) -> Counts {
        let Some(name) = self.dims.get(dim) else {
            return counts.clone();
        };
        #[allow(clippy::cast_precision_loss)]
        let total: f64 = counts.values().sum::<usize>() as f64;
        #[allow(clippy::cast_precision_loss)]
        let here: f64 = counts.get(name).copied().unwrap_or(0) as f64;
        let rest = total - here;
        // 割合を戻して数え上げを解く。 c' = want * rest / (1 - want)
        let want = want.clamp(0.0, 0.99);
        if rest <= 0.0 {
            return counts.clone();
        }
        let target = want * rest / (1.0 - want);
        let mut out = counts.clone();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        out.insert(name.clone(), target.round().max(0.0) as usize);
        out
    }

    /// 標準化した値を、相対頻度に戻す。
    #[must_use]
    pub fn unstandardize(&self, dim: usize, z: f64) -> f64 {
        match (self.mean.get(dim), self.sd.get(dim)) {
            (Some(m), Some(sd)) if *sd > 0.0 => z * sd + m,
            (Some(m), _) => *m,
            _ => 0.0,
        }
    }
}

/// 部分ベクトルの束。1 系統ぶんである。
///
/// [読点の打ち方](kakiburi_metrics::matching::comma_position)のように、系統が 3 つの
/// 部分ベクトルを持つことがある。連結する前に、それぞれの中で相対頻度に直す——
/// 1 つにまとめて割れば、間隔の分布が文字の分布の分母に混ざる。
#[derive(Debug, Clone, PartialEq)]
pub struct FrozenSet {
    parts: Vec<Frozen>,
}

impl FrozenSet {
    /// 全体から 1 度だけ作る。
    ///
    /// `all` は 1 要素が 1 単位ぶんの部分ベクトルの並び。部分の数が `limits` と違う
    /// 単位は落とす——数が揃わなければ、絞る先を 1 つずれて当てる。
    #[must_use]
    pub fn fit(all: &[Vec<Counts>], limits: &[Option<usize>]) -> Self {
        Self::fit_against(all, all, limits)
    }

    /// 次元は `all` から、平均と標準偏差は `reference` から
    /// （[部分ベクトル](Frozen::fit_against)）。
    #[must_use]
    pub fn fit_against(
        all: &[Vec<Counts>],
        reference: &[Vec<Counts>],
        limits: &[Option<usize>],
    ) -> Self {
        let parts = limits
            .iter()
            .enumerate()
            .map(|(i, limit)| {
                let column = |src: &[Vec<Counts>]| -> Vec<Counts> {
                    src.iter()
                        .filter(|u| u.len() == limits.len())
                        .map(|u| u[i].clone())
                        .collect()
                };
                Frozen::fit_against(&column(all), &column(reference), *limit)
            })
            .collect();
        Self { parts }
    }

    /// 部分ごとに投影して連結する。
    ///
    /// 部分の数が合わなければ投影しない。 合わないまま連結すれば、次元の意味が
    /// ずれたベクトルが出る。
    #[must_use]
    pub fn project(&self, parts: &[Counts]) -> Option<Vec<f64>> {
        if parts.len() != self.parts.len() {
            return None;
        }
        Some(
            self.parts
                .iter()
                .zip(parts)
                .flat_map(|(f, c)| f.project(c))
                .collect(),
        )
    }

    /// 連結したあとの次元の数。
    #[must_use]
    pub fn len(&self) -> usize {
        self.parts.iter().map(Frozen::len).sum()
    }

    /// 空か。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 部分ごとの語彙。指紋に入る。
    #[must_use]
    pub fn parts(&self) -> &[Frozen] {
        &self.parts
    }
}

/// 次元を選ぶ。頻度の降順、同順位は識別子の昇順。
fn pick_dims(all: &[Counts], limit: Option<usize>) -> Vec<String> {
    let mut total: BTreeMap<&str, usize> = BTreeMap::new();
    for c in all {
        for (k, v) in c {
            *total.entry(k.as_str()).or_default() += v;
        }
    }
    let mut keys: Vec<(&str, usize)> = total.into_iter().collect();
    // 同順位が N 番目に跨るときは識別子の昇順で先に来たものを採る。
    keys.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let take = limit.unwrap_or(keys.len()).min(keys.len());
    keys[..take].iter().map(|(k, _)| (*k).to_owned()).collect()
}

/// 歩幅の下限。その系統の平均の歩幅に対する割合。 暫定値である。
///
/// まれな次元は歩幅が 0 に近い。 そのまま割れば z 得点が爆発し、
/// 1 つの次元がベクトル全体を支配する——実測で、文字種の「半角数字」が
/// 246 まで飛び、いちばん外れている次元がどの文書でもそれになった。
pub const MIN_SD_RATIO: f64 = 0.1;

/// 歩幅に下限を敷く。
///
/// 0 の次元は 0 のまま残す。 全部の文書で同じ値なら、そこに情報は無い
/// ——投影の側が 0 を返す。
fn floor_sd(sd: &mut [f64]) {
    let live: Vec<f64> = sd.iter().copied().filter(|x| *x > 0.0).collect();
    if live.is_empty() {
        return;
    }
    #[allow(clippy::cast_precision_loss)]
    let floor = live.iter().sum::<f64>() / live.len() as f64 * MIN_SD_RATIO;
    for x in sd.iter_mut() {
        if *x > 0.0 && *x < floor {
            *x = floor;
        }
    }
}

/// 相対頻度。分母は選ぶ前の全体である。
///
/// 上位 N の中で割り直すと、N をいくつにするかが値そのものを動かす。
/// 割り直さなければ、N は「どこまで見るか」だけの話になる。
fn relative(counts: &Counts, dims: &[String]) -> Vec<f64> {
    let total: usize = counts.values().sum();
    #[allow(clippy::cast_precision_loss)]
    let denom = if total == 0 { 1.0 } else { total as f64 };
    dims.iter()
        .map(|d| {
            #[allow(clippy::cast_precision_loss)]
            let n = counts.get(d).copied().unwrap_or(0) as f64;
            n / denom
        })
        .collect()
}

/// Cosine Delta。z 得点のうえでコサイン距離を取る。
///
/// ベクトル正規化はコサイン距離そのものに含まれる。これが効いている要素で、
/// 語彙の大きさの選び方に対して頑健にする。
#[must_use]
pub fn cosine_delta(a: &[f64], b: &[f64]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return 1.0;
    }
    1.0 - dot / (na * nb)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(pairs: &[(&str, usize)]) -> Counts {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
    }

    #[test]
    fn 次元は頻度の降順で並ぶ() {
        let all = vec![counts(&[("a", 1), ("b", 5), ("c", 3)])];
        let f = Frozen::fit(&all, None);
        assert_eq!(f.dims(), ["b", "c", "a"]);
    }

    #[test]
    fn 同順位は識別子の昇順になる() {
        // 決めておかないと、作り直しても同じものが出るが成り立たない。
        let all = vec![counts(&[("z", 2), ("a", 2), ("m", 2)])];
        let f = Frozen::fit(&all, None);
        assert_eq!(f.dims(), ["a", "m", "z"]);
    }

    #[test]
    fn 同順位が_n_番目に跨っても本数は動かない() {
        let all = vec![counts(&[("a", 3), ("b", 2), ("c", 2), ("d", 2)])];
        let f = Frozen::fit(&all, Some(2));
        assert_eq!(f.dims(), ["a", "b"], "識別子の昇順で先に来たものを採る");
        assert_eq!(f.len(), 2, "何番目までかは動かさない");
    }

    #[test]
    fn 分母は選ぶ前の全体である() {
        // 上位 N の中で割り直すと、N が値そのものを動かす。
        let c = counts(&[("a", 5), ("b", 3), ("c", 2)]);
        let wide = relative(&c, &["a".into(), "b".into(), "c".into()]);
        let narrow = relative(&c, &["a".into()]);
        assert!((wide[0] - 0.5).abs() < 1e-9, "{wide:?}");
        assert!(
            (narrow[0] - 0.5).abs() < 1e-9,
            "N を絞っても a の値は変わらない: {narrow:?}"
        );
    }

    #[test]
    fn 投影は語彙を作り直さない() {
        let all = vec![counts(&[("a", 5), ("b", 1)]), counts(&[("a", 1), ("b", 5)])];
        let f = Frozen::fit(&all, None);
        // 語彙に無い識別子を持つ文書を投影しても、次元は増えない。
        let v = f.project(&counts(&[("a", 1), ("z", 100)]));
        assert_eq!(v.len(), f.len());
        assert_eq!(f.dims(), ["a", "b"], "選び直していない");
    }

    #[test]
    fn 標準偏差が_0_の次元は_0_になる() {
        let all = vec![counts(&[("a", 1)]), counts(&[("a", 1)])];
        let f = Frozen::fit(&all, None);
        assert_eq!(f.project(&counts(&[("a", 1)])), vec![0.0]);
    }

    #[test]
    fn 部分ごとに割る() {
        // 1 つにまとめて割れば、間隔の分布が文字の分布の分母に混ざる。
        let a = vec![vec![counts(&[("あ", 3)]), counts(&[("1字", 1)])]];
        let f = FrozenSet::fit(&a, &[None, None]);
        assert_eq!(f.len(), 2);
        // どちらの部分も、その部分の中で 1.0 になる。
        let raw = relative(&counts(&[("あ", 3)]), &["あ".into()]);
        assert!((raw[0] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn 部分の数が合わなければ投影しない() {
        // 合わないまま連結すれば、次元の意味がずれたベクトルが出る。
        let a = vec![vec![counts(&[("あ", 1)]), counts(&[("い", 1)])]];
        let f = FrozenSet::fit(&a, &[None, None]);
        assert!(f.project(&[counts(&[("あ", 1)])]).is_none());
        assert!(f
            .project(&[counts(&[("あ", 1)]), counts(&[("い", 1)])])
            .is_some());
    }

    #[test]
    fn 部分の数が違う単位は語彙から落とす() {
        let all = vec![
            vec![counts(&[("あ", 1)]), counts(&[("い", 1)])],
            vec![counts(&[("う", 9)])], // 部分の数が違う
        ];
        let f = FrozenSet::fit(&all, &[None, None]);
        assert_eq!(f.parts()[0].dims(), ["あ"], "落とした単位の語は入らない");
    }

    #[test]
    fn コサイン距離は尺度に依らない() {
        let a = vec![1.0, 2.0, 3.0];
        let scaled = vec![10.0, 20.0, 30.0];
        assert!(cosine_delta(&a, &scaled).abs() < 1e-12);
    }

    #[test]
    fn 同じ向きなら距離は_0_である() {
        let a = vec![1.0, -1.0];
        assert!(cosine_delta(&a, &a).abs() < 1e-12);
    }

    #[test]
    fn 逆向きなら距離は_2_である() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        assert!((cosine_delta(&a, &b) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn 長さ_0_のベクトルは最大の距離にする() {
        assert!((cosine_delta(&[0.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn 語彙の大きさを変えても順位が保たれる() {
        // ベクトル正規化が効いていることの確かめ。暫定の N がそのまま結果を
        // 左右してはいけない。
        let all = vec![
            counts(&[("a", 10), ("b", 5), ("c", 1), ("d", 1)]),
            counts(&[("a", 9), ("b", 6), ("c", 1), ("d", 1)]),
            counts(&[("a", 1), ("b", 1), ("c", 10), ("d", 5)]),
        ];
        for n in [2usize, 3, 4] {
            let f = Frozen::fit(&all, Some(n));
            let v: Vec<Vec<f64>> = all.iter().map(|c| f.project(c)).collect();
            let near = cosine_delta(&v[0], &v[1]);
            let far = cosine_delta(&v[0], &v[2]);
            assert!(near < far, "N = {n} で順位が崩れた: {near} vs {far}");
        }
    }
}
