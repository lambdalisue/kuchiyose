//! コーパスから見つけた語。辞書に無い語が割れるのを直す。
//!
//! 形態素解析の辞書は、書き手の名前も、新しい製品名も、その分野の言い回しも
//! 知らない。知らない語は複数の語に割れる。
//!
//! 割れたままだと、その語のところで全部が狂う——機能語の分布も、品詞 bigram も、
//! [型](../../kakiburi-scale/src/assemble.rs)も、割れた欠片を数えることになる。
//!
//! しかも割れ方が揺れる。 実測で、ある書き手の名前は同じ綴りなのに 3 通りに
//! 割られていた——名詞＋名詞が 7 回、動詞＋動詞が 1 回、動詞＋固有名詞が 2 回。
//! だからコーパスの側で 1 つに固定する。
//!
//! 語は手で並べない。 道具は書き手も分野も選ばないので、特定の語を実装に持たない。

use std::collections::{BTreeMap, BTreeSet};

use crate::morph::Morpheme;

/// 連なりを語と認める、最小の出現回数。暫定値である。
pub const MIN_COUNT: usize = 3;

/// 後ろの語の直前が、その前の語である割合の下限。暫定値である。
///
/// 1 語なら、後ろの欠片は前の欠片の直後にしか現れない。
pub const BOUND: f64 = 0.9;

/// その出現で両方が名詞だった割合の下限。暫定値である。
///
/// 多数決にする。 揺れているからこそ辞書に載せるのであって、
/// 全部が名詞であることを求めれば、いちばん直したい語が落ちる。
pub const NOUN_SHARE: f64 = 0.5;

/// コーパスから見つけた語。
///
/// カセットが持つ。 素材から作るものなので、素材が変われば変わる——
/// [指紋](../../../docs/spec/200-extract.md#何で測ったかを指紋にする)に入る。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lexicon {
    words: BTreeSet<(String, String)>,
}

impl Lexicon {
    /// 素材の形態素列から語を見つける。
    ///
    /// > 隣り合う 2 形態素がその出現でおおむね名詞で、後ろの語がほぼ必ず
    /// > その前の語の直後にしか現れないなら、1 語である。
    ///
    /// 名詞に絞るのは、絞らないと助動詞の連なりが畳まれるからである——
    /// `て+いる` `と+いう` は正しく割れている。
    ///
    /// 日本語の字を含む連なりだけを見る。 英数字の連なりは
    /// [識別子を伏せる](kakiburi_doc::prose::mask_identifiers)側の仕事である。
    #[must_use]
    pub fn find(units: &[Vec<Vec<Morpheme>>]) -> Self {
        let mut alone: BTreeMap<&str, usize> = BTreeMap::new();
        let mut pair: BTreeMap<(&str, &str), usize> = BTreeMap::new();
        let mut noun: BTreeMap<(&str, &str), usize> = BTreeMap::new();
        for unit in units {
            for seg in unit {
                for (i, m) in seg.iter().enumerate() {
                    *alone.entry(m.surface.as_str()).or_default() += 1;
                    let Some(prev) = i.checked_sub(1).map(|j| &seg[j]) else {
                        continue;
                    };
                    let key = (prev.surface.as_str(), m.surface.as_str());
                    *pair.entry(key).or_default() += 1;
                    if prev.pos1 == "名詞" && m.pos1 == "名詞" {
                        *noun.entry(key).or_default() += 1;
                    }
                }
            }
        }
        let words = pair
            .into_iter()
            .filter(|&((a, b), n)| {
                #[allow(clippy::cast_precision_loss)]
                let n = n as f64;
                #[allow(clippy::cast_precision_loss)]
                let total = alone.get(b).copied().unwrap_or(0) as f64;
                #[allow(clippy::cast_precision_loss)]
                let nouns = noun.get(&(a, b)).copied().unwrap_or(0) as f64;
                n >= MIN_COUNT as f64
                    && nouns / n >= NOUN_SHARE
                    && total > 0.0
                    && n / total >= BOUND
                    && has_japanese(a)
                    && has_japanese(b)
            })
            .map(|((a, b), _)| (a.to_owned(), b.to_owned()))
            .collect();
        Self { words }
    }

    /// 保存した並びから組み立てる。
    #[must_use]
    pub fn from_pairs(pairs: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            words: pairs.into_iter().collect(),
        }
    }

    /// 持っている語。前の欠片と後ろの欠片の組。
    #[must_use]
    pub fn pairs(&self) -> Vec<(String, String)> {
        self.words.iter().cloned().collect()
    }

    /// 語の数。
    #[must_use]
    pub fn len(&self) -> usize {
        self.words.len()
    }

    /// 1 語も持たないか。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// 割れた語を畳む。
    ///
    /// 畳んだ語は名詞にする。 見つける条件が「おおむね名詞」なので、
    /// 揺れていた品詞をここで 1 つに決める。
    pub fn fold(&self, seg: &mut Vec<Morpheme>) {
        if self.words.is_empty() {
            return;
        }
        let mut out: Vec<Morpheme> = Vec::with_capacity(seg.len());
        for m in seg.drain(..) {
            let joins = out.last().is_some_and(|prev: &Morpheme| {
                self.words
                    .contains(&(prev.surface.clone(), m.surface.clone()))
            });
            if joins {
                let prev = out.last_mut().expect("直前がある");
                prev.surface.push_str(&m.surface);
                prev.lemma.push_str(&m.lemma);
                prev.pos1 = "名詞".to_owned();
                prev.pos2 = "普通名詞".to_owned();
                continue;
            }
            out.push(m);
        }
        *seg = out;
    }
}

/// 日本語の字を含むか。
fn has_japanese(s: &str) -> bool {
    s.chars().any(kakiburi_doc::text::is_japanese)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(surface: &str, pos1: &str) -> Morpheme {
        Morpheme {
            surface: surface.into(),
            lemma: surface.into(),
            pos1: pos1.into(),
            pos2: "普通名詞".into(),
        }
    }

    fn unit(words: &[(&str, &str)]) -> Vec<Vec<Morpheme>> {
        vec![words.iter().map(|(s, p)| m(s, p)).collect()]
    }

    #[test]
    fn 常に一緒に現れる名詞の連なりは_1_語になる() {
        let units: Vec<_> = (0..MIN_COUNT)
            .map(|_| unit(&[("あり", "名詞"), ("すえ", "名詞"), ("です", "助動詞")]))
            .collect();
        let lex = Lexicon::find(&units);
        assert_eq!(lex.pairs(), vec![("あり".to_owned(), "すえ".to_owned())]);
    }

    #[test]
    fn 割れ方が揺れていても多数決で拾う() {
        // 揺れているからこそ辞書に載せる。 全部が名詞であることを求めれば、
        // いちばん直したい語が落ちる。
        let mut units: Vec<_> = (0..MIN_COUNT)
            .map(|_| unit(&[("あり", "名詞"), ("すえ", "名詞")]))
            .collect();
        units.push(unit(&[("あり", "動詞"), ("すえ", "動詞")]));
        assert_eq!(Lexicon::find(&units).len(), 1);
    }

    #[test]
    fn 付属語の連なりは畳まない() {
        // `て+いる` は正しく割れている。
        let units: Vec<_> = (0..MIN_COUNT + 2)
            .map(|_| unit(&[("し", "動詞"), ("て", "助詞"), ("いる", "動詞")]))
            .collect();
        assert!(Lexicon::find(&units).is_empty());
    }

    #[test]
    fn 単独でも現れる語は畳まない() {
        // 後ろの欠片が別のところにも出るなら、1 語の一部ではない。
        let mut units: Vec<_> = (0..MIN_COUNT)
            .map(|_| unit(&[("文字", "名詞"), ("列", "名詞")]))
            .collect();
        units.extend((0..MIN_COUNT * 2).map(|_| unit(&[("行", "名詞"), ("列", "名詞")])));
        assert!(
            Lexicon::find(&units).is_empty(),
            "どちらの直前も 9 割に届かない"
        );
    }

    #[test]
    fn 英数字だけの連なりは畳まない() {
        // 識別子を伏せる側の仕事である。
        let units: Vec<_> = (0..MIN_COUNT)
            .map(|_| unit(&[("Hello", "名詞"), ("World", "名詞")]))
            .collect();
        assert!(Lexicon::find(&units).is_empty());
    }

    #[test]
    fn 畳んだ語は名詞になる() {
        let lex = Lexicon::from_pairs([("あり".to_owned(), "すえ".to_owned())]);
        let mut seg = vec![m("あり", "動詞"), m("すえ", "名詞"), m("です", "助動詞")];
        lex.fold(&mut seg);
        assert_eq!(seg.len(), 2);
        assert_eq!(seg[0].surface, "ありすえ");
        assert_eq!(seg[0].pos1, "名詞");
    }

    #[test]
    fn 辞書が空なら何も変わらない() {
        let mut seg = vec![m("あり", "名詞"), m("すえ", "名詞")];
        Lexicon::default().fold(&mut seg);
        assert_eq!(seg.len(), 2);
    }
}
