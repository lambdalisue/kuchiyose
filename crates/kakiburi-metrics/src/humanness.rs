//! 人らしさの 4 指標。<strong>その人らしさではない。</strong>
//!
//! 人が書いたものに見えるかを測る（[通るための 2 つ目の条件](../../../docs/spec/010-strategy.md#通るには-2-つ要る)）。
//!
//! <strong>4 つを独立した 4 つの証拠として数えない。</strong> 圧縮率・繰り返し・語彙の豊富さ・
//! エントロピーは<strong>同じ現象を別の角度から見ている</strong>——寄せる向きも同じである。
//!
//! <strong>語は表層形で数える。</strong> 語彙素に畳むと、活用の使い分けが消えて値が下がる
//! （[数え方](../../../docs/spec/100-metrics.md#語を数えるときは表層形である)）。

use std::collections::BTreeMap;
use std::io::Write;

use kakiburi_doc::prose::Segment;

use crate::morph::Analyzed;
use crate::{floor, Measured};

/// 圧縮の水準。<strong>固定する。</strong> 変えれば値が変わり、過去の値と比べられなくなる。
pub const COMPRESSION_LEVEL: u32 = 6;

/// 圧縮器の名前と版。<strong>指紋に入る。</strong>
///
/// <strong>「zlib」だけでは値が決まらない。</strong> zlib 形式（RFC 1950）が決めているのは容器で
/// あって、同じ水準 6 でも実装ごとに符号化の選び方が違う。<strong>だから実装と版まで名乗る。</strong>
///
/// <strong>版は丸めない。</strong> `0.8` と書くと `0.8.9` と `0.8.10` が同じ指紋になり、符号化が
/// 変わっても[圧縮率](../../../docs/spec/metrics/圧縮率.md)の古い値が使い回される。
pub const COMPRESSOR: (&str, &str) = ("miniz_oxide (flate2, zlib 形式 水準 6)", "0.8.9");

/// 繰り返しで見る n の並び。
pub const REPETITION_N: [usize; 4] = [2, 3, 4, 5];

/// 語彙の豊富さを測る窓の大きさ。
pub const WINDOW: usize = 1000;

/// 人らしさの指標。<strong>4 つである。</strong>
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Metric {
    /// 圧縮率。1 次元。
    Compression,
    /// 繰り返し。8 次元。
    Repetition,
    /// 語彙の豊富さ。1 次元。
    Richness,
    /// エントロピー。2 次元。
    Entropy,
}

impl Metric {
    /// 全部。<strong>合算の入力の並びはこの順である。</strong>
    pub const ALL: [Metric; 4] = [
        Metric::Compression,
        Metric::Repetition,
        Metric::Richness,
        Metric::Entropy,
    ];

    /// 指標の名前。定義ファイルの 1 行目と同じ。
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Metric::Compression => "圧縮率",
            Metric::Repetition => "繰り返し",
            Metric::Richness => "語彙の豊富さ",
            Metric::Entropy => "エントロピー",
        }
    }

    /// この指標の次元の名前。
    #[must_use]
    pub fn dims(self) -> Vec<String> {
        match self {
            Metric::Compression => vec!["圧縮率".into()],
            Metric::Repetition => REPETITION_N
                .iter()
                .flat_map(|n| [format!("{n}gram の再来率"), format!("{n}gram の最多率")])
                .collect(),
            Metric::Richness => vec!["異なり語率".into()],
            Metric::Entropy => vec!["語のエントロピー".into(), "文字のエントロピー".into()],
        }
    }

    /// 寄せる向き。<strong>4 つとも片側だけを持つ。</strong>
    ///
    /// 繰り返しは<strong>下限</strong>——生成文は繰り返しが足りない側に出る。ほかの 3 つは上限で、
    /// 機械の側が高く出る。
    #[must_use]
    pub fn upper_bound(self) -> bool {
        !matches!(self, Metric::Repetition)
    }
}

/// 12 次元の全体。<strong>指標ごとにまとめたまま持つ</strong>——3 段目で指標ごとに平均するからである。
#[derive(Debug, Clone, PartialEq)]
pub struct Humanness {
    values: Vec<(Metric, Vec<(String, Measured)>)>,
}

impl Humanness {
    /// 測る。
    ///
    /// `analyzed` が `None` なら、形態素を要る 3 指標は「測っていない」になる——
    /// <strong>0 を返さない。</strong> 0 は「測って 0 だった」という値である。
    #[must_use]
    pub fn measure(prose: &[Segment], analyzed: Option<&Analyzed>) -> Self {
        let mut values = Vec::with_capacity(Metric::ALL.len());
        for m in Metric::ALL {
            let got = match m {
                Metric::Compression => vec![compression_ratio(prose)],
                Metric::Repetition => repetition(analyzed),
                Metric::Richness => vec![richness(analyzed)],
                Metric::Entropy => entropy(prose, analyzed),
            };
            let named = m.dims().into_iter().zip(got).collect();
            values.push((m, named));
        }
        Self { values }
    }

    /// 指標ごとの次元。
    #[must_use]
    pub fn per_metric(&self) -> &[(Metric, Vec<(String, Measured)>)] {
        &self.values
    }

    /// 12 次元を並びで返す。
    #[must_use]
    pub fn flat(&self) -> Vec<(String, Measured)> {
        self.values
            .iter()
            .flat_map(|(_, v)| v.iter().cloned())
            .collect()
    }

    /// <strong>4 指標が全部測れたか。</strong>
    ///
    /// 1 つでも欠ければ人らしさ値を出さない——[欠けた分を抜いて合算しない](crate::morph)。
    #[must_use]
    pub fn all_measured(&self) -> bool {
        self.values
            .iter()
            .all(|(_, v)| v.iter().all(|(_, m)| m.is_measured()))
    }

    /// 測れていない次元の名前。
    #[must_use]
    pub fn missing(&self) -> Vec<String> {
        self.values
            .iter()
            .flat_map(|(_, v)| v.iter())
            .filter(|(_, m)| !m.is_measured())
            .map(|(n, _)| n.clone())
            .collect()
    }
}

/// 地の文を <strong>node の順に改行 1 つで繋ぐ。</strong>
///
/// <strong>繋ぎ方が定義の一部である。</strong> node ごとに圧縮して足すのと、繋いでから圧縮するのと
/// では値が大きく変わる——繋げば node をまたぐ反復が拾える。<strong>拾いたいのはそちらである。</strong>
#[must_use]
pub fn joined(prose: &[Segment]) -> String {
    prose
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// 圧縮率。<strong>可逆圧縮したあとのバイト数 ÷ 元のバイト数。</strong>
///
/// 形態素解析を要らない、数少ない指標である。
#[must_use]
pub fn compression_ratio(prose: &[Segment]) -> Measured {
    let text = joined(prose);
    let raw = text.as_bytes();
    // 地の文が短いと、圧縮器のヘッダが結果を支配する。
    if raw.len() < floor::PROSE_BYTES {
        return Measured::BelowFloor;
    }
    let mut z =
        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(COMPRESSION_LEVEL));
    // <strong>圧縮器が返さないのは環境の壊れである。</strong> 素材が短いのと混ぜない——混ぜれば、
    // 壊れた道具が「素材が足りない」という顔で回り続ける。
    if z.write_all(raw).is_err() {
        return Measured::ToolFailed;
    }
    let Ok(out) = z.finish() else {
        return Measured::ToolFailed;
    };
    #[allow(clippy::cast_precision_loss)]
    Measured::Value(out.len() as f64 / raw.len() as f64)
}

/// 繰り返し。<strong>n ごとに 2 つ、合わせて 8 次元。</strong>
///
/// | | 何を出すか |
/// | --- | --- |
/// | 再来率 | 2 回以上現れた n-gram の数 ÷ <strong>異なり n-gram の数</strong> |
/// | 最多率 | 最も多く現れた n-gram の出現回数 ÷ <strong>延べ語数</strong> |
///
/// <strong>n-gram は node を跨がない。</strong> 跨げば、構造が作った隣接を繰り返しとして数える。
///
/// <strong>回数は延べ語数で割る。</strong> 割らなければ、長い文書ほど大きく出る。
#[must_use]
pub fn repetition(analyzed: Option<&Analyzed>) -> Vec<Measured> {
    let n_dims = REPETITION_N.len() * 2;
    // <strong>解析器が無いのと、語が足りないのを分ける。</strong> 前者は環境の壊れで、素材を
    // いくら足しても直らない。
    let Some(a) = analyzed else {
        return vec![Measured::ToolMissing; n_dims];
    };
    if !a.enough_tokens() {
        return vec![Measured::BelowFloor; n_dims];
    }
    #[allow(clippy::cast_precision_loss)]
    let tokens = a.tokens() as f64;
    let mut out = Vec::with_capacity(n_dims);
    for n in REPETITION_N {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for seg in a.segments() {
            let words: Vec<&str> = seg.iter().map(|m| m.surface.as_str()).collect();
            for w in words.windows(n) {
                *counts.entry(w.join("\u{1F}")).or_default() += 1;
            }
        }
        if counts.is_empty() {
            // <strong>n-gram が 1 つも取れない。分母が 0 である。</strong> node がすべて n 語未満なら
            // 素材を足しても同じことが起きる——下限未満とは別の理由である。
            out.push(Measured::NoDenominator);
            out.push(Measured::NoDenominator);
            continue;
        }
        let again = counts.values().filter(|&&c| c >= 2).count();
        #[allow(clippy::cast_precision_loss)]
        out.push(Measured::Value(again as f64 / counts.len() as f64));
        let top = counts.values().copied().max().unwrap_or(0);
        #[allow(clippy::cast_precision_loss)]
        out.push(Measured::Value(top as f64 / tokens));
    }
    out
}

/// 語彙の豊富さ。<strong>固定長の窓で測り、窓ごとの値の平均を取る。</strong>
///
/// <strong>正規化しないと、測っているのは長さである</strong>——Type-Token 比は文書が長いほど下がる。
///
/// <strong>node を跨いで 1 つの列にする。</strong> 跨がなければ、記事 1 本でも窓が 1 つも取れない。
/// <strong>窓は重ねず、順に切る。</strong> 端の 1,000 語に満たない分は捨てる。
#[must_use]
pub fn richness(analyzed: Option<&Analyzed>) -> Measured {
    let Some(a) = analyzed else {
        return Measured::ToolMissing;
    };
    if !a.enough_tokens() {
        return Measured::BelowFloor;
    }
    // 約物と記号の形態素も含める——外すと、読点の多い書き手ほど窓が長くなる。
    let words: Vec<&str> = a.all().map(|m| m.surface.as_str()).collect();
    let mut ratios = Vec::new();
    for w in words.chunks_exact(WINDOW) {
        let types: std::collections::BTreeSet<&str> = w.iter().copied().collect();
        #[allow(clippy::cast_precision_loss)]
        ratios.push(types.len() as f64 / w.len() as f64);
    }
    if ratios.is_empty() {
        // 窓が 1 つも取れない。<strong>平均を取る分母が 0 である。</strong>
        return Measured::NoDenominator;
    }
    #[allow(clippy::cast_precision_loss)]
    Measured::Value(ratios.iter().sum::<f64>() / ratios.len() as f64)
}

/// エントロピー。<strong>語と文字の 2 次元。</strong>
///
/// <strong>対数の底は 2 とする。</strong> 単位がビットになり、圧縮率と同じ向きで読める。
///
/// <strong>node を跨いで 1 つの分布にまとめる</strong>——分布であって並びではないので、隣接の
/// 産物が入りこまない。
///
/// <strong>文字の側も延べ語数の下限で外す。</strong> 除外は指標に掛かるものであって、次元ごとに
/// 違う下限を持たない。
#[must_use]
pub fn entropy(prose: &[Segment], analyzed: Option<&Analyzed>) -> Vec<Measured> {
    let Some(a) = analyzed else {
        return vec![Measured::ToolMissing; 2];
    };
    if !a.enough_tokens() {
        return vec![Measured::BelowFloor; 2];
    }
    let words = shannon(a.all().map(|m| m.surface.as_str()));
    // 文字は日本語の文字に限らない全文字である。
    let chars = shannon(prose.iter().flat_map(|s| s.text.chars()).map(CharKey));
    vec![Measured::Value(words), Measured::Value(chars)]
}

/// 文字を鍵にする。`char` のままでは`shannon`の型が合わない。
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct CharKey(char);

/// シャノンエントロピー。<strong>底は 2。</strong>
fn shannon<K: Ord>(items: impl Iterator<Item = K>) -> f64 {
    let mut counts: BTreeMap<K, usize> = BTreeMap::new();
    let mut total = 0usize;
    for k in items {
        *counts.entry(k).or_default() += 1;
        total += 1;
    }
    if total == 0 {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let n = total as f64;
    -counts
        .values()
        .map(|&c| {
            #[allow(clippy::cast_precision_loss)]
            let p = c as f64 / n;
            p * p.log2()
        })
        .sum::<f64>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::morph::stub::Stub;
    use kakiburi_doc::node::Kind;

    fn seg(t: &str) -> Segment {
        Segment {
            kind: Kind::Paragraph,
            text: t.to_owned(),
        }
    }

    /// 圧縮率の下限を越える地の文。
    fn long(unit: &str) -> Vec<Segment> {
        let mut v = Vec::new();
        while v.iter().map(|s: &Segment| s.text.len()).sum::<usize>() < floor::PROSE_BYTES * 2 {
            v.push(seg(unit));
        }
        v
    }

    /// 延べ 1,000 語を越える形態素列。<strong>Stub は空白で切る。</strong>
    fn tokens(unit: &str, times: usize) -> (Vec<Segment>, Analyzed) {
        let prose: Vec<Segment> = (0..times).map(|_| seg(unit)).collect();
        let a = Analyzed::of(&prose, &Stub::unidic()).unwrap();
        (prose, a)
    }

    #[test]
    fn 次元は合わせて_12_である() {
        let total: usize = Metric::ALL.iter().map(|m| m.dims().len()).sum();
        assert_eq!(
            total, 12,
            "圧縮率 1 ＋ 繰り返し 8 ＋ 豊富さ 1 ＋ エントロピー 2"
        );
    }

    #[test]
    fn 繰り返しだけ下限を持つ() {
        // 生成文は繰り返しが足りない側に出る。ほかは機械の側が高く出る。
        assert!(!Metric::Repetition.upper_bound());
        for m in [Metric::Compression, Metric::Richness, Metric::Entropy] {
            assert!(m.upper_bound(), "{m:?}");
        }
    }

    #[test]
    fn 繰り返しが多いほど圧縮率は下がる() {
        let same = long("おなじことをくりかえして書いている。");
        let varied = long("烏兎匕叉巴丕些乖亙什仔仄仟价伉佚佶侃侏侑俐俑倅倏倆偃");
        let a = compression_ratio(&same).value().unwrap();
        let b = compression_ratio(&varied).value().unwrap();
        assert!(a < b, "繰り返しのほうが縮む: {a} vs {b}");
    }

    #[test]
    fn 短い地の文では圧縮率を測らない() {
        // 圧縮器のヘッダが結果を支配する。
        assert_eq!(compression_ratio(&[seg("短い。")]), Measured::BelowFloor);
    }

    #[test]
    fn 圧縮率は繋いでから測る() {
        // node ごとに圧縮して足すと、node をまたぐ反復が拾えない。
        let prose = vec![seg("あいうえお"), seg("かきくけこ")];
        assert_eq!(joined(&prose), "あいうえお\nかきくけこ");
    }

    #[test]
    fn 形態素が無ければ_3_指標は測っていないとする() {
        // 0 を返さない。0 は「測って 0 だった」という値である。
        let prose = long("これは 文 である 。");
        let h = Humanness::measure(&prose, None);
        assert!(!h.all_measured());
        assert_eq!(h.missing().len(), 11, "圧縮率だけが測れる");
        let (_, comp) = &h.per_metric()[0];
        assert!(comp[0].1.is_measured(), "圧縮率は解析器を要らない");
    }

    #[test]
    fn 延べ_1000_語に届かなければ測らない() {
        let (_, a) = tokens("これ は 文 で ある 。", 10);
        assert!(!a.enough_tokens());
        assert_eq!(richness(Some(&a)), Measured::BelowFloor);
        assert!(repetition(Some(&a)).iter().all(|m| !m.is_measured()));
    }

    #[test]
    fn 繰り返しの再来率は同じ言い回しで上がる() {
        let (_, same) = tokens("これ は 同じ 言い回し で ある 。", 200);
        let r = repetition(Some(&same));
        assert_eq!(r.len(), 8);
        let again = r[0].value().unwrap();
        assert!(again > 0.9, "同じ node の繰り返しなら再来率は高い: {again}");
    }

    #[test]
    fn 繰り返しは_node_を跨がない() {
        // 跨げば、構造が作った隣接を繰り返しとして数える。
        let prose: Vec<Segment> = (0..600).map(|_| seg("あ い")).collect();
        let a = Analyzed::of(&prose, &Stub::unidic()).unwrap();
        let r = repetition(Some(&a));
        // node ごとに「あ い」の bigram が 1 つだけ取れる。「い あ」は出ない。
        let top = r[1].value().unwrap();
        #[allow(clippy::cast_precision_loss)]
        let expected = 600.0 / a.tokens() as f64;
        assert!((top - expected).abs() < 1e-9, "{top} vs {expected}");
    }

    #[test]
    fn 最多率は延べ語数で割る() {
        // 割らなければ、長い文書ほど大きく出る。
        let (_, short) = tokens("あ い う え お か き く け こ", 120);
        let (_, long_one) = tokens("あ い う え お か き く け こ", 240);
        let a = repetition(Some(&short))[1].value().unwrap();
        let b = repetition(Some(&long_one))[1].value().unwrap();
        assert!((a - b).abs() < 1e-9, "長さに依らない: {a} vs {b}");
    }

    #[test]
    fn 豊富さは窓で測る() {
        // 窓を切らなければ、長いほど下がる——測っているのは長さになる。
        let (_, short) = tokens("あ い う え お か き く け こ", 100);
        let (_, long_one) = tokens("あ い う え お か き く け こ", 300);
        let a = richness(Some(&short)).value().unwrap();
        let b = richness(Some(&long_one)).value().unwrap();
        assert!((a - b).abs() < 1e-9, "窓ごとに同じ値になる: {a} vs {b}");
    }

    #[test]
    fn 端の窓に満たない分は捨てる() {
        let (_, a) = tokens("あ い う え お か き く け こ", 150);
        assert_eq!(a.tokens(), 1500);
        // 窓は 1 つだけ取れる。500 語は捨てる。
        assert!(richness(Some(&a)).is_measured());
    }

    #[test]
    fn 語彙が散るほどエントロピーは高い() {
        let (p1, same) = tokens("あ あ あ あ あ あ あ あ あ あ", 150);
        let words: String = (0..1500)
            .map(|i| format!("w{i} "))
            .collect::<Vec<_>>()
            .concat();
        let p2 = vec![seg(&words)];
        let varied = Analyzed::of(&p2, &Stub::unidic()).unwrap();
        let a = entropy(&p1, Some(&same))[0].value().unwrap();
        let b = entropy(&p2, Some(&varied))[0].value().unwrap();
        assert!(a < b, "散るほうが高い: {a} vs {b}");
    }

    #[test]
    fn エントロピーは底を_2_にする() {
        // 4 つの語が等しく出れば 2 ビットである。
        let e = shannon(["a", "b", "c", "d"].into_iter());
        assert!((e - 2.0).abs() < 1e-12, "{e}");
    }

    #[test]
    fn 文字のエントロピーも延べ語数で外す() {
        // 除外は指標に掛かる。次元ごとに違う下限を持たない。
        let (p, a) = tokens("これ は 文 。", 10);
        let e = entropy(&p, Some(&a));
        assert!(e.iter().all(|m| !m.is_measured()), "2 次元とも外れる");
    }

    #[test]
    fn 揃えば_12_次元が全部出る() {
        let prose: Vec<Segment> = (0..200)
            .map(|i| seg(&format!("これ は {i} 番目 の 文 で ある 。")))
            .collect();
        let a = Analyzed::of(&prose, &Stub::unidic()).unwrap();
        let h = Humanness::measure(&prose, Some(&a));
        assert!(h.all_measured(), "測れていない: {:?}", h.missing());
        assert_eq!(h.flat().len(), 12);
    }
}
