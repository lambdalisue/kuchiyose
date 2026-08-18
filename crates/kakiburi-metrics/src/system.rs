//! 系統と層。
//!
//! <strong>層は系統の属性である。</strong> 指標は自分の系統を名指しし、層はそこから引く。
//! 50 を超える定義ファイルに層を書き写せば、系統の層が変わったときに直し忘れた
//! ものが古い層のまま残る。

/// 照合の系統。
///
/// <strong>層はここで決まる。</strong> 表を 1 つにするために、層を返す関数もここに置く。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum System {
    /// 文字 bigram。
    CharBigram,
    /// 機能語。助詞・助動詞・接続詞・副詞・感動詞。
    FunctionWord,
    /// 品詞 bigram。
    PosBigram,
    /// 読点の打ち方。
    Comma,
    /// 文字種。
    CharType,
    /// 文節パターン。<strong>保留中。</strong>
    BunsetsuPattern,
    /// 埋め込み。<strong>定義が無く、当面使わない。</strong>
    Embedding,
    /// 文末表現。
    SentenceEnding,
    /// 語の文体値。
    WordStyle,
    /// 表記の選択。
    Orthography,
    /// 構造。見出し・箇条書き・表・引用などの使い方。
    Structure,
    /// 長さ。段落・節・項目の長さとそのばらつき。
    Length,
}

/// 層。<strong>裏付けの強さの段である。</strong>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Layer {
    /// 日本語の書き手識別で確かめられているもの。
    One,
    /// 別の目的の研究から取るもの。系統は名指しできる。
    Two,
    /// 系統に遡れないもの。測ってよいが、指摘にも判定にも使わない。
    Three,
}

impl System {
    /// 札に書く名前。
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            System::CharBigram => "文字bigram",
            System::FunctionWord => "機能語",
            System::PosBigram => "品詞bigram",
            System::Comma => "読点の打ち方",
            System::CharType => "文字種",
            System::BunsetsuPattern => "文節パターン",
            System::Embedding => "埋め込み",
            System::SentenceEnding => "文末表現",
            System::WordStyle => "語の文体値",
            System::Orthography => "表記",
            System::Structure => "構造",
            System::Length => "長さ",
        }
    }

    /// 名前から引く。表に無い名前は受けない——綴りが違えば層が引けない。
    #[must_use]
    pub fn from_name(name: impl AsRef<str>) -> Option<Self> {
        // 札には「文字 bigram」と空白入りで書かれることがある。空白は記法で
        // あって名前の一部ではない。
        let n = name.as_ref().replace([' ', '\u{3000}'], "");
        [
            System::CharBigram,
            System::FunctionWord,
            System::PosBigram,
            System::Comma,
            System::CharType,
            System::BunsetsuPattern,
            System::Embedding,
            System::SentenceEnding,
            System::WordStyle,
            System::Orthography,
            System::Structure,
            System::Length,
        ]
        .into_iter()
        .find(|s| s.name() == n)
    }

    /// この系統の層。<strong>ここが唯一の出どころである。</strong>
    #[must_use]
    pub fn layer(self) -> Layer {
        match self {
            // 層 1——日本語の書き手識別で確かめられているもの。
            System::CharBigram
            | System::FunctionWord
            | System::PosBigram
            | System::Comma
            | System::CharType
            | System::BunsetsuPattern
            | System::Embedding => Layer::One,
            // 層 2——別の目的の研究から取るもの。
            System::SentenceEnding
            | System::WordStyle
            | System::Orthography
            | System::Structure
            | System::Length => Layer::Two,
        }
    }

    /// 保留中か。集合にはあるが、判定に使わない。
    #[must_use]
    pub fn on_hold(self) -> bool {
        matches!(self, System::BunsetsuPattern | System::Embedding)
    }

    /// 判定に使える系統か。
    ///
    /// [層 1](Layer::One) だけから取る。保留中のものは外す。
    #[must_use]
    pub fn usable_for_verdict(self) -> bool {
        self.layer() == Layer::One && !self.on_hold()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [System; 12] = [
        System::CharBigram,
        System::FunctionWord,
        System::PosBigram,
        System::Comma,
        System::CharType,
        System::BunsetsuPattern,
        System::Embedding,
        System::SentenceEnding,
        System::WordStyle,
        System::Orthography,
        System::Structure,
        System::Length,
    ];

    #[test]
    fn 名前は往復する() {
        for s in ALL {
            assert_eq!(System::from_name(s.name()), Some(s), "{s:?}");
        }
    }

    #[test]
    fn 札の空白は名前の一部ではない() {
        assert_eq!(System::from_name("文字 bigram"), Some(System::CharBigram));
        assert_eq!(System::from_name("品詞 bigram"), Some(System::PosBigram));
    }

    #[test]
    fn 表に無い名前は受けない() {
        // 綴りが違えば層が引けない。黙って通さない。
        assert_eq!(System::from_name("文字ngram"), None);
        assert_eq!(System::from_name("読点"), None);
    }

    #[test]
    fn 層_1_は日本語の書き手識別で確かめられた_5_つと保留_2_つ() {
        for s in [
            System::CharBigram,
            System::FunctionWord,
            System::PosBigram,
            System::Comma,
            System::CharType,
        ] {
            assert_eq!(s.layer(), Layer::One, "{s:?}");
            assert!(s.usable_for_verdict(), "{s:?} は判定に使える");
        }
    }

    #[test]
    fn 保留中の系統は層_1_でも判定に使わない() {
        for s in [System::BunsetsuPattern, System::Embedding] {
            assert_eq!(s.layer(), Layer::One, "{s:?}");
            assert!(s.on_hold(), "{s:?} は保留中");
            assert!(!s.usable_for_verdict(), "{s:?} を判定に使ってはいけない");
        }
    }

    #[test]
    fn 層_2_は判定に使わない() {
        for s in [
            System::SentenceEnding,
            System::WordStyle,
            System::Orthography,
            System::Structure,
            System::Length,
        ] {
            assert_eq!(s.layer(), Layer::Two, "{s:?}");
            assert!(!s.usable_for_verdict(), "{s:?} を判定に使ってはいけない");
        }
    }

    #[test]
    fn 層は系統から引くしかない() {
        // 層を持つ道が 1 つであることを型で確かめる。
        // System::layer 以外に層を返す経路が無いので、書き写せない。
        let counts = ALL.iter().filter(|s| s.layer() == Layer::One).count();
        assert_eq!(counts, 7, "層 1 は 7 系統");
    }
}
