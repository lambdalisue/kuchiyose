//! 形態素解析の口。
//!
//! 辞書を選ぶことは、品詞の体系を選ぶことである。 記録すれば済む話ではない——
//! どの語が接続詞かが辞書で変わり、[接続詞直後の読点](../../../docs/spec/metrics/接続詞直後の読点.md)の
//! 次元が消える。
//!
//! そして外の表を語彙素で引く指標がある。 別の体系で解析すれば鍵が合わず、
//! 0 件として静かに落ちる。
//!
//! だからここは解析器を差し替えられる口にし、UniDic 以外は断る。

/// 形態素。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Morpheme {
    /// 表層形。機能語はこれで数える——「は」と「わ」、「けれど」と「けど」を分ける。
    pub surface: String,
    /// 語彙素。外の表を引く鍵。
    pub lemma: String,
    /// 品詞の第 1 層。名詞・動詞・助詞……
    pub pos1: String,
    /// 品詞の第 2 層。第 1 層で始めるのは暫定なので、持っておく。
    pub pos2: String,
}

impl Morpheme {
    /// 機能語か。助詞・助動詞・接続詞・副詞・感動詞。
    #[must_use]
    pub fn is_function_word(&self) -> bool {
        matches!(
            self.pos1.as_str(),
            "助詞" | "助動詞" | "接続詞" | "副詞" | "感動詞"
        )
    }

    /// 接続詞か。
    #[must_use]
    pub fn is_conjunction(&self) -> bool {
        self.pos1 == "接続詞"
    }

    /// 前の語に付く語か。助詞と助動詞。
    ///
    /// 単独で文節を始められない。 だから読点の直後にこれが来ていたら、
    /// [語を割っているかもしれない](splitting_commas)。
    #[must_use]
    pub fn is_clinging(&self) -> bool {
        matches!(self.pos1.as_str(), "助詞" | "助動詞")
    }
}

/// 辞書の体系。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dictionary {
    /// UniDic の短単位。仕様が要求する体系。
    UnidicShort,
    /// それ以外。断る。
    Other,
}

/// 解析器。
///
/// 体系を名乗らせる。 名乗らないものは通さない——黙って別の体系で測れば、
/// 語彙素で引く指標が 0 件として静かに落ちる。
pub trait Analyzer {
    /// この解析器の辞書の体系。
    fn dictionary(&self) -> Dictionary;
    /// 辞書の名前と版。指紋に入る。
    fn dictionary_version(&self) -> (String, String);
    /// 1 本の文字列を解析する。
    fn analyze(&self, text: &str) -> Vec<Morpheme>;

    /// まとめて解析する。
    ///
    /// 外の実行ファイルを呼ぶ解析器は、ここをまとめて速くする。 node ごとに
    /// 起こせば、200 MB の辞書を node の数だけ読み直すことになる——
    /// [測るのが高ければ周回数が減る](../../../docs/spec/100-metrics.md#測るのを安くする)。
    ///
    /// 返す並びは渡した並びと同じでなければならない。 ずれれば、node を跨がない
    /// はずの指標が別の node の形態素を数える。
    fn analyze_all(&self, texts: &[&str]) -> Vec<Vec<Morpheme>> {
        texts.iter().map(|t| self.analyze(t)).collect()
    }
}

/// 解析できない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MorphError {
    /// 辞書の体系が違う。
    WrongDictionary {
        /// 名乗った名前。
        name: String,
        /// その版。
        version: String,
    },
}

impl std::fmt::Display for MorphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MorphError::WrongDictionary { name, version } => write!(
                f,
                "辞書が UniDic の短単位ではない: {name} {version}。\
                 語彙素で引く指標が 0 件として静かに落ちるので通さない"
            ),
        }
    }
}

impl std::error::Error for MorphError {}

/// 体系を確かめる。測る前に必ず通す。
pub fn check(analyzer: &dyn Analyzer) -> Result<(), MorphError> {
    if analyzer.dictionary() == Dictionary::UnidicShort {
        return Ok(());
    }
    let (name, version) = analyzer.dictionary_version();
    Err(MorphError::WrongDictionary { name, version })
}

/// 読点を外したら語が繋がる箇所を探す。
///
/// **道具が「読点を増やせ」と言った結果、語の内側に読点が入ることがある。**
/// `あらため、て` は `改めて` を割っている。`ある、という` は割っていない。
///
/// 読点を抜いて解析し直し、**読点があった位置に語の切れ目が残るか**を見る。
/// 残らなければ、読点は語の内側にあったということである。
///
/// **直後が付属語のものだけを見る。** 絞らないと `あ、あと` や
/// `リンタ、フォーマッタ` で誤る——名詞どうしの並びは、読点を外せば別の語に
/// 読めてしまう。実測では、絞らないと素材 71 本のうち 5 本で誤り、絞ると 0 本になった。
///
/// **窓は前後 3 形態素に切る。** 文書ぜんぶを解析し直すと、離れた場所の切れ目の
/// 変化まで拾ってしまう。
fn splitting_commas(segments: &[Vec<Morpheme>], analyzer: &dyn Analyzer) -> Vec<String> {
    const NEAR: usize = 3;
    let mut windows: Vec<String> = Vec::new();
    let mut cuts: Vec<(String, usize)> = Vec::new();
    for seg in segments {
        for i in 1..seg.len().saturating_sub(1) {
            if seg[i].surface != "、" || !seg[i + 1].is_clinging() {
                continue;
            }
            let lo = i.saturating_sub(NEAR);
            let hi = (i + NEAR + 1).min(seg.len());
            let near: Vec<&str> = seg[lo..hi].iter().map(|m| m.surface.as_str()).collect();
            let cut = i - lo;
            let at: usize = near[..cut].iter().map(|s| s.len()).sum();
            windows.push(near[..cut].concat() + &near[cut + 1..].concat());
            cuts.push((near.concat(), at));
        }
    }
    if windows.is_empty() {
        return Vec::new();
    }
    let refs: Vec<&str> = windows.iter().map(String::as_str).collect();
    let got = analyzer.analyze_all(&refs);
    if got.len() != refs.len() {
        return Vec::new();
    }
    got.iter()
        .zip(cuts)
        .filter(|(ms, (_, at))| {
            let mut n = 0usize;
            // 位置 0 は必ず切れ目である。
            !std::iter::once(0)
                .chain(ms.iter().map(|m| {
                    n += m.surface.len();
                    n
                }))
                .any(|e| e == *at)
        })
        .map(|(_, (context, _))| context)
        .collect()
}

/// 解析し終えた地の文。
///
/// 1 度だけ解析して、指標のあいだで使い回す。 指標ごとに解析器を呼べば、外の
/// 実行ファイルを指標の数だけ起こす——[測るのが高ければ周回数が減り、そのまま品質が
/// 落ちる](../../../docs/spec/100-metrics.md#測るのを安くする)。
///
/// そして体系の確かめがここで済む。 作る道が[`Analyzed::of`]しかないので、
/// 指標ごとに書き忘れられない——持っていること自体が確かめた証になる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analyzed {
    segments: Vec<Vec<Morpheme>>,
    /// 各 node の種類。並びは `segments` と同じである。
    ///
    /// 文体は node の種類ごとに違うので、種類で絞って数える指標がある。
    kinds: Vec<kakiburi_doc::node::Kind>,
    split_commas: Vec<String>,
}

impl Analyzed {
    /// 地の文を解析する。node ごとに分けて持つ——跨がない指標があるからである。
    pub fn of(
        prose: &[kakiburi_doc::prose::Segment],
        analyzer: &dyn Analyzer,
    ) -> Result<Self, MorphError> {
        Self::with_lexicon(prose, analyzer, &crate::lexicon::Lexicon::default())
    }

    /// 地の文を解析し、コーパスから見つけた語を畳む。
    ///
    /// 辞書に無い語は複数の語に割れる。割れたままだと、その語のところで
    /// 機能語の分布も品詞 bigram も型も狂う（[語](crate::lexicon)）。
    pub fn with_lexicon(
        prose: &[kakiburi_doc::prose::Segment],
        analyzer: &dyn Analyzer,
        lexicon: &crate::lexicon::Lexicon,
    ) -> Result<Self, MorphError> {
        check(analyzer)?;
        let kinds: Vec<kakiburi_doc::node::Kind> = prose.iter().map(|s| s.kind).collect();
        let texts: Vec<&str> = prose.iter().map(|s| s.text.as_str()).collect();
        let mut segments = analyzer.analyze_all(&texts);
        for seg in &mut segments {
            lexicon.fold(seg);
        }
        // 並びがずれたら受け取らない。 ずれれば、node を跨がないはずの指標が
        // 別の node の形態素を数える——エラーにならず、値だけが違う。
        if segments.len() != texts.len() {
            return Ok(Self {
                segments: vec![Vec::new(); texts.len()],
                kinds,
                split_commas: Vec::new(),
            });
        }
        let split_commas = splitting_commas(&segments, analyzer);
        Ok(Self {
            segments,
            kinds,
            split_commas,
        })
    }

    /// 語を割っている読点の、前後の並び。
    ///
    /// 解析器を要るのでここで数える。[`Analyzed`]を作る道は 1 つしかないので、
    /// 指標の側から解析器を呼び直さずに済む。
    #[must_use]
    pub fn split_commas(&self) -> &[String] {
        &self.split_commas
    }

    /// node ごとの形態素列。跨がない指標はこちらを使う。
    #[must_use]
    pub fn segments(&self) -> &[Vec<Morpheme>] {
        &self.segments
    }

    /// その種類の node の形態素列だけ。
    ///
    /// 文体は node の種類ごとに違うので、段落と項目を混ぜて数えると
    /// 使い分けが袋の中で消える。
    pub fn segments_of(
        &self,
        kind: kakiburi_doc::node::Kind,
    ) -> impl Iterator<Item = &Vec<Morpheme>> {
        self.kinds
            .iter()
            .zip(&self.segments)
            .filter(move |(k, _)| **k == kind)
            .map(|(_, s)| s)
    }

    /// node を跨いだ 1 つの列。分布を出す指標はこちらを使う。
    pub fn all(&self) -> impl Iterator<Item = &Morpheme> {
        self.segments.iter().flatten()
    }

    /// 延べ語数。約物と記号の形態素も含める。
    #[must_use]
    pub fn tokens(&self) -> usize {
        self.segments.iter().map(Vec::len).sum()
    }

    /// 延べ語数の下限に届くか。
    #[must_use]
    pub fn enough_tokens(&self) -> bool {
        self.tokens() >= crate::floor::TOKENS
    }
}

#[cfg(test)]
pub(crate) mod stub {
    //! 試験用の解析器。UniDic を名乗る。
    //!
    //! 空白で切り、決めた表で品詞を当てる。仕様の手続きを試すためのものであって、
    //! 日本語を解析するものではない。

    use super::{Analyzer, Dictionary, Morpheme};

    /// 試験用。
    pub struct Stub {
        /// 名乗る体系。
        pub dictionary: Dictionary,
    }

    impl Stub {
        /// UniDic を名乗る。
        pub fn unidic() -> Self {
            Self {
                dictionary: Dictionary::UnidicShort,
            }
        }

        /// 別の体系を名乗る。
        pub fn other() -> Self {
            Self {
                dictionary: Dictionary::Other,
            }
        }
    }

    /// 表層形から品詞を当てる。試験のための最小の表。
    fn pos(surface: &str) -> (&'static str, &'static str) {
        match surface {
            "は" | "が" | "の" | "を" | "に" | "で" | "と" | "も" => ("助詞", "係助詞"),
            "である" | "だ" | "です" | "ある" => ("助動詞", "*"),
            "しかし" | "そして" | "だが" | "また" | "しかしながら" => {
                ("接続詞", "*")
            }
            "とても" | "やはり" | "すでに" | "ようやく" => ("副詞", "一般"),
            // な形容詞の語幹は 形状詞 である。 学校文法の「形容動詞」を
            // 名詞に寄せると、地味・静か・便利が語として見えなくなる。
            "地味" | "静か" | "便利" => ("形状詞", "一般"),
            "速い" | "古い" | "新しい" => ("形容詞", "一般"),
            "ああ" | "ええ" => ("感動詞", "*"),
            "。" | "、" | "！" | "？" => ("記号", "句点"),
            _ => ("名詞", "一般"),
        }
    }

    impl Analyzer for Stub {
        fn dictionary(&self) -> Dictionary {
            self.dictionary
        }

        fn dictionary_version(&self) -> (String, String) {
            match self.dictionary {
                Dictionary::UnidicShort => ("UniDic".into(), "3.1.0-test".into()),
                Dictionary::Other => ("IPADic".into(), "2.7.0".into()),
            }
        }

        fn analyze(&self, text: &str) -> Vec<Morpheme> {
            let mut out = Vec::new();
            for token in text.split_whitespace() {
                // 約物は 1 形態素として切り出す。
                let mut buf = String::new();
                for c in token.chars() {
                    if matches!(c, '。' | '、' | '！' | '？') {
                        if !buf.is_empty() {
                            out.push(make(&std::mem::take(&mut buf)));
                        }
                        out.push(make(&c.to_string()));
                        continue;
                    }
                    buf.push(c);
                }
                if !buf.is_empty() {
                    out.push(make(&buf));
                }
            }
            out
        }
    }

    fn make(surface: &str) -> Morpheme {
        let (pos1, pos2) = pos(surface);
        Morpheme {
            surface: surface.to_owned(),
            lemma: surface.to_owned(),
            pos1: pos1.to_owned(),
            pos2: pos2.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::stub::Stub;
    use super::{check, Analyzer, MorphError};

    #[test]
    fn unidic_なら通る() {
        assert!(check(&Stub::unidic()).is_ok());
    }

    #[test]
    fn 別の体系は断る() {
        // 黙って測れば、語彙素で引く指標が 0 件として静かに落ちる。
        let e = check(&Stub::other()).unwrap_err();
        assert!(matches!(e, MorphError::WrongDictionary { .. }), "{e:?}");
        assert!(e.to_string().contains("静かに落ちる"), "{e}");
    }

    #[test]
    fn 辞書の名前と版を名乗る() {
        // 指紋に入る。体系を選んだうえで、なお版で値が動く。
        let (name, version) = Stub::unidic().dictionary_version();
        assert_eq!(name, "UniDic");
        assert!(!version.is_empty());
    }

    #[test]
    fn 機能語は_5_つの品詞である() {
        let a = Stub::unidic();
        let ms = a.analyze("これ は とても 大事 である 。");
        let fw: Vec<&str> = ms
            .iter()
            .filter(|m| m.is_function_word())
            .map(|m| m.surface.as_str())
            .collect();
        assert_eq!(fw, vec!["は", "とても", "である"]);
    }

    #[test]
    fn 名詞は機能語ではない() {
        let a = Stub::unidic();
        let ms = a.analyze("文章");
        assert!(!ms[0].is_function_word());
    }

    #[test]
    fn 約物を形態素として切り出す() {
        let a = Stub::unidic();
        let ms = a.analyze("そうだ 。");
        assert_eq!(ms.len(), 2);
        assert_eq!(ms[1].surface, "。");
    }
}
