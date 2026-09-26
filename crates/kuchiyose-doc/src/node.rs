//! node の型。
//!
//! node は意味で定める。書き手が「これは補足である」と示していれば、記法が
//! 何であっても補足の node になる。記法の違いは取り込み元の事情であって、
//! 書きぶりではない。
//!
//! この表が、構造について測れることの全体である。

/// node の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// 段落。
    Paragraph,
    /// 見出し。
    Heading,
    /// 箇条書き。項目を持つ。
    Bullet,
    /// 番号リスト。項目を持つ。
    Ordered,
    /// 箇条書きまたは番号リストの項目。
    Item,
    /// 引用。
    Quote,
    /// 補足。
    Note,
    /// 警告。
    Warning,
    /// 折りたたみ。
    Details,
    /// 脚注。
    Footnote,
    /// 表。セルを持つ。
    Table,
    /// 表のセル。
    Cell,
    /// コードブロック。子を持たない。
    CodeBlock,
    /// 区切り線。子を持たない。
    Divider,
    /// 画像。子を持たない。
    Image,
    /// 強調。
    Emphasis,
    /// インラインコード。子を持たない。
    InlineCode,
    /// リンク。
    Link,
}

impl Kind {
    /// 子を持つ種類か。
    #[must_use]
    pub fn has_children(self) -> bool {
        !matches!(
            self,
            Kind::CodeBlock | Kind::Divider | Kind::Image | Kind::InlineCode
        )
    }

    /// [地の文](crate::prose)に入る種類。並びは軸の名前に出るので固定する。
    ///
    /// 種類ごとに測る指標は、この一覧から軸を作る。 種類を足したときに、
    /// 指標の側を書き足さずに済む——書き足す形にすると、足し忘れた指標だけが
    /// 混ぜたままになる。
    pub const PROSE: [Kind; 9] = [
        Kind::Paragraph,
        Kind::Heading,
        Kind::Item,
        Kind::Quote,
        Kind::Note,
        Kind::Warning,
        Kind::Details,
        Kind::Footnote,
        Kind::Cell,
    ];

    /// 軸の名前に出す呼び名。
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Kind::Paragraph => "段落",
            Kind::Heading => "見出し",
            Kind::Bullet => "箇条書き",
            Kind::Ordered => "番号リスト",
            Kind::Item => "項目",
            Kind::Quote => "引用",
            Kind::Note => "補足",
            Kind::Warning => "警告",
            Kind::Details => "折りたたみ",
            Kind::Footnote => "脚注",
            Kind::Table => "表",
            Kind::Cell => "セル",
            Kind::CodeBlock => "コードブロック",
            Kind::Emphasis => "強調",
            Kind::Divider => "区切り線",
            Kind::Image => "画像",
            Kind::InlineCode => "インラインコード",
            Kind::Link => "リンク",
        }
    }

    /// この node のテキストが[地の文](crate::prose)に入るか。
    ///
    /// 入らないのは、書き手が日本語で書いた部分ではないものである。外さなければ、
    /// コードの多い記事ほど記号の率が上がり、書きぶりではなく題材を測る。
    ///
    /// `Cell` はここでは `true` を返す。日本語の無いセルを落とす判定は文字を
    /// 見る必要があるので[`crate::prose`]が持つ。
    #[must_use]
    pub fn contributes_to_prose(self) -> bool {
        matches!(
            self,
            Kind::Paragraph
                | Kind::Heading
                | Kind::Item
                | Kind::Quote
                | Kind::Note
                | Kind::Warning
                | Kind::Details
                | Kind::Footnote
                | Kind::Cell
        )
    }

    /// 段落として数える種類か。
    ///
    /// 箇条書き・番号リスト・引用・コードブロック・表・見出しは段落ではない。
    /// 段落に数えると、箇条書きの多い文書ほど「段落が短い」ことになる。
    #[must_use]
    pub fn is_paragraph(self) -> bool {
        self == Kind::Paragraph
    }
}

/// 文書の node。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// 種類。
    pub kind: Kind,
    /// この node が直接持つテキスト。入れ子の node の中身は含まない。
    pub text: String,
    /// 見出しの深さ。見出し以外は `None`。
    ///
    /// 取り込み時の生の深さである。文書内で最も浅いものからの相対は
    /// [`crate::Document::heading_depth`]が出す。
    pub raw_depth: Option<u8>,
    /// 子の node。
    pub children: Vec<Node>,
}

impl Node {
    /// テキストだけを持つ node を作る。
    #[must_use]
    pub fn leaf(kind: Kind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
            raw_depth: None,
            children: Vec::new(),
        }
    }

    /// 子を持つ node を作る。
    #[must_use]
    pub fn branch(kind: Kind, children: Vec<Node>) -> Self {
        Self {
            kind,
            text: String::new(),
            raw_depth: None,
            children,
        }
    }

    /// 見出しを作る。
    #[must_use]
    pub fn heading(depth: u8, text: impl Into<String>) -> Self {
        Self {
            kind: Kind::Heading,
            text: text.into(),
            raw_depth: Some(depth),
            children: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 子を持たない種類は仕様の表どおり() {
        for k in [
            Kind::CodeBlock,
            Kind::Divider,
            Kind::Image,
            Kind::InlineCode,
        ] {
            assert!(!k.has_children(), "{k:?} は子を持たない");
        }
        for k in [
            Kind::Paragraph,
            Kind::Table,
            Kind::Cell,
            Kind::Link,
            Kind::Emphasis,
        ] {
            assert!(k.has_children(), "{k:?} は子を持つ");
        }
    }

    #[test]
    fn コードとインラインコードと画像は地の文に入らない() {
        for k in [
            Kind::CodeBlock,
            Kind::InlineCode,
            Kind::Image,
            Kind::Divider,
        ] {
            assert!(!k.contributes_to_prose(), "{k:?} は地の文ではない");
        }
    }

    #[test]
    fn セルは地の文に入る() {
        assert!(Kind::Cell.contributes_to_prose());
    }

    #[test]
    fn 表そのものは地の文に入らない() {
        // 表はセルを持つだけで、自分ではテキストを持たない。
        // 入れると、セルと表で二重に数える。
        assert!(!Kind::Table.contributes_to_prose());
    }

    #[test]
    fn 段落以外は段落ではない() {
        assert!(Kind::Paragraph.is_paragraph());
        for k in [
            Kind::Bullet,
            Kind::Ordered,
            Kind::Item,
            Kind::Quote,
            Kind::CodeBlock,
            Kind::Table,
            Kind::Heading,
        ] {
            assert!(!k.is_paragraph(), "{k:?} は段落ではない");
        }
    }
}
