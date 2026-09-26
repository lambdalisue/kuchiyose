//! 調整する（[調整する](../../../docs/design/200-command.md#調整する)）。
//!
//! `cassette list`・`mute`・`unmute`・`first-person`・`register`・`edit`。
//! どれも測った値に触らないので、作り直しは要らない。書いたら次の `review` から効く。
//!
//! 型・基準の型・基準の語は、組み立てるまで決まらない。 並べるときも名前を照らす
//! ときも、基準と組み合わせて組み立てた一覧を使う。

use std::io::{IsTerminal, Write};

use kakiburi_cassette::{save, sha256, Cassette, MuteKind, Register, Tuning};
use kakiburi_scale::assembly::{self, Built};
use kakiburi_scale::select::RegisterCount;
use kakiburi_scale::stats::CassetteStats;

use crate::cassettes::{self, Origin};
use crate::directive_names;
use crate::exit::Exit;
use crate::pair;
use crate::remedies::FromDefinitions;

/// 調整の種類。無効にする種類と、申告する種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subject {
    /// 無効にする。
    Mute(MuteKind),
    /// 一人称を申告する。
    FirstPerson,
    /// 文体を申告する。
    Register,
}

impl Subject {
    /// 全部。`--kind` に渡せる名前はここから出す。
    pub const ALL: [Subject; 6] = [
        Subject::Mute(MuteKind::Metric),
        Subject::Mute(MuteKind::Kata),
        Subject::Mute(MuteKind::BaselineKata),
        Subject::Mute(MuteKind::BaselineGoi),
        Subject::FirstPerson,
        Subject::Register,
    ];

    /// `--kind` と一覧の 1 列目に出す名前。
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Subject::Mute(k) => k.name(),
            Subject::FirstPerson => "一人称",
            Subject::Register => "文体",
        }
    }

    /// 名前から引く。
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }

    /// `--kind` に渡せる名前を並べる。
    #[must_use]
    pub fn names() -> String {
        Self::ALL.map(Subject::name).join("、")
    }
}

/// 一覧の 1 行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// 種類。
    pub subject: Subject,
    /// 名前か ID。`mute` に渡すもの。
    pub name: String,
    /// `tuning.json` に持つ文字列。指標は名前、型は言い回し、語は語彙素。
    pub text: String,
    /// 無効にしてあるか。申告する種類では `None`。
    pub muted: Option<bool>,
    /// 状態の欄。無効にする種類では `on` か `off`、申告する種類では今の値。
    pub state: String,
    /// 説明。
    pub description: String,
}

impl Item {
    /// タブで区切った 1 行。見出しの行は出さない——fzf のような道具とそのまま組み合わせる。
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}",
            self.subject.name(),
            self.name,
            self.state,
            self.description
        )
    }
}

/// 言い回しの ID。種類の名前と、言い回しの文字列から作る短いハッシュを繋ぐ。
///
/// 同じ言い回しなら基準を替えても同じ ID になる。 並びの番号にすると、基準を
/// 替えただけで別の言い回しを指す。
#[must_use]
pub fn phrase_id(kind: MuteKind, text: &str) -> String {
    format!("{}-{}", kind.name(), &sha256::hex(text)[..6])
}

/// 本人のカセットから数えた、一人称と文体。申告と並べて見せる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counted {
    /// いちばん多くの文書に現れた一人称。
    pub first_person: Option<String>,
    /// 文体の内訳。
    pub register: RegisterCount,
}

impl Counted {
    /// 統計値から数える。
    #[must_use]
    pub fn of(stats: &CassetteStats) -> Self {
        let mut docs: std::collections::BTreeMap<&str, (usize, usize)> =
            std::collections::BTreeMap::new();
        for d in &stats.documents {
            for (name, n) in d.first_person.iter().filter(|(_, n)| **n > 0) {
                let e = docs.entry(name.as_str()).or_default();
                e.0 += 1;
                e.1 += n;
            }
        }
        // 現れた文書の数、延べの数、名前の順。
        let first_person = docs
            .into_iter()
            .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0)))
            .map(|(n, _)| n.to_owned());
        Self {
            first_person,
            register: RegisterCount::of(stats.documents.iter().map(|d| d.polite_share)),
        }
    }

    fn register_name(&self) -> &'static str {
        self.register.majority().map_or("決まらない", |r| match r {
            kakiburi_scale::select::Register::Polite => "polite",
            kakiburi_scale::select::Register::Plain => "plain",
        })
    }
}

/// 百分率。
fn percent(rate: f64) -> String {
    format!("{:.0}%", rate * 100.0)
}

/// 型が文書のどこで使われるか。
fn position(at: f64) -> &'static str {
    if at < 0.2 {
        "書き出し"
    } else if at > 0.8 {
        "結び"
    } else {
        "本文"
    }
}

/// 無効にする種類の 1 行を作る。
fn mute_item(t: &Tuning, kind: MuteKind, name: String, text: String, about: String) -> Item {
    let muted = t.is_muted(kind, &text);
    let by_kind = t.mute_kinds.contains(&kind) && !t.muted(kind).contains(&text.as_str());
    Item {
        subject: Subject::Mute(kind),
        name,
        text,
        muted: Some(muted),
        state: if muted { "off" } else { "on" }.to_owned(),
        description: if by_kind {
            format!("{about}（種類ごと無効）")
        } else {
            about
        },
    }
}

/// 調整できるものを全部並べる。
///
/// `built` が無ければ、組み立てて決まる言い回しは、無効にしてあるものだけを並べる。
/// 無効にした言い回しは、今の組み合わせで出ていなくても捨てない——別の基準と
/// 組み合わせたときに出てくれば、そこで効く。戻せるように並べておく。
#[must_use]
pub fn items(
    t: &Tuning,
    defs: &FromDefinitions,
    built: Option<&Built>,
    counted: &Counted,
) -> Vec<Item> {
    let mut out: Vec<Item> = directive_names()
        .into_iter()
        .map(|name| {
            let about = defs.meaning(&name).unwrap_or_default().to_owned();
            mute_item(t, MuteKind::Metric, name.clone(), name, about)
        })
        .collect();

    let mut phrases: Vec<Item> = Vec::new();
    if let Some(b) = built {
        let s = &b.scale;
        for k in &s.katas {
            let text = k.shown();
            let about = format!("{text}（本人 {} / {}）", percent(k.rate), position(k.at));
            phrases.push(mute_item(
                t,
                MuteKind::Kata,
                phrase_id(MuteKind::Kata, &text),
                text,
                about,
            ));
        }
        // 基準の型は役を入れ替えて取り出したものなので、割合は基準の側のものである。
        for k in &s.machine_katas {
            let text = k.shown();
            let about = format!(
                "{text}（基準 {} / 本人 {}）",
                percent(k.rate),
                percent(k.base)
            );
            phrases.push(mute_item(
                t,
                MuteKind::BaselineKata,
                phrase_id(MuteKind::BaselineKata, &text),
                text,
                about,
            ));
        }
        for g in &s.machine_gois {
            let mut about = format!(
                "{}（基準 {} / 本人 {}）",
                g.text,
                percent(g.rate),
                percent(g.base)
            );
            if !g.theirs.is_empty() {
                about.push_str(&format!("。本人は {}", g.theirs.join("、")));
            }
            phrases.push(mute_item(
                t,
                MuteKind::BaselineGoi,
                g.text.clone(),
                g.text.clone(),
                about,
            ));
        }
    }
    for kind in [
        MuteKind::Kata,
        MuteKind::BaselineKata,
        MuteKind::BaselineGoi,
    ] {
        for text in t.muted(kind) {
            let shown = phrases
                .iter()
                .any(|i| i.subject == Subject::Mute(kind) && i.text == text);
            if shown {
                continue;
            }
            let name = if kind == MuteKind::BaselineGoi {
                text.to_owned()
            } else {
                phrase_id(kind, text)
            };
            phrases.push(mute_item(
                t,
                kind,
                name,
                text.to_owned(),
                format!("{text}（今の組み合わせでは出ていない）"),
            ));
        }
    }
    // 種類の順に並べる。 同じ種類の中は組み立てが出した順のまま。
    for kind in [
        MuteKind::Kata,
        MuteKind::BaselineKata,
        MuteKind::BaselineGoi,
    ] {
        out.extend(
            phrases
                .iter()
                .filter(|i| i.subject == Subject::Mute(kind))
                .cloned(),
        );
    }

    let counted_person = counted.first_person.as_deref().unwrap_or("無い");
    out.push(Item {
        subject: Subject::FirstPerson,
        name: Subject::FirstPerson.name().to_owned(),
        text: t.first_person.clone().unwrap_or_default(),
        muted: None,
        state: match &t.first_person {
            Some(f) => format!("{f}（申告）"),
            None => format!("{counted_person}（数えた結果）"),
        },
        description: format!("数えた結果は {counted_person}"),
    });
    out.push(Item {
        subject: Subject::Register,
        name: Subject::Register.name().to_owned(),
        text: t
            .register
            .map(Register::name)
            .unwrap_or_default()
            .to_owned(),
        muted: None,
        state: match t.register {
            Some(r) => format!("{}（申告）", r.name()),
            None => format!("{}（数えた結果）", counted.register_name()),
        },
        description: format!(
            "数えた結果は {}（敬体 {} 本 / 常体 {} 本）",
            counted.register_name(),
            counted.register.polite,
            counted.register.plain
        ),
    });
    out
}

/// 絞る。`--state` は無効にする種類にだけ効く。
#[must_use]
pub fn filter(items: Vec<Item>, kind: Option<Subject>, state: Option<bool>) -> Vec<Item> {
    items
        .into_iter()
        .filter(|i| kind.is_none_or(|k| i.subject == k))
        .filter(|i| match (state, i.muted) {
            (Some(want_off), Some(off)) => want_off == off,
            _ => true,
        })
        .collect()
}

/// 名前か ID か文字列で、無効にする種類の項目を引く。
fn resolve<'a>(items: &'a [Item], arg: &str) -> Vec<&'a Item> {
    items
        .iter()
        .filter(|i| i.muted.is_some() && (i.name == arg || i.text == arg))
        .collect()
}

/// 名前を照らした結果。
#[derive(Debug, PartialEq, Eq)]
pub enum Lookup {
    /// 全部引けた。種類と `tuning.json` に持つ文字列。
    Found(Vec<(MuteKind, String)>),
    /// 引けなかった名前がある。
    Unknown(Vec<String>),
    /// 2 つ以上に当たる名前がある。名前と、当たった ID。
    Ambiguous(Vec<(String, Vec<String>)>),
}

/// 名前を全部照らす。1 つでも引けなければ何も返さない——半分だけ書けば、
/// どこまで効いたかを使う人が確かめ直すことになる。
#[must_use]
pub fn lookup(items: &[Item], args: &[String]) -> Lookup {
    let mut found = Vec::new();
    let mut unknown = Vec::new();
    let mut ambiguous = Vec::new();
    for arg in args {
        match resolve(items, arg).as_slice() {
            [] => unknown.push(arg.clone()),
            [one] => {
                if let Subject::Mute(k) = one.subject {
                    found.push((k, one.text.clone()));
                }
            }
            many => ambiguous.push((
                arg.clone(),
                many.iter()
                    .map(|i| format!("{} {}", i.subject.name(), i.name))
                    .collect(),
            )),
        }
    }
    if !unknown.is_empty() {
        Lookup::Unknown(unknown)
    } else if !ambiguous.is_empty() {
        Lookup::Ambiguous(ambiguous)
    } else {
        Lookup::Found(found)
    }
}

/// 申告できる一人称。数える一人称の閉じた集合から出す。
///
/// 集合の外の語を申告すると、草稿の側では数えられないので、使っていても
/// 「違う一人称」と言い続ける。
#[must_use]
pub fn first_person_choices() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for lemma in kakiburi_metrics::word::FIRST_PERSON {
        let shown = kakiburi_metrics::word::first_person_name(lemma);
        if !out.contains(&shown) {
            out.push(shown);
        }
    }
    out
}

/// 一人称の申告を読む。`auto` なら申告を消す。
///
/// # Errors
///
/// 数える一人称の集合に無ければ断る。
pub fn parse_first_person(v: &str) -> Result<Option<String>, String> {
    if v == "auto" {
        return Ok(None);
    }
    if first_person_choices().contains(&v) {
        Ok(Some(v.to_owned()))
    } else {
        Err(format!(
            "申告できる一人称ではない: {v}。{} か auto",
            first_person_choices().join("、")
        ))
    }
}

/// 文体の申告を読む。`auto` なら申告を消す。
///
/// # Errors
///
/// `polite`・`plain`・`auto` のほかは断る。
pub fn parse_register(v: &str) -> Result<Option<Register>, String> {
    if v == "auto" {
        return Ok(None);
    }
    Register::from_name(v)
        .map(Some)
        .ok_or_else(|| format!("文体は polite（敬体）か plain（常体）か auto: {v}"))
}

/// 引数。位置引数・`--kind`・`--state`・`--baseline`。
#[derive(Default)]
struct Args {
    cassette: Option<String>,
    rest: Vec<String>,
    kind: Option<String>,
    state: Option<String>,
    baseline: Option<String>,
}

fn parse_args(args: &[String], takes: &[&str]) -> Result<Args, Exit> {
    let mut out = Args::default();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if a.starts_with("--") {
            if !takes.contains(&a) {
                eprintln!("知らない引数: {a}");
                return Err(Exit::Usage);
            }
            let Some(v) = args.get(i + 1) else {
                eprintln!("{a} に値を渡す");
                return Err(Exit::Usage);
            };
            let slot = match a {
                "--kind" => &mut out.kind,
                "--state" => &mut out.state,
                _ => &mut out.baseline,
            };
            *slot = Some(v.clone());
            i += 2;
            continue;
        }
        if out.cassette.is_none() {
            out.cassette = Some(a.to_owned());
        } else {
            out.rest.push(a.to_owned());
        }
        i += 1;
    }
    Ok(out)
}

/// 本人のカセットを読む。容器・版・中身のハッシュを検める。
fn read(path: Option<&str>, help: &str) -> Result<(String, Cassette), Exit> {
    let Some(path) = path else {
        eprintln!("カセットを渡す");
        eprintln!("{help}");
        return Err(Exit::Usage);
    };
    let c = cassettes::read_file(path)?;
    Ok((path.to_owned(), c))
}

/// 書く。読んだときの世代と照らしてから置き換える。
fn write(path: &str, c: &Cassette) -> Result<(), Exit> {
    save::save(path, c, Some(c.generation)).map_err(|e| {
        eprintln!("断る: 書けない: {e}");
        Exit::Unreadable
    })
}

/// 基準と組み合わせて組み立て、並べるものを全部作る。
///
/// 目盛りが組み立てられなくても止めない。 言い回しが並ばないだけで、指標と
/// 申告は並べられる。止まった理由は stderr に出す。
fn assembled_items(
    path: &str,
    baseline: Option<String>,
    defs: &FromDefinitions,
) -> Result<Vec<Item>, Exit> {
    let pair = pair::open(
        Origin::File(path.to_owned()),
        pair::baseline_origin(baseline),
        defs,
    )?;
    let built = pair.assemble(&assembly::assemble_stats, defs);
    if let Err(stopped) = &built.outcome {
        eprintln!(
            "目盛りが組み立てられないので、型・基準の型・基準の語は並べられない: {}",
            stopped.error
        );
    }
    Ok(items(
        &pair.target.cassette.tuning,
        defs,
        built.outcome.as_ref().ok(),
        &Counted::of(&pair.target.stats),
    ))
}

/// 統計値から数えたものだけ。組み立てない。
fn counted_of(c: &Cassette, path: &str) -> Result<Counted, Exit> {
    let stats = cassettes::decode(c, &Origin::File(path.to_owned()))?;
    Ok(Counted::of(&stats))
}

/// `cassette list`。
pub fn list(args: &[String]) -> Exit {
    let a = match parse_args(args, &["--kind", "--state", "--baseline"]) {
        Ok(a) => a,
        Err(e) => return e,
    };
    if let Some(extra) = a.rest.first() {
        eprintln!("カセットは 1 つだけ渡す: {extra}");
        return Exit::Usage;
    }
    let kind = match a.kind.as_deref().map(|k| (k, Subject::from_name(k))) {
        None => None,
        Some((_, Some(s))) => Some(s),
        Some((k, None)) => {
            eprintln!("知らない種類: {k}。{} のどれか", Subject::names());
            return Exit::Usage;
        }
    };
    let state = match a.state.as_deref() {
        None => None,
        Some("on") => Some(false),
        Some("off") => Some(true),
        Some(other) => {
            eprintln!("--state は on か off: {other}");
            return Exit::Usage;
        }
    };
    let Some(path) = a.cassette.as_deref() else {
        eprintln!("カセットを渡す");
        eprintln!("{}", crate::tuning_help());
        return Exit::Usage;
    };
    let defs = FromDefinitions::load();
    let all = match assembled_items(path, a.baseline, &defs) {
        Ok(i) => i,
        Err(e) => return e,
    };
    // 受け手が先に閉じても落ちない。 fzf や head に繋ぐための出力である。
    let mut out = std::io::stdout().lock();
    for item in filter(all, kind, state) {
        if writeln!(out, "{}", item.line()).is_err() {
            break;
        }
    }
    Exit::Pass
}

/// `cassette mute` と `unmute`。
pub fn mute(args: &[String], muted: bool) -> Exit {
    let help = &crate::tuning_help();
    let a = match parse_args(args, &["--kind", "--baseline"]) {
        Ok(a) => a,
        Err(e) => return e,
    };
    if a.kind.is_some() && !a.rest.is_empty() {
        eprintln!("名前と --kind は一緒に渡さない。どちらか一方にする");
        return Exit::Usage;
    }
    if a.kind.is_none() && a.rest.is_empty() {
        eprintln!("名前か ID、または --kind を渡す");
        eprintln!("{help}");
        return Exit::Usage;
    }
    let kind = match a.kind.as_deref().map(|k| (k, Subject::from_name(k))) {
        None => None,
        Some((_, Some(Subject::Mute(k)))) => Some(k),
        Some((k, Some(_))) => {
            eprintln!("{k} は無効にする種類ではない。申告は first-person / register で書く");
            return Exit::Usage;
        }
        Some((k, None)) => {
            let kinds: Vec<&str> = MuteKind::ALL.iter().map(|k| k.name()).collect();
            eprintln!("知らない種類: {k}。{} のどれか", kinds.join("、"));
            return Exit::Usage;
        }
    };
    let (path, mut c) = match read(a.cassette.as_deref(), help) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let verb = if muted {
        "無効にした"
    } else {
        "戻した"
    };
    let same = if muted {
        "既に無効にしてある"
    } else {
        "無効にしていない"
    };

    if let Some(kind) = kind {
        if !c.tuning.set_kind_muted(kind, muted) {
            println!("{same}: {}（種類ごと）", kind.name());
            return Exit::Pass;
        }
        if let Err(e) = write(&path, &c) {
            return e;
        }
        println!("{verb}: {}（種類ごと）", kind.name());
        if !muted && !c.tuning.muted(kind).is_empty() {
            println!(
                "1 つずつ無効にしたものは残っている: {}",
                c.tuning.muted(kind).join("、")
            );
        }
        return Exit::Pass;
    }

    let defs = FromDefinitions::load();
    // 指標と、無効にしてある言い回しは組み立てずに引ける。 引けないものが
    // 残ったときだけ組み立てる。
    let counted = match counted_of(&c, &path) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let mut known = lookup(&items(&c.tuning, &defs, None, &counted), &a.rest);
    if matches!(known, Lookup::Unknown(_)) {
        let all = match assembled_items(&path, a.baseline, &defs) {
            Ok(i) => i,
            Err(e) => return e,
        };
        known = lookup(&all, &a.rest);
    }
    let targets = match known {
        Lookup::Found(t) => t,
        Lookup::Unknown(names) => {
            // 綴りを間違えたまま書けば、無効にしたつもりの指標がいつまでも指摘に出続ける。
            for n in &names {
                eprintln!("知らない名前: {n}");
            }
            eprintln!(
                "指標は登録簿の名前、言い回しは今の組み合わせで組み立てた一覧の名前か ID で渡す"
            );
            eprintln!("kakiburi cassette list {path} で名前と ID を見る");
            return Exit::Usage;
        }
        Lookup::Ambiguous(names) => {
            for (n, ids) in &names {
                eprintln!("2 つ以上に当たる: {n}（{}）", ids.join("、"));
            }
            eprintln!("ID で渡す");
            return Exit::Usage;
        }
    };
    let mut changed = Vec::new();
    let mut unchanged = Vec::new();
    for (kind, text) in targets {
        let line = format!("{} {text}", kind.name());
        if c.tuning.set_muted(kind, &text, muted) {
            changed.push(line);
        } else {
            unchanged.push(line);
        }
    }
    if !changed.is_empty() {
        if let Err(e) = write(&path, &c) {
            return e;
        }
    }
    for line in &changed {
        println!("{verb}: {line}");
    }
    for line in &unchanged {
        println!("{same}: {line}");
    }
    Exit::Pass
}

/// `cassette first-person`。
pub fn first_person(args: &[String]) -> Exit {
    declare(args, &crate::tuning_help(), |t, v| {
        let value = parse_first_person(v)?;
        t.first_person.clone_from(&value);
        Ok(value.map_or_else(
            || "一人称の申告を消した。数えた結果を使う".to_owned(),
            |f| format!("一人称を申告した: {f}"),
        ))
    })
}

/// `cassette register`。
pub fn register(args: &[String]) -> Exit {
    declare(args, &crate::tuning_help(), |t, v| {
        let value = parse_register(v)?;
        t.register = value;
        Ok(value.map_or_else(
            || "文体の申告を消した。数えた結果を使う".to_owned(),
            |r| format!("文体を申告した: {}", r.name()),
        ))
    })
}

/// 申告を書く。
fn declare(
    args: &[String],
    help: &str,
    apply: impl Fn(&mut Tuning, &str) -> Result<String, String>,
) -> Exit {
    let a = match parse_args(args, &[]) {
        Ok(a) => a,
        Err(e) => return e,
    };
    let [value] = a.rest.as_slice() else {
        eprintln!("カセットと値を 1 つずつ渡す");
        eprintln!("{help}");
        return Exit::Usage;
    };
    // 値を先に見る。 使い方の誤りを、カセットを読んだ後に言わない。
    let mut probe = Tuning::default();
    if let Err(e) = apply(&mut probe, value) {
        eprintln!("断る: {e}");
        return Exit::Usage;
    }
    let (path, mut c) = match read(a.cassette.as_deref(), help) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let before = c.tuning.clone();
    let said = match apply(&mut c.tuning, value) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("断る: {e}");
            return Exit::Usage;
        }
    };
    if c.tuning != before {
        if let Err(e) = write(&path, &c) {
            return e;
        }
    }
    println!("{said}");
    Exit::Pass
}

/// `cassette edit`。調整を対話画面で行う。
pub fn edit(args: &[String]) -> Exit {
    let tty = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    edit_with(args, tty)
}

/// 端末かどうかを渡して `edit` を走らせる。
///
/// 端末でなければ断る。 スクリプトからは引数で受けるサブコマンドを使う。
fn edit_with(args: &[String], tty: bool) -> Exit {
    let a = match parse_args(args, &["--baseline"]) {
        Ok(a) => a,
        Err(e) => return e,
    };
    if !tty {
        eprintln!("断る: 対話画面は端末でしか開けない");
        eprintln!(
            "スクリプトからは cassette list / mute / unmute / first-person / register を使う"
        );
        return Exit::Usage;
    }
    if let Some(extra) = a.rest.first() {
        eprintln!("カセットは 1 つだけ渡す: {extra}");
        return Exit::Usage;
    }
    let (path, mut c) = match read(a.cassette.as_deref(), &crate::tuning_help()) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let defs = FromDefinitions::load();
    let pair = match pair::open(
        Origin::File(path.clone()),
        pair::baseline_origin(a.baseline),
        &defs,
    ) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let built = pair.assemble(&assembly::assemble_stats, &defs);
    if let Err(stopped) = &built.outcome {
        eprintln!(
            "目盛りが組み立てられないので、型・基準の型・基準の語は並べられない: {}",
            stopped.error
        );
    }
    let counted = Counted::of(&pair.target.stats);
    let before = c.tuning.clone();
    match crate::edit_ui::run(&mut c.tuning, &|t| {
        items(t, &defs, built.outcome.as_ref().ok(), &counted)
    }) {
        Ok(true) if c.tuning != before => match write(&path, &c) {
            Ok(()) => {
                println!("書いた: {path}");
                Exit::Pass
            }
            Err(e) => e,
        },
        Ok(true) => {
            println!("変えたものが無いので書かない");
            Exit::Pass
        }
        Ok(false) => {
            println!("書かずに終えた");
            Exit::Pass
        }
        Err(e) => {
            eprintln!("対話画面が止まった: {e}");
            Exit::Usage
        }
    }
}

/// チェックリストで選んだ結果を当てる。
///
/// 並べた項目だけを書き換える。 並んでいない言い回し——今の組み合わせでは出て
/// いないもの——は、選べなかっただけなので触らない。
pub fn apply_checklist(
    t: &mut Tuning,
    kind: MuteKind,
    shown: &[&Item],
    whole_kind: bool,
    checked: &[bool],
) {
    t.set_kind_muted(kind, whole_kind);
    for (item, on) in shown.iter().zip(checked) {
        t.set_muted(kind, &item.text, *on);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;
    use crate::testdir::TempDir;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    fn counted() -> Counted {
        Counted {
            first_person: Some("僕".into()),
            register: RegisterCount {
                polite: 3,
                plain: 12,
            },
        }
    }

    /// 本人と基準のカセットを作る。
    fn cassettes(dir: &TempDir) -> (String, String) {
        let person = fixture::write_corpus(dir, "本人", false);
        let baseline = fixture::write_corpus(dir, "基準", true);
        let (p, b) = (dir.join("本人.kb"), dir.join("基準.kb"));
        for (folder, out) in [(&person, &p), (&baseline, &b)] {
            assert_eq!(
                crate::run(&args(&["cassette", "build", folder, "-o", out])),
                Exit::Pass
            );
        }
        (p, b)
    }

    #[test]
    fn 言い回しの_id_は種類と文字列だけで決まる() {
        let a = phrase_id(MuteKind::BaselineKata, "のではなく、");
        assert_eq!(a, phrase_id(MuteKind::BaselineKata, "のではなく、"));
        assert!(a.starts_with("基準の型-"), "{a}");
        assert_eq!(a.chars().count(), "基準の型-".chars().count() + 6);
        assert_ne!(a, phrase_id(MuteKind::Kata, "のではなく、"));
    }

    #[test]
    fn 一覧は種類と名前と状態と説明をタブで区切る() {
        let defs = FromDefinitions::load();
        let mut t = Tuning::default();
        t.set_muted(MuteKind::Metric, "強調", true);
        t.first_person = Some("私".into());
        let all = items(&t, &defs, None, &counted());
        let emphasis = all.iter().find(|i| i.name == "強調").expect("指標がある");
        assert_eq!(emphasis.line(), "指標\t強調\toff\t強調をどれだけ置くか。");
        let person = all
            .iter()
            .find(|i| i.subject == Subject::FirstPerson)
            .unwrap();
        assert_eq!(person.line(), "一人称\t一人称\t私（申告）\t数えた結果は 僕");
        let register = all.iter().find(|i| i.subject == Subject::Register).unwrap();
        assert_eq!(
            register.line(),
            "文体\t文体\tplain（数えた結果）\t数えた結果は plain（敬体 3 本 / 常体 12 本）"
        );
    }

    #[test]
    fn 無効にした言い回しは今の組み合わせに出ていなくても並ぶ() {
        // 戻せるように並べる。 別の基準と組み合わせたときに出てくれば、そこで効く。
        let mut t = Tuning::default();
        t.set_muted(MuteKind::BaselineKata, "のではなく、", true);
        t.set_muted(MuteKind::BaselineGoi, "地味", true);
        let all = items(&t, &FromDefinitions::load(), None, &counted());
        let kata = all.iter().find(|i| i.text == "のではなく、").expect("並ぶ");
        assert_eq!(kata.name, phrase_id(MuteKind::BaselineKata, "のではなく、"));
        assert_eq!(kata.state, "off");
        let goi = all.iter().find(|i| i.text == "地味").expect("並ぶ");
        assert_eq!(goi.name, "地味", "語は語彙素のまま");
    }

    #[test]
    fn 種類ごと無効にしたものは_off_で種類ごとと添える() {
        let mut t = Tuning::default();
        t.set_kind_muted(MuteKind::Metric, true);
        let all = items(&t, &FromDefinitions::load(), None, &counted());
        let metrics: Vec<&Item> = all
            .iter()
            .filter(|i| i.subject == Subject::Mute(MuteKind::Metric))
            .collect();
        assert!(!metrics.is_empty());
        assert!(metrics.iter().all(|i| i.state == "off"));
        assert!(metrics
            .iter()
            .all(|i| i.description.ends_with("（種類ごと無効）")));
    }

    #[test]
    fn 状態で絞っても申告は残る() {
        let mut t = Tuning::default();
        t.set_muted(MuteKind::Metric, "強調", true);
        let all = items(&t, &FromDefinitions::load(), None, &counted());
        let off = filter(all.clone(), None, Some(true));
        assert!(off.iter().any(|i| i.name == "強調"));
        assert!(off.iter().all(|i| i.muted != Some(false)));
        assert!(off.iter().any(|i| i.subject == Subject::Register));
        let metrics = filter(all, Some(Subject::Mute(MuteKind::Metric)), Some(false));
        assert!(metrics
            .iter()
            .all(|i| i.subject == Subject::Mute(MuteKind::Metric)));
        assert!(!metrics.iter().any(|i| i.name == "強調"));
    }

    #[test]
    fn 名前は全部引けなければ何も返さない() {
        let all = items(
            &Tuning::default(),
            &FromDefinitions::load(),
            None,
            &counted(),
        );
        assert_eq!(
            lookup(&all, &args(&["強調"])),
            Lookup::Found(vec![(MuteKind::Metric, "強調".into())])
        );
        assert_eq!(
            lookup(&all, &args(&["強調", "強調しすぎ"])),
            Lookup::Unknown(vec!["強調しすぎ".into()])
        );
        assert!(
            matches!(lookup(&all, &args(&["一人称"])), Lookup::Unknown(_)),
            "申告は無効にできない"
        );
    }

    #[test]
    fn 同じ文字列が_2_つの種類にあれば_id_で渡させる() {
        let mut t = Tuning::default();
        t.set_muted(MuteKind::Kata, "のだ。", true);
        t.set_muted(MuteKind::BaselineKata, "のだ。", true);
        let all = items(&t, &FromDefinitions::load(), None, &counted());
        assert!(matches!(
            lookup(&all, &args(&["のだ。"])),
            Lookup::Ambiguous(_)
        ));
        assert_eq!(
            lookup(&all, &[phrase_id(MuteKind::Kata, "のだ。")]),
            Lookup::Found(vec![(MuteKind::Kata, "のだ。".into())])
        );
    }

    #[test]
    fn 一人称は数える集合の中からだけ申告できる() {
        assert_eq!(parse_first_person("僕"), Ok(Some("僕".into())));
        assert_eq!(parse_first_person("auto"), Ok(None));
        assert!(parse_first_person("拙者").is_err());
        assert!(
            !first_person_choices().contains(&"私-代名詞"),
            "札を見せない"
        );
    }

    #[test]
    fn 文体は_3_つの綴りだけを受ける() {
        assert_eq!(parse_register("polite"), Ok(Some(Register::Polite)));
        assert_eq!(parse_register("auto"), Ok(None));
        for bad in ["敬体", "Polite", "desu"] {
            assert!(parse_register(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn チェックリストは並べたものだけを書き換える() {
        let mut t = Tuning::default();
        t.set_muted(MuteKind::BaselineKata, "並んでいない", true);
        t.set_muted(MuteKind::BaselineKata, "戻す", true);
        let item = |text: &str| Item {
            subject: Subject::Mute(MuteKind::BaselineKata),
            name: phrase_id(MuteKind::BaselineKata, text),
            text: text.into(),
            muted: Some(false),
            state: "on".into(),
            description: String::new(),
        };
        let (a, b) = (item("戻す"), item("無効にする"));
        apply_checklist(
            &mut t,
            MuteKind::BaselineKata,
            &[&a, &b],
            true,
            &[false, true],
        );
        assert_eq!(
            t.muted(MuteKind::BaselineKata),
            vec!["並んでいない", "無効にする"]
        );
        assert!(t.mute_kinds.contains(&MuteKind::BaselineKata));
    }

    #[test]
    fn 端末でなければ対話画面を開かずに断る() {
        let dir = TempDir::new("edit-no-tty");
        let (p, _) = cassettes(&dir);
        let before = std::fs::read(&p).unwrap();
        assert_eq!(edit_with(&args(&[&p]), false), Exit::Usage);
        assert_eq!(std::fs::read(&p).unwrap(), before, "書かない");
    }

    #[test]
    fn 指標を無効にして戻す() {
        let dir = TempDir::new("mute-metric");
        let (p, _) = cassettes(&dir);
        let run = |v: &[&str]| crate::run(&args(v));
        assert_eq!(run(&["cassette", "mute", &p, "強調"]), Exit::Pass);
        let c = cassettes::read_file(&p).unwrap();
        assert!(c.tuning.is_muted(MuteKind::Metric, "強調"));
        assert_eq!(c.generation, 2, "書けば世代が進む");
        assert_eq!(run(&["cassette", "mute", &p, "強調"]), Exit::Pass);
        assert_eq!(
            cassettes::read_file(&p).unwrap().generation,
            2,
            "変わらなければ書かない"
        );
        assert_eq!(run(&["cassette", "unmute", &p, "強調"]), Exit::Pass);
        assert!(!cassettes::read_file(&p)
            .unwrap()
            .tuning
            .is_muted(MuteKind::Metric, "強調"));
    }

    #[test]
    fn 知らない名前は書かずに_64_で断る() {
        let dir = TempDir::new("mute-unknown");
        let (p, b) = cassettes(&dir);
        let before = std::fs::read(&p).unwrap();
        assert_eq!(
            crate::run(&args(&[
                "cassette",
                "mute",
                &p,
                "強調",
                "無い指標",
                "--baseline",
                &b
            ])),
            Exit::Usage
        );
        assert_eq!(std::fs::read(&p).unwrap(), before, "1 つも書かない");
    }

    #[test]
    fn 言い回しは組み立てた一覧の_id_で無効にできる() {
        let dir = TempDir::new("mute-phrase");
        let (p, b) = cassettes(&dir);
        let defs = FromDefinitions::load();
        let all = assembled_items(&p, Some(b.clone()), &defs).expect("並ぶ");
        let Some(target) = all.iter().find(|i| {
            matches!(
                i.subject,
                Subject::Mute(MuteKind::BaselineKata | MuteKind::Kata | MuteKind::BaselineGoi)
            )
        }) else {
            panic!("言い回しが 1 つも出ない素材では確かめられない");
        };
        assert_eq!(
            crate::run(&args(&[
                "cassette",
                "mute",
                &p,
                &target.name,
                "--baseline",
                &b
            ])),
            Exit::Pass
        );
        let Subject::Mute(kind) = target.subject else {
            unreachable!()
        };
        let c = cassettes::read_file(&p).unwrap();
        assert!(c.tuning.is_muted(kind, &target.text));
        // 戻すときは組み立てずに引ける。
        assert_eq!(
            crate::run(&args(&["cassette", "unmute", &p, &target.name])),
            Exit::Pass
        );
        assert!(!cassettes::read_file(&p)
            .unwrap()
            .tuning
            .is_muted(kind, &target.text));
    }

    #[test]
    fn 種類ごと無効にして戻しても_1_つずつのものは残る() {
        let dir = TempDir::new("mute-kind");
        let (p, _) = cassettes(&dir);
        let run = |v: &[&str]| crate::run(&args(v));
        assert_eq!(run(&["cassette", "mute", &p, "強調"]), Exit::Pass);
        assert_eq!(run(&["cassette", "mute", &p, "--kind", "指標"]), Exit::Pass);
        assert!(cassettes::read_file(&p)
            .unwrap()
            .tuning
            .mute_kinds
            .contains(&MuteKind::Metric));
        assert_eq!(
            run(&["cassette", "unmute", &p, "--kind", "指標"]),
            Exit::Pass
        );
        let t = cassettes::read_file(&p).unwrap().tuning;
        assert!(t.mute_kinds.is_empty());
        assert!(t.is_muted(MuteKind::Metric, "強調"));
        for bad in [
            &["cassette", "mute", &p, "--kind", "一人称"][..],
            &["cassette", "mute", &p, "--kind", "無い種類"],
            &["cassette", "mute", &p, "強調", "--kind", "指標"],
            &["cassette", "mute", &p],
        ] {
            assert_eq!(run(bad), Exit::Usage, "{bad:?}");
        }
    }

    #[test]
    fn 申告を書いて_auto_で消す() {
        let dir = TempDir::new("declare");
        let (p, _) = cassettes(&dir);
        let run = |v: &[&str]| crate::run(&args(v));
        assert_eq!(run(&["cassette", "first-person", &p, "僕"]), Exit::Pass);
        assert_eq!(run(&["cassette", "register", &p, "polite"]), Exit::Pass);
        let t = cassettes::read_file(&p).unwrap().tuning;
        assert_eq!(t.first_person.as_deref(), Some("僕"));
        assert_eq!(t.register, Some(Register::Polite));
        assert_eq!(run(&["cassette", "first-person", &p, "auto"]), Exit::Pass);
        assert_eq!(run(&["cassette", "register", &p, "auto"]), Exit::Pass);
        assert_eq!(cassettes::read_file(&p).unwrap().tuning, Tuning::default());
        let before = std::fs::read(&p).unwrap();
        assert_eq!(run(&["cassette", "register", &p, "desu"]), Exit::Usage);
        assert_eq!(run(&["cassette", "first-person", &p, "拙者"]), Exit::Usage);
        assert_eq!(run(&["cassette", "register", &p]), Exit::Usage);
        assert_eq!(std::fs::read(&p).unwrap(), before);
    }

    #[test]
    fn 一覧を出す() {
        let dir = TempDir::new("list");
        let (p, b) = cassettes(&dir);
        let run = |v: &[&str]| crate::run(&args(v));
        assert_eq!(run(&["cassette", "list", &p, "--baseline", &b]), Exit::Pass);
        assert_eq!(
            run(&[
                "cassette",
                "list",
                &p,
                "--baseline",
                &b,
                "--kind",
                "基準の型",
                "--state",
                "on"
            ]),
            Exit::Pass
        );
        assert_eq!(
            run(&["cassette", "list", &p, "--kind", "無い"]),
            Exit::Usage
        );
        assert_eq!(
            run(&["cassette", "list", &p, "--state", "yes"]),
            Exit::Usage
        );
        assert_eq!(
            run(&["cassette", "list", &dir.write("壊れた.kb", "PK")]),
            Exit::Unreadable
        );
    }
}
