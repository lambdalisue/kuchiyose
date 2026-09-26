//! 代筆する（[仕様](../../../docs/spec/400-write.md)）。ペルソナの形を確かめ、LLM の道具に
//! 渡すプロンプトを組み立てる。
//!
//! | | |
//! | --- | --- |
//! | [ペルソナの読み方](persona::parse) | 見出しと項目と引用を取り出す。形が合わなければ断る |
//! | [引用の照らし方](persona::check) | 単位ごとの node の文字列を受け取り、引用が現れるかを返す |
//! | [代筆のプロンプト](draft::draft_prompt) | ペルソナ・文体の事実・要約・保存先から組み立てる |
//! | [直させるプロンプト](revise::revise_prompt) | 検めた結果の散文・読む経路・書く経路から組み立てる |
//! | [ペルソナを作らせるプロンプト](persona_prompt::persona_prompt) | 素材のフォルダ・書く経路・確かめるコマンドから組み立てる |
//!
//! 入出力を持たない。 受け取るのは文字列と、組み立て層が詰めた平らな値だけである。
//! 同じ入力からは同じバイト列が出る（[決定的に作る](../../../docs/spec/400-write.md#決定的に作る)）。
//! 日時も乱数も入れない。
//!
//! どのクレートにも依存しない。 目盛りも判定も知らないので、プロンプトの側から
//! 作り直す経路が書けない（[境界](../../../docs/design/000-architecture.md#kuchiyose-prompt)）。

pub mod draft;
pub mod persona;
pub mod persona_prompt;
pub mod revise;

pub use draft::{
    draft_prompt, Avoid, AvoidKind, DraftRequest, FirstPerson, Kata, Length, Register,
    RegisterFact, Spread, StyleFacts, MAX_PHRASES,
};
pub use persona::{
    check, parse, Citations, Item, Persona, Quote, Reason, Section, ShapeError, Unresolved,
    HEADINGS, QUOTE_MIN_CHARS,
};
pub use persona_prompt::persona_prompt;
pub use revise::revise_prompt;

/// シェルに渡す 1 語にする。単引用符で囲み、中の単引用符は閉じて繋ぐ。
///
/// プロンプトに書くコマンドは、道具がそのまま写して打つ。 空白を含む経路が
/// 2 語に割れれば、別のファイルを指す。
#[must_use]
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// 中身をコードブロックに入れるときの囲み。中身の最も長い `` ` `` の並びより 1 つ長くする。
///
/// 中身に囲みと同じ並びがあれば、そこでブロックが閉じ、残りが指示として読まれる。
#[must_use]
pub fn fence_for(body: &str) -> String {
    let mut longest = 0usize;
    let mut run = 0usize;
    for c in body.chars() {
        if c == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    "`".repeat((longest + 1).max(3))
}

/// 桁を区切る。
pub(crate) fn with_commas(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 空白と単引用符を含む経路も_1_語になる() {
        assert_eq!(shell_quote("/a b/c"), "'/a b/c'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn 囲みは中身のどの並びよりも長い() {
        assert_eq!(fence_for("ふつうの文"), "```");
        assert_eq!(fence_for("```rust\n```"), "````");
        assert_eq!(fence_for("`````"), "``````");
    }

    #[test]
    fn 桁を区切る() {
        assert_eq!(with_commas(999), "999");
        assert_eq!(with_commas(3120), "3,120");
    }
}
