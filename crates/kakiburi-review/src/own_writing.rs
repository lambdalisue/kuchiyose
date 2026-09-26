//! 草稿の作られ方の申告。
//!
//! 本人の文章を見せて書かせた草稿では測定が膨らむ
//! （[草稿の作られ方を疑う](../../../docs/spec/300-revise.md#草稿の作られ方を疑う)）。
//! 書かせる側を持たないので、漏れたかどうかを知る道も防ぐ道も無い。だから訊く。

/// 本人の文章を LLM に渡したか。
///
/// 真偽の 2 値にしない。 2 値では「渡していない」と「申告していない」が
/// 同じ形になり、既定を「分からない」に置いた意味が消える。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OwnWriting {
    /// 手本・書き出し・過去記事などを依頼文に含めた。
    Shown,
    /// 依頼文に本人の文章を含めていない。
    NotShown,
    /// 草稿の作られ方を知らない。
    ///
    /// 既定である。 申告が無いことを「渡していない」と読めば、いちばん
    /// 膨らみやすい草稿が、いちばん問題の無い草稿と同じ扱いで通る。
    #[default]
    Unknown,
}

/// 申告が膨らみを否定できないときに添える但し書き。
///
/// 判定を変えない。 渡したかどうかは値をどう読むかの問題であって、値が
/// 出せない理由ではない——判定できないに落とせば、本当に判定できないときの
/// 重みが下がる。
pub const OWN_WRITING_CAVEAT: &str =
    "本人の文章を見せて書かせた草稿では測定が膨らむ。この値が膨らんでいないことは確かめられていない";

impl OwnWriting {
    /// 引数の綴りから読む。知らない綴りは `None`。
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "shown" => Some(Self::Shown),
            "not-shown" => Some(Self::NotShown),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }

    /// 引数の綴り。道具向けの出力にもこれを使う。
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Shown => "shown",
            Self::NotShown => "not-shown",
            Self::Unknown => "unknown",
        }
    }

    /// 添える但し書き。渡していないと申告されたときだけ `None`。
    ///
    /// 3 値のどれを返すときも添える。 通らないも判定できないも、膨らんだ
    /// かもしれない値の上に出ている。
    #[must_use]
    pub fn caveat(self) -> Option<&'static str> {
        match self {
            Self::NotShown => None,
            Self::Shown | Self::Unknown => Some(OWN_WRITING_CAVEAT),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 申告が無ければ分からないとして読む() {
        // 省略を「渡していない」と読めば、いちばん膨らみやすい草稿が通る。
        assert_eq!(OwnWriting::default(), OwnWriting::Unknown);
    }

    #[test]
    fn 渡していないときだけ但し書きを添えない() {
        assert_eq!(OwnWriting::NotShown.caveat(), None);
        assert_eq!(OwnWriting::Shown.caveat(), Some(OWN_WRITING_CAVEAT));
        assert_eq!(OwnWriting::Unknown.caveat(), Some(OWN_WRITING_CAVEAT));
    }

    #[test]
    fn 綴りは往復する() {
        for o in [OwnWriting::Shown, OwnWriting::NotShown, OwnWriting::Unknown] {
            assert_eq!(OwnWriting::parse(o.name()), Some(o));
        }
    }

    #[test]
    fn 真偽の綴りは受けない() {
        // 2 値で受ければ、渡していないと申告していないが同じ形になる。
        for s in ["true", "false", "yes", "no", ""] {
            assert_eq!(OwnWriting::parse(s), None, "{s}");
        }
    }
}
