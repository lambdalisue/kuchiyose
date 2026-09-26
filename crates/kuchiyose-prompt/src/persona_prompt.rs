//! ペルソナを作らせるプロンプト（[仕様](../../../docs/spec/400-write.md#ペルソナを作る)）。
//!
//! 道具には、取り込まずに確かめるコマンドだけを打たせる。知らせを受けて引用を直す
//! 機会を渡すためである。取り込むのは道具が終わったあとの組み立て層で、道具に形代を
//! 書き換える道を渡さない。

use std::fmt::Write as _;

use crate::persona::{HEADINGS, QUOTE_MIN_CHARS};

/// ペルソナを作らせるプロンプトを組み立てる。同じ材料からは同じバイト列が出る。
///
/// `check` は確かめるコマンドで、道具がそのまま打てる形（経路は引用符で囲んだもの）で渡す。
#[must_use]
pub fn persona_prompt(material: &str, write: &str, check: &str) -> String {
    let mut out = String::new();
    out.push_str("# ペルソナの下書きの依頼\n\n");
    out.push_str(
        "素材のフォルダにある記事を読み、この人のペルソナを Markdown で下書きする。\
         ペルソナは、この人が何を大事にし、誰に向けて、どう話を運ぶかを書いたもので、\
         この人の代わりに文章を書くときに使う。下書きは本人が読んで直す。\n\n",
    );
    let _ = writeln!(out, "- 素材のフォルダ: {material}\n- 書く経路: {write}");
    out.push_str("\n## 形\n\n");
    out.push_str(
        "見出しは `##` で、次の名前をこの順に置く。ほかの `##` 見出しは置かない。\
         最初の見出しより前に置けるのは `#` の題だけである。書くことが無い見出しは、\
         下を空にしておく。\n\n",
    );
    for (name, what) in HEADINGS {
        let _ = writeln!(out, "- `## {name}`: {what}");
    }
    out.push_str(
        "\n項目は `-` の箇条で書く。項目ごとに、根拠になった記事の文字列を、その下の箇条に\
         引用として付ける。\n\n",
    );
    out.push_str(
        "```markdown\n## 大事にすること\n\n- 手を動かして確かめたことだけを書く\n  \
         - 「実際に手元で動かしてみると」（2024-05-vim-filer）\n```\n\n",
    );
    let _ = writeln!(
        out,
        "- 引用は `「…」` で括り、続けて `（…）` で記事の名前を括る。記事の名前は、素材の\
         ファイル名から拡張子を除いたものである。\n\
         - 引用は記事の地の文の文字列どおりに写す。言い換えない。途中を `…` で省かない。\n\
         - 引用は日本語で {QUOTE_MIN_CHARS} 字以上にする。短い引用はどの記事にも現れて、\
         根拠にならない。\n\
         - 誰にでも当てはまる一般論は書かない。この人の記事から引用できない項目は書かない。"
    );
    out.push_str("\n## 確かめる\n\n");
    let _ = writeln!(
        out,
        "書き終えたら、次のコマンドで確かめる。このコマンドを、このとおりに打つ。\
         ほかのコマンドは打たない。\n\n```sh\n{check}\n```\n\n\
         解決しない引用や、解決する引用を持たない項目が知らされたら、引用を記事どおりに\
         直すか、項目ごと消して、もう一度確かめる。見出しが揃っていないと断られたら、\
         見出しを直して確かめ直す。知らせが無くなるまで繰り返す。\
         形代への取り込みは、書き終えたあとに kuchiyose がする。"
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn got() -> String {
        persona_prompt(
            "/home/a/articles",
            "/home/a/a.persona.md",
            "'/bin/kuchiyose' katashiro persona --check '/home/a/a.persona.md' --material '/home/a/articles'",
        )
    }

    #[test]
    fn 同じ材料からは同じバイト列が出る() {
        assert_eq!(got(), got());
    }

    #[test]
    fn 素材のフォルダと書く経路と確かめるコマンドを言う() {
        let g = got();
        assert!(g.contains("/home/a/articles"), "{g}");
        assert!(g.contains("書く経路: /home/a/a.persona.md"), "{g}");
        assert!(
            g.contains("\n'/bin/kuchiyose' katashiro persona --check '/home/a/a.persona.md' --material '/home/a/articles'\n"),
            "コマンドを 1 行で、そのまま打てる形で置く: {g}"
        );
        assert!(g.contains("ほかのコマンドは打たない"), "{g}");
    }

    #[test]
    fn 見出しを決めた順に全部並べる() {
        let g = got();
        let at: Vec<usize> = HEADINGS
            .iter()
            .map(|(n, _)| g.find(&format!("`## {n}`")).expect(n))
            .collect();
        assert!(at.windows(2).all(|w| w[0] < w[1]), "{g}");
    }

    #[test]
    fn 引用は記事どおりに写し短くしないと言う() {
        let g = got();
        assert!(g.contains("文字列どおりに写す"), "{g}");
        assert!(g.contains(&format!("{QUOTE_MIN_CHARS} 字以上")), "{g}");
    }
}
