//! 基準の短い文書を束ねて、本人の長さに届かせる。
//!
//! 基準の文書は何本かをまとめて 1 単位にすることがある。本人の文書は束ねない
//! （[短い文書は束ねる](../../../docs/spec/200-extract.md#短い文書は束ねる)）。
//!
//! LLM に書かせた文書は地の文 4,500 字あたりで頭打ちになるので、本人に長い記事が
//! あると[長さの範囲](crate::length_range_ok)で断られ、目盛りが作れない。
//!
//! 束ね方は道具が決める。 どこまで束ねれば長さの範囲に収まるかは測れた単位だけで
//! 決まるので、案を順に試し、長さで断られたときだけ次の案へ進む。

use std::collections::BTreeSet;

/// 目盛りに乗りそうな文書の長さ。地の文の日本語の文字数。
///
/// [長さの範囲](crate::length_range_ok)は測れた単位だけで測られる。 短すぎて落ちる
/// 文書まで数えて束ね方を決めると、実際より低いところから始まる分布に合わせてしまう。
///
/// 実測で、362 字の記事まで数えていたために本人の下端が低く見え、束ねた基準が
/// 上へ寄って防護柵に当たった——取り置きの総当たりで 4 分割のうち 1 つが目盛りを
/// 作れず、取り置いた 12 本が丸ごと判定できないになっていた。
///
/// 掛けるのは字数と読点の[除外](kakiburi_metrics::floor)だけである。
#[must_use]
pub fn measurable_length(chars: usize, commas: usize) -> Option<usize> {
    (chars >= kakiburi_metrics::floor::JAPANESE_CHARS
        && commas >= kakiburi_metrics::matching::COMMA_FLOOR)
        .then_some(chars)
}

/// 試す順に並べた束ね方。1 案目が本命で、残りは断られたときの控えである。
///
/// 返すのは基準の何番目をどう束ねるかである。中の並びは基準の添字で、束の中は
/// 短い順（同じ長さなら添字の順）に並ぶ。
///
/// 通るかどうかは作ってみないと分からない。 長さの範囲は測れた単位だけで
/// 測られ、どれが測れるかは全部を測るまで決まらない——ここで計算する重なりは
/// 近似でしかない。 だから選ぶのではなく、順番を付けて渡す。
///
/// 1 案目は、小さいほうから積んで本人の上端に届かせ、残りは単独で置く。 こうすると
/// 単独の分が下から中ほどを埋め、束ねた分が上端に届く。積むのに使う本数は基準の
/// 3 分の 1 までである。本人の上端が基準の上端より短ければ、この案は作らない。
///
/// 一律に束ねない。 全部を同じ本数で束ねると、大きいものどうしが合わさって
/// 本人の上端を大きく超え、今度は基準側の範囲が広がりすぎる。実測で、一律 3 本に
/// したら本人側 70% / 基準側 42% になった。
///
/// 最大化しにいくと別の場所が壊れる。 実測で、重なりを最大にする案に置き換え
/// たら目盛りはすべて作れるようになったが、本人の通過が 15 本から 9 本へ落ちた
/// ——長さの重なりが最大の案が、値の帯まで良くしてくれるわけではない。
#[must_use]
pub fn bundle_plans(person: &[usize], pool: &[usize]) -> Vec<Vec<Vec<usize>>> {
    let single = || -> Vec<Vec<usize>> { (0..pool.len()).map(|i| vec![i]).collect() };
    let (Some(&p_hi), Some(&b_hi)) = (person.iter().max(), pool.iter().max()) else {
        return vec![single()];
    };
    let mut order: Vec<usize> = (0..pool.len()).collect();
    order.sort_by_key(|&i| pool[i]);

    let mut out: Vec<Vec<Vec<usize>>> = Vec::new();
    if b_hi > 0 && p_hi > b_hi {
        out.push(bundle_to(&order, pool, p_hi, pool.len() / 3));
    }
    out.push(single());

    // 控えは重なりの良い順。 近似でしかないが、順番を付ける材料はこれしかない。
    let mut rest: Vec<(f64, Vec<Vec<usize>>)> = Vec::new();
    for step in 1..=12u32 {
        let target = p_hi * step as usize / 12;
        for div in [2usize, 3, 4, 6] {
            let plan = bundle_to(&order, pool, target, pool.len() / div);
            if out.contains(&plan) || rest.iter().any(|(_, p)| *p == plan) {
                continue;
            }
            rest.push((overlap_score(person, &lengths_of(&plan, pool)), plan));
        }
    }
    rest.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    out.extend(rest.into_iter().map(|(_, p)| p));
    out
}

/// 目標の上端まで小さいほうから積む。残りは単独で置く。
fn bundle_to(order: &[usize], pool: &[usize], target: usize, budget: usize) -> Vec<Vec<usize>> {
    let mut plan: Vec<Vec<usize>> = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    let mut sum = 0usize;
    for (used, &i) in order.iter().enumerate() {
        if used >= budget {
            break;
        }
        cur.push(i);
        sum += pool[i];
        if sum >= target {
            plan.push(std::mem::take(&mut cur));
            sum = 0;
        }
    }
    if cur.len() > 1 {
        plan.push(cur);
    }
    let bundled: BTreeSet<usize> = plan.iter().flatten().copied().collect();
    plan.extend(
        order
            .iter()
            .filter(|i| !bundled.contains(i))
            .map(|&i| vec![i]),
    );
    plan
}

/// 束ねた結果の、単位ごとの長さ。
#[must_use]
pub fn lengths_of(plan: &[Vec<usize>], pool: &[usize]) -> Vec<usize> {
    plan.iter()
        .map(|g| g.iter().map(|&i| pool[i]).sum())
        .collect()
}

/// [防護柵](crate::length_range_ok)の採点。小さいほうの比を返す。
///
/// 片方だけ良くても通らないので、最大化するのは悪いほうである。
#[must_use]
pub fn overlap_score(person: &[usize], baseline: &[usize]) -> f64 {
    let span = |v: &[usize]| -> Option<(f64, f64)> {
        #[allow(clippy::cast_precision_loss)]
        Some((*v.iter().min()? as f64, *v.iter().max()? as f64))
    };
    let (Some((plo, phi)), Some((blo, bhi))) = (span(person), span(baseline)) else {
        return 0.0;
    };
    let overlap = (phi.min(bhi) - plo.max(blo)).max(0.0);
    (overlap / (phi - plo).max(1.0)).min(overlap / (bhi - blo).max(1.0))
}

/// 目盛りを作ろうとして、どう終わったか。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attempt {
    /// 作れた。
    Built,
    /// [長さの範囲](crate::length_range_ok)で断られた。 束ね方で
    /// 基準の長さが変わるので、別の束ね方なら通りうる。
    LengthRange,
    /// ほかの理由で止まった。 束ね直しでは直らない。
    Stopped,
}

/// 束ね方を順に試す。長さの範囲で断られたときだけ次の案へ進む。
///
/// ほかの止まり方で次へ進まない。 天井と床が重なった、単位が足りない、
/// 自己検査が崩れた——どれも束ね方で直るものではない。 それでも試し続ければ、
/// 通るまで基準の組み合わせを探したことになり、通った 1 案だけが残って、
/// 最初に止まった理由が消える。
///
/// 全部の案が断られたら、最後の断りを返す。
///
/// # Errors
///
/// 試している途中で `attempt` がエラーを返せば、そこで止めて返す。
pub fn try_plans<P, E>(
    plans: Vec<P>,
    mut attempt: impl FnMut(usize, P) -> Result<Attempt, E>,
) -> Result<Attempt, E> {
    let mut last = Attempt::Stopped;
    for (i, plan) in plans.into_iter().enumerate() {
        last = attempt(i, plan)?;
        if last != Attempt::LengthRange {
            break;
        }
    }
    Ok(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle_plan(person: &[usize], pool: &[usize]) -> Vec<Vec<usize>> {
        bundle_plans(person, pool).swap_remove(0)
    }

    #[test]
    fn 基準が本人の長さに届かなければ束ねる() {
        // 基準の 1 本は地の文 4,500 字あたりで頭打ちになる。 本人に長い記事が
        // あると長さの範囲の防護柵に当たり、目盛りが作れない。
        let pool = vec![2200, 2900, 3000, 4400, 2400, 2600, 3200, 3800, 2300];
        let plan = bundle_plan(&[1500, 3000, 4000], &pool);
        assert!(
            plan.iter().all(|g| g.len() == 1),
            "届くなら束ねない: {plan:?}"
        );

        let plan = bundle_plan(&[1500, 7000], &pool);
        assert!(
            plan.iter().any(|g| g.len() > 1),
            "届かないなら束ねる: {plan:?}"
        );
        assert!(
            plan.iter().filter(|g| g.len() == 1).count() >= pool.len() / 2,
            "単独の分を残す——全部束ねると単位の本数が下限を割る: {plan:?}"
        );
        let used: Vec<usize> = plan.iter().flatten().copied().collect();
        assert_eq!(used.len(), pool.len(), "基準を余さず使う");
    }

    #[test]
    fn 束の中は短い順に並ぶ() {
        // 並びが変われば node の並びが変わり、決定性が壊れる。
        let pool = vec![3000, 2000, 2000, 4000];
        let plan = bundle_plan(&[1500, 9000], &pool);
        for g in plan.iter().filter(|g| g.len() > 1) {
            let lens: Vec<(usize, usize)> = g.iter().map(|&i| (pool[i], i)).collect();
            let mut sorted = lens.clone();
            sorted.sort_unstable();
            assert_eq!(lens, sorted, "短い順、同じ長さなら添字の順");
        }
    }

    #[test]
    fn 束ね方は防護柵の式で選ぶ() {
        // 勘で決めた 1 案では落ちる素材がある。 候補を作って、通さなければ
        // ならない式そのもので採点する。
        let pool = vec![2200, 2900, 3000, 4400, 2400, 2600, 3200, 3800, 2300];
        for person in [
            vec![1500, 7000],
            vec![900, 12000],
            vec![3000, 3200, 18000],
            vec![2500, 2600, 2700],
        ] {
            let plan = bundle_plan(&person, &pool);
            let got = overlap_score(&person, &lengths_of(&plan, &pool));
            let flat: Vec<Vec<usize>> = (0..pool.len()).map(|i| vec![i]).collect();
            let flat_score = overlap_score(&person, &lengths_of(&flat, &pool));
            assert!(
                got >= flat_score,
                "束ねて悪くなる案は採らない: {person:?} で {got} < {flat_score}"
            );
            let used: Vec<usize> = plan.iter().flatten().copied().collect();
            assert_eq!(used.len(), pool.len(), "基準を余さず使う: {person:?}");
        }
    }

    #[test]
    fn 長さの範囲以外で止まったら束ね直さない() {
        // 帯が重なった、単位が足りない——束ね方を変えて通るまで試せば、
        // 通った 1 案だけが残り、止まった理由が消える。
        let mut tried = Vec::new();
        let got: Result<Attempt, ()> = try_plans(vec!["a", "b", "c"], |_, p| {
            tried.push(p);
            Ok(Attempt::Stopped)
        });
        assert_eq!(got, Ok(Attempt::Stopped));
        assert_eq!(tried, vec!["a"], "1 案目で止める");
    }

    #[test]
    fn 長さの範囲で断られたときだけ次の案へ進む() {
        let mut tried = Vec::new();
        let got: Result<Attempt, ()> = try_plans(vec!["a", "b", "c", "d"], |_, p| {
            tried.push(p);
            Ok(if p == "c" {
                Attempt::Built
            } else {
                Attempt::LengthRange
            })
        });
        assert_eq!(got, Ok(Attempt::Built));
        assert_eq!(tried, vec!["a", "b", "c"], "通ったら残りは試さない");

        // 全部断られたら、最後の断りをそのまま返す。
        let got: Result<Attempt, ()> = try_plans(vec!["a", "b"], |_, _| Ok(Attempt::LengthRange));
        assert_eq!(got, Ok(Attempt::LengthRange));
    }

    #[test]
    fn 束ね直しの途中で誤りが出たらそこで返す() {
        let mut tried = 0;
        let got: Result<Attempt, &str> = try_plans(vec!["a", "b"], |_, _| {
            tried += 1;
            Err("誤り")
        });
        assert_eq!(got, Err("誤り"));
        assert_eq!(tried, 1);
    }

    #[test]
    fn 採点は悪いほうの比を返す() {
        // 片方だけ良くても通らない。 防護柵は両方に 0.5 を要求する。
        let wide = overlap_score(&[1000, 5000], &[2000, 3000]);
        assert!(wide < 0.5, "基準が狭すぎれば落ちる: {wide}");
        let same = overlap_score(&[1000, 5000], &[1000, 5000]);
        assert!((same - 1.0).abs() < 1e-9, "同じ範囲なら 1.0: {same}");
    }

    #[test]
    fn 基準が空でも落ちない() {
        assert!(bundle_plan(&[1500], &[]).is_empty());
        assert_eq!(bundle_plan(&[], &[2000]).len(), 1);
    }

    #[test]
    fn 長さは字数と読点の除外に掛かる文書を数えない() {
        let floor = kakiburi_metrics::floor::JAPANESE_CHARS;
        assert_eq!(measurable_length(floor, 5), Some(floor));
        assert_eq!(measurable_length(floor - 1, 50), None, "字数が足りない");
        assert_eq!(measurable_length(floor * 3, 4), None, "読点が足りない");
    }
}
