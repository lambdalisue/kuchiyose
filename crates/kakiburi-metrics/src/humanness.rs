//! 人らしさの指標。<strong>その人らしさではない。</strong>
//!
//! 人が書いたものに見えるかを測る（[通るための 2 つ目の条件](../../../docs/spec/010-strategy.md#通るには-2-つ要る)）。
//!
//! <strong>語彙の狭さを見る 5 つを、独立した 5 つの証拠として数えない。</strong> 圧縮率・短い繰り返し・
//! 長い繰り返し・語彙の豊富さ・エントロピーは<strong>同じ現象を別の角度から見ている</strong>。
//!
//! <strong>句読点の密度だけは、その 5 つと現象が違う。</strong> 語を数えないので、題材の広い文章が
//! 誰の手でも機械の側へ出る交絡を受けない（[Przystalski ほか 2025](../../../docs/references/przystalski-2025.md)
//! は句点・句読点・読点を重要度の上位 10 に挙げている）。
//!
//! <strong>ただし寄せる向きは同じとはかぎらない。</strong> 短い言い回しの反復と長い言い回しの
//! 再来は逆に出ることがあるので、[別の指標として持つ](Metric::RepetitionShort)。
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

/// 短い繰り返しで見る n の並び。
pub const SHORT_N: [usize; 2] = [2, 3];

/// 長い繰り返しで見る n の並び。
pub const LONG_N: [usize; 2] = [4, 5];

/// 繰り返しで見る n の並び。<strong>短いほうが先である。</strong>
///
/// <strong>公開しない。</strong> 本番は[短い](SHORT_N)と[長い](LONG_N)を別々に測る——
/// まとめた並びを外に出すと、<strong>混ぜてよいものとして読まれる</strong>。
#[cfg(test)]
const REPETITION_N: [usize; 4] = [2, 3, 4, 5];

/// 人らしさを測る窓の大きさ。**語で数える。**
///
/// **どの次元も、この窓ごとに測って平均を取る。** 揃えなければ、測っているのは
/// 書きぶりではなく長さである。
///
/// 実測で確かめた。窓を掛けていなかった次元は長さと強く相関していた——
/// 語のエントロピー +0.585、文字のエントロピー +0.517、圧縮率 −0.481。
/// 窓を掛けていた語彙の豊富さだけが +0.029 だった。
pub const WINDOW: usize = 1000;

/// 圧縮率を測る窓の大きさ。**バイトで数える。**
///
/// 圧縮率は[形態素解析を要らない数少ない指標](compression_ratio)なので、
/// 語ではなくバイトで切る。**要らないものを要ることにしない。**
pub const WINDOW_BYTES: usize = floor::PROSE_BYTES;

/// 人らしさの指標。
///
/// 繰り返しは<strong>短いと長いに割れている</strong>——2〜3 語の反復と 4〜5 語の再来は別の
/// 現象で、まとめると向きが指標の中で割れる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Metric {
    /// 圧縮率。1 次元。
    Compression,
    /// 短い繰り返し。4 次元。
    ///
    /// <strong>長いほうと分ける。</strong> 2〜3 語の言い回しの反復と、4〜5 語の言い回しの
    /// 再来は<strong>別の現象である</strong>——実測では、生成文は短いほうを人より多く
    /// 繰り返し、長いほうを人より少なく再来させる。まとめると
    /// <strong>向きが指標の中で割れ、どちらへ動かせばよいかを言えなくなる。</strong>
    ///
    /// [粗い括りに丸めない](../../../docs/spec/100-metrics.md#粗い括りに丸めない)を、
    /// 人らしさの側で実行したものである。
    RepetitionShort,
    /// 長い繰り返し。4 次元。
    RepetitionLong,
    /// 語彙の豊富さ。1 次元。
    Richness,
    /// エントロピー。2 次元。
    Entropy,
    /// 句読点の密度。2 次元。
    ///
    /// <strong>ほかの指標と現象が違う。</strong> ほかの 5 つは語彙の狭さを見ているので、
    /// <strong>題材の広い文章は誰が書いても機械の側に出る</strong>。この指標は語を数えない。
    ///
    /// 形態素解析を要らない——[圧縮率](Metric::Compression)と 2 つだけである。
    Punctuation,
}

impl Metric {
    /// 全部。<strong>合算の入力の並びはこの順である。</strong>
    pub const ALL: [Metric; 6] = [
        Metric::Compression,
        Metric::RepetitionShort,
        Metric::RepetitionLong,
        Metric::Richness,
        Metric::Entropy,
        Metric::Punctuation,
    ];

    /// 指標の名前。定義ファイルの 1 行目と同じ。
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Metric::Compression => "圧縮率",
            Metric::RepetitionShort => "短い繰り返し",
            Metric::RepetitionLong => "長い繰り返し",
            Metric::Richness => "語彙の豊富さ",
            Metric::Entropy => "エントロピー",
            Metric::Punctuation => "句読点の密度",
        }
    }

    /// この指標の次元の名前。
    #[must_use]
    pub fn dims(self) -> Vec<String> {
        match self {
            Metric::Compression => vec!["圧縮率".into()],
            Metric::RepetitionShort => SHORT_N
                .iter()
                .flat_map(|n| [format!("{n}gram の再来率"), format!("{n}gram の最多率")])
                .collect(),
            Metric::RepetitionLong => LONG_N
                .iter()
                .flat_map(|n| [format!("{n}gram の再来率"), format!("{n}gram の最多率")])
                .collect(),
            Metric::Richness => vec!["異なり語率".into()],
            Metric::Entropy => vec!["語のエントロピー".into(), "文字のエントロピー".into()],
            Metric::Punctuation => vec!["句点の密度".into(), "読点の密度".into()],
        }
    }

    /// 定義が名乗る「機械の側」。<strong>`true` なら機械が高い。</strong>
    ///
    /// 繰り返しは<strong>下限</strong>——先行研究は生成文が足りない側に出ると言う。ほかは上限で、
    /// 機械の側が高く出る。
    ///
    /// <strong>寄せる向きそのものではない。</strong> どちらへ寄せるかは
    /// [カセットごとの較正](../../../docs/spec/200-extract.md#人らしさの境目は同じ材料から出る)
    /// が決める——素材がこの向きを支えていないことは実際に起きる。
    #[must_use]
    pub fn upper_bound(self) -> bool {
        !matches!(self, Metric::RepetitionShort | Metric::RepetitionLong)
    }
}

/// 人らしさの次元の全体。<strong>指標ごとにまとめたまま持つ</strong>——3 段目で指標ごとに平均するからである。
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
                Metric::RepetitionShort => repetition(analyzed, &SHORT_N),
                Metric::RepetitionLong => repetition(analyzed, &LONG_N),
                Metric::Richness => vec![richness(analyzed)],
                Metric::Entropy => entropy(analyzed),
                Metric::Punctuation => punctuation(prose),
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

    /// 人らしさの次元を並びで返す。
    #[must_use]
    pub fn flat(&self) -> Vec<(String, Measured)> {
        self.values
            .iter()
            .flat_map(|(_, v)| v.iter().cloned())
            .collect()
    }

    /// <strong>指標が全部測れたか。</strong>
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
    // <strong>窓ごとに圧縮して、比の平均を取る。</strong> 通しで圧縮すると、長い文書ほど
    // 辞書が育って比が下がる——測っているのは書きぶりではなく長さになる。
    //
    // 地の文が短いと、圧縮器のヘッダが結果を支配する。窓が 1 つも取れなければ下限未満。
    let mut ratios = Vec::new();
    let mut window = String::new();
    for c in text.chars() {
        window.push(c);
        if window.len() < WINDOW_BYTES {
            continue;
        }
        let Some(r) = deflated(&window) else {
            // <strong>圧縮器が返さないのは環境の壊れである。</strong> 素材が短いのと混ぜない
            // ——混ぜれば、壊れた道具が「素材が足りない」という顔で回り続ける。
            return Measured::ToolFailed;
        };
        ratios.push(r);
        window.clear();
    }
    if ratios.is_empty() {
        return Measured::BelowFloor;
    }
    #[allow(clippy::cast_precision_loss)]
    Measured::Value(ratios.iter().sum::<f64>() / ratios.len() as f64)
}

/// 句読点の密度。<strong>句点と読点を別々に、1,000 字あたりで数える。</strong>
///
/// <strong>ほかの人らしさの指標と現象が違う。</strong> 語を数えないので、題材の広い文章が
/// 誰の手でも機械の側へ出る交絡を受けない。
///
/// 形態素解析を要らないので、[圧縮率](compression_ratio)と同じくバイトの窓で切る。
#[must_use]
pub fn punctuation(prose: &[Segment]) -> Vec<Measured> {
    let text = joined(prose);
    let mut periods = Vec::new();
    let mut commas = Vec::new();
    let mut window = String::new();
    for c in text.chars() {
        window.push(c);
        if window.len() < WINDOW_BYTES {
            continue;
        }
        push_rates(&window, &mut periods, &mut commas);
        window.clear();
    }
    if periods.is_empty() {
        // 窓が 1 つも取れない。<strong>0 を返さない</strong>——0 は「測って 0 だった」である。
        return vec![Measured::BelowFloor, Measured::BelowFloor];
    }
    vec![mean(&periods), mean(&commas)]
}

/// 1 つの窓から、句点と読点の 1,000 字あたりの数を出して足す。
fn push_rates(window: &str, periods: &mut Vec<f64>, commas: &mut Vec<f64>) {
    let chars = window.chars().count();
    if chars == 0 {
        return;
    }
    // <strong>全角のみを数える。</strong> 和文の句読点と `.` `,` は用途が違う。
    let count = |target: char| window.chars().filter(|c| *c == target).count();
    #[allow(clippy::cast_precision_loss)]
    let per_thousand = |n: usize| n as f64 / chars as f64 * 1000.0;
    periods.push(per_thousand(count('。')));
    commas.push(per_thousand(count('、')));
}

/// 窓ごとの値の平均。
fn mean(values: &[f64]) -> Measured {
    #[allow(clippy::cast_precision_loss)]
    Measured::Value(values.iter().sum::<f64>() / values.len() as f64)
}

/// 1 つの窓を圧縮して、比を返す。<strong>圧縮器が返さなければ `None`。</strong>
fn deflated(window: &str) -> Option<f64> {
    let raw = window.as_bytes();
    let mut z =
        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(COMPRESSION_LEVEL));
    z.write_all(raw).ok()?;
    let out = z.finish().ok()?;
    #[allow(clippy::cast_precision_loss)]
    Some(out.len() as f64 / raw.len() as f64)
}

/// 繰り返し。<strong>n ごとに 2 つ。</strong>
///
/// | | 何を出すか |
/// | --- | --- |
/// | 再来率 | 2 回以上現れた n-gram の数 ÷ <strong>異なり n-gram の数</strong> |
/// | 最多率 | 最も多く現れた n-gram の出現回数 ÷ <strong>延べ語数</strong> |
///
/// <strong>n-gram は node を跨がない。</strong> 跨げば、構造が作った隣接を繰り返しとして数える。
///
/// 伏せ字の跡か。<strong>1 文字でも触れていれば渡さない。</strong>
///
/// 識別子を畳んだ跡であって、書き手が選んだ言い回しではない。渡せば
/// 「ゐゑと書け」と言うことになる。
///
/// <strong>並び全体で照らしてはいけない。</strong> 伏せ字が隣り合うと、境目を跨いだ並びが
/// 伏せ字を逆順に並べた形（`ゑゐ`）になる——伏せ字そのものを含まないので素通りし、
/// 本人の記事の指摘に「この文章が繰り返しているのは『ゑゐ』」として出た。
fn is_sentinel_debris(s: &str) -> bool {
    s.chars().any(|c| kakiburi_doc::prose::SENTINEL.contains(c))
}

/// その単位で<strong>一度しか出てこない語</strong>を挙げる。
///
/// <strong>「語を散らすな」と言うなら、どれが散らしているのかを言わなければ直せない。</strong>
/// 実測では、この指示を受けた側が<strong>散らす方向へ直してしまった</strong>——どの語を
/// 潰せばよいかが分からず、言い換えを別の言い換えに置き換えたためである。
///
/// <strong>自立語だけを挙げる。</strong> 助詞や助動詞が一度きりでも、それは言い換えでは
/// なく文の形である。
#[must_use]
pub fn once_only(analyzed: Option<&Analyzed>, top: usize) -> Vec<String> {
    let Some(a) = analyzed else {
        return Vec::new();
    };
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut content: BTreeMap<&str, bool> = BTreeMap::new();
    for m in a.all() {
        *counts.entry(m.surface.as_str()).or_default() += 1;
        content.insert(m.surface.as_str(), !m.is_function_word());
    }
    let mut out: Vec<&str> = counts
        .iter()
        .filter(|(w, k)| **k == 1 && content.get(*w).copied().unwrap_or(false))
        // <strong>1 文字の語は挙げない。</strong> 潰しようがない。
        .filter(|(w, _)| w.chars().count() >= 2)
        .filter(|(w, _)| !is_sentinel_debris(w))
        .map(|(w, _)| *w)
        .collect();
    // <strong>長い順。</strong> 長い語ほど言い換えである見込みが高い。
    out.sort_by(|a, b| {
        b.chars()
            .count()
            .cmp(&a.chars().count())
            .then_with(|| a.cmp(b))
    });
    out.truncate(top);
    out.into_iter().map(str::to_owned).collect()
}

/// その単位が<strong>繰り返しすぎている言い回し</strong>を、多い順に挙げる。
///
/// <strong>「減らせ」と言うなら、どれを減らすのかを言わなければ直せない。</strong> 実測では、
/// 生成文の最多は「ます。」が延べ語の 3.4% を占めていた——本人の 1.3% の 2.7 倍で、
/// <strong>文末がほぼ 1 種類に潰れている</strong>ことを意味する。
#[must_use]
pub fn overused(analyzed: Option<&Analyzed>, ns: &[usize], top: usize) -> Vec<String> {
    let Some(a) = analyzed else {
        return Vec::new();
    };
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for &n in ns {
        for seg in a.segments() {
            let words: Vec<&str> = seg.iter().map(|m| m.surface.as_str()).collect();
            for w in words.windows(n) {
                *counts.entry(w.concat()).or_default() += 1;
            }
        }
    }
    let mut out: Vec<(usize, String)> = counts
        .into_iter()
        .filter(|(g, _)| !is_sentinel_debris(g))
        .map(|(g, k)| (k, g))
        .collect();
    // <strong>多い順。同じなら文字の順。</strong> 決めておかないと並びが実装で変わる。
    out.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    out.truncate(top);
    out.into_iter().map(|(_, g)| g).collect()
}

/// その単位の中で<strong>実際に再来した言い回し</strong>を挙げる。
///
/// <strong>直し方が「その人が現に繰り返している言い回しを繰り返す」と言うなら、その
/// 言い回しを渡さなければ直せない。</strong> 数値と向きだけでは、受け取った側は自分で
/// でっち上げた定型句を挿し込むことになる。
///
/// <strong>1 本の中で 2 回以上出たものだけを取る。</strong> 1 回きりの並びは、その文書の題材が
/// 作ったものであって癖ではない。
#[must_use]
pub fn recurring(analyzed: Option<&Analyzed>, ns: &[usize]) -> Vec<String> {
    let Some(a) = analyzed else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for &n in ns {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for seg in a.segments() {
            let words: Vec<&str> = seg.iter().map(|m| m.surface.as_str()).collect();
            for w in words.windows(n) {
                *counts.entry(w.concat()).or_default() += 1;
            }
        }
        // <strong>1 本の中で 2 回以上出たものだけを、その人の癖として数える。</strong>
        // 1 回きりの並びは、その文書の題材が作ったものである。
        //
        out.extend(
            counts
                .into_iter()
                .filter(|(g, k)| *k >= 2 && !is_sentinel_debris(g))
                .map(|(g, _)| g),
        );
    }
    out
}

/// 窓ごとに切った語の並び。<strong>窓の中では node の切れ目を保つ。</strong>
///
/// n-gram は node を跨がないので、切れ目を落とすと構造が作った隣接を繰り返しとして
/// 数えることになる。
///
/// <strong>端の半端は捨てる。</strong> 大きさの揃わない窓を混ぜれば、平均が長さで動く。
fn token_windows(a: &Analyzed) -> Vec<Vec<Vec<&str>>> {
    let mut out: Vec<Vec<Vec<&str>>> = Vec::new();
    let mut cur: Vec<Vec<&str>> = Vec::new();
    let mut run: Vec<&str> = Vec::new();
    let mut n = 0usize;
    for seg in a.segments() {
        for m in seg {
            run.push(m.surface.as_str());
            n += 1;
            if n == WINDOW {
                cur.push(std::mem::take(&mut run));
                out.push(std::mem::take(&mut cur));
                n = 0;
            }
        }
        if !run.is_empty() {
            cur.push(std::mem::take(&mut run));
        }
    }
    out
}

/// 窓ごとの値をならす。
///
/// <strong>値を返さない窓は平均から外す。</strong> 分母が 0 の窓を 0 として混ぜれば、
/// 測れなかったことが値になる。1 つも残らなければ分母が無い。
fn averaged(windows: &[Vec<Vec<&str>>], f: impl Fn(&[Vec<&str>]) -> Option<f64>) -> Measured {
    let vals: Vec<f64> = windows.iter().filter_map(|w| f(w)).collect();
    if vals.is_empty() {
        return Measured::NoDenominator;
    }
    #[allow(clippy::cast_precision_loss)]
    Measured::Value(vals.iter().sum::<f64>() / vals.len() as f64)
}

/// 繰り返しの数え上げ。
#[must_use]
pub fn repetition(analyzed: Option<&Analyzed>, ns: &[usize]) -> Vec<Measured> {
    let n_dims = ns.len() * 2;
    // <strong>解析器が無いのと、語が足りないのを分ける。</strong> 前者は環境の壊れで、素材を
    // いくら足しても直らない。
    let Some(a) = analyzed else {
        return vec![Measured::ToolMissing; n_dims];
    };
    let ws = token_windows(a);
    if ws.is_empty() {
        return vec![Measured::BelowFloor; n_dims];
    }
    let count = |w: &[Vec<&str>], n: usize| -> BTreeMap<String, usize> {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for run in w {
            for g in run.windows(n) {
                *counts.entry(g.join("\u{1F}")).or_default() += 1;
            }
        }
        counts
    };
    let mut out = Vec::with_capacity(n_dims);
    for &n in ns {
        // <strong>再来率。</strong> 2 回以上現れた n-gram の数 ÷ 異なり n-gram の数。
        //
        // <strong>n-gram が 1 つも取れない窓は平均から外す。分母が 0 である。</strong>
        out.push(averaged(&ws, |w| {
            let counts = count(w, n);
            if counts.is_empty() {
                return None;
            }
            #[allow(clippy::cast_precision_loss)]
            Some(counts.values().filter(|&&c| c >= 2).count() as f64 / counts.len() as f64)
        }));
        // <strong>最多率。</strong> 最も多く現れた n-gram の出現回数 ÷ 窓の語数。
        out.push(averaged(&ws, |w| {
            let counts = count(w, n);
            #[allow(clippy::cast_precision_loss)]
            counts
                .values()
                .copied()
                .max()
                .map(|top| top as f64 / WINDOW as f64)
        }));
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
    // 約物と記号の形態素も含める——外すと、読点の多い書き手ほど窓が長くなる。
    let ws = token_windows(a);
    if ws.is_empty() {
        return Measured::BelowFloor;
    }
    averaged(&ws, |w| {
        let types: std::collections::BTreeSet<&str> = w.iter().flatten().copied().collect();
        #[allow(clippy::cast_precision_loss)]
        Some(types.len() as f64 / WINDOW as f64)
    })
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
pub fn entropy(analyzed: Option<&Analyzed>) -> Vec<Measured> {
    let Some(a) = analyzed else {
        return vec![Measured::ToolMissing; 2];
    };
    let ws = token_windows(a);
    if ws.is_empty() {
        return vec![Measured::BelowFloor; 2];
    }
    let words = averaged(&ws, |w| Some(shannon(w.iter().flatten().copied())));
    // 文字は日本語の文字に限らない全文字である。
    //
    // <strong>語の窓から取る。</strong> 地の文をそのまま数えると、語の側と文字の側で
    // 窓が揃わない——[除外は指標に掛かる](../../../docs/spec/100-metrics.md#除外の既定)
    // ものであって、次元ごとに違う切り方を持たない。
    let chars = averaged(&ws, |w| {
        Some(shannon(
            w.iter().flatten().flat_map(|t| t.chars()).map(CharKey),
        ))
    });
    vec![words, chars]
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

    /// 同じ文を n 回並べた地の文。
    fn long_times(unit: &str, times: usize) -> Vec<Segment> {
        (0..times).map(|_| seg(unit)).collect()
    }

    /// 延べ 1,000 語を越える形態素列。<strong>Stub は空白で切る。</strong>
    fn tokens(unit: &str, times: usize) -> (Vec<Segment>, Analyzed) {
        let prose: Vec<Segment> = (0..times).map(|_| seg(unit)).collect();
        let a = Analyzed::of(&prose, &Stub::unidic()).unwrap();
        (prose, a)
    }

    #[test]
    fn 次元は合わせて_14_である() {
        let total: usize = Metric::ALL.iter().map(|m| m.dims().len()).sum();
        assert_eq!(
            total, 14,
            "圧縮率 1 ＋ 繰り返し 8 ＋ 豊富さ 1 ＋ エントロピー 2 ＋ 句読点 2"
        );
    }

    #[test]
    fn 再来した言い回しだけを挙げる() {
        // <strong>1 回きりの並びは癖ではない。</strong> その文書の題材が作ったものである。
        // Stub は空白で切る。
        let a = Analyzed::of(
            &[seg("あとで 書く よ あとで 書く よ いま は 書か ない")],
            &Stub::unidic(),
        )
        .ok();
        let got = recurring(a.as_ref(), &[3]);
        assert!(
            got.iter().any(|g| g.contains("あとで")),
            "2 回出た並びは挙げる: {got:?}"
        );
        assert!(
            !got.iter().any(|g| g.contains("いま")),
            "1 回きりは挙げない: {got:?}"
        );
    }

    #[test]
    fn 伏せ字を含む並びは渡さない() {
        // <strong>識別子を畳んだ跡であって、書き手が選んだ言い回しではない。</strong>
        // 渡せば「ゐゑゐゑと書け」と言うことになる。
        let t = format!("{0} を使う {0} を使う", kakiburi_doc::prose::SENTINEL);
        let a = Analyzed::of(&[seg(t.replace("", " ").trim())], &Stub::unidic()).ok();
        let got = recurring(a.as_ref(), &[2, 3]);
        assert!(
            !got.iter()
                .any(|g| g.contains(kakiburi_doc::prose::SENTINEL)),
            "{got:?}"
        );
    }

    #[test]
    fn 伏せ字の欠片も渡さない() {
        // <strong>伏せ字が隣り合うと、境目を跨いだ並びが伏せ字を逆順に並べた形になる。</strong>
        // 「ゐゑ ゐゑ」からは「ゑゐ」が取れる——<strong>これは伏せ字そのものを含まない</strong>ので、
        // 並び全体で照らす除け方では素通りする。実際に本人の記事の指摘へ出た。
        //
        // 落とすのは 1 文字ずつ照らしたときである。
        let t = format!("{0} {0} {0}", kakiburi_doc::prose::SENTINEL);
        let a = Analyzed::of(&[seg(t.replace("", " ").trim())], &Stub::unidic()).ok();
        let got = recurring(a.as_ref(), &[2, 3]);
        for g in &got {
            assert!(
                !g.chars().any(|c| kakiburi_doc::prose::SENTINEL.contains(c)),
                "伏せ字の欠片が渡っている: {g:?}（全体: {got:?}）"
            );
        }
    }

    #[test]
    fn 繰り返しすぎにも伏せ字の欠片を渡さない() {
        // 本人の記事の指摘に「この文章が繰り返しているのは『ゑゐ』」と出た。
        // <strong>原文に 1 度も無い並びである。</strong>
        let t = format!("{0} {0} {0}", kakiburi_doc::prose::SENTINEL);
        let a = Analyzed::of(&[seg(t.replace("", " ").trim())], &Stub::unidic()).ok();
        let got = overused(a.as_ref(), &[2, 3], 5);
        for g in &got {
            assert!(
                !g.chars().any(|c| kakiburi_doc::prose::SENTINEL.contains(c)),
                "伏せ字の欠片が渡っている: {g:?}（全体: {got:?}）"
            );
        }
    }

    #[test]
    fn 一度きりの語にも伏せ字を渡さない() {
        let t = format!("{} を使う", kakiburi_doc::prose::SENTINEL);
        let a = Analyzed::of(&[seg(&t)], &Stub::unidic()).ok();
        let got = once_only(a.as_ref(), 10);
        for g in &got {
            assert!(
                !g.chars().any(|c| kakiburi_doc::prose::SENTINEL.contains(c)),
                "伏せ字が語として渡っている: {g:?}（全体: {got:?}）"
            );
        }
    }

    #[test]
    fn 解析器が無ければ挙げない() {
        assert!(recurring(None, &[4]).is_empty());
    }

    #[test]
    fn 繰り返しだけ下限を持つ() {
        // 生成文は繰り返しが足りない側に出る。ほかは機械の側が高く出る。
        assert!(!Metric::RepetitionShort.upper_bound());
        assert!(!Metric::RepetitionLong.upper_bound());
        for m in [Metric::Compression, Metric::Richness, Metric::Entropy] {
            assert!(m.upper_bound(), "{m:?}");
        }
    }

    #[test]
    fn 短い繰り返しと長い繰り返しは別の次元を持つ() {
        // <strong>まとめると向きが指標の中で割れ、どちらへ動かせばよいかを言えなくなる。</strong>
        let short = Metric::RepetitionShort.dims();
        let long = Metric::RepetitionLong.dims();
        assert_eq!(short.len(), 4);
        assert_eq!(long.len(), 4);
        assert!(short
            .iter()
            .all(|n| n.starts_with('2') || n.starts_with('3')));
        assert!(long
            .iter()
            .all(|n| n.starts_with('4') || n.starts_with('5')));
        assert!(short.iter().all(|n| !long.contains(n)));
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
        assert!(repetition(Some(&a), &REPETITION_N)
            .iter()
            .all(|m| !m.is_measured()));
    }

    #[test]
    fn 繰り返しの再来率は同じ言い回しで上がる() {
        let (_, same) = tokens("これ は 同じ 言い回し で ある 。", 200);
        let r = repetition(Some(&same), &REPETITION_N);
        assert_eq!(r.len(), 8);
        let again = r[0].value().unwrap();
        assert!(again > 0.9, "同じ node の繰り返しなら再来率は高い: {again}");
    }

    #[test]
    fn 繰り返しは_node_を跨がない() {
        // 跨げば、構造が作った隣接を繰り返しとして数える。
        let prose: Vec<Segment> = (0..600).map(|_| seg("あ い")).collect();
        let a = Analyzed::of(&prose, &Stub::unidic()).unwrap();
        let r = repetition(Some(&a), &REPETITION_N);
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
        let a = repetition(Some(&short), &REPETITION_N)[1].value().unwrap();
        let b = repetition(Some(&long_one), &REPETITION_N)[1]
            .value()
            .unwrap();
        assert!((a - b).abs() < 1e-9, "長さに依らない: {a} vs {b}");
    }

    // <strong>ここから 6 本は、定義ファイルの `上` と実装の向きを結ぶ見張りである。</strong>
    //
    // 定義の `上` は<strong>「値が高すぎるときの直し方」</strong>である。取り違えると、道具は
    // 較正が「減らせ」と言った場面で「増やせ」と指示する——<strong>従うほど人らしさが
    // 下がる。</strong> 実際に 1 度そう書いた（句読点の密度）。
    //
    // <strong>文言の意味は機械で読めない。</strong> だから<strong>上の直し方を当てた前後の文</strong>を置いて、
    // 値が実際に下がることを見る。定義を書き換えただけでは、この見張りは動かない
    // ——<strong>指標ごとに 1 本ずつ要る。</strong>
    //
    // | 指標 | 見張り |
    // | --- | --- |
    // | 圧縮率 | [`繰り返しが多いほど圧縮率は下がる`] |
    // | 短い繰り返し・長い繰り返し | [`繰り返しの再来率は同じ言い回しで上がる`] |
    // | 語彙の豊富さ | [`上の直し方を当てると語彙の豊富さは下がる`] |
    // | エントロピー | [`語彙が散るほどエントロピーは高い`] |
    // | 句読点の密度 | [`上の直し方を当てると句読点の密度は下がる`] |

    #[test]
    fn 上の直し方を当てると語彙の豊富さは下がる() {
        // 上の直し方は「語を散らさない。同じものを指すのに別の語を使い分けない」。
        let (_, scattered) = tokens("あ い う え お か き く け こ", 150);
        let (_, narrow) = tokens("あ あ あ い い い あ あ い い", 150);
        let wide = richness(Some(&scattered)).value().expect("測れる");
        let tight = richness(Some(&narrow)).value().expect("測れる");
        assert!(tight < wide, "語を散らさないほうが低い: {tight} vs {wide}");
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
        let (_, same) = tokens("あ あ あ あ あ あ あ あ あ あ", 150);
        let words: String = (0..1500)
            .map(|i| format!("w{i} "))
            .collect::<Vec<_>>()
            .concat();
        let p2 = vec![seg(&words)];
        let varied = Analyzed::of(&p2, &Stub::unidic()).unwrap();
        let a = entropy(Some(&same))[0].value().unwrap();
        let b = entropy(Some(&varied))[0].value().unwrap();
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
        let (_, a) = tokens("これ は 文 。", 10);
        let e = entropy(Some(&a));
        assert!(e.iter().all(|m| !m.is_measured()), "2 次元とも外れる");
    }

    #[test]
    fn 揃えば_14_次元が全部出る() {
        let prose: Vec<Segment> = (0..200)
            .map(|i| seg(&format!("これ は {i} 番目 の 文 で ある 。")))
            .collect();
        let a = Analyzed::of(&prose, &Stub::unidic()).unwrap();
        let h = Humanness::measure(&prose, Some(&a));
        assert!(h.all_measured(), "測れていない: {:?}", h.missing());
        assert_eq!(h.flat().len(), 14);
    }

    #[test]
    fn 句読点の密度は_1000_字あたりで数える() {
        // 7 字の文に句点 1・読点 1。**繋ぎの改行も 1 字に数える**ので 8 字あたり 1 つ。
        let v = punctuation(&long("あい、うえお。"));
        let period = v[0].value().expect("句点が測れる");
        let comma = v[1].value().expect("読点が測れる");
        assert!((period - 125.0).abs() < 1.0, "8 字に句点 1 つ: {period}");
        assert!((comma - period).abs() < 1.0, "読点も同じ数: {comma}");
    }

    #[test]
    fn 句読点の密度は長さに依らない() {
        // **窓ごとの割合の平均である。** 長い文書ほど大きくなるなら、測っているのは
        // 書きぶりではなく長さになる。
        let short = punctuation(&long_times("あい、うえお。", 400))[0]
            .value()
            .unwrap();
        let doubled = punctuation(&long_times("あい、うえお。", 800))[0]
            .value()
            .unwrap();
        assert!(
            (short - doubled).abs() < 1.0,
            "長さで動いた: {short} vs {doubled}"
        );
    }

    #[test]
    fn 句読点の密度は形態素解析を要らない() {
        // 圧縮率と 2 つだけである。辞書の無い環境でも残る。
        let prose = long("あい、うえお。");
        let h = Humanness::measure(&prose, None);
        let got: Vec<(String, Measured)> = h
            .per_metric()
            .iter()
            .find(|(m, _)| *m == Metric::Punctuation)
            .expect("句読点の密度がある")
            .1
            .clone();
        assert!(got.iter().all(|(_, m)| m.is_measured()), "{got:?}");
    }

    #[test]
    fn 短い地の文では句読点の密度を測らない() {
        // 窓が 1 つも取れない。**0 を返さない。**
        let v = punctuation(&[seg("短い。")]);
        assert_eq!(v, vec![Measured::BelowFloor, Measured::BelowFloor]);
    }

    #[test]
    fn 上の直し方を当てると句読点の密度は下がる() {
        // <strong>定義ファイルの `上` は「値が高すぎるときの直し方」である。</strong>
        // 逆に書くと、較正が「減らせ」と言った場面で道具は「増やせ」と指示する
        // ——<strong>直し方に従うほど人らしさが下がる。</strong> 実際に 1 度そう書いた。
        //
        // 上の直し方は「文を繋いで長くし、読点を減らす」。当てて下がることを見る。
        let before = punctuation(&long("あい、うえお。"));
        let after = punctuation(&long("あいうえおかきくけこさしすせそたちつてと。"));
        for (i, name) in ["句点", "読点"].iter().enumerate() {
            let b = before[i].value().expect("測れる");
            let a = after[i].value().expect("測れる");
            assert!(a < b, "{name}の密度が下がらない: {b} → {a}");
        }
    }

    #[test]
    fn 半角の句読点は数えない() {
        // 和文の句読点と用途が違ううえ、識別子を伏せたあとの半角記号は題材の残りかす。
        let zenkaku = punctuation(&long("あい、うえお。"))[0].value().unwrap();
        let hankaku = punctuation(&long("あいxうえおy."))[0].value().unwrap();
        assert!(zenkaku > 0.0, "全角は数える: {zenkaku}");
        assert_eq!(hankaku, 0.0, "半角は数えない: {hankaku}");
    }
}
