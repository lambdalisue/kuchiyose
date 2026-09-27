//! 直させるプロンプト（[仕様](../../../docs/spec/400-write.md#直させるプロンプト)）。
//!
//! | 順 | 中身 |
//! | --- | --- |
//! | 1 | 役目。指摘された表現だけを直すこと |
//! | 2 | 守ること |
//! | 3 | 検めた結果。`review` の散文の出力を、そのまま入れる |
//! | 4 | 言い回しの上限。今の版が使っている言い回しと、本人の上限までの余地 |
//! | 5 | 捨てた直し。今の版を直して捨てた版があるときだけ |
//! | 6 | 読む経路と書く経路 |
//!
//! 推論設定の直し方は入れない。 起動した道具からは設定を変えられないので、直させる
//! 指示にならない。

use std::fmt::Write as _;

use crate::changes::Change;
use crate::{fence_for, with_commas};

/// 捨てた直しのうち、プロンプトに載せる数。新しいほうから数える。暫定値である。
///
/// 捨てるたびに 1 つずつ足せば、プロンプトが周回の数だけ伸びる。 実測では、同じ版に
/// 11 回続けて同じ向きの直しを返した。 向きが同じなら、直近の数回で足りる。
pub const MAX_REJECTED: usize = 3;

/// 捨てた直しに載せる、変えたところの数。文書の順に先頭から数える。暫定値である。
pub const MAX_CHANGES: usize = 5;

/// 言い回しの上限に載せる本数。上限に近いほうから数える。暫定値である。
///
/// 余地を見せたいのは、次の直しで超えそうな言い回しである。 上限から遠いものまで
/// 並べても、直す側が扱いきれない。
pub const MAX_CEILINGS: usize = 5;

/// 今の版が使っている言い回し 1 つと、本人の上限。
///
/// 並べ方は組み立て層が決めて渡す。上限に近い順である。
#[derive(Debug, Clone, PartialEq)]
pub struct Ceiling {
    /// 言い回し。
    pub text: String,
    /// 今の版に現れた回数。
    pub times: usize,
    /// 今の版での、日本語 1,000 字あたりの回数。
    pub density: f64,
    /// 本人が 1 本の中で使う、日本語 1,000 字あたりの最大。
    pub ceiling: f64,
    /// 今の版の長さで、本人の上限まで使える回数。
    pub allowed: usize,
}

/// 今の版を直して捨てた版 1 つ。
#[derive(Debug, Clone, PartialEq)]
pub struct Rejected {
    /// 何周目の版か。
    pub round: usize,
    /// 採らなかった理由。比べ方の最初に差が付いた鍵と、その前後の値。
    pub reason: String,
    /// 悪い向きに動いた指標。`"圧縮率 -0.820 → -0.857"` の形で、動いた量の大きい順。
    pub worse: Vec<String>,
    /// 変えたところ。文書の順。
    pub changes: Vec<Change>,
}

/// 直させるプロンプトを組み立てる。同じ材料からは同じバイト列が出る。
///
/// `review` は検めた結果の散文で、判定を止める指摘と止めない知らせの区切りも含めて
/// そのまま入れる。直す側に貼るための形として決めた出力だからである。
///
/// `ceilings` は今の版が使っている言い回しで、上限に近い順に渡す。 載せるのは先頭から
/// [`MAX_CEILINGS`] 本までである。
///
/// `rejected` は今の版を直して捨てた版で、古い順に渡す。 載せるのは新しいほうから
/// [`MAX_REJECTED`] 個までである。
#[must_use]
pub fn revise_prompt(
    review: &str,
    ceilings: &[Ceiling],
    rejected: &[Rejected],
    read: &str,
    write: &str,
) -> String {
    let mut out = String::new();
    out.push_str("# 表現を直す依頼\n\n");
    out.push_str("次の文章の、検めた結果が指摘した表現だけを直す。直した全文を別の経路に書く。\n");
    out.push_str("\n## 守ること\n\n");
    out.push_str(
        "- 内容を変えない。主張、事実、例、見出しの構成、コード、リンクはそのまま残す。\n\
         - 指摘に無い箇所を書き換えない。\n\
         - 言い回しの上限を超えない。上限が示された言い回しは、その回数までにとどめる。\
         言い換えや文末を揃えるときも同じである。\n\
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
    ceilings_section(&mut out, ceilings);
    rejected_section(&mut out, rejected);
    out.push_str("\n## 読む経路と書く経路\n\n");
    let _ = writeln!(
        out,
        "- 読む: {read}\n- 書く: {write}\n\n読む経路の文章を読み、直した全文を書く経路に書く。\
         読む経路のファイルは書き換えない。ほかのファイルを作らない。"
    );
    out
}

/// 言い回しの上限。無ければ何も書かない。
///
/// 使いすぎの知らせは、超えてからしか出ない。 超える前に余地が見えなければ、直す側は
/// 超えるまで足す。 足されるのはたいてい文末である。
fn ceilings_section(out: &mut String, ceilings: &[Ceiling]) {
    if ceilings.is_empty() {
        return;
    }
    out.push_str("\n## 言い回しの上限\n\n");
    out.push_str(
        "読む経路の文章が使っている言い回しのうち、この人の上限に近いものから示す。\
         上限は、この人が 1 本の記事で使った、日本語 1,000 字あたりの最大である。\
         言い換えや文末を揃えるときも、この長さで使える回数を超えない。\n\n",
    );
    for c in ceilings.iter().take(MAX_CEILINGS) {
        let room = match c.times.cmp(&c.allowed) {
            std::cmp::Ordering::Less => format!("あと {} 回。", c.allowed - c.times),
            std::cmp::Ordering::Equal => "上限に達している。これ以上増やさない。".to_owned(),
            std::cmp::Ordering::Greater => {
                format!("{} 回超えている。減らす。", c.times - c.allowed)
            }
        };
        let _ = writeln!(
            out,
            "- 「{}」: 今 {} 回（1,000 字あたり {:.1} 回）。この人は 1,000 字あたり {:.1} 回まで。\
             この長さなら {} 回まで。{room}",
            c.text,
            with_commas(c.times),
            c.density,
            c.ceiling,
            with_commas(c.allowed),
        );
    }
}

/// 捨てた直し。無ければ何も書かない。
fn rejected_section(out: &mut String, rejected: &[Rejected]) {
    if rejected.is_empty() {
        return;
    }
    out.push_str("\n## 捨てた直し\n\n");
    let shown = &rejected[rejected.len().saturating_sub(MAX_REJECTED)..];
    if shown.len() == rejected.len() {
        let _ = writeln!(
            out,
            "読む経路の文章を直した版を、これまでに {} 回捨てた。どれも採らなかった。",
            rejected.len()
        );
    } else {
        let _ = writeln!(
            out,
            "読む経路の文章を直した版を、これまでに {} 回捨てた。どれも採らなかった。\
             新しいほうから {} 回ぶんを示す。",
            rejected.len(),
            shown.len()
        );
    }
    for r in shown {
        let _ = writeln!(out, "\n### {} 周目\n", r.round);
        let _ = writeln!(out, "- 採らなかった理由: {}", r.reason);
        if !r.worse.is_empty() {
            let _ = writeln!(out, "- 悪い向きに動いた指標: {}", r.worse.join("、"));
        }
        if r.changes.is_empty() {
            out.push_str("- 変えたところ: 無い。読む経路の文章と同じだった\n");
            continue;
        }
        if r.changes.len() > MAX_CHANGES {
            let _ = writeln!(
                out,
                "- 変えたところ {} か所のうち、先頭から {MAX_CHANGES} か所:",
                r.changes.len()
            );
        } else {
            let _ = writeln!(out, "- 変えたところ {} か所:", r.changes.len());
        }
        for c in r.changes.iter().take(MAX_CHANGES) {
            let _ = match (c.before.is_empty(), c.after.is_empty()) {
                (true, _) => writeln!(out, "  - 「{}」を足した", c.after),
                (_, true) => writeln!(out, "  - 「{}」を消した", c.before),
                _ => writeln!(out, "  - 「{}」→「{}」", c.before, c.after),
            };
        }
    }
    out.push_str(
        "\n同じ直しを繰り返さない。\n\n\
         - ここに示した変更と同じ変更をしない。同じ言い回しを、同じ向きに言い換えない。\n\
         - まだ手を付けていない指摘に取り組むか、同じ指摘に別のやり方で取り組む。\n\
         - 残っている指摘が、内容を変えずに言い回しだけでは直せないなら、変えるのを最小に\
         とどめる。\n",
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const REVIEW: &str = "判定: 判定できない\n止まった段: 照合値\n\n指摘 1 本\n  - 読点を減らす";

    fn rejected(round: usize, changes: usize) -> Rejected {
        Rejected {
            round,
            reason: "基準との距離が -0.957 から -0.997 に下がった。大きいほうが良い".into(),
            worse: vec![
                "圧縮率 -0.820 → -0.857".into(),
                "エントロピー 0.064 → 0.034".into(),
            ],
            changes: (0..changes)
                .map(|i| Change {
                    before: format!("{i} を落としています。"),
                    after: format!("{i} を落とすようにしました。"),
                })
                .collect(),
        }
    }

    #[test]
    fn 同じ材料からは同じバイト列が出る() {
        let r = [rejected(2, 3)];
        assert_eq!(
            revise_prompt(REVIEW, &[], &r, "/w/round-0.md", "/w/round-1.md"),
            revise_prompt(REVIEW, &[], &r, "/w/round-0.md", "/w/round-1.md")
        );
    }

    fn ceiling(text: &str, times: usize, allowed: usize) -> Ceiling {
        Ceiling {
            text: text.into(),
            times,
            #[allow(clippy::cast_precision_loss)]
            density: times as f64 / 5.0,
            ceiling: 2.8,
            allowed,
        }
    }

    #[test]
    fn 役目_守ること_検めた結果_言い回しの上限_捨てた直し_経路の順に並ぶ() {
        let got = revise_prompt(
            REVIEW,
            &[ceiling("しています。", 13, 14)],
            &[rejected(2, 1)],
            "/w/round-1.md",
            "/w/round-3.md",
        );
        let at = |s: &str| got.find(s).unwrap_or_else(|| panic!("{s} が無い:\n{got}"));
        let order = [
            at("指摘した表現だけを直す"),
            at("## 守ること"),
            at("内容を変えない"),
            at("## 検めた結果"),
            at("止まった段: 照合値"),
            at("## 言い回しの上限"),
            at("## 捨てた直し"),
            at("- 読む: /w/round-1.md"),
            at("- 書く: /w/round-3.md"),
        ];
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{got}");
    }

    #[test]
    fn 言い回しの上限は今の回数と本人の上限とこの長さで使える回数と残りを言う() {
        let got = revise_prompt(
            REVIEW,
            &[
                ceiling("しています。", 16, 14),
                ceiling("ています。", 14, 14),
                ceiling("になります。", 3, 14),
            ],
            &[],
            "/a",
            "/b",
        );
        let want = "\n## 言い回しの上限\n\n\
            読む経路の文章が使っている言い回しのうち、この人の上限に近いものから示す。\
            上限は、この人が 1 本の記事で使った、日本語 1,000 字あたりの最大である。\
            言い換えや文末を揃えるときも、この長さで使える回数を超えない。\n\n\
            - 「しています。」: 今 16 回（1,000 字あたり 3.2 回）。この人は 1,000 字あたり 2.8 回まで。\
            この長さなら 14 回まで。2 回超えている。減らす。\n\
            - 「ています。」: 今 14 回（1,000 字あたり 2.8 回）。この人は 1,000 字あたり 2.8 回まで。\
            この長さなら 14 回まで。上限に達している。これ以上増やさない。\n\
            - 「になります。」: 今 3 回（1,000 字あたり 0.6 回）。この人は 1,000 字あたり 2.8 回まで。\
            この長さなら 14 回まで。あと 11 回。\n";
        assert!(got.contains(want), "{got}");
    }

    #[test]
    fn 言い回しの上限は渡した順に上限の本数までを載せる() {
        let all: Vec<Ceiling> = (0..MAX_CEILINGS + 2)
            .map(|i| ceiling(&format!("言い回し{i}"), 1, 14))
            .collect();
        let got = revise_prompt(REVIEW, &all, &[], "/a", "/b");
        let at = |s: &str| got.find(s).unwrap_or_else(|| panic!("{s} が無い:\n{got}"));
        assert!(at("「言い回し0」") < at("「言い回し1」"), "{got}");
        assert!(got.contains(&format!("「言い回し{}」", MAX_CEILINGS - 1)));
        assert!(
            !got.contains(&format!("「言い回し{MAX_CEILINGS}」")),
            "{got}"
        );
    }

    #[test]
    fn 言い回しの上限が無ければその節を出さない() {
        let got = revise_prompt(REVIEW, &[], &[], "/a", "/b");
        assert!(!got.contains("## 言い回しの上限"), "{got}");
    }

    #[test]
    fn 捨てた直しが無ければその節を出さない() {
        let got = revise_prompt(REVIEW, &[], &[], "/a", "/b");
        assert!(!got.contains("捨てた直し"), "{got}");
    }

    #[test]
    fn 捨てた直しは理由と悪い向きに動いた指標と変えたところと繰り返さない指示を言う() {
        let got = revise_prompt(REVIEW, &[], &[rejected(2, 2)], "/a", "/b");
        let want = "\n## 捨てた直し\n\n\
            読む経路の文章を直した版を、これまでに 1 回捨てた。どれも採らなかった。\n\
            \n### 2 周目\n\n\
            - 採らなかった理由: 基準との距離が -0.957 から -0.997 に下がった。大きいほうが良い\n\
            - 悪い向きに動いた指標: 圧縮率 -0.820 → -0.857、エントロピー 0.064 → 0.034\n\
            - 変えたところ 2 か所:\n  \
            - 「0 を落としています。」→「0 を落とすようにしました。」\n  \
            - 「1 を落としています。」→「1 を落とすようにしました。」\n\
            \n同じ直しを繰り返さない。\n\n\
            - ここに示した変更と同じ変更をしない。同じ言い回しを、同じ向きに言い換えない。\n\
            - まだ手を付けていない指摘に取り組むか、同じ指摘に別のやり方で取り組む。\n\
            - 残っている指摘が、内容を変えずに言い回しだけでは直せないなら、変えるのを最小にとどめる。\n";
        assert!(got.contains(want), "{got}");
    }

    #[test]
    fn 載せる捨てた直しは新しいほうから上限までで古い順に並ぶ() {
        let all: Vec<Rejected> = (1..=5).map(|n| rejected(n, 1)).collect();
        let got = revise_prompt(REVIEW, &[], &all, "/a", "/b");
        assert!(
            got.contains(
                "これまでに 5 回捨てた。どれも採らなかった。新しいほうから 3 回ぶんを示す。"
            ),
            "{got}"
        );
        assert!(!got.contains("### 2 周目"), "{got}");
        let at = |s: &str| got.find(s).unwrap_or_else(|| panic!("{s} が無い:\n{got}"));
        assert!(at("### 3 周目") < at("### 4 周目") && at("### 4 周目") < at("### 5 周目"));
    }

    #[test]
    fn 変えたところは上限までを載せ全体の数を言う() {
        let got = revise_prompt(REVIEW, &[], &[rejected(2, MAX_CHANGES + 2)], "/a", "/b");
        assert!(
            got.contains(&format!(
                "- 変えたところ {} か所のうち、先頭から {MAX_CHANGES} か所:",
                MAX_CHANGES + 2
            )),
            "{got}"
        );
        assert!(got.contains(&format!("「{} を落としています。」", MAX_CHANGES - 1)));
        assert!(!got.contains(&format!("「{} を落としています。」", MAX_CHANGES)));
    }

    #[test]
    fn 足した文と消した文と変えなかった版はそう言う() {
        let mut r = rejected(2, 0);
        r.changes = vec![
            Change {
                before: String::new(),
                after: "足した文。".into(),
            },
            Change {
                before: "消した文。".into(),
                after: String::new(),
            },
        ];
        let got = revise_prompt(REVIEW, &[], &[r, rejected(3, 0)], "/a", "/b");
        assert!(got.contains("  - 「足した文。」を足した\n"), "{got}");
        assert!(got.contains("  - 「消した文。」を消した\n"), "{got}");
        assert!(
            got.contains("- 変えたところ: 無い。読む経路の文章と同じだった\n"),
            "{got}"
        );
    }

    #[test]
    fn 検めた結果はそのまま入れ囲みが途中で閉じない() {
        let review = "指摘\n```\nコード\n```";
        let got = revise_prompt(review, &[], &[], "/a", "/b");
        assert!(got.contains(review), "{got}");
        assert!(got.contains("````text\n"), "{got}");
    }

    #[test]
    fn 推論設定の直し方は入れない() {
        let got = revise_prompt(REVIEW, &[], &[rejected(2, 1)], "/a", "/b");
        assert!(!got.contains("推論設定"), "{got}");
    }
}
