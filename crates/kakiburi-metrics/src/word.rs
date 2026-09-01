//! 形態素解析を要る指標。
//!
//! <strong>受け取るのは[解析し終えた形](crate::morph::Analyzed)である。</strong> 体系の確かめは
//! そこで済んでいる——UniDic 以外で測れば、語彙素で引く指標が 0 件として静かに落ちる。

use std::collections::BTreeMap;

/// 数え上げ。識別子ごとの生の回数。
pub type Counts = BTreeMap<String, usize>;

use crate::morph::Analyzed;

use crate::Measured;

/// 機能語の分布。
///
/// 品詞が助詞・助動詞・接続詞・副詞・感動詞である形態素の <strong>表層形</strong>ごとに数える。
///
/// <strong>表層形で数える。</strong>「は」と「わ」、「けれど」と「けど」は別に扱う——
/// そこが人によって違う。
#[must_use]
pub fn function_words(a: &Analyzed) -> Counts {
    let mut counts: Counts = BTreeMap::new();
    for m in a.all() {
        if m.is_function_word() {
            *counts.entry(m.surface.clone()).or_default() += 1;
        }
    }
    counts
}

/// 品詞 bigram の分布。
///
/// <strong>第 1 層のタグを使う。</strong> 暫定である——[金 2013](../../../docs/references/jin-2013.md)は
/// 第 2 層が最良だった報告を持つ。
///
/// <strong>node を跨がない。</strong> 跨げば、見出しの末尾と次の段落の先頭の組ができる——
/// その隣接は書き手が選んだものではない。
#[must_use]
pub fn pos_bigrams(a: &Analyzed) -> Counts {
    let mut counts: Counts = BTreeMap::new();
    for seg in a.segments() {
        let tags: Vec<&str> = seg.iter().map(|m| m.pos1.as_str()).collect();
        for w in tags.windows(2) {
            *counts.entry(format!("{}+{}", w[0], w[1])).or_default() += 1;
        }
    }
    counts
}

/// 接続詞直後の読点で見る語彙素。<strong>この一覧は定義の一部である。</strong>
///
/// <strong>コーパスから選ばない。</strong> 書き手ごとに違う語彙素を採れば、指標の名前が書き手
/// ごとに変わる。
pub const CONJUNCTIONS: [&str; 12] = [
    "で",
    "が",
    "だが",
    "然しながら",
    "又",
    "猶",
    "ですから",
    "しかし",
    "また",
    "そして",
    "ただし",
    "つまり",
];

/// 文頭か文中か。
///
/// <strong>文頭の接続詞は打たれやすいことが分かっており、そこは規範の層である。</strong>
/// 人が出るのは文中の側である。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Position {
    /// 文頭。
    Head,
    /// 文中。
    Middle,
}

impl Position {
    /// 名前。指標の名前に入る。
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Position::Head => "文頭",
            Position::Middle => "文中",
        }
    }
}

/// 接続詞直後の読点が展開する軸の名前。<strong>12 語彙素 × 2 位置 = 24 本。</strong>
///
/// <strong>解析器が無くても名前は決まる。</strong> 一覧が定義の一部なので、コーパスから選ばない
/// ——選べば書き手ごとに軸の名前が変わり、
/// [名前は 1 度しか書かない](../../../docs/spec/100-metrics.md#名前は-1-度しか書かない)が
/// 守れなくなる。
///
/// 登録簿は<strong>展開後の軸</strong>を持つ。親の名前だけを置くと、値の側にある 24 本と
/// 一致しなくなる。
#[must_use]
pub fn conjunction_comma_names() -> Vec<String> {
    let mut out = Vec::with_capacity(CONJUNCTIONS.len() * 2);
    for lemma in CONJUNCTIONS {
        for pos in [Position::Head, Position::Middle] {
            out.push(format!("接続詞直後の読点・{lemma}・{}", pos.name()));
        }
    }
    out
}

/// 形態素列を文節に割る。<strong>自立語 1 つと、それに続く付属語。</strong>
///
/// <strong>文節が並びの単位である。</strong> 形態素の窓で切ると、`が地味` `に分け` `が含ま` の
/// ような<strong>言葉として立たない断片</strong>が候補を埋める——実測で、機械の型を
/// 出したときに上位がそういう断片ばかりになった。<strong>並べ替えでは直らない。</strong>
/// 候補の集合そのものが断片でできているからである。
///
/// <strong>自立語が続くときは切らない。</strong> `フロント`＋`エンド` のような複合語は 1 つの
/// 文節である。切ると、複合語の途中から始まる並びが出る。
fn bunsetsu(words: &[(&str, &str)]) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for (i, w) in words.iter().enumerate() {
        // <strong>自立語で始まる。</strong> 直前も自立語なら複合語なので切らない。
        let starts = is_content(w) && !out.last().is_some_and(|_| is_content(&words[i - 1]));
        if starts || out.is_empty() {
            out.push((i, i + 1));
        } else if let Some(last) = out.last_mut() {
            last.1 = i + 1;
        }
    }
    // <strong>先頭が付属語で始まる塊は落とす。</strong> 文節ではなく、前の文の残りである。
    if out.first().is_some_and(|&(s, _)| !is_content(&words[s])) {
        out.remove(0);
    }
    out
}

/// その単位に現れる語の並びと、文書の中での位置。
///
/// <strong>型を取り出すための材料である</strong>——並びが本人の多くの単位に現れ、基準にほとんど
/// 現れないなら、それはその人の型である（[取り出し](../../kakiburi-scale/src/assemble.rs)）。
///
/// <strong>数えるのは[文節](bunsetsu)の並びである。</strong> `ns` は文節の数を指す。
///
/// <strong>英数字を含む並びは落とす。</strong> URL・パス・製品名は題材であって書きぶりではない
/// ——[識別子を伏せる](kakiburi_doc::prose::mask_identifiers)のと同じ理由である。
///
/// 位置は <strong>node の番号 ÷ node の総数</strong>。書き出しにしか現れない並びは、書き出しの型
/// である。<strong>node の番号も返す</strong>——同じ node に現れる 2 つの型は、
/// [穴あきの型](../../kakiburi-scale/src/assemble.rs)として繋がる。
#[must_use]
pub fn grams_with_position(analyzed: Option<&Analyzed>, ns: &[usize]) -> Vec<(String, f64, usize)> {
    let Some(a) = analyzed else {
        return Vec::new();
    };
    let total = a.segments().len().max(1);
    let mut out = Vec::new();
    for (i, seg) in a.segments().iter().enumerate() {
        let words: Vec<(&str, &str)> = seg
            .iter()
            .map(|m| (m.surface.as_str(), m.pos1.as_str()))
            .collect();
        #[allow(clippy::cast_precision_loss)]
        let at = i as f64 / total as f64;
        let bs = bunsetsu(&words);
        for &n in ns {
            for span in bs.windows(n) {
                let (from, to) = (span[0].0, span[span.len() - 1].1);
                let w = &words[from..to];
                let text: String = w.iter().map(|(s, _)| *s).collect();
                // <strong>伏せ字を含む並びは型ではない。</strong> 識別子を畳んだ跡であって、
                // 書き手が選んだ言い回しではない——渡せば「ゐゑと書け」と言うことになる。
                // <strong>伏せ字の一部でも落とす。</strong> 並びが伏せ字の途中から始まれば、
                // 全体は含まないのに欠片が残る。
                if text
                    .chars()
                    .any(|c| kakiburi_doc::prose::SENTINEL.contains(c))
                    || text.chars().any(|c| c.is_ascii_alphanumeric())
                    || !text.chars().any(kakiburi_doc::text::is_japanese)
                {
                    continue;
                }
                out.push((text, at, i));
            }
        }
    }
    out
}

/// 自立語か。<strong>単独で文節を始められる語。</strong>
///
/// <strong>UniDic の体系をそのまま使う。</strong> 学校文法の「名詞」「形容動詞」を当てはめると
/// 落ちるものが出る——<strong>UniDic は な形容詞の語幹を `形状詞` に、`私` `これ` を
/// `代名詞` に分ける。</strong>
///
/// 実測で、この 2 つを落としていたために<strong>「地味に」が並びとして一度も拾えなかった</strong>
/// ——`地味` は `形状詞` なので、`地味`＋`に` は自立語を含まない並びと見なされていた。
/// 同じ理由で 静か・便利・重要・簡単・快適 も、すべて見えていなかった。
fn is_content(m: &(&str, &str)) -> bool {
    matches!(
        m.1,
        "名詞" | "代名詞" | "形状詞" | "動詞" | "形容詞" | "副詞" | "接続詞" | "感動詞"
    )
}

/// 語を割っている読点の数。
///
/// <strong>道具が「読点を増やせ」と言った結果、語の内側に読点が入ることがある。</strong>
/// `あらため、て取得し直す` は `改めて` を割っている。指標は満たされ、日本語は壊れる。
///
/// <strong>数え方は[解析のとき](kakiburi_metrics::morph::Analyzed)に済ませてある</strong>——
/// 読点を抜いて解析し直す必要があり、解析器を持っているのはそこだけである。
///
/// <strong>線は 0 である。</strong> 実測で、素材 71 本・読点 3,721 個のうち<strong>1 つも当たらなかった</strong>
/// ——`ある、という` も `さて、では` も `はい、なので` も、読点を外して語が繋がらない。
/// 壊れた草稿の `あらため、て` だけが当たる。
#[must_use]
pub fn splitting_commas(analyzed: Option<&Analyzed>) -> Measured {
    let Some(a) = analyzed else {
        return Measured::ToolMissing;
    };
    #[allow(clippy::cast_precision_loss)]
    Measured::Value(a.split_commas().len() as f64)
}

/// 接続詞直後の読点。<strong>語彙素 × 位置ごとのスカラー。</strong>
///
/// 名前は「接続詞直後の読点・&lt;語彙素&gt;・&lt;文頭|文中&gt;」。12 × 2 = 24 本になる。
///
/// <strong>まとめて 1 つの割合にしない。</strong> まとめると、規範で決まる分に薄まる。
#[must_use]
pub fn conjunction_comma(a: &Analyzed) -> BTreeMap<String, Measured> {
    // (語彙素, 位置) → (読点あり, 全体)
    let mut tally: BTreeMap<(&'static str, Position), (usize, usize)> = BTreeMap::new();
    for lemma in CONJUNCTIONS {
        for pos in [Position::Head, Position::Middle] {
            tally.insert((lemma, pos), (0, 0));
        }
    }

    for ms in a.segments() {
        // 文頭の判定。<strong>直前が終端記号か、node の先頭。</strong>
        let mut at_head = true;
        for (i, m) in ms.iter().enumerate() {
            if m.is_conjunction() {
                if let Some(&lemma) = CONJUNCTIONS.iter().find(|&&c| c == m.lemma) {
                    let pos = if at_head {
                        Position::Head
                    } else {
                        Position::Middle
                    };
                    let entry = tally.entry((lemma, pos)).or_default();
                    entry.1 += 1;
                    if ms.get(i + 1).is_some_and(|n| n.surface == "、") {
                        entry.0 += 1;
                    }
                }
            }
            // 次の形態素が文頭になるか。
            at_head = matches!(m.surface.as_str(), "。" | "！" | "？" | "!" | "?");
        }
    }

    // <strong>除外は位置ごとに掛ける。</strong> 文頭に 10 回、文中に 0 回の語彙素は、
    // 文中側の分母が 0 になる。
    let mut out = BTreeMap::new();
    for ((lemma, pos), (hit, total)) in tally {
        let name = format!("接続詞直後の読点・{lemma}・{}", pos.name());
        // <strong>1 度も現れないのと、現れたが足りないのを分ける。</strong> 前者は素材を足しても
        // 直るとは限らない——その語彙素をその位置で使わない書き手である。
        let m = match total {
            0 => Measured::NoDenominator,
            n if n < 10 => Measured::BelowFloor,
            _ =>
            {
                #[allow(clippy::cast_precision_loss)]
                Measured::Value(hit as f64 / total as f64)
            }
        };
        out.insert(name, m);
    }
    out
}

/// 語の文体値の表。<strong>外から与える。</strong>
///
/// 語彙素から (硬さ, 語り性) を引く。
pub trait StyleTable {
    /// 語彙素を引く。表に無ければ `None`。
    fn lookup(&self, lemma: &str) -> Option<(f64, f64)>;
    /// 表に載る語の値を昇順に並べた、10 等分位の境目。
    ///
    /// <strong>階級は表そのものから決める。</strong> 手で境目を置かない。
    fn deciles(&self) -> ([f64; 9], [f64; 9]);
}

/// 語の文体値の分布。
///
/// <strong>平均を取らない。</strong> 硬い語と軟らかい語を混ぜて書く人と、中庸な語だけで書く人が、
/// 同じ平均になる。<strong>分布として持つ。</strong>
///
/// 次元は 2 軸 × 各 10 階級 + 表に無い語の割合 = 21。
#[must_use]
pub fn word_style(a: &Analyzed, table: &dyn StyleTable) -> Option<Vec<f64>> {
    let (hard_cuts, narr_cuts) = table.deciles();
    let mut hard = [0usize; 10];
    let mut narr = [0usize; 10];
    let (mut found, mut total) = (0usize, 0usize);
    // <strong>同じ語彙素の重複は 1 回として数える。</strong>
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for m in a.all() {
        if !seen.insert(m.lemma.as_str()) {
            continue;
        }
        total += 1;
        if let Some((h, n)) = table.lookup(&m.lemma) {
            found += 1;
            hard[bucket(h, &hard_cuts)] += 1;
            narr[bucket(n, &narr_cuts)] += 1;
        }
    }
    // 表に載る語が 100 未満の文書では測らない。
    if found < 100 {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    let f = found as f64;
    let mut out: Vec<f64> = Vec::with_capacity(21);
    #[allow(clippy::cast_precision_loss)]
    out.extend(hard.iter().map(|&n| n as f64 / f));
    #[allow(clippy::cast_precision_loss)]
    out.extend(narr.iter().map(|&n| n as f64 / f));
    #[allow(clippy::cast_precision_loss)]
    out.push((total - found) as f64 / total.max(1) as f64);
    Some(out)
}

/// 値が何番目の階級に入るか。
fn bucket(v: f64, cuts: &[f64; 9]) -> usize {
    cuts.iter().filter(|&&c| v >= c).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::morph::stub::Stub;
    use kakiburi_doc::node::Kind;

    fn prose(texts: &[&str]) -> Vec<kakiburi_doc::prose::Segment> {
        texts
            .iter()
            .map(|t| kakiburi_doc::prose::Segment {
                kind: Kind::Paragraph,
                text: (*t).to_owned(),
            })
            .collect()
    }

    /// 解析し終えた形にする。<strong>体系の確かめはここで済む。</strong>
    fn analyzed(texts: &[&str]) -> Analyzed {
        Analyzed::of(&prose(texts), &Stub::unidic()).unwrap()
    }

    #[test]
    fn 別の体系では解析し終えた形が作れない() {
        // 黙って測れば、語彙素で引く指標が 0 件として静かに落ちる。
        // <strong>指標ごとに確かめるのではなく、作る道が 1 つだから書き忘れられない。</strong>
        let p = prose(&["これ は 大事 である 。"]);
        assert!(Analyzed::of(&p, &Stub::other()).is_err());
        assert!(Analyzed::of(&p, &Stub::unidic()).is_ok());
    }

    #[test]
    fn 機能語は表層形で数える() {
        // 「は」と「わ」を別に扱う。そこが人によって違う。
        let c = function_words(&analyzed(&["これ は 大事 で ある 。", "それ は 別 だ 。"]));
        assert_eq!(c.get("は"), Some(&2));
        assert_eq!(c.get("で"), Some(&1));
        assert_eq!(c.get("大事"), None, "名詞は入らない");
    }

    #[test]
    fn 品詞_bigram_は_node_を跨がない() {
        // 跨げば、見出しの末尾と次の段落の先頭の組ができる。
        assert_eq!(
            pos_bigrams(&analyzed(&["名詞 は"])).get("名詞+助詞"),
            Some(&1)
        );
        assert_eq!(
            pos_bigrams(&analyzed(&["名詞", "は"])).get("名詞+助詞"),
            None,
            "node を跨いだ組を作ってはいけない"
        );
    }

    #[test]
    fn 接続詞直後の読点は語彙素と位置で分ける() {
        // まとめると、規範で決まる分に薄まる。
        let mut text = String::new();
        // 文頭の「しかし」を 10 回、うち 10 回とも読点あり。
        for _ in 0..10 {
            text.push_str("しかし 、 そう だ 。 ");
        }
        let m = conjunction_comma(&analyzed(&[&text]));
        assert_eq!(
            m.get("接続詞直後の読点・しかし・文頭")
                .and_then(|x| x.value()),
            Some(1.0)
        );
    }

    #[test]
    fn 除外は位置ごとに掛かる() {
        // 文頭に 10 回、文中に 0 回の語彙素は、文中側の分母が 0 になる。
        let mut text = String::new();
        for _ in 0..10 {
            text.push_str("しかし 、 そう だ 。 ");
        }
        let m = conjunction_comma(&analyzed(&[&text]));
        assert!(m["接続詞直後の読点・しかし・文頭"].is_measured());
        assert_eq!(
            m["接続詞直後の読点・しかし・文中"],
            Measured::NoDenominator,
            "文中側は 1 度も現れないので分母が 0"
        );
    }

    #[test]
    fn 語彙素の一覧は固定である() {
        // コーパスから選ばない。書き手ごとに指標の名前が変わってはいけない。
        let empty = conjunction_comma(&analyzed(&[]));
        assert_eq!(empty.len(), 24, "12 語彙素 × 2 位置");
        for lemma in CONJUNCTIONS {
            assert!(empty.contains_key(&format!("接続詞直後の読点・{lemma}・文頭")));
            assert!(empty.contains_key(&format!("接続詞直後の読点・{lemma}・文中")));
        }
    }

    #[test]
    fn 一覧に無い接続詞は数えない() {
        let mut text = String::new();
        for _ in 0..10 {
            text.push_str("だが 、 そう だ 。 ");
        }
        let m = conjunction_comma(&analyzed(&[&text]));
        // 「だが」は一覧にある。
        assert!(m["接続詞直後の読点・だが・文頭"].is_measured());
        assert_eq!(m.len(), 24, "一覧の外を足さない");
    }

    #[test]
    fn 文頭は終端記号の直後である() {
        // 「そうだ。しかし、」の「しかし」は文頭。
        let mut text = String::new();
        for _ in 0..10 {
            text.push_str("そう だ 。 しかし 、 別 だ 。 ");
        }
        let m = conjunction_comma(&analyzed(&[&text]));
        assert!(
            m["接続詞直後の読点・しかし・文頭"].is_measured(),
            "文頭として数える"
        );
    }

    #[test]
    fn 延べ_1000_語に届かなければ測らない() {
        assert!(!analyzed(&["これ は 短い 。"]).enough_tokens());
        let long = "これ は 日本語 の 文章 で ある 。 ".repeat(200);
        assert!(analyzed(&[&long]).enough_tokens());
    }

    struct Table;
    impl StyleTable for Table {
        fn lookup(&self, lemma: &str) -> Option<(f64, f64)> {
            // 語の長さで決める、試験のための表。
            let n = lemma.chars().count();
            if n == 0 || lemma == "。" {
                return None;
            }
            #[allow(clippy::cast_precision_loss)]
            Some((n as f64, (10 - n.min(10)) as f64))
        }
        fn deciles(&self) -> ([f64; 9], [f64; 9]) {
            let cuts = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
            (cuts, cuts)
        }
    }

    #[test]
    fn 語の文体値は分布として持つ() {
        // 平均を取らない。混ぜて書く人と中庸な語だけの人が同じ平均になる。
        let mut text = String::new();
        for i in 0..150 {
            text.push_str(&format!("語{i} "));
        }
        let v = word_style(&analyzed(&[&text]), &Table).unwrap();
        assert_eq!(v.len(), 21, "2 軸 × 10 階級 + 表に無い語の割合");
    }

    #[test]
    fn 表に載る語が_100_未満なら測らない() {
        assert_eq!(word_style(&analyzed(&["これ は 短い 。"]), &Table), None);
    }

    #[test]
    fn 同じ語彙素は_1_回として数える() {
        let repeated = "同じ ".repeat(200);
        // 異なり語が 1 つしかないので、100 に届かない。
        assert_eq!(word_style(&analyzed(&[&repeated]), &Table), None);
    }

    #[test]
    fn 語を割る読点は付属語が続くものだけを候補にする() {
        // 名詞が続く並びは候補にしない。**絞らないと `あ、あと` で誤る。**
        let a = analyzed(&["あ 、 あと ちなみに"]);
        assert_eq!(splitting_commas(Some(&a)), Measured::Value(0.0));
    }

    #[test]
    fn 語を割る読点は解析器が無ければ測らない() {
        // **0 を返さない。** 0 は「数えて 0 だった」という値である。
        assert_eq!(splitting_commas(None), Measured::ToolMissing);
    }
}
