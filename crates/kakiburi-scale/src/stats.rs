//! 文書ごとの統計値。目盛りを組み立てる材料である。
//!
//! どれも、その文書 1 本だけから決まる値である（[文書ごとの統計値を持つ](../../../docs/spec/200-extract.md#文書ごとの統計値を持つ)）。
//! 比べる相手に依る値——語彙で切った頻度、z 得点、較正の重み——は持たない。
//! どれも組み立てるときに、2 つのカセットを見てから作る。
//!
//! 語のまとめ方と、言い回しの上限の数え方だけは、同じカセットの文書を全部見てから
//! 決まる。相手のカセットは見ない。
//!
//! 束ねた単位は、文書ごとの統計値から[組み立てる](compose)。本文を連結して
//! 測り直さない。
//!
//! | 統計値 | 束ねるとき |
//! | --- | --- |
//! | 系統の頻度、長さ、語彙素と一人称の数 | 足す。系統の下限は足した量で掛け直す |
//! | 指示できる指標 | 分子と分母を足してから割る。変動係数は値を並べ直す |
//! | 人らしさの窓 | 文書ごとの窓を並べる。窓は文書の境目をまたがない |
//! | 言い回し | 合併する。node の番号は束の中の通し番号へずらす |

use std::collections::{BTreeMap, BTreeSet};

use kakiburi_doc::prose::Segment;
use kakiburi_metrics::directive::{self, Counted};
use kakiburi_metrics::humanness::HumannessWindows;
use kakiburi_metrics::lexicon::Lexicon;
use kakiburi_metrics::matching::{self, Amounts, FOR_VERDICT};
use kakiburi_metrics::morph::{Analyzed, Analyzer};
use kakiburi_metrics::system::System;
use kakiburi_metrics::word::Counts;
use kakiburi_metrics::{Humanness, Unmeasured};

use crate::assemble::{lexicon_of, Measurements, Report, Sample, KATA_N};
use crate::examples::{self, ExampleTable};

/// 1 つの言い回しの、1 単位の中での出方。
#[derive(Debug, Clone, PartialEq)]
pub struct Phrase {
    /// 最初に現れた位置。node の番号 ÷ node の総数。
    ///
    /// 束ねた単位では、最初に現れた文書の中での位置である。 束ねたのは長さを
    /// 届かせるためで、2 本を 1 本の文書とみなすためではない。
    pub first: f64,
    /// 現れた回数。
    pub count: usize,
    /// 現れた node の番号。同じ node に現れる 2 つの型は[穴あきの型](crate::assemble::Kata::tail)になる。
    pub nodes: BTreeSet<usize>,
}

/// 言い回しの表。並び → 出方。
pub type Phrases = BTreeMap<String, Phrase>;

/// 言い回しの表を作る。ひらがなを含む 1〜4 [文節](kakiburi_metrics::word::grams_with_position)の並びだけを持つ。
///
/// ひらがなを 1 つも含まない並びは題材である。 付属語も活用もひらがなで
/// 書かれる——漢字とカタカナだけの並びは、言い方ではなく語そのものである。
/// [型](crate::assemble::Kata)はそれを見ないので、持っても使われない。
#[must_use]
pub fn phrases_in(analyzed: Option<&Analyzed>) -> Phrases {
    let mut out = Phrases::new();
    for (g, at, node) in kakiburi_metrics::word::grams_with_position(analyzed, &KATA_N) {
        if !g.chars().any(|c| ('\u{3041}'..='\u{309f}').contains(&c)) {
            continue;
        }
        let e = out.entry(g).or_insert_with(|| Phrase {
            first: at,
            count: 0,
            nodes: BTreeSet::new(),
        });
        e.count += 1;
        e.nodes.insert(node);
    }
    out
}

/// 1 文書の統計値。
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentStats {
    /// 単位の名前。カセットの中で一意である。
    pub name: String,
    /// 地の文の日本語の文字数。長さの範囲と束ね方に使う。
    pub chars: usize,
    /// 識別子を伏せたあとの、地の文の日本語の文字数。系統の下限に使う。
    pub masked_chars: usize,
    /// 読点の数。
    pub commas: usize,
    /// 延べ語数。解析器が無ければ `None`。
    pub tokens: Option<usize>,
    /// 地の文の node の数。束ねるとき、言い回しの node の番号をずらす。
    pub nodes: usize,
    /// 段落の[敬体率](kakiburi_metrics::structure::register_rates)。測れなければ `None`。
    ///
    /// 基準として使われるとき、文体で絞るのに使う。
    pub polite_share: Option<f64>,
    /// 2 文字以上の自立語。基準として使われるとき、題材で絞るのに使う。
    ///
    /// 語のまとめ方を掛けずに集める。 まとめ方はカセットごとに違うので、掛けると
    /// 同じ語が本人と基準で別の語になり、重なりが数えられない。
    pub content_words: BTreeSet<String>,
    /// 系統ごとの部分ベクトルの数え上げ。語彙で切らず、下限も掛けない。
    ///
    /// 数えられなかった系統（解析器が無い）は入っていない。
    pub systems: BTreeMap<System, Vec<Counts>>,
    /// 人らしさの、窓ごとの値。
    pub windows: HumannessWindows,
    /// 指示できる指標。値と、その元になった数。
    pub directives: Vec<(String, Counted)>,
    /// 言い回しの表。
    pub phrases: Phrases,
    /// 1 本の中で 2 回以上出た、長い言い回し。本人の言い回しを渡すのに使う。
    pub recurring: Vec<String>,
    /// カセットのどこかで[再来した言い回し](Self::recurring)が、この文書の地の文に
    /// 現れた回数。0 回のものは持たない。
    ///
    /// 言い回しを渡すなら上限も渡す。上限は本人の単位での 1,000 字あたりの最大で、
    /// 再来していない文書にも現れうるので、カセット全体の候補で数える。
    pub phrase_hits: BTreeMap<String, usize>,
    /// 形容詞・形状詞・副詞の語彙素と、品詞と回数。
    pub lexemes: BTreeMap<String, (String, usize)>,
    /// 一人称ごとの回数。
    pub first_person: BTreeMap<String, usize>,
    /// 書き出しの node の種類。
    pub opening: Option<String>,
    /// 読める系統の次元ごとの実例。
    pub examples: ExampleTable,
}

impl DocumentStats {
    /// 1 本を測る。系統・人らしさ・言い回しは識別子を伏せてから、`lexicon` で畳んで測る。
    ///
    /// 地の文をつないだものも返す。言い回しの上限を数えるのに要り、統計値には残さない。
    fn measure(
        sample: Sample<'_>,
        analyzer: Option<&dyn Analyzer>,
        lexicon: &Lexicon,
    ) -> (Self, String) {
        let prose = sample.document.prose();
        // 識別子を伏せてから測る。 掛けるのは照合と人らしさと型だけで、指示できる
        // 指標は和欧間スペースや半角英字そのものを測るので、生の文から測る。
        let masked = kakiburi_doc::prose::mask_identifiers(&prose);
        let analyzed = analyzer.and_then(|a| Analyzed::with_lexicon(&masked, a, lexicon).ok());
        let plain = analyzer.and_then(|a| Analyzed::with_lexicon(&prose, a, lexicon).ok());
        let mut systems = BTreeMap::new();
        for s in FOR_VERDICT {
            if let Some(p) = matching::raw_parts(s, &masked, analyzed.as_ref()) {
                systems.insert(s, p);
            }
        }
        let stats = Self {
            name: sample.name.to_owned(),
            chars: sample.document.japanese_chars(),
            masked_chars: kakiburi_doc::prose::japanese_chars(&masked),
            commas: matching::comma_count(&masked),
            tokens: analyzed.as_ref().map(Analyzed::tokens),
            nodes: masked.len(),
            polite_share: polite_share(&prose),
            content_words: content_words(&prose, analyzer),
            examples: examples::examples_in(sample.document, &masked, &systems),
            systems,
            windows: HumannessWindows::measure(&masked, analyzed.as_ref()),
            directives: directive::measure(sample.document, plain.as_ref()),
            phrases: phrases_in(analyzed.as_ref()),
            // 長いほうだけを取る。 短い言い回しは誰でも繰り返すので、
            // 渡しても癖にならない。
            recurring: kakiburi_metrics::humanness::recurring(
                analyzed.as_ref(),
                &kakiburi_metrics::humanness::LONG_N,
            ),
            phrase_hits: BTreeMap::new(),
            lexemes: kakiburi_metrics::word::goi(analyzed.as_ref()),
            first_person: kakiburi_metrics::word::first_person(analyzed.as_ref()),
            opening: sample.document.opening().map(|k| k.name().to_owned()),
        };
        (stats, kakiburi_metrics::humanness::joined(&masked))
    }

    /// 系統の下限を見る量。
    #[must_use]
    pub fn amounts(&self) -> Amounts {
        Amounts {
            japanese: self.masked_chars,
            commas: self.commas,
            tokens: self.tokens,
        }
    }

    /// 1 本で 1 単位としたときに、測れたかの内訳。
    #[must_use]
    pub fn report(&self) -> Report {
        compose(&[self]).report(&self.name)
    }

    /// 環境の側の理由で測れなかった指標と、その理由。
    ///
    /// コーパスの性質（下限未満・分母 0・書けない記法）は入れない。 道具が無い・
    /// 道具が失敗したは素材を足しても直らない。
    #[must_use]
    pub fn broken_environment(&self) -> Vec<(String, Unmeasured)> {
        let mut out: Vec<(String, Unmeasured)> = Vec::new();
        for (name, m) in Humanness::from_windows(&self.windows).flat() {
            if let Some(u) = m.unmeasured().filter(|u| !u.is_corpus()) {
                out.push((name, u));
            }
        }
        for (name, c) in &self.directives {
            if let Some(u) = c.measured().unmeasured().filter(|u| !u.is_corpus()) {
                out.push((name.clone(), u));
            }
        }
        out
    }
}

/// 段落の敬体率。
fn polite_share(prose: &[Segment]) -> Option<f64> {
    let name = format!("敬体率・{}", kakiburi_doc::node::Kind::Paragraph.name());
    kakiburi_metrics::structure::register_rates(prose, None)
        .into_iter()
        .find(|(n, _)| *n == name)
        .and_then(|(_, c)| c.value())
}

/// 2 文字以上の自立語。助詞や助動詞は誰が書いても同じで、題材を分けない。
fn content_words(prose: &[Segment], analyzer: Option<&dyn Analyzer>) -> BTreeSet<String> {
    let Some(a) = analyzer.and_then(|a| Analyzed::of(prose, a).ok()) else {
        return BTreeSet::new();
    };
    a.all()
        .filter(|m| !m.is_function_word())
        .filter(|m| m.surface.chars().count() >= 2)
        .map(|m| m.surface.clone())
        .collect()
}

/// 1 つのカセットの統計値。文書ごとの統計値と、語のまとめ方。
#[derive(Debug, Clone, PartialEq)]
pub struct CassetteStats {
    /// 文書ごとの統計値。単位名の昇順。
    pub documents: Vec<DocumentStats>,
    /// このカセットの文書から見つけた語のまとめ方。
    ///
    /// 本人の側として使われたとき、草稿もこれで測る。基準の側として使われたときは
    /// 読まない——基準の文書を測ったときに使ったきりである。
    pub lexicon: Lexicon,
}

/// 統計値を作れない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatsError {
    /// 同じ名前の単位が 2 つある。黙って上書きしない。
    DuplicateName(String),
    /// 単位の名前に制御文字がある。
    ///
    /// 組み立ては名前を 1 つの文字列として引き回し、役の前置と束の繋ぎに制御文字を
    /// 使う。 名前に混ざれば、別の単位と同じ名前を作れてしまう。
    ControlInName(String),
}

impl std::fmt::Display for StatsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StatsError::DuplicateName(n) => write!(f, "単位の名前が重なっている: `{n}`"),
            StatsError::ControlInName(n) => write!(f, "単位の名前に制御文字がある: {n:?}"),
        }
    }
}

impl std::error::Error for StatsError {}

/// 単位の名前を確かめる。 測って書くときも、カセットから読み戻すときも、これを通す。
///
/// # Errors
///
/// 名前に制御文字があれば断る。
pub fn check_name(name: &str) -> Result<(), StatsError> {
    if name.chars().any(char::is_control) {
        return Err(StatsError::ControlInName(name.to_owned()));
    }
    Ok(())
}

/// 単位の名前の並びを確かめる。1 つずつ[確かめ](check_name)、重なりも断る。
///
/// # Errors
///
/// 名前に制御文字があるか、同じ名前が 2 つあれば断る。
pub fn check_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<(), StatsError> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for name in names {
        check_name(name)?;
        if !seen.insert(name) {
            return Err(StatsError::DuplicateName(name.to_owned()));
        }
    }
    Ok(())
}

impl CassetteStats {
    /// 文書を測る。
    ///
    /// 2 度測る。 1 度目で語のまとめ方を見つけ、2 度目でその語を畳んで測る。
    /// まとめ方はこの文書たちだけから見つける。相手のカセットは見ない。
    ///
    /// # Errors
    ///
    /// 同じ名前の文書が 2 つあるか、名前に制御文字があれば断る。
    pub fn measure(
        samples: &[Sample<'_>],
        analyzer: Option<&dyn Analyzer>,
    ) -> Result<Self, StatsError> {
        Self::measure_with(samples, analyzer, lexicon_of(samples, analyzer))
    }

    /// 決まった語のまとめ方で測る。
    pub(crate) fn measure_with(
        samples: &[Sample<'_>],
        analyzer: Option<&dyn Analyzer>,
        lexicon: Lexicon,
    ) -> Result<Self, StatsError> {
        check_names(samples.iter().map(|s| s.name))?;
        let mut measured: Vec<(DocumentStats, String)> = samples
            .iter()
            .map(|s| DocumentStats::measure(*s, analyzer, &lexicon))
            .collect();
        let candidates: BTreeSet<String> = measured
            .iter()
            .flat_map(|(d, _)| d.recurring.iter().cloned())
            .collect();
        for (d, text) in &mut measured {
            d.phrase_hits = candidates
                .iter()
                .filter_map(|p| {
                    let n = text.matches(p.as_str()).count();
                    (n > 0).then(|| (p.clone(), n))
                })
                .collect();
        }
        let mut documents: Vec<DocumentStats> = measured.into_iter().map(|(d, _)| d).collect();
        documents.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Self { documents, lexicon })
    }

    /// 名前で引く。
    #[must_use]
    pub fn document(&self, name: &str) -> Option<&DocumentStats> {
        self.documents.iter().find(|d| d.name == name)
    }

    /// 言い回し → その言い回しを使う、日本語 1,000 字あたりの最大。
    ///
    /// [繰り返せと言うなら上限も言う](../../../docs/spec/300-revise.md#繰り返せと言うなら上限も言う)
    /// は、どの言い回しを訊かれるかが検めるまで決まらない。 だから
    /// [`PHRASE_UNITS`] 本以上の文書に出る言い回しを全部表にしておく。
    ///
    /// 1 本にしか出ない言い回しは持たない。 その記事の題材であって書きぶりではなく、
    /// 持っても上限がほぼ 0 で、持たないのと同じ結論になる。表に無い言い回しを草稿が
    /// 繰り返していれば、本人が使っていないことがそのまま指摘になる。
    #[must_use]
    pub fn phrase_ceilings(&self) -> BTreeMap<String, f64> {
        let mut seen: BTreeMap<&str, (usize, f64)> = BTreeMap::new();
        for d in self.documents.iter().filter(|d| d.chars > 0) {
            for (text, p) in &d.phrases {
                if text.chars().count() < PHRASE_CHARS {
                    continue;
                }
                #[allow(clippy::cast_precision_loss)]
                let rate = 1000.0 * p.count as f64 / d.chars as f64;
                let e = seen.entry(text).or_insert((0, 0.0));
                e.0 += 1;
                e.1 = e.1.max(rate);
            }
        }
        seen.into_iter()
            .filter(|(_, (units, _))| *units >= PHRASE_UNITS)
            .map(|(text, (_, ceiling))| (text.to_owned(), ceiling))
            .collect()
    }
}

/// 上限の表に載せる言い回しの最短の字数。草稿を見る側と同じ線である。
pub const PHRASE_CHARS: usize = 4;

/// 上限の表に載せるのに要る文書の数。
pub const PHRASE_UNITS: usize = 2;

/// 文書 1 本の統計値を決める設定のうち、目盛りの側が持つもの。指紋の材料である。
///
/// 残りは[指標の側](kakiburi_metrics::measurement_settings)が持つ。
#[must_use]
pub fn measurement_settings() -> Vec<(&'static str, String)> {
    let list = |v: &[usize]| {
        v.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    vec![
        ("scale::stats::KATA_N", list(&KATA_N)),
        (
            "scale::examples::EXAMPLE_SYSTEMS",
            examples::EXAMPLE_SYSTEMS
                .iter()
                .map(|s| s.name())
                .collect::<Vec<_>>()
                .join(","),
        ),
        ("scale::examples::WANT", examples::WANT.to_string()),
        ("scale::examples::AROUND", examples::AROUND.to_string()),
    ]
}

/// 束の中の名前を組み立ての中で繋ぐ文字。
///
/// 単位の名前は[制御文字を持てない](check_name)ので、束の名前が 1 本の文書の名前と
/// 重ならない。 `+` で繋げば、`a` と `b` の束が `a+b` という文書と重なる。
const BUNDLE_JOINER: char = '\u{1f}';

/// 束ねた単位を組み立ての中で指す名前。見せるときは[表示の名前](display_name)にする。
pub(crate) fn bundle_key(docs: &[&DocumentStats]) -> String {
    docs.iter()
        .map(|d| d.name.as_str())
        .collect::<Vec<_>>()
        .join(&BUNDLE_JOINER.to_string())
}

/// 組み立ての中の名前を、見せる名前にする。束の繋ぎを `+` にする。
pub(crate) fn display_name(key: &str) -> String {
    key.replace(BUNDLE_JOINER, "+")
}

/// 束ねた単位の見せる名前。束ねた文書の名前を束ねた順に `+` で繋ぐ。
///
/// 見せるためだけのもので、単位を引くのには使わない。`a+b` という文書と重なりうる。
#[must_use]
pub fn bundle_name(docs: &[&DocumentStats]) -> String {
    display_name(&bundle_key(docs))
}

/// 何本かを 1 単位に束ねて、測り終えた形にする。1 本なら、その文書を測ったものと一致する。
pub(crate) fn compose(docs: &[&DocumentStats]) -> Measurements {
    let amounts = Amounts::sum(&docs.iter().map(|d| d.amounts()).collect::<Vec<_>>());
    let mut parts = BTreeMap::new();
    for s in FOR_VERDICT {
        let raws: Option<Vec<&Vec<Counts>>> = docs.iter().map(|d| d.systems.get(&s)).collect();
        let Some(raws) = raws else {
            continue;
        };
        if let Some(p) = matching::floored(s, sum_counts(&raws), &amounts) {
            parts.insert(s, p);
        }
    }
    let windows: Vec<&HumannessWindows> = docs.iter().map(|d| &d.windows).collect();
    let mut phrases = Phrases::new();
    let mut offset = 0usize;
    for d in docs {
        for (g, p) in &d.phrases {
            let e = phrases.entry(g.clone()).or_insert_with(|| Phrase {
                first: p.first,
                count: 0,
                nodes: BTreeSet::new(),
            });
            e.count += p.count;
            e.nodes.extend(p.nodes.iter().map(|n| n + offset));
        }
        offset += d.nodes;
    }
    let mut lexemes: BTreeMap<String, (String, usize)> = BTreeMap::new();
    let mut first_person: BTreeMap<String, usize> = BTreeMap::new();
    let mut phrase_hits: BTreeMap<String, usize> = BTreeMap::new();
    let mut recurring: BTreeSet<String> = BTreeSet::new();
    for d in docs {
        for (lemma, (pos, n)) in &d.lexemes {
            lexemes
                .entry(lemma.clone())
                .or_insert_with(|| (pos.clone(), 0))
                .1 += n;
        }
        for (name, n) in &d.first_person {
            *first_person.entry(name.clone()).or_default() += n;
        }
        for (p, n) in &d.phrase_hits {
            *phrase_hits.entry(p.clone()).or_default() += n;
        }
        recurring.extend(d.recurring.iter().cloned());
    }
    Measurements {
        parts,
        recurring: match docs {
            [one] => one.recurring.clone(),
            _ => recurring.into_iter().collect(),
        },
        overused: Vec::new(),
        once_only: Vec::new(),
        phrases,
        goi: lexemes,
        first_person,
        opening: docs.first().and_then(|d| d.opening.clone()),
        humanness: Humanness::from_windows(&HumannessWindows::concat(&windows)),
        chars: docs.iter().map(|d| d.chars).sum(),
        phrase_hits,
        examples: {
            let tables: Vec<&ExampleTable> = docs.iter().map(|d| &d.examples).collect();
            let keys: BTreeSet<&(String, String)> = tables.iter().flat_map(|t| t.keys()).collect();
            keys.into_iter()
                .map(|k| (k.clone(), examples::merge(&k.0, &k.1, &tables)))
                .collect()
        },
    }
}

/// 数え上げを部分ベクトルごとに足す。
fn sum_counts(raws: &[&Vec<Counts>]) -> Vec<Counts> {
    let mut out: Vec<Counts> = Vec::new();
    for raw in raws {
        if out.is_empty() {
            out = (*raw).clone();
            continue;
        }
        for (acc, part) in out.iter_mut().zip(raw.iter()) {
            for (k, v) in part {
                *acc.entry(k.clone()).or_default() += v;
            }
        }
    }
    out
}

/// 束ねた単位の、指示できる指標。名前ごとに数を足してから値にする。
#[must_use]
pub fn compose_directives(docs: &[&DocumentStats]) -> Vec<(String, Counted)> {
    let Some(first) = docs.first() else {
        return Vec::new();
    };
    first
        .directives
        .iter()
        .map(|(name, _)| {
            let all: Vec<&Counted> = docs
                .iter()
                .filter_map(|d| d.directives.iter().find(|(n, _)| n == name).map(|(_, c)| c))
                .collect();
            let c = if all.len() == docs.len() {
                Counted::compose(&all)
            } else {
                // 同じ道具で測れば名前は揃う。揃わないのは、違う道具で測った文書を混ぜたときである。
                Counted::unmeasured(Unmeasured::ToolFailed)
            };
            (name.clone(), c)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kakiburi_doc::Document;
    use kakiburi_metrics::Measured;

    use crate::testing::{document, rich, Chars};

    fn a() -> Option<&'static dyn Analyzer> {
        Some(&Chars)
    }

    /// 名前を付けた文書を、決まった語のまとめ方で測る。
    fn measured(docs: &[(&str, &Document)], lexicon: &Lexicon) -> CassetteStats {
        let samples: Vec<Sample<'_>> = docs
            .iter()
            .map(|(name, document)| Sample { name, document })
            .collect();
        CassetteStats::measure_with(&samples, a(), lexicon.clone()).expect("測れる")
    }

    fn lexicon_for(docs: &[&Document]) -> Lexicon {
        let samples: Vec<Sample<'_>> = docs
            .iter()
            .map(|document| Sample {
                name: "x",
                document,
            })
            .collect();
        lexicon_of(&samples, a())
    }

    /// 2 本を 1 本に繋いだ文書。文書の境界は node の境界として残る。
    fn joined(x: &Document, y: &Document) -> Document {
        Document::new(x.nodes.iter().chain(&y.nodes).cloned().collect())
    }

    #[test]
    fn 一文書の統計値から組み立てた値は直接測った値と一致する() {
        // 統計値が測った値を運べていなければ、目盛りは本文から作ったものと違う。
        for doc in [rich(3), document(4, false), document(2, true)] {
            let lexicon = lexicon_for(&[&doc]);
            let stats = measured(&[("u", &doc)], &lexicon);
            let got = compose(&[&stats.documents[0]]);
            let want = Measurements::of(
                Sample {
                    name: "u",
                    document: &doc,
                },
                a(),
                &lexicon,
            );
            assert_eq!(got.parts, want.parts, "系統");
            assert_eq!(got.humanness, want.humanness, "人らしさ");
            assert_eq!(got.phrases, want.phrases, "言い回し");
            assert_eq!(got.recurring, want.recurring, "再来した言い回し");
            assert_eq!(got.goi, want.goi, "語彙素");
            assert_eq!(got.first_person, want.first_person, "一人称");
            assert_eq!(got.opening, want.opening, "書き出し");
            assert_eq!(got.chars, want.chars, "長さ");

            let plain = Analyzed::with_lexicon(&doc.prose(), &Chars, &lexicon).ok();
            let direct: Vec<(String, Measured)> = directive::measure(&doc, plain.as_ref())
                .into_iter()
                .map(|(n, c)| (n, c.measured()))
                .collect();
            let composed: Vec<(String, Measured)> = compose_directives(&[&stats.documents[0]])
                .into_iter()
                .map(|(n, c)| (n, c.measured()))
                .collect();
            assert_eq!(composed, direct, "指示できる指標");
        }
    }

    #[test]
    fn 構造の文書では一人称と語彙素と指示できる指標が値を持つ() {
        // 一致の試験が空どうしの比較にならないように、材料が揃っていることを見る。
        let doc = rich(3);
        let stats = measured(&[("u", &doc)], &Lexicon::default());
        let d = &stats.documents[0];
        assert!(d.first_person.contains_key("僕"), "{:?}", d.first_person);
        assert!(d.lexemes.contains_key("静か"), "{:?}", d.lexemes);
        let value = |name: &str| {
            d.directives
                .iter()
                .find(|(n, _)| n == name)
                .and_then(|(_, c)| c.value())
        };
        for name in [
            "深い見出し",
            "節の長さの変動係数",
            "箇条書き項目長の変動係数",
            "段落長の変動係数",
        ] {
            assert!(value(name).is_some(), "{name}");
        }
    }

    #[test]
    fn 束ねた単位の値は文書ごとの統計値の足し合わせと一致する() {
        // 足し合わせで作れる値は、繋いだ本文を測ったものとビットまで一致する。
        let (x, y) = (rich(1), rich(2));
        let xy = joined(&x, &y);
        let lexicon = lexicon_for(&[&x, &y]);
        let parts = measured(&[("x", &x), ("y", &y)], &lexicon);
        let whole = measured(&[("xy", &xy)], &lexicon);
        let docs: Vec<&DocumentStats> = parts.documents.iter().collect();
        let got = compose(&docs);
        let want = compose(&[&whole.documents[0]]);
        assert_eq!(got.parts, want.parts, "系統の頻度");
        assert_eq!(got.chars, want.chars, "長さ");
        assert_eq!(got.goi, want.goi, "語彙素");
        assert_eq!(got.first_person, want.first_person, "一人称");
        assert_eq!(got.opening, want.opening, "書き出しは先頭の文書のもの");

        let value = |v: &[(String, Counted)], name: &str| -> Measured {
            v.iter()
                .find(|(n, _)| n == name)
                .map(|(_, c)| c.measured())
                .expect("名前がある")
        };
        let (bundled, concatenated) = (
            compose_directives(&docs),
            compose_directives(&[&whole.documents[0]]),
        );
        for (name, c) in &concatenated {
            // 見出しの深さと節の切れ目は文書の中で決まる。 連結すると 2 本目の見出しの
            // 深さや 1 本目の最後の節の長さが変わるので、この 2 つだけは一致しない。
            if name == "深い見出し" || name == "節の長さの変動係数" {
                continue;
            }
            assert_eq!(value(&bundled, name), c.measured(), "{name}");
        }
    }

    #[test]
    fn 深い見出しと節の長さの変動係数は文書ごとに数えて足す() {
        // 文書の境界を node の境界として残す決まりに合うのは、足し合わせのほうである。
        let (x, y) = (rich(1), rich(2));
        let stats = measured(&[("x", &x), ("y", &y)], &Lexicon::default());
        let docs: Vec<&DocumentStats> = stats.documents.iter().collect();
        let bundled = compose_directives(&docs);
        let find = |v: &[(String, Counted)], name: &str| -> Counted {
            v.iter()
                .find(|(n, _)| n == name)
                .map(|(_, c)| c.clone())
                .unwrap()
        };
        for name in ["深い見出し", "節の長さの変動係数"] {
            let each: Vec<Counted> = docs.iter().map(|d| find(&d.directives, name)).collect();
            let want = Counted::compose(&each.iter().collect::<Vec<_>>());
            assert_eq!(find(&bundled, name), want, "{name}");
        }
        let deep = |d: &Document| {
            d.nodes
                .iter()
                .filter(|n| d.heading_depth(n).is_some_and(|x| x >= 3))
                .count()
        };
        assert_eq!(
            find(&bundled, "深い見出し").parts(),
            Some(&directive::Parts::Ratio {
                num: deep(&x) + deep(&y),
                den: x.japanese_chars() + y.japanese_chars(),
                per: 1000,
                floor: kakiburi_metrics::floor::JAPANESE_CHARS,
                zero_is_absent: false,
            }),
            "文書ごとの深い見出しの数を足し、字数を足して割る"
        );
    }

    #[test]
    fn 束ねた単位の人らしさの窓は文書の境目をまたがない() {
        let (x, y) = (document(0, true), document(1, true));
        let lexicon = lexicon_for(&[&x, &y]);
        let stats = measured(&[("x", &x), ("y", &y)], &lexicon);
        let docs: Vec<&DocumentStats> = stats.documents.iter().collect();
        let want = Humanness::from_windows(&HumannessWindows::concat(&[
            &docs[0].windows,
            &docs[1].windows,
        ]));
        assert_eq!(compose(&docs).humanness, want, "文書ごとの窓を並べる");

        let whole = measured(&[("xy", &joined(&x, &y))], &lexicon);
        assert_ne!(
            compose(&docs).humanness,
            compose(&[&whole.documents[0]]).humanness,
            "繋いで測れば、窓が文書をまたぐ"
        );
    }

    #[test]
    fn 束ねた言い回しは合併し_node_の番号をずらす() {
        let (x, y) = (rich(1), rich(2));
        let stats = measured(&[("x", &x), ("y", &y)], &Lexicon::default());
        let docs: Vec<&DocumentStats> = stats.documents.iter().collect();
        let got = compose(&docs).phrases;
        let shift = docs[0].nodes;
        for (g, p) in &docs[1].phrases {
            let merged = &got[g];
            assert!(
                p.nodes.iter().all(|n| merged.nodes.contains(&(n + shift))),
                "{g}"
            );
            let before = docs[0].phrases.get(g).map_or(0, |q| q.count);
            assert_eq!(merged.count, before + p.count, "{g}");
        }
    }

    #[test]
    fn 言い回しの上限はカセット全体で再来したものを数える() {
        // 再来していない文書にも現れる。 そこを数えなければ上限が低く出る。
        let x = rich(1);
        let stats = measured(
            &[("x", &x), ("y", &document(3, false))],
            &Lexicon::default(),
        );
        let candidates: BTreeSet<&String> =
            stats.documents.iter().flat_map(|d| &d.recurring).collect();
        for d in &stats.documents {
            for p in d.phrase_hits.keys() {
                assert!(candidates.contains(p), "{p}");
            }
        }
        assert!(stats.documents.iter().all(|d| !d.phrase_hits.is_empty()));
    }

    #[test]
    fn 同じ名前の文書は断る() {
        let doc = document(0, false);
        let samples = [
            Sample {
                name: "同じ",
                document: &doc,
            },
            Sample {
                name: "同じ",
                document: &doc,
            },
        ];
        assert_eq!(
            CassetteStats::measure(&samples, a()),
            Err(StatsError::DuplicateName("同じ".to_owned()))
        );
    }

    #[test]
    fn 名前に制御文字がある文書は断る() {
        // 組み立ては制御文字で役を分け、束を繋ぐ。 名前に混ざれば別の単位と同じ名前になりうる。
        let doc = document(0, false);
        for bad in ["\u{0}u00", "改\n行", "タブ\t", "a\u{1f}b"] {
            let samples = [Sample {
                name: bad,
                document: &doc,
            }];
            assert_eq!(
                CassetteStats::measure(&samples, a()),
                Err(StatsError::ControlInName(bad.to_owned())),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn 束の内側の名前は表示の名前と分かれる() {
        // `a` と `b` の束は、`a+b` という名前の文書と重ならない。 表示は `+` で繋ぐ。
        let doc = document(0, false);
        let lexicon = lexicon_for(&[&doc]);
        let stats = measured(&[("a", &doc), ("b", &doc), ("a+b", &doc)], &lexicon);
        let a = stats.document("a").expect("a");
        let b = stats.document("b").expect("b");
        let ab = stats.document("a+b").expect("a+b");
        assert_ne!(bundle_key(&[a, b]), bundle_key(&[ab]));
        assert_eq!(bundle_name(&[a, b]), "a+b");
        assert_eq!(display_name(&bundle_key(&[a, b])), "a+b");
    }

    #[test]
    fn 語のまとめ方はそのカセットの文書から見つける() {
        let person: Vec<(String, Document)> = (0..4)
            .map(|i| (format!("p{i}"), document(i, false)))
            .collect();
        let samples: Vec<Sample<'_>> = person
            .iter()
            .map(|(name, document)| Sample { name, document })
            .collect();
        let stats = CassetteStats::measure(&samples, a()).expect("測れる");
        assert_eq!(stats.lexicon, lexicon_of(&samples, a()));
        assert!(
            !stats.lexicon.is_empty(),
            "作り物の文は繰り返しが強いので語が見つかる"
        );
    }

    #[test]
    fn 文書は単位名の昇順に並ぶ() {
        let (x, y) = (document(0, false), document(1, false));
        let stats = measured(&[("b", &x), ("a", &y)], &Lexicon::default());
        let names: Vec<&str> = stats.documents.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn 題材で選ぶ語はまとめ方を掛けずに集める() {
        // まとめ方はカセットごとに違う。 掛ければ同じ語が本人と基準で別の語になる。
        let doc = document(0, false);
        let lexicon = lexicon_for(&[&doc]);
        assert!(!lexicon.is_empty());
        let folded: BTreeSet<String> = Analyzed::with_lexicon(&doc.prose(), &Chars, &lexicon)
            .unwrap()
            .all()
            .filter(|m| !m.is_function_word() && m.surface.chars().count() >= 2)
            .map(|m| m.surface.clone())
            .collect();
        assert!(!folded.is_empty(), "畳めば 2 字以上の語ができる");
        let with = measured(&[("u", &doc)], &lexicon);
        assert!(
            with.documents[0].content_words.is_disjoint(&folded),
            "畳んだ語は題材の語に入らない"
        );
    }

    fn phrase(count: usize) -> Phrase {
        Phrase {
            first: 0.0,
            count,
            nodes: BTreeSet::from([0]),
        }
    }

    #[test]
    fn 言い回しの上限は_2_本以上に出るものを_1000_字あたりの最大で持つ() {
        let mut stats = measured(
            &[("a", &document(0, false)), ("b", &document(1, false))],
            &Lexicon::default(),
        );
        stats.documents[0].chars = 2000;
        stats.documents[0].phrases = [
            ("と思います。".to_owned(), phrase(4)),
            ("かもしれない".to_owned(), phrase(9)),
            ("です。".to_owned(), phrase(9)),
        ]
        .into();
        stats.documents[1].chars = 1000;
        stats.documents[1].phrases = [
            ("と思います。".to_owned(), phrase(1)),
            ("です。".to_owned(), phrase(9)),
        ]
        .into();

        let got = stats.phrase_ceilings();

        assert_eq!(
            got.get("と思います。"),
            Some(&2.0),
            "2000 字に 4 回と 1000 字に 1 回"
        );
        assert_eq!(got.get("かもしれない"), None, "1 本にしか出ない");
        assert_eq!(got.get("です。"), None, "{PHRASE_CHARS} 字に届かない");
    }
}
