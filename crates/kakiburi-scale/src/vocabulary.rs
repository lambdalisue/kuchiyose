//! 語彙と z 得点。<strong>先に決めて固定する。</strong>
//!
//! 検めが選び直せば、比べたものに意味が無い——違う軸のベクトルどうしの距離になる。
//! だから型で `Frozen` にして、作り直す道を検め側に渡さない。

use std::collections::BTreeMap;

/// 数え上げ。識別子ごとの生の回数。
pub type Counts = BTreeMap<String, usize>;

/// 固定した語彙と z 得点。<strong>派生物として保存する。</strong>
///
/// <strong>作り方はここにしかない。</strong>[`Frozen::fit`]は全体を要求するので、検める 1 本から
/// 作り直せない。
#[derive(Debug, Clone, PartialEq)]
pub struct Frozen {
    /// 次元の並び。<strong>頻度の降順、同順位は識別子の昇順。</strong>
    dims: Vec<String>,
    /// 次元ごとの平均。
    mean: Vec<f64>,
    /// 次元ごとの標準偏差。
    sd: Vec<f64>,
}

impl Frozen {
    /// 全体から 1 度だけ作る。
    ///
    /// <strong>割る前に行う。</strong> 相手集合と測る分に分ける前に、全体から選ぶ——側ごとに違う
    /// 語彙を使えば、側ごとに次元の意味が変わる。
    ///
    /// `limit` が `None` なら次元を絞らない（文字種・品詞 bigram・語の文体値のように
    /// 次元が固定の系統）。
    #[must_use]
    pub fn fit(all: &[Counts], limit: Option<usize>) -> Self {
        let dims = pick_dims(all, limit);
        let rows: Vec<Vec<f64>> = all.iter().map(|c| relative(c, &dims)).collect();
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

    /// 保存した派生物から組み立てる。
    ///
    /// <strong>作り直しではない。</strong>[`Frozen::fit`]が全体から作ったものを読み戻す道である——
    /// 長さが揃わなければ組み立てない。
    #[must_use]
    pub fn restore(dims: Vec<String>, mean: Vec<f64>, sd: Vec<f64>) -> Option<Self> {
        if dims.len() != mean.len() || dims.len() != sd.len() {
            return None;
        }
        Some(Self { dims, mean, sd })
    }

    /// 数え上げを、固定した軸に投影する。
    ///
    /// <strong>語彙も平均も標準偏差も作り直さない。</strong> 検めが呼ぶのはこれだけである。
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

/// 部分ベクトルの束。<strong>1 系統ぶんである。</strong>
///
/// [読点の打ち方](kakiburi_metrics::matching::comma_position)のように、系統が 3 つの
/// 部分ベクトルを持つことがある。<strong>連結する前に、それぞれの中で相対頻度に直す</strong>——
/// 1 つにまとめて割れば、間隔の分布が文字の分布の分母に混ざる。
#[derive(Debug, Clone, PartialEq)]
pub struct FrozenSet {
    parts: Vec<Frozen>,
}

impl FrozenSet {
    /// 全体から 1 度だけ作る。
    ///
    /// `all` は 1 要素が 1 単位ぶんの部分ベクトルの並び。<strong>部分の数が `limits` と違う
    /// 単位は落とす</strong>——数が揃わなければ、絞る先を 1 つずれて当てる。
    #[must_use]
    pub fn fit(all: &[Vec<Counts>], limits: &[Option<usize>]) -> Self {
        let parts = limits
            .iter()
            .enumerate()
            .map(|(i, limit)| {
                let column: Vec<Counts> = all
                    .iter()
                    .filter(|u| u.len() == limits.len())
                    .map(|u| u[i].clone())
                    .collect();
                Frozen::fit(&column, *limit)
            })
            .collect();
        Self { parts }
    }

    /// 部分ごとに投影して連結する。
    ///
    /// <strong>部分の数が合わなければ投影しない。</strong> 合わないまま連結すれば、次元の意味が
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

    /// 部分から組み立てる。<strong>保存した派生物を読み戻す道である。</strong>
    #[must_use]
    pub fn from_parts(parts: Vec<Frozen>) -> Self {
        Self { parts }
    }
}

/// 次元を選ぶ。<strong>頻度の降順、同順位は識別子の昇順。</strong>
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

/// 相対頻度。<strong>分母は選ぶ前の全体である。</strong>
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

/// Cosine Delta。<strong>z 得点のうえでコサイン距離を取る。</strong>
///
/// ベクトル正規化はコサイン距離そのものに含まれる。これが効いている要素で、
/// <strong>語彙の大きさの選び方に対して頑健にする。</strong>
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
    fn 読み戻しは長さが揃わなければ組み立てない() {
        assert!(Frozen::restore(vec!["a".into()], vec![0.0], vec![1.0]).is_some());
        assert!(Frozen::restore(vec!["a".into()], vec![], vec![1.0]).is_none());
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
