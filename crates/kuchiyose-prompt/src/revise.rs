//! 直させるプロンプト（[仕様](../../../docs/spec/400-write.md#直させるプロンプト)）。
//!
//! | 順 | 中身 |
//! | --- | --- |
//! | 1 | 役目。指摘された表現だけを直すこと |
//! | 2 | 守ること |
//! | 3 | 検めた結果。`review` の散文の出力を、そのまま入れる |
//! | 4 | 読む経路と書く経路 |
//!
//! 推論設定の直し方は入れない。 起動した道具からは設定を変えられないので、直させる
//! 指示にならない。

use std::fmt::Write as _;

use crate::fence_for;

/// 直させるプロンプトを組み立てる。同じ材料からは同じバイト列が出る。
///
/// `review` は検めた結果の散文で、判定を止める指摘と止めない知らせの区切りも含めて
/// そのまま入れる。直す側に貼るための形として決めた出力だからである。
#[must_use]
pub fn revise_prompt(review: &str, read: &str, write: &str) -> String {
    let mut out = String::new();
    out.push_str("# 表現を直す依頼\n\n");
    out.push_str("次の文章の、検めた結果が指摘した表現だけを直す。直した全文を別の経路に書く。\n");
    out.push_str("\n## 守ること\n\n");
    out.push_str(
        "- 内容を変えない。主張、事実、例、見出しの構成、コード、リンクはそのまま残す。\n\
         - 指摘に無い箇所を書き換えない。\n\
         - 言い回しの上限を超えない。上限が示された言い回しは、その回数までにとどめる。\n\
         - 検めを自分で走らせない。検めて、採るかを決めるのは kuchiyose である。\n\
         - 指摘が頻度ペナルティや温度に触れていたら、その部分は読み飛ばす。\
         文章の直し方だけに従う。\n",
    );
    out.push_str("\n## 検めた結果\n\n");
    out.push_str(
        "kuchiyose review の出力である。「ここから下は判定に使っていない」より下は、判定を\
         止めない知らせである。そのうち、使いすぎている言い回し、残っている基準の型、残って\
         いる基準の語は、直せばこの人に寄るので直してよい。ほかの知らせは直さなくてよい。\n\n",
    );
    let fence = fence_for(review);
    let _ = writeln!(out, "{fence}text\n{}\n{fence}", review.trim_end());
    out.push_str("\n## 読む経路と書く経路\n\n");
    let _ = writeln!(
        out,
        "- 読む: {read}\n- 書く: {write}\n\n読む経路の文章を読み、直した全文を書く経路に書く。\
         読む経路のファイルは書き換えない。ほかのファイルを作らない。"
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const REVIEW: &str = "判定: 判定できない\n止まった段: 照合値\n\n指摘 1 本\n  - 読点を減らす";

    #[test]
    fn 同じ材料からは同じバイト列が出る() {
        assert_eq!(
            revise_prompt(REVIEW, "/w/round-0.md", "/w/round-1.md"),
            revise_prompt(REVIEW, "/w/round-0.md", "/w/round-1.md")
        );
    }

    #[test]
    fn 役目_守ること_検めた結果_経路の順に並ぶ() {
        let got = revise_prompt(REVIEW, "/w/round-0.md", "/w/round-1.md");
        let at = |s: &str| got.find(s).unwrap_or_else(|| panic!("{s} が無い:\n{got}"));
        let order = [
            at("指摘した表現だけを直す"),
            at("## 守ること"),
            at("内容を変えない"),
            at("## 検めた結果"),
            at("止まった段: 照合値"),
            at("- 読む: /w/round-0.md"),
            at("- 書く: /w/round-1.md"),
        ];
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{got}");
    }

    #[test]
    fn 検めた結果はそのまま入れ囲みが途中で閉じない() {
        let review = "指摘\n```\nコード\n```";
        let got = revise_prompt(review, "/a", "/b");
        assert!(got.contains(review), "{got}");
        assert!(got.contains("````text\n"), "{got}");
    }

    #[test]
    fn 推論設定の直し方は入れない() {
        let got = revise_prompt(REVIEW, "/a", "/b");
        assert!(!got.contains("推論設定"), "{got}");
    }
}
