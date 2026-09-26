//! 2 つのカセットを開いて、目盛りを組み立てる（[目盛りを組み立てる](../../../docs/design/200-command.md#目盛りを組み立てる)）。
//!
//! `review`、`cassette diff`、`cassette list` が同じ手順を通す。 口ごとに書けば、
//! 同じ組み合わせが口によって違う目盛りになる。

use kakiburi_cassette::json::Value;
use kakiburi_cassette::{Register, Tuning};
use kakiburi_scale::assembly::{self, Assembly};
use kakiburi_scale::select::{self, RegisterFilter, RegisterSource};
use kakiburi_scale::self_check::{SelfCheck, HIGHER_RATE_FLOOR};
use kakiburi_scale::stats::CassetteStats;

use crate::cassettes::{self, Opened, Origin};
use crate::exit::Exit;
use crate::remedies::FromDefinitions;

/// 組み立ての関数。本人と基準の統計値と、組み立てに効く調整だけを受け取る。
pub type Assemble<'a> = dyn Fn(&CassetteStats, &CassetteStats, assembly::Tuning, &dyn Fn(&str) -> bool) -> Assembly
    + 'a;

/// 開いた 2 つのカセット。本人の側と基準の側は別の欄に入り、混ざらない。
pub struct Pair {
    /// 本人の側。
    pub target: Opened,
    /// 基準の側。
    pub baseline: Opened,
}

/// `--baseline` の値から基準の在りかを決める。省けば同梱の基準である。
#[must_use]
pub fn baseline_origin(path: Option<String>) -> Origin {
    path.map_or(Origin::Bundled, Origin::File)
}

/// 2 つを開き、それぞれ同じ検査を通し、互いの指紋を照らす。
///
/// # Errors
///
/// 読めなければ 65、指紋が今の道具か互いと合わなければ 66 で断る。
pub fn open(target: Origin, baseline: Origin, defs: &FromDefinitions) -> Result<Pair, Exit> {
    let target = cassettes::open(target, defs)?;
    let baseline = cassettes::open(baseline, defs)?;
    // 解析器や定義の違う 2 つを並べれば、書き手の差ではなく道具の差を測ることになる。
    let diff = target
        .cassette
        .fingerprint()
        .differences(&baseline.cassette.inputs);
    if !diff.is_empty() {
        eprintln!("本人と基準のカセットの指紋が合わない: {}", diff.join("、"));
        eprintln!("同じ道具で作ったカセットどうしでなければ並べない");
        return Err(Exit::FingerprintMismatch);
    }
    Ok(Pair { target, baseline })
}

impl Pair {
    /// 目盛りを組み立てる。本人のカセットの文体の申告だけが組み立てに効く。
    pub fn assemble(&self, assemble: &Assemble<'_>, defs: &FromDefinitions) -> Assembly {
        assemble(
            &self.target.stats,
            &self.baseline.stats,
            assembly::Tuning {
                register: register_of(&self.target.cassette.tuning),
            },
            &|n| defs.by_appearance(n),
        )
    }
}

/// 本人のカセットの文体の申告を、組み立てに渡す形にする。
#[must_use]
pub fn register_of(t: &Tuning) -> Option<select::Register> {
    t.register.map(|r| match r {
        Register::Polite => select::Register::Polite,
        Register::Plain => select::Register::Plain,
    })
}

/// 基準を文体でどう絞ったか。
#[must_use]
pub fn register_note(f: &RegisterFilter) -> String {
    let source = |s: &RegisterSource| match s {
        RegisterSource::Counted => "数えた結果",
        RegisterSource::Declared => "申告",
    };
    match f {
        RegisterFilter::Matched {
            register,
            source: s,
            person,
            candidates,
        } => format!(
            "本人は{}で書いている（{}。敬体 {} 本 / 常体 {} 本）。基準から{}の {candidates} 本を候補にした",
            register.name(),
            source(s),
            person.polite,
            person.plain,
            register.name()
        ),
        RegisterFilter::Undecided { person } => format!(
            "本人の文体が決まらない（敬体 {} 本 / 常体 {} 本）。基準を全部候補にした",
            person.polite, person.plain
        ),
        RegisterFilter::Missing {
            register,
            source: s,
            person,
        } => format!(
            "本人は{}で書いている（{}。敬体 {} 本 / 常体 {} 本）が、基準に{}の文書が無い。基準を全部候補にした",
            register.name(),
            source(s),
            person.polite,
            person.plain,
            register.name()
        ),
    }
}

/// 長さで断られて基準を選び直したことを言う。選び直していなければ `None`。
#[must_use]
pub fn length_window_note(a: &Assembly) -> Option<String> {
    let w = a.length_window.as_ref()?;
    Some(format!(
        "長さの範囲で断られたので、基準を本人の記事の長さ（地の文 {}〜{} 字）に入る文書から選び直した",
        w.start(),
        w.end()
    ))
}

/// 選び直したときの長さの窓。道具向け。選び直していなければ null。
#[must_use]
pub fn length_window_json(a: &Assembly) -> Value {
    #[allow(clippy::cast_precision_loss)]
    a.length_window.as_ref().map_or(Value::Null, |w| {
        Value::obj([
            ("low".to_owned(), Value::Number(*w.start() as f64)),
            ("high".to_owned(), Value::Number(*w.end() as f64)),
        ])
    })
}

/// 自己検査が崩れていれば、見分けられない理由を返す。
#[must_use]
pub fn self_check_failure(check: Option<&SelfCheck>) -> Option<String> {
    /// 名指す逆に出た対の数。いちばん惜しい対から並ぶ。
    const INVERTED_SHOWN: usize = 5;
    let Some(check) = check else {
        return Some(
            "本人がいちばん高く出るかを確かめられない。照合値を出せる単位が片側に無い".into(),
        );
    };
    if check.passed() {
        return None;
    }
    let pairs: Vec<String> = check
        .inverted
        .iter()
        .take(INVERTED_SHOWN)
        .map(|(p, pv, b, bv)| format!("本人 {p} {pv:.3} ≤ 基準 {b} {bv:.3}"))
        .collect();
    Some(format!(
        "本人を基準と見分けられない（本人が高く出た対の割合 {:.3} / 下限 {HIGHER_RATE_FLOOR}、{} 対のうち逆に出た対 {} 対: {}）",
        check.rate,
        check.pairs,
        check.inverted.len(),
        pairs.join("、")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kakiburi_cassette::Register;

    #[test]
    fn 自己検査が崩れていれば逆に出た対を添えて理由にする() {
        let check = SelfCheck::of(
            &[("p1".to_owned(), 1.0), ("p2".to_owned(), 3.0)],
            &[("b1".to_owned(), 2.0)],
        )
        .expect("確かめられる");
        let reason = self_check_failure(Some(&check)).expect("崩れている");
        assert!(reason.contains("本人を基準と見分けられない"), "{reason}");
        assert!(reason.contains("p1"), "{reason}");
        assert!(reason.contains("b1"), "{reason}");
        let fine = SelfCheck::of(&[("p1".to_owned(), 3.0)], &[("b1".to_owned(), 1.0)]).unwrap();
        assert_eq!(
            self_check_failure(Some(&fine)),
            None,
            "崩れていなければ何も言わない"
        );
    }

    #[test]
    fn 文体の申告は組み立てに渡る() {
        let mut t = Tuning::default();
        assert_eq!(register_of(&t), None);
        t.register = Some(Register::Polite);
        assert_eq!(register_of(&t), Some(select::Register::Polite));
    }

    #[test]
    fn 長さで選び直したときだけ窓を名乗る() {
        let mut a = Assembly {
            register: RegisterFilter::Undecided {
                person: select::RegisterCount::default(),
            },
            picked: Vec::new(),
            length_window: None,
            bundles: Vec::new(),
            tried: 1,
            outcome: Err(assembly::Stopped {
                error: kakiburi_scale::ScaleError::SharedPairs,
                person: Vec::new(),
                baseline: Vec::new(),
            }),
        };
        assert_eq!(length_window_note(&a), None);
        assert_eq!(length_window_json(&a), Value::Null);

        a.length_window = Some(1001..=2278);
        let note = length_window_note(&a).expect("選び直した");
        assert!(note.contains("1001〜2278 字"), "{note}");
        assert_eq!(
            length_window_json(&a),
            Value::obj([
                ("low".to_owned(), Value::Number(1001.0)),
                ("high".to_owned(), Value::Number(2278.0)),
            ])
        );
    }

    #[test]
    fn 基準を省けば同梱の基準を使う() {
        assert_eq!(baseline_origin(None), Origin::Bundled);
        assert_eq!(
            baseline_origin(Some("a.kb".into())),
            Origin::File("a.kb".into())
        );
    }
}
