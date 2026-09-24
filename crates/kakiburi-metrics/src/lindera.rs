//! 形態素解析。辞書を同梱する。
//!
//! 利用者に何も用意させない。 外の実行ファイルも、手で落とす辞書も要らない
//! ——[道具の壊れが素材の不足という顔で出る](crate::morph)いちばんの原因が、
//! 環境に置いた依存そのものだった。
//!
//! 実測で、nix store から MeCab が消えた環境が「測れた単位が 0 本」と報告した。
//! 実行ファイルに畳んでしまえば、消えようがない。
//!
//! 辞書は UniDic である。[語彙素で引く](crate::word::goi)ので、体系を替えられない
//! ——IPADic には語彙素の欄が無い。

use std::borrow::Cow;
use std::sync::OnceLock;

use lindera::dictionary::{DictionaryKind, load_embedded_dictionary};
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;

use crate::morph::{Analyzer, Dictionary, Morpheme};

/// 同梱した辞書の名前。指紋に入る。
pub const DICT_NAME: &str = "UniDic";

/// 同梱した辞書の版。指紋に入る。
///
/// `lindera-unidic` が `unidic-mecab-2.1.2` を取ってくる。 版を上げたら
/// ここも上げる——上げなければ、辞書が変わったのに過去の値と比べられてしまう。
pub const DICT_VERSION: &str = "2.1.2";

/// 解析器の名前。指紋に入る。
///
/// MeCab とは別の実装である。 同じ辞書でも Viterbi の実装が違えば
/// 分かち書きが変わりうるので、名前で分ける——混ぜれば、実装を替えたことが
/// 指紋から読めない。
pub const ENGINE: &str = "Lindera";

/// 解析器の版。
pub const ENGINE_VERSION: &str = "6.0";

/// 語彙素が入っている欄。UniDic の 8 番目。
const LEMMA: usize = 7;

/// 同梱の辞書で動く解析器。
///
/// 辞書は 1 度だけ読む。 解析のたびに読めば、200 MB の表を何度も展開する
/// ——[測るのが高ければ周回数が減る](../../../docs/spec/100-metrics.md#測るのを安くする)。
pub struct Lindera {
    segmenter: &'static Segmenter,
}

impl Default for Lindera {
    fn default() -> Self {
        Self::new()
    }
}

impl Lindera {
    /// 同梱の辞書で組み立てる。
    ///
    /// 失敗しない。 辞書は実行ファイルの中にあるので、環境に左右されない
    /// ——読めないならそれは実行ファイルが壊れている。
    #[must_use]
    pub fn new() -> Self {
        static SEGMENTER: OnceLock<Segmenter> = OnceLock::new();
        let segmenter = SEGMENTER.get_or_init(|| {
            let dictionary = load_embedded_dictionary(DictionaryKind::UniDic)
                .expect("同梱の辞書が読める");
            Segmenter::new(Mode::Normal, dictionary, None)
        });
        Self { segmenter }
    }
}

impl Analyzer for Lindera {
    fn dictionary(&self) -> Dictionary {
        Dictionary::UnidicShort
    }

    fn dictionary_version(&self) -> (String, String) {
        (DICT_NAME.to_owned(), DICT_VERSION.to_owned())
    }

    fn analyze(&self, text: &str) -> Vec<Morpheme> {
        let Ok(mut tokens) = self.segmenter.segment(Cow::Borrowed(text)) else {
            // 呼べなかった。 形態素 0 なので除外に掛かる。
            return Vec::new();
        };
        tokens
            .iter_mut()
            .map(|t| {
                let surface = t.surface.to_string();
                let d = t.details();
                let at = |i: usize| d.get(i).copied().unwrap_or("*");
                // 辞書に無い語は欄が `*` で埋まる。 そのとき語彙素は表層形である。
                let lemma = {
                    let v = at(LEMMA);
                    if v.is_empty() || v == "*" {
                        surface.clone()
                    } else {
                        v.to_owned()
                    }
                };
                Morpheme {
                    lemma,
                    pos1: at(0).to_owned(),
                    pos2: at(1).to_owned(),
                    surface,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 同梱の辞書で動く() {
        // 環境に何も置かせない。 これが成り立たなければ、利用者は
        // また辞書を手で落とすことになる。
        let a = Lindera::new();
        let ms = a.analyze("これは、地味に便利なファイラーです。");
        assert!(!ms.is_empty(), "形態素が返る");
        assert_eq!(a.dictionary(), Dictionary::UnidicShort);
        assert_eq!(
            a.dictionary_version(),
            (DICT_NAME.to_owned(), DICT_VERSION.to_owned())
        );
    }

    #[test]
    fn 語彙素を返す() {
        // 語彙素で引くので、活用形では困る。「書い」は「書く」で引く。
        let ms = Lindera::new().analyze("書いてみました。");
        let lemma = |s: &str| {
            ms.iter()
                .find(|m| m.surface == s)
                .map(|m| m.lemma.clone())
                .unwrap_or_default()
        };
        assert_eq!(lemma("書い"), "書く");
        assert_eq!(lemma("まし"), "ます");
    }

    #[test]
    fn 形状詞を名詞に寄せない() {
        // 学校文法の「形容動詞」を名詞に寄せる体系だと、地味・静か・便利が
        // 語として見えなくなる。 UniDic はここを分けている。
        let ms = Lindera::new().analyze("地味に便利です。");
        let pos = |s: &str| {
            ms.iter()
                .find(|m| m.surface == s)
                .map(|m| m.pos1.clone())
                .unwrap_or_default()
        };
        assert_eq!(pos("地味"), "形状詞");
    }

    #[test]
    fn 辞書に無い語は表層形を語彙素にする() {
        // 欄が `*` で埋まるので、そのまま使うと語彙素が `*` になる。
        let ms = Lindera::new().analyze("ファイラー");
        assert!(ms.iter().all(|m| m.lemma != "*"), "{ms:?}");
    }

    #[test]
    fn 機能語を拾える() {
        // 日本語の個人性は助詞や助動詞に宿る。ここが取れなければ照合が成り立たない。
        let ms = Lindera::new().analyze("これは、そうだと思います。");
        assert!(ms.iter().any(|m| m.is_function_word()), "{ms:?}");
    }
}
