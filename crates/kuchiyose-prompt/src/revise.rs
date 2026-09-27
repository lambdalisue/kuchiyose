//! 直させるプロンプト（[仕様](../../../docs/spec/400-write.md#直させるプロンプト)）。
//!
//! | 順 | 中身 |
//! | --- | --- |
//! | 1 | 役目。指摘された表現だけを直すこと |
//! | 2 | 守ること |
//! | 3 | 検めた結果。`review` の散文の出力を、そのまま入れる |
//! | 4 | 言い回しの上限。今の版が使っている言い回しと、本人の上限までの余地 |
//! | 5 | 捨てた直し。今の版を直して捨てた版があるときだけ |
//! | 6 | 語を揃える。今の版が基準との距離で止まっているときだけ |
//! | 7 | 自分で検める。検めるコマンドを渡したときだけ |
//! | 8 | 読む経路と書く経路 |
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

/// 1 周のうちに道具が自分で検めてよい回数。暫定値である。
///
/// 1 周の費用の上限である。 道具の側で数えて止める手段は無いので、指示で掛ける。
pub const MAX_CHECKS: usize = 6;

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

/// 直させるプロンプトの材料。
#[derive(Debug, Clone, Copy)]
pub struct ReviseRequest<'a> {
    /// 検めた結果の散文。判定を止める指摘と止めない知らせの区切りも含めてそのまま入れる。
    /// 直す側に貼るための形として決めた出力だからである。
    pub review: &'a str,
    /// 今の版が使っている言い回し。上限に近い順に渡す。載せるのは先頭から
    /// [`MAX_CEILINGS`] 本までである。
    pub ceilings: &'a [Ceiling],
    /// 今の版を直して捨てた版。古い順に渡す。載せるのは新しいほうから
    /// [`MAX_REJECTED`] 個までである。
    pub rejected: &'a [Rejected],
    /// 今の版が基準との距離で止まっているか。語を揃える直しを文章全体に許す。
    pub baseline_distance: bool,
    /// 道具が自分の版を検めるコマンド。道具がそのまま打てる形で渡す。空なら検めさせない。
    pub checks: &'a [String],
    /// 読む経路。
    pub read: &'a str,
    /// 書く経路。
    pub write: &'a str,
}

/// 直させるプロンプトを組み立てる。同じ材料からは同じバイト列が出る。
#[must_use]
pub fn revise_prompt(r: &ReviseRequest<'_>) -> String {
    let mut out = String::new();
    out.push_str("# 表現を直す依頼\n\n");
    out.push_str("次の文章の、検めた結果が指摘した表現だけを直す。直した全文を別の経路に書く。\n");
    out.push_str("\n## 守ること\n\n");
    out.push_str(
        "- 内容を変えない。主張、事実、例、見出しの構成、コード、リンクはそのまま残す。\n",
    );
    // 基準との距離で止まったときは、指摘に無い箇所も語を揃えるためになら書き換えさせる。
    // 指摘された箇所だけで語を揃えても、同じものを指す別の語が残り、基準との距離が動かない。
    if r.baseline_distance {
        out.push_str("- 指摘に無い箇所は、下の「語を揃える」に当たる直しのほかは書き換えない。\n");
    } else {
        out.push_str("- 指摘に無い箇所を書き換えない。\n");
    }
    out.push_str(
        "- 言い回しの上限を超えない。上限が示された言い回しは、その回数までにとどめる。\
         言い換えや文末を揃えるときも同じである。\n",
    );
    if r.checks.is_empty() {
        out.push_str("- 検めを自分で走らせない。検めて、採るかを決めるのは kuchiyose である。\n");
    } else {
        out.push_str(
            "- 検めは下の「自分で検める」のコマンドでだけ走らせる。採るかを決めるのは kuchiyose で\
             ある。\n",
        );
    }
    out.push_str(
        "- 指摘が頻度ペナルティや温度に触れていたら、その部分は読み飛ばす。\
         文章の直し方だけに従う。\n",
    );
    out.push_str("\n## 検めた結果\n\n");
    out.push_str(
        "kuchiyose review の出力である。「ここから下は判定に使っていない」より下は、判定を\
         止めない知らせである。そのうち、使いすぎている言い回し、残っている基準の型、残って\
         いる基準の語は、直せばこの人に寄るので直してよい。ほかの知らせは直さなくてよい。\n\n",
    );
    let fence = fence_for(r.review);
    let _ = writeln!(out, "{fence}text\n{}\n{fence}", r.review.trim_end());
    ceilings_section(&mut out, r.ceilings);
    rejected_section(&mut out, r.rejected);
    if r.baseline_distance {
        unify_section(&mut out);
    }
    checks_section(&mut out, r.checks);
    out.push_str("\n## 読む経路と書く経路\n\n");
    let _ = writeln!(
        out,
        "- 読む: {}\n- 書く: {}\n\n読む経路の文章を読み、直した全文を書く経路に書く。\
         読む経路のファイルは書き換えない。ほかのファイルを作らない。",
        r.read, r.write
    );
    out
}

/// 語を揃える。基準との距離で止まったときだけ置く。
///
/// 基準との距離を決める指標の多くは、語と言い回しの繰り返しで上がる。 いちばん安く
/// 上げる手は文末を揃えることで、それは本人に寄ったことにならない。 語と名詞句で稼がせ、
/// 文末で稼ぐことを名指しで禁じる。
fn unify_section(out: &mut String) {
    out.push_str("\n## 語を揃える\n\n");
    out.push_str(
        "今の版は基準との距離で止まっている。この節の直しに限り、指摘された箇所に限らず、\
         文章全体を書き換えてよい。\n\n\
         - 同じものや同じことを指すのに、別の語や言い回しを使い分けているところは、文章が\
         すでに使っている 1 つの語・言い回しに揃える。類義語で言い換えない。\n\
         - 「一度しか出てこない語」に挙がった語は、文章にすでに出ている語で同じ意味を言える\
         なら、その語に置き換える。題材の固有名や用語は残す。\n\
         - 1 つの指すものには 1 つの語を使い、同じ説明には同じ表現を繰り返す。指示語（これ、\
         それ、この仕組み）より、同じ名詞を繰り返す。\n\
         - この人が繰り返している言い回し（検めた結果に「本人が繰り返しているのは」として\
         出ている）は、内容に合う箇所ではそのまま使う。\n\
         - 文末を揃えて繰り返しを稼がない。文末は言い回しの上限を超えない。語や名詞句の\
         繰り返しで稼ぐ。\n\
         - 内容、主張、事実、例、見出し、コード、リンク、数字はそのまま残す。\n",
    );
}

/// 自分で検める。コマンドが無ければ何も書かない。
///
/// 検めは 1 回で指標ごとの値まで返すので、1 周の中で何度も直して確かめられる。
/// 文末を揃えることの禁止をいつも添える。 検めながら直す道具は、基準との距離を
/// いちばん安く上げる文末の揃えに流れる。
fn checks_section(out: &mut String, checks: &[String]) {
    if checks.is_empty() {
        return;
    }
    out.push_str("\n## 自分で検める\n\n");
    out.push_str(
        "書く経路に書いた版は、次のコマンドで自分で検めてよい。kuchiyose が採るかを決める\
         ときと同じ形代と基準で検める。\n\n",
    );
    let _ = writeln!(out, "```sh\n{}\n```\n", checks.join("\n"));
    if checks.len() > 1 {
        out.push_str(
            "1 つ目は検めた結果の散文を出す。2 つ目は指標ごとの値と、基準との距離の帯を出す。\n\n",
        );
    }
    let _ = writeln!(
        out,
        "- 先に書く経路に全文を書いてから検める。書くのはファイルを書く道具で行い、コマンドで\
         ファイルを作ったり写したりしない。\n\
         - コマンドは上に書いたとおりの文字列で、1 回に 1 つだけ打つ。`&&`、`;`、`|`、\
         リダイレクトでほかのコマンドとつながない。ほかのコマンドは打たない。\n\
         - 検めるのは合わせて {MAX_CHECKS} 回までにする。\n\
         - 良し悪しは kuchiyose と同じ順に見る。判定、止まった段、使いすぎている言い回しを\
         増やしていないか、止まった段の量の順である。\n\
         - 検めた版が前に検めた版より悪くなったら、前の版に戻してから別のやり方を試す。\n\
         - 文末を揃えて繰り返しを稼がない。使いすぎている言い回しが出たら減らす。使いすぎを\
         増やした版は kuchiyose が採らない。\n\
         - 終えるときは、検めた中でいちばん良かった版を書く経路に残す。採るかは kuchiyose が\
         検め直して決める。"
    );
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

    /// 照合値の段で止まり、検めるコマンドを渡さない依頼。
    fn plain(
        review: &str,
        ceilings: &[Ceiling],
        rejected: &[Rejected],
        read: &str,
        write: &str,
    ) -> String {
        revise_prompt(&ReviseRequest {
            review,
            ceilings,
            rejected,
            baseline_distance: false,
            checks: &[],
            read,
            write,
        })
    }

    const CHECKS: [&str; 2] = [
        "'/bin/kuchiyose' review '/w/round-1.md' --katashiro '/k/a.katashiro'",
        "'/bin/kuchiyose' review '/w/round-1.md' --katashiro '/k/a.katashiro' --values",
    ];

    fn with(baseline_distance: bool, checks: &[String]) -> String {
        revise_prompt(&ReviseRequest {
            review: REVIEW,
            ceilings: &[],
            rejected: &[rejected(2, 1)],
            baseline_distance,
            checks,
            read: "/w/round-0.md",
            write: "/w/round-1.md",
        })
    }

    fn checks() -> Vec<String> {
        CHECKS.iter().map(|c| (*c).to_owned()).collect()
    }

    #[test]
    fn 基準との距離で止まったときだけ文章全体で語を揃えてよいと言う() {
        let got = with(true, &[]);
        assert!(
            got.contains(
                "- 指摘に無い箇所は、下の「語を揃える」に当たる直しのほかは書き換えない。\n"
            ),
            "{got}"
        );
        assert!(!got.contains("- 指摘に無い箇所を書き換えない。\n"), "{got}");
        let want = "\n## 語を揃える\n\n\
            今の版は基準との距離で止まっている。この節の直しに限り、指摘された箇所に限らず、\
            文章全体を書き換えてよい。\n\n\
            - 同じものや同じことを指すのに、別の語や言い回しを使い分けているところは、文章が\
            すでに使っている 1 つの語・言い回しに揃える。類義語で言い換えない。\n";
        assert!(got.contains(want), "{got}");
        assert!(
            got.contains("- 文末を揃えて繰り返しを稼がない。文末は言い回しの上限を超えない。"),
            "文末で稼ぐことを禁じる: {got}"
        );
        assert!(
            got.contains("- 内容、主張、事実、例、見出し、コード、リンク、数字はそのまま残す。\n"),
            "{got}"
        );
    }

    #[test]
    fn ほかの段では指摘に無い箇所を書き換えさせない() {
        let got = with(false, &[]);
        assert!(got.contains("- 指摘に無い箇所を書き換えない。\n"), "{got}");
        assert!(!got.contains("## 語を揃える"), "{got}");
    }

    #[test]
    fn 検めるコマンドが無ければ検めを自分で走らせないと言う() {
        let got = with(true, &[]);
        assert!(
            got.contains(
                "- 検めを自分で走らせない。検めて、採るかを決めるのは kuchiyose である。\n"
            ),
            "{got}"
        );
        assert!(!got.contains("## 自分で検める"), "{got}");
    }

    #[test]
    fn 検めるコマンドを渡せばそのとおりの文字列と打ち方の決まりを言う() {
        let got = with(false, &checks());
        assert!(!got.contains("検めを自分で走らせない"), "{got}");
        assert!(
            got.contains("- 検めは下の「自分で検める」のコマンドでだけ走らせる。"),
            "{got}"
        );
        let block = format!("\n```sh\n{}\n{}\n```\n", CHECKS[0], CHECKS[1]);
        assert!(got.contains(&block), "{got}");
        for rule in [
            "- 先に書く経路に全文を書いてから検める。",
            "1 回に 1 つだけ打つ。`&&`、`;`、`|`、リダイレクトでほかのコマンドとつながない。",
            &format!("- 検めるのは合わせて {MAX_CHECKS} 回までにする。\n"),
            "- 文末を揃えて繰り返しを稼がない。",
            "- 終えるときは、検めた中でいちばん良かった版を書く経路に残す。",
        ] {
            assert!(got.contains(rule), "{rule} が無い: {got}");
        }
    }

    #[test]
    fn 語を揃える節と自分で検める節は捨てた直しのあと経路の前に並ぶ() {
        let got = with(true, &checks());
        let at = |s: &str| got.find(s).unwrap_or_else(|| panic!("{s} が無い:\n{got}"));
        let order = [
            at("## 捨てた直し"),
            at("## 語を揃える"),
            at("## 自分で検める"),
            at("## 読む経路と書く経路"),
        ];
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{got}");
    }

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
            plain(REVIEW, &[], &r, "/w/round-0.md", "/w/round-1.md"),
            plain(REVIEW, &[], &r, "/w/round-0.md", "/w/round-1.md")
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
        let got = plain(
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
        let got = plain(
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
        let got = plain(REVIEW, &all, &[], "/a", "/b");
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
        let got = plain(REVIEW, &[], &[], "/a", "/b");
        assert!(!got.contains("## 言い回しの上限"), "{got}");
    }

    #[test]
    fn 捨てた直しが無ければその節を出さない() {
        let got = plain(REVIEW, &[], &[], "/a", "/b");
        assert!(!got.contains("捨てた直し"), "{got}");
    }

    #[test]
    fn 捨てた直しは理由と悪い向きに動いた指標と変えたところと繰り返さない指示を言う() {
        let got = plain(REVIEW, &[], &[rejected(2, 2)], "/a", "/b");
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
        let got = plain(REVIEW, &[], &all, "/a", "/b");
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
        let got = plain(REVIEW, &[], &[rejected(2, MAX_CHANGES + 2)], "/a", "/b");
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
        let got = plain(REVIEW, &[], &[r, rejected(3, 0)], "/a", "/b");
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
        let got = plain(review, &[], &[], "/a", "/b");
        assert!(got.contains(review), "{got}");
        assert!(got.contains("````text\n"), "{got}");
    }

    #[test]
    fn 推論設定の直し方は入れない() {
        let got = plain(REVIEW, &[], &[rejected(2, 1)], "/a", "/b");
        assert!(!got.contains("推論設定"), "{got}");
    }
}
