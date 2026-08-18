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
        let m = if total < 10 {
            Measured::BelowFloor
        } else {
            #[allow(clippy::cast_precision_loss)]
            Measured::Value(hit as f64 / total as f64)
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
            Measured::BelowFloor,
            "文中側は測れない"
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
}
