//! `kuchiyose polish`。表現を寄せる周回を回す（[polish](../../../docs/design/200-command.md#polish)）。
//!
//! 周回を LLM の道具に任せない。 1 周ごとに kuchiyose が道具を対話なしで起動して
//! 直させ、kuchiyose が検め、採るかを決める（[周回は kuchiyose が回す](../../../docs/spec/400-write.md#周回は-kuchiyose-が回す)）。
//! 目盛りは周回の最初に 1 度だけ組み立て、全部の版に同じものを当てる。

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use kuchiyose_katashiro::json::{self, Value};
use kuchiyose_prompt::{Ceiling, Change, Rejected};
use kuchiyose_review::{Stage, Standing, Verdict, Version};
use kuchiyose_scale::assembly;

use crate::agent::{self, Agent, Failure, Launch, Task};
use crate::env::Env;
use crate::exit::Exit;
use crate::pair::Assemble;
use crate::review::{Checked, Session};
use crate::{config, folder, katashiros};

/// 上限の周回数の既定。暫定値である。
///
/// 上限は道具を起動する回数、つまり費用の上限である。 実測では、機械の草稿が通るまでに
/// 1 周で済んだ例と 12 周要った例がある。周ごとの記録から、通るまでに要った周回の
/// 分布を集めて導き直す。
pub const DEFAULT_ROUNDS: usize = 4;

/// `polish` の help。
pub const HELP: &str = "\
kuchiyose polish <ファイル> [-o <ファイル>] [--katashiro <形代>] [--baseline <形代>]
                 [--agent <道具>] [--rounds <数>] [--print]
    手元の文章の表現だけを寄せる。検めて、指摘された表現だけを LLM の道具に対話なしで
    直させ、また検める。直した版は、それまでに採った版より良くなったときだけ採る。
    採った版が通るか、上限の周回数に達するか、道具が失敗したら止める。
    元のファイルは上書きしない。版と各周のプロンプトと検めた結果と捨てた版の記録は
    <ファイルの名前>.kuchiyose/ に残る。捨てた版があれば、次の周のプロンプトに
    何をして捨てたかを載せる。
    -o         最後に採った版を書く経路。省けば <ファイルの名前>.polished.<拡張子>。
               1 周も採らなければ書かない。
    --rounds   上限の周回数。1 以上 1000 以下。既定は 4。捨てた周も 1 周と数える。
    --print    最初の周の直させるプロンプトを出して終わる。道具は起動しない。
    終了コード: 最後に採った版の判定（0 / 1 / 2）。道具が見つからないか失敗して
    止まったら 69。それまでに採った版は書いてある。";

/// 引数。
#[derive(Debug, Default)]
struct Args {
    file: Option<String>,
    output: Option<String>,
    katashiro: Option<String>,
    baseline: Option<String>,
    agent: Option<String>,
    rounds: Option<usize>,
    print: bool,
}

fn parse_args(args: &[String]) -> Result<Args, Exit> {
    let mut a = Args::default();
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].as_str();
        match flag {
            "--print" => a.print = true,
            "-o" | "--output" | "--katashiro" | "--baseline" | "--agent" | "--rounds" => {
                let Some(v) = args.get(i + 1).cloned() else {
                    eprintln!("{flag} に値を渡す");
                    return Err(Exit::Usage);
                };
                match flag {
                    "--katashiro" => a.katashiro = Some(v),
                    "--baseline" => a.baseline = Some(v),
                    "--agent" => a.agent = Some(v),
                    "--rounds" => a.rounds = Some(parse_rounds(&v)?),
                    _ => a.output = Some(v),
                }
                i += 1;
            }
            other if other.starts_with('-') => {
                eprintln!("知らない引数: {other}");
                return Err(Exit::Usage);
            }
            other if a.file.is_none() => a.file = Some(other.to_owned()),
            other => {
                eprintln!("ファイルは 1 つだけ渡す: {other}");
                return Err(Exit::Usage);
            }
        }
        i += 1;
    }
    Ok(a)
}

/// 上限の周回数の上限。設定ファイルの `rounds` と `--rounds` に同じだけ掛ける。
pub const MAX_ROUNDS: usize = 1000;

/// 上限の周回数。1 以上 [`MAX_ROUNDS`] 以下の整数。
///
/// # Errors
///
/// 読めなければ 64。
pub fn parse_rounds(v: &str) -> Result<usize, Exit> {
    match v.parse::<usize>() {
        Ok(n) if (1..=MAX_ROUNDS).contains(&n) => Ok(n),
        _ => {
            eprintln!("断る: --rounds は 1 以上 {MAX_ROUNDS} 以下の整数: {v}");
            Err(Exit::Usage)
        }
    }
}

/// 周回の置き場と行き先。
#[derive(Debug, Clone)]
pub struct Plan {
    /// 元のファイル。上書きしない。
    pub original: PathBuf,
    /// 作業用のフォルダ。`<ファイルの名前>.kuchiyose/`。
    pub work: PathBuf,
    /// 版の拡張子。元のファイルに揃える——取り込み元は拡張子から決まる。
    pub ext: String,
    /// 最後に採った版を書く経路。
    pub output: PathBuf,
    /// 上限の周回数。
    pub rounds: usize,
}

impl Plan {
    /// 元のファイルから決める。`output` を省けば `<ファイルの名前>.polished.<拡張子>`。
    #[must_use]
    pub fn new(original: PathBuf, output: Option<PathBuf>, rounds: usize) -> Self {
        let dir = original.parent().map(Path::to_path_buf).unwrap_or_default();
        let stem = original
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let ext = original
            .extension()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            work: dir.join(format!("{stem}.kuchiyose")),
            output: output.unwrap_or_else(|| dir.join(format!("{stem}.polished.{ext}"))),
            original,
            ext,
            rounds,
        }
    }

    /// その番号の版の経路。
    #[must_use]
    pub fn version(&self, n: usize) -> PathBuf {
        self.work.join(format!("round-{n}.{}", self.ext))
    }

    fn prompt(&self, n: usize) -> PathBuf {
        self.work.join(format!("round-{n}.prompt.md"))
    }

    fn review(&self, n: usize) -> PathBuf {
        self.work.join(format!("round-{n}.review.json"))
    }

    fn rejected(&self, n: usize) -> PathBuf {
        self.work.join(format!("round-{n}.rejected.json"))
    }

    /// 作業用のフォルダに残っている版の、次の番号。無ければ 0。
    ///
    /// 既にあれば、断らずに続きの番号から書く。続きの番号が数えられる上限を超えれば `None`。
    #[must_use]
    pub fn next_number(&self) -> Option<usize> {
        let Ok(entries) = std::fs::read_dir(&self.work) else {
            return Some(0);
        };
        entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let rest = name.strip_prefix("round-")?;
                rest.split('.').next()?.parse::<usize>().ok()
            })
            .max()
            .map_or(Some(0), |n| n.checked_add(1))
    }
}

/// 最初の検めが周回に入れないなら、その理由（[周回に入れないとき](../../../docs/spec/400-write.md#周回に入れないとき)）。
#[must_use]
pub fn not_entering(first: &Checked) -> Option<String> {
    if first.verdict == Verdict::Pass {
        return Some("通る。直すものが無い".to_owned());
    }
    if let Version::Unmeasurable(why) = &first.version {
        return Some(why.clone());
    }
    (first.directives == 0).then(|| {
        "前に出す指標が無い。文章を直しても変わらない。形代と基準の組み合わせの問題である"
            .to_owned()
    })
}

/// 周回の結果。
#[derive(Debug)]
pub struct Report {
    /// 元の版を検めた結果。
    pub first: Checked,
    /// 最後に採った版を検めた結果。
    pub kept: Checked,
    /// 最後に採った版の経路。
    pub kept_path: PathBuf,
    /// 回した周の数。道具を起動した回数である。
    pub rounds_run: usize,
    /// 採った周の数。
    pub adopted: usize,
    /// 周回に入らなかった理由。
    pub not_entered: Option<String>,
    /// 道具が失敗して止まったなら、その言い方。
    pub failure: Option<String>,
}

/// 道具を起動する関数。
pub type Launcher<'a> = dyn Fn(&Launch<'_>) -> Result<(), Failure> + 'a;

fn write_file(path: &Path, body: &str) -> Result<(), Exit> {
    std::fs::write(path, body).map_err(|e| {
        eprintln!("断る: 書けない: {}: {e}", path.display());
        Exit::Unreadable
    })
}

/// 周回を回す。版と各周のプロンプトと検めた結果を作業用のフォルダに残す。
///
/// `judge` は版を検める関数で、目盛りは呼ぶ側が周回の前に 1 度だけ組み立てておく。
/// 採るかは [`kuchiyose_review::adopt`] が決める。
///
/// # Errors
///
/// 作業用のフォルダに書けないときに断る。
pub fn run_rounds(
    plan: &Plan,
    judge: &dyn Fn(&Path) -> Checked,
    launch: &Launcher<'_>,
) -> Result<Report, Exit> {
    std::fs::create_dir_all(&plan.work).map_err(|e| {
        eprintln!(
            "断る: 作業用のフォルダが作れない: {}: {e}",
            plan.work.display()
        );
        Exit::Unreadable
    })?;
    let Some((start, last)) = plan
        .next_number()
        .and_then(|s| Some((s, s.checked_add(plan.rounds)?)))
    else {
        eprintln!(
            "断る: 作業用のフォルダにある版の番号が大きすぎて、続きの番号を振れない: {}",
            plan.work.display()
        );
        return Err(Exit::Usage);
    };
    let copy = plan.version(start);
    std::fs::copy(&plan.original, &copy).map_err(|e| {
        eprintln!("断る: 元の版を写せない: {}: {e}", plan.original.display());
        Exit::Unreadable
    })?;
    let first = judge(&copy);
    write_file(&plan.review(start), &first.json)?;
    let mut report = Report {
        kept: first.clone(),
        first,
        kept_path: copy,
        rounds_run: 0,
        adopted: 0,
        not_entered: None,
        failure: None,
    };
    if let Some(why) = not_entering(&report.first) {
        report.not_entered = Some(why);
        return Ok(report);
    }
    let Version::Measured(mut standing) = report.first.version.clone() else {
        unreachable!("測れない版では周回に入らない");
    };
    for n in (start + 1)..=last {
        let write = plan.version(n);
        // 直させるのはいつも、それまでに採った版である。 捨てた版から続けない。
        // 捨てた版は、何をして捨てられたかだけを伝える。
        let prompt = kuchiyose_prompt::revise_prompt(
            &report.kept.prose,
            &ceilings(&report.kept),
            &rejected_history(plan, &report.kept_path, &report.kept.json),
            &report.kept_path.display().to_string(),
            &write.display().to_string(),
        );
        let prompt_file = plan.prompt(n);
        write_file(&prompt_file, &prompt)?;
        report.rounds_run += 1;
        let launched = launch(&Launch {
            task: Task::Polish {
                read: &report.kept_path,
                write: &write,
            },
            prompt: &prompt,
            prompt_file: &prompt_file,
            cwd: &plan.work,
        });
        // 失敗した周を繰り返さない。 同じ呼び方を繰り返しても直らないことが多い。
        if let Err(f) = launched {
            report.failure = Some(format!("{n} 周目で{f}"));
            break;
        }
        if std::fs::metadata(&write).map_or(true, |m| m.len() == 0) {
            report.failure = Some(format!(
                "{n} 周目で、道具が書くはずの経路に何も書かなかった: {}",
                write.display()
            ));
            break;
        }
        let checked = judge(&write);
        write_file(&plan.review(n), &checked.json)?;
        if kuchiyose_review::adopt(&checked.version, &standing) {
            if let Version::Measured(s) = &checked.version {
                standing = s.clone();
            }
            report.kept = checked;
            report.kept_path = write;
            report.adopted += 1;
            if report.kept.verdict == Verdict::Pass {
                break;
            }
        } else {
            let record =
                rejection_record(&report.kept_path, &report.kept, &standing, &write, &checked);
            write_file(&plan.rejected(n), &format!("{}\n", record.write()))?;
        }
    }
    Ok(report)
}

/// 直させるプロンプトに載せる、言い回しの上限。検めた結果の並びのまま渡す。
fn ceilings(kept: &Checked) -> Vec<Ceiling> {
    kept.headroom
        .iter()
        .map(|h| Ceiling {
            text: h.text.clone(),
            times: h.times,
            density: h.density,
            ceiling: h.ceiling,
            allowed: h.allowed,
        })
        .collect()
}

/// 目盛りを名乗る欄。捨てた版の記録は、同じ目盛りで検めた版にだけ当てる。
const SCALE_FIELDS: [&str; 2] = ["content_hash", "baseline_content_hash"];

fn scale_of(review_json: &str) -> Vec<Option<String>> {
    let v = json::parse(review_json.trim()).ok();
    SCALE_FIELDS
        .iter()
        .map(|k| {
            v.as_ref()
                .and_then(|v| v.get(k))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}

/// 悪い向きに動いた指標。人らしさ値は正が人の側なので、下がったものである。動いた量の大きい順。
fn worse_metrics(kept: &Checked, candidate: &Checked) -> Vec<String> {
    let mut worse: Vec<(f64, &str, f64, f64)> = candidate
        .humanness_by_metric
        .iter()
        .filter_map(|(name, after)| {
            let (_, before) = kept.humanness_by_metric.iter().find(|(k, _)| k == name)?;
            (after < before).then(|| (before - after, name.as_str(), *before, *after))
        })
        .collect();
    worse.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    worse
        .into_iter()
        .map(|(_, name, before, after)| format!("{name} {before:.3} → {after:.3}"))
        .collect()
}

/// 捨てた版の記録（[捨てた直しを伝える](../../../docs/spec/400-write.md#捨てた直しを伝える)）。
///
/// 続きから回したときにも同じものを組み直せるよう、プロンプトに載せる値をそのまま残す。
/// 読んだ版は、作業用のフォルダの中の名前で持つ。
fn rejection_record(
    read: &Path,
    kept: &Checked,
    standing: &Standing,
    write: &Path,
    candidate: &Checked,
) -> Value {
    let text =
        |p: &Path| String::from_utf8_lossy(&std::fs::read(p).unwrap_or_default()).into_owned();
    // 基準との距離で止まった版どうしのときだけ、指標ごとの向きを言う。 段が違えば、
    // 指標ごとの値は採否を決めていない。
    let worse = match &candidate.version {
        Version::Measured(s)
            if s.stage == Stage::Humanness && standing.stage == Stage::Humanness =>
        {
            worse_metrics(kept, candidate)
        }
        _ => Vec::new(),
    };
    let changes = kuchiyose_prompt::changes(&text(read), &text(write));
    let name = |p: &Path| {
        p.file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let mut fields = vec![
        ("read".to_owned(), Value::s(name(read))),
        (
            "reason".to_owned(),
            Value::s(kuchiyose_review::rejection(&candidate.version, standing)),
        ),
        ("worse".to_owned(), crate::machine::strings(&worse)),
        (
            "changes".to_owned(),
            Value::Array(
                changes
                    .iter()
                    .map(|c| {
                        Value::obj([
                            ("before".to_owned(), Value::s(&c.before)),
                            ("after".to_owned(), Value::s(&c.after)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ];
    for (k, v) in SCALE_FIELDS.iter().zip(scale_of(&kept.json)) {
        fields.push(((*k).to_owned(), v.map_or(Value::Null, Value::s)));
    }
    Value::obj(fields)
}

/// 今の版を直して捨てた版を、作業用のフォルダの記録から古い順に組み直す。
///
/// 当てるのは、読んだ版の中身が今の版とバイトまで同じで、同じ目盛りで検めた記録だけである。
/// 採った版が変われば、それより前の記録は当たらなくなる。 続きから回したときも、元の版が
/// 同じなら、前に回したときの記録が当たる。
fn rejected_history(plan: &Plan, kept_path: &Path, kept_json: &str) -> Vec<Rejected> {
    let (Ok(kept), Ok(entries)) = (std::fs::read(kept_path), std::fs::read_dir(&plan.work)) else {
        return Vec::new();
    };
    let scale = scale_of(kept_json);
    let mut numbers: Vec<usize> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.strip_prefix("round-")?
                .strip_suffix(".rejected.json")?
                .parse()
                .ok()
        })
        .collect();
    numbers.sort_unstable();
    numbers
        .into_iter()
        .filter_map(|n| {
            let body = std::fs::read_to_string(plan.rejected(n)).ok()?;
            let v = json::parse(body.trim()).ok()?;
            let read = plan.work.join(v.get("read")?.as_str()?);
            let same_scale = SCALE_FIELDS
                .iter()
                .zip(&scale)
                .all(|(k, s)| v.get(k).and_then(Value::as_str) == s.as_deref());
            (same_scale && std::fs::read(read).ok()? == kept).then_some(())?;
            let strings = |key: &str| -> Vec<String> {
                v.get(key)
                    .and_then(Value::as_array)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|s| s.as_str().map(str::to_owned))
                    .collect()
            };
            let changes = v
                .get("changes")
                .and_then(Value::as_array)
                .unwrap_or_default()
                .iter()
                .filter_map(|c| {
                    Some(Change {
                        before: c.get("before")?.as_str()?.to_owned(),
                        after: c.get("after")?.as_str()?.to_owned(),
                    })
                })
                .collect();
            Some(Rejected {
                round: n,
                reason: v.get("reason")?.as_str()?.to_owned(),
                worse: strings("worse"),
                changes,
            })
        })
        .collect()
}

/// 推論設定の直し方。基準との距離で止まったときだけ言う。
///
/// 起動した道具からは設定を変えられないので、直させる指示にはしない。報告に残す。
fn reasoning_note(kept: &Checked) -> Option<&'static str> {
    matches!(
        &kept.version,
        Version::Measured(Standing {
            stage: Stage::Humanness,
            ..
        })
    )
    .then_some(
        "推論設定の直し方: 基準との距離で止まっている。繰り返しの量は推論設定で動く。\
         頻度ペナルティや温度を変えた道具で書き直させる。kuchiyose は設定に触れない",
    )
}

/// 報告を組み立てる。`review` の散文に、回した周・採った周・字数・推論設定の直し方を足す。
#[must_use]
pub fn report_text(plan: &Plan, r: &Report, wrote: Option<&Path>) -> String {
    let mut out = r.kept.prose.clone();
    let _ = writeln!(out);
    if let Some(why) = &r.not_entered {
        let _ = writeln!(out, "周回に入らなかった: {why}");
    }
    let _ = writeln!(
        out,
        "回した周: {}（上限 {}） / 採った周: {}",
        r.rounds_run, plan.rounds, r.adopted
    );
    // 字数は内容の代わりにはならない。 大きく変わっていれば読むべきところがある、という合図である。
    let _ = writeln!(
        out,
        "地の文の日本語: 元の版 {} 字 / 最後に採った版 {} 字",
        crate::with_commas(r.first.chars),
        crate::with_commas(r.kept.chars)
    );
    let _ = writeln!(out, "最後に採った版: {}", r.kept_path.display());
    match wrote {
        Some(p) => {
            let _ = writeln!(out, "書いた: {}", p.display());
        }
        None if r.not_entered.is_none() => {
            let _ = writeln!(out, "1 周も採らなかったので書いていない");
        }
        None => {}
    }
    let _ = writeln!(out, "版と検めた結果: {}", plan.work.display());
    if let Some(note) = reasoning_note(&r.kept) {
        let _ = writeln!(out, "{note}");
    }
    out
}

/// 周回を回し、最後に採った版を書き、報告する。
///
/// # Errors
///
/// 作業用のフォルダや書き出し先に書けないときに断る。
pub fn polish(plan: &Plan, judge: &dyn Fn(&Path) -> Checked, launch: &Launcher<'_>) -> Exit {
    let r = match run_rounds(plan, judge, launch) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let mut wrote = None;
    if r.adopted > 0 {
        // `-o` に元のファイルを渡したときだけ上書きする。 そのときも元の版は作業用の
        // フォルダに残っている。
        if let Err(e) = std::fs::copy(&r.kept_path, &plan.output) {
            eprintln!("断る: 書けない: {}: {e}", plan.output.display());
            return Exit::Unreadable;
        }
        wrote = Some(plan.output.as_path());
    }
    print!("{}", report_text(plan, &r, wrote));
    if let Some(f) = &r.failure {
        eprintln!("止めた: {f}");
        eprintln!("それまでに採った版は残っている");
        return Exit::Environment;
    }
    Exit::from_verdict(r.kept.verdict)
}

/// 最初の周の直させるプロンプト。`--print` で出す。
///
/// 道具は起動せず、作業用のフォルダも作らない。 読む経路は元のファイル、書く経路は
/// 最後に採った版の行き先にする。 使う人が自分の道具に貼って使うためである。
///
/// # Errors
///
/// 周回に入らない草稿なら、その理由を返す。
pub fn first_prompt(plan: &Plan, first: &Checked) -> Result<String, String> {
    if let Some(why) = not_entering(first) {
        return Err(why);
    }
    Ok(kuchiyose_prompt::revise_prompt(
        &first.prose,
        &ceilings(first),
        &rejected_history(plan, &plan.original, &first.json),
        &plan.original.display().to_string(),
        &plan.output.display().to_string(),
    ))
}

/// `kuchiyose polish`。
pub fn run(args: &[String], env: &Env) -> Exit {
    run_with(args, env, &assembly::assemble_stats)
}

/// 組み立ての関数を差し替えて回す。
pub fn run_with(args: &[String], env: &Env, assemble: &Assemble<'_>) -> Exit {
    let a = match parse_args(args) {
        Ok(a) => a,
        Err(e) => return e,
    };
    let Some(file) = &a.file else {
        eprintln!("直すファイルを渡す");
        eprintln!("{HELP}");
        return Exit::Usage;
    };
    if let Err(e) = folder::source_or_refuse(file) {
        return e;
    }
    let original = env.absolute(file);
    if !original.is_file() {
        eprintln!("読めない: {file}");
        return Exit::Unreadable;
    }
    let cfg = match config::load(env) {
        Ok(c) => c,
        Err(e) => return e,
    };
    let rounds = a.rounds.or(cfg.rounds).unwrap_or(DEFAULT_ROUNDS);
    let plan = Plan::new(
        original,
        a.output.as_deref().map(|o| env.absolute(o)),
        rounds,
    );
    let katashiro = match katashiros::resolve(a.katashiro.as_deref(), env) {
        Ok(k) => k,
        Err(e) => return e,
    };
    let agent = if a.print {
        None
    } else {
        match agent::resolve(a.agent.as_deref(), env, &cfg) {
            Ok(g) => Some(g),
            Err(e) => return e,
        }
    };
    let session = match Session::open(katashiro, a.baseline.clone(), assemble) {
        Ok(s) => s,
        Err(e) => return e,
    };
    run_session(&plan, &session, agent.as_ref())
}

/// 開いた目盛りで回す。`agent` が無ければ `--print` である。
pub fn run_session(plan: &Plan, session: &Session, agent: Option<&Agent>) -> Exit {
    let judge = |p: &Path| session.check(&p.display().to_string());
    let Some(agent) = agent else {
        let first = session.check(&plan.original.display().to_string());
        return match first_prompt(plan, &first) {
            Ok(p) => {
                print!("{p}");
                Exit::Pass
            }
            Err(why) => {
                eprintln!("周回に入らないので、直させるプロンプトは無い: {why}");
                Exit::from_verdict(first.verdict)
            }
        };
    };
    polish(plan, &judge, &|l| agent::run_batch(agent, l))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Kind;
    use crate::testdir::TempDir;
    use kuchiyose_review::Amount;

    /// 版の中身から検めた結果を決める、試験のための検め。
    ///
    /// 中身は `通らない 0.5` のような 1 行で、判定と基準との距離を名乗る。 指標ごとの
    /// 人らしさ値は `圧縮率` の 1 本だけで、基準との距離と同じ値にする。
    /// `短い` は測れない版、`通る` は通る版である。
    ///
    /// 距離のあとに `しています。=16` のように言い回しと回数を並べられる。 どれも 5,000 字の
    /// 版で本人の上限 2.8 回、つまり 14 回まで使える、とする。 超えたぶんが使いすぎである。
    fn stub_judge(p: &Path) -> Checked {
        let body = std::fs::read_to_string(p).unwrap_or_default();
        let body = body.trim();
        let used: Vec<(String, usize)> = body
            .split_whitespace()
            .filter_map(|w| {
                let (text, n) = w.split_once('=')?;
                Some((text.to_owned(), n.parse().ok()?))
            })
            .collect();
        let headroom = used
            .iter()
            .map(|(text, times)| kuchiyose_review::Headroom {
                text: text.clone(),
                times: *times,
                #[allow(clippy::cast_precision_loss)]
                density: *times as f64 / 5.0,
                ceiling: 2.8,
                allowed: 14,
            })
            .collect();
        let overused: Vec<(String, usize)> = used
            .iter()
            .filter(|(_, n)| *n > 14)
            .map(|(t, n)| (t.clone(), n - 14))
            .collect();
        let checked = |verdict: Verdict, version: Version| Checked {
            verdict,
            prose: format!("検めた: {body}\n"),
            json: format!("{{\"draft\":\"{body}\"}}\n"),
            humanness_by_metric: match &version {
                Version::Measured(Standing {
                    amount: Amount::Humanness { distance },
                    ..
                }) => vec![("圧縮率".to_owned(), *distance)],
                _ => Vec::new(),
            },
            version,
            directives: 3,
            chars: body.chars().count(),
            headroom,
        };
        let at = |verdict: Verdict, stage: Stage, amount: Amount| {
            Version::Measured(Standing {
                verdict,
                stage,
                amount,
                overused: overused.clone(),
                baseline_times: 0,
            })
        };
        match body.split_whitespace().collect::<Vec<_>>().as_slice() {
            ["通る"] => checked(
                Verdict::Pass,
                at(
                    Verdict::Pass,
                    Stage::Directive,
                    Amount::Directive {
                        outside: 0,
                        excess: 0.0,
                    },
                ),
            ),
            ["短い"] => checked(
                Verdict::Unknown,
                Version::Unmeasurable("短すぎて測れない".into()),
            ),
            ["通らない", d, ..] => checked(
                Verdict::Fail,
                at(
                    Verdict::Fail,
                    Stage::Humanness,
                    Amount::Humanness {
                        distance: d.parse().unwrap(),
                    },
                ),
            ),
            _ => panic!("試験の版が読めない: {body}"),
        }
    }

    /// 決まった版を決まった順に書く偽の道具。プロンプトの「- 書く: 」の行から経路を取る。
    ///
    /// `versions` の n 番目を n 周目に書く。空の文字列なら何も書かずに 0 で終わり、
    /// `失敗` なら 3 で終わる。
    fn fake_agent(dir: &TempDir, versions: &[&str]) -> Agent {
        for (i, v) in versions.iter().enumerate() {
            dir.write(&format!("偽/{}.txt", i + 1), v);
        }
        let script = dir.write(
            "偽/道具.sh",
            format!(
                "#!/bin/sh\n\
                 [ \"$1\" = batch ] || exit 9\n\
                 n=$(cat '{d}/偽/count' 2>/dev/null || echo 0)\n\
                 n=$((n + 1))\n\
                 echo $n > '{d}/偽/count'\n\
                 line=$(grep '^- 書く: ' \"$2\")\n\
                 out=${{line#- 書く: }}\n\
                 v=$(cat '{d}/偽/'$n'.txt')\n\
                 [ \"$v\" = 失敗 ] && {{ echo 道具が落ちた >&2; exit 3; }}\n\
                 [ -z \"$v\" ] && exit 0\n\
                 printf '%s\\n' \"$v\" > \"$out\"\n",
                d = dir.path()
            ),
        );
        Agent {
            kind: Kind::Custom(format!("sh '{script}' {{mode}} {{prompt_file}}")),
            program: PathBuf::from("/bin/sh"),
        }
    }

    fn plan(dir: &TempDir, first: &str, rounds: usize) -> Plan {
        let original = dir.write("草稿.md", format!("{first}\n"));
        Plan::new(PathBuf::from(original), None, rounds)
    }

    fn run(plan: &Plan, agent: &Agent) -> Report {
        run_rounds(plan, &stub_judge, &|l| agent::run_batch(agent, l)).expect("回せる")
    }

    fn body(p: &Path) -> String {
        std::fs::read_to_string(p).unwrap().trim().to_owned()
    }

    #[test]
    fn 悪くなった版は採らず前の版が残る() {
        let dir = TempDir::new("polish-worse");
        let p = plan(&dir, "通らない 0.5", 1);
        let r = run(&p, &fake_agent(&dir, &["通らない 0.4"]));
        assert_eq!(r.adopted, 0);
        assert_eq!(r.kept_path, p.version(0));
        assert_eq!(r.rounds_run, 1);
    }

    #[test]
    fn 良くなった版は採り次の周はその版から直させる() {
        let dir = TempDir::new("polish-better");
        let p = plan(&dir, "通らない 0.5", 2);
        let r = run(&p, &fake_agent(&dir, &["通らない 0.6", "通らない 0.7"]));
        assert_eq!(r.adopted, 2);
        assert_eq!(r.kept_path, p.version(2));
        let second = std::fs::read_to_string(p.work.join("round-2.prompt.md")).unwrap();
        assert!(
            second.contains(&format!("- 読む: {}", p.version(1).display())),
            "{second}"
        );
        assert!(
            second.contains("検めた: 通らない 0.6"),
            "採った版の検めを渡す"
        );
    }

    #[test]
    fn 捨てた版からは続けない() {
        let dir = TempDir::new("polish-no-continue");
        let p = plan(&dir, "通らない 0.5", 2);
        let r = run(&p, &fake_agent(&dir, &["通らない 0.1", "通らない 0.2"]));
        assert_eq!(r.adopted, 0);
        let second = std::fs::read_to_string(p.work.join("round-2.prompt.md")).unwrap();
        assert!(
            second.contains(&format!("- 読む: {}", p.version(0).display())),
            "{second}"
        );
    }

    fn prompt_of(p: &Plan, n: usize) -> String {
        std::fs::read_to_string(p.work.join(format!("round-{n}.prompt.md"))).unwrap()
    }

    #[test]
    fn 捨てた周の次の周には捨てた理由と悪い向きに動いた指標と変えたところを伝える() {
        let dir = TempDir::new("polish-rejected-feedback");
        let p = plan(&dir, "通らない 0.5", 2);
        run(&p, &fake_agent(&dir, &["通らない 0.4", "通らない 0.3"]));
        assert!(
            !prompt_of(&p, 1).contains("## 捨てた直し"),
            "まだ何も捨てていない"
        );
        let second = prompt_of(&p, 2);
        let want = "\n## 捨てた直し\n\n\
            読む経路の文章を直した版を、これまでに 1 回捨てた。どれも採らなかった。\n\
            \n### 1 周目\n\n\
            - 採らなかった理由: 基準との距離が 0.500 から 0.400 に下がった。大きいほうが良い\n\
            - 悪い向きに動いた指標: 圧縮率 0.500 → 0.400\n\
            - 変えたところ 1 か所:\n  \
            - 「通らない 0.5」→「通らない 0.4」\n";
        assert!(second.contains(want), "{second}");
        assert!(second.contains("同じ直しを繰り返さない。"), "{second}");
        assert!(p.work.join("round-1.rejected.json").exists());
        assert!(p.work.join("round-2.rejected.json").exists());
    }

    #[test]
    fn 捨てた直しは採った版が変わると伝えなくなる() {
        let dir = TempDir::new("polish-rejected-reset");
        let p = plan(&dir, "通らない 0.5", 4);
        run(
            &p,
            &fake_agent(
                &dir,
                &[
                    "通らない 0.4",
                    "通らない 0.6",
                    "通らない 0.3",
                    "通らない 0.2",
                ],
            ),
        );
        assert!(prompt_of(&p, 2).contains("### 1 周目"));
        assert!(
            !prompt_of(&p, 3).contains("## 捨てた直し"),
            "2 周目で採った版は、まだ直して捨てていない"
        );
        let fourth = prompt_of(&p, 4);
        assert!(fourth.contains("### 3 周目"), "{fourth}");
        assert!(
            !fourth.contains("### 1 周目"),
            "前の版への直しは伝えない: {fourth}"
        );
        assert!(
            fourth.contains("「通らない 0.6」→「通らない 0.3」"),
            "今の版からの変えたところ: {fourth}"
        );
        assert!(
            !p.work.join("round-2.rejected.json").exists(),
            "採った版は捨てた版の記録を残さない"
        );
    }

    #[test]
    fn 続きから回すと元の版が同じなら前に捨てた直しを伝える() {
        let dir = TempDir::new("polish-rejected-continue");
        let p = plan(&dir, "通らない 0.5", 1);
        let agent = fake_agent(&dir, &["通らない 0.4", "通らない 0.3"]);
        run(&p, &agent);
        let printed = first_prompt(&p, &stub_judge(&p.original)).expect("周回に入る");
        assert!(
            printed.contains("### 1 周目"),
            "表示用のプロンプトも同じ: {printed}"
        );
        run(&p, &agent);
        let third = prompt_of(&p, 3);
        assert!(
            third.contains(&format!("- 読む: {}", p.version(2).display())),
            "{third}"
        );
        assert!(third.contains("### 1 周目"), "{third}");
    }

    #[test]
    fn 目盛りが違う記録は伝えない() {
        let dir = TempDir::new("polish-rejected-scale");
        let p = plan(&dir, "通らない 0.5", 1);
        let agent = fake_agent(&dir, &["通らない 0.4", "通らない 0.3"]);
        run(&p, &agent);
        let other = |path: &Path| Checked {
            json: "{\"content_hash\":\"sha256:別の形代\"}\n".to_owned(),
            ..stub_judge(path)
        };
        run_rounds(&p, &other, &|l| agent::run_batch(&agent, l)).expect("回せる");
        assert!(!prompt_of(&p, 3).contains("## 捨てた直し"));
    }

    #[test]
    fn 基準との距離が上がっても使いすぎを増やした版は採らず次の周にそう伝える() {
        let dir = TempDir::new("polish-overuse-grew");
        let p = plan(&dir, "通らない 0.5 しています。=13", 2);
        let r = run(
            &p,
            &fake_agent(
                &dir,
                &[
                    "通らない 0.6 しています。=16",
                    "通らない 0.6 しています。=14",
                ],
            ),
        );
        assert_eq!(r.adopted, 1);
        assert_eq!(r.kept_path, p.version(2), "増やさなかった版を採る");
        let second = prompt_of(&p, 2);
        assert!(
            second.contains(
                "- 採らなかった理由: 本人の上限を超えた言い回しが 0 本から 1 本に増えた\
                 （「しています。」）。使いすぎを増やした版は、止まった段の量が良くなっても採らない\n"
            ),
            "{second}"
        );
    }

    #[test]
    fn 直させるプロンプトに今の版の言い回しの上限までの余地を載せる() {
        let dir = TempDir::new("polish-headroom");
        let p = plan(&dir, "通らない 0.5 しています。=13", 1);
        run(&p, &fake_agent(&dir, &["通らない 0.4"]));
        let first = prompt_of(&p, 1);
        assert!(
            first.contains(
                "- 「しています。」: 今 13 回（1,000 字あたり 2.6 回）。この人は 1,000 字あたり 2.8 回まで。\
                 この長さなら 14 回まで。あと 1 回。\n"
            ),
            "{first}"
        );
        let printed = first_prompt(&p, &stub_judge(&p.original)).expect("周回に入る");
        assert!(
            printed.contains("## 言い回しの上限"),
            "表示用も同じ: {printed}"
        );
    }

    #[test]
    fn 良し悪しが同じ版は採らない() {
        let dir = TempDir::new("polish-same");
        let p = plan(&dir, "通らない 0.5", 1);
        let r = run(&p, &fake_agent(&dir, &["通らない 0.5"]));
        assert_eq!(r.adopted, 0);
    }

    #[test]
    fn 測れない版はほかの比べ方を見ずに捨てる() {
        let dir = TempDir::new("polish-unmeasurable");
        let p = plan(&dir, "通らない 0.5", 1);
        let r = run(&p, &fake_agent(&dir, &["短い"]));
        assert_eq!(r.adopted, 0);
        assert_eq!(r.kept_path, p.version(0));
    }

    #[test]
    fn 通る版を書くとそこで止まる() {
        let dir = TempDir::new("polish-pass");
        let p = plan(&dir, "通らない 0.5", 4);
        let r = run(&p, &fake_agent(&dir, &["通る", "通らない 0.9"]));
        assert_eq!(r.rounds_run, 1);
        assert_eq!(r.kept.verdict, Verdict::Pass);
        assert!(!p.version(2).exists(), "2 周目は起動しない");
    }

    #[test]
    fn 何も書かないか_0_以外で終わるとそこで止まり_69_で終わる() {
        for (name, versions) in [
            ("empty", ["通らない 0.6", ""]),
            ("fail", ["通らない 0.6", "失敗"]),
        ] {
            let dir = TempDir::new(&format!("polish-stop-{name}"));
            let p = plan(&dir, "通らない 0.5", 4);
            let a = fake_agent(&dir, &versions);
            let exit = polish(&p, &stub_judge, &|l| agent::run_batch(&a, l));
            assert_eq!(exit, Exit::Environment, "{name}");
            assert_eq!(
                body(&p.output),
                "通らない 0.6",
                "{name}: それまでに採った版が残る"
            );
            assert!(!p.version(3).exists(), "{name}: 失敗した周を繰り返さない");
        }
    }

    #[test]
    fn 上限の周回数に達すると止まり捨てた周も数える() {
        let dir = TempDir::new("polish-limit");
        let p = plan(&dir, "通らない 0.5", 2);
        let r = run(
            &p,
            &fake_agent(&dir, &["通らない 0.1", "通らない 0.6", "通らない 0.9"]),
        );
        assert_eq!(r.rounds_run, 2);
        assert_eq!(r.adopted, 1);
        assert!(!p.version(3).exists());
    }

    #[test]
    fn 元のファイルは上書きされず最後に採った版を別の経路に書く() {
        let dir = TempDir::new("polish-keep-original");
        let p = plan(&dir, "通らない 0.5", 1);
        let a = fake_agent(&dir, &["通らない 0.6"]);
        let exit = polish(&p, &stub_judge, &|l| agent::run_batch(&a, l));
        assert_eq!(exit, Exit::Fail);
        assert_eq!(body(&p.original), "通らない 0.5");
        assert_eq!(body(&p.output), "通らない 0.6");
        assert_eq!(p.output, PathBuf::from(dir.join("草稿.polished.md")));
    }

    #[test]
    fn 周回の版とプロンプトと検めた結果を残す() {
        let dir = TempDir::new("polish-records");
        let p = plan(&dir, "通らない 0.5", 1);
        run(&p, &fake_agent(&dir, &["通らない 0.6"]));
        for name in [
            "round-0.md",
            "round-0.review.json",
            "round-1.prompt.md",
            "round-1.md",
            "round-1.review.json",
        ] {
            assert!(p.work.join(name).exists(), "{name} が無い");
        }
        assert_eq!(body(&p.version(0)), "通らない 0.5", "元の版の写し");
    }

    #[test]
    fn 作業用のフォルダが既にあれば続きの番号から書く() {
        let dir = TempDir::new("polish-continue");
        let p = plan(&dir, "通らない 0.5", 1);
        run(&p, &fake_agent(&dir, &["通らない 0.6", "通らない 0.7"]));
        let r = run(&p, &fake_agent(&dir, &[]));
        assert_eq!(r.kept_path, p.version(3), "{:?}", r.kept_path);
        assert!(p.version(2).exists(), "元の版の写しは続きの番号に置く");
    }

    #[test]
    fn 通る草稿と測れない草稿は周回に入らない() {
        for first in ["通る", "短い"] {
            let dir = TempDir::new("polish-not-entering");
            let p = plan(&dir, first, 4);
            let r = run(&p, &fake_agent(&dir, &[]));
            assert!(r.not_entered.is_some(), "{first}");
            assert_eq!(r.rounds_run, 0, "{first}");
            assert!(!p.work.join("round-1.prompt.md").exists());
        }
    }

    #[test]
    fn 前に出す指標が無ければ周回に入らない() {
        let dir = TempDir::new("polish-no-directives");
        let p = plan(&dir, "通らない 0.5", 1);
        let judge = |path: &Path| Checked {
            directives: 0,
            ..stub_judge(path)
        };
        let a = fake_agent(&dir, &["通らない 0.9"]);
        let r = run_rounds(&p, &judge, &|l| agent::run_batch(&a, l)).unwrap();
        assert!(r.not_entered.unwrap().contains("前に出す指標が無い"));
    }

    #[test]
    fn 報告は回した周と採った周と字数を足す() {
        let dir = TempDir::new("polish-report");
        let p = plan(&dir, "通らない 0.5", 2);
        let r = run(&p, &fake_agent(&dir, &["通らない 0.6", "通らない 0.1"]));
        let text = report_text(&p, &r, Some(&p.output));
        assert!(text.starts_with("検めた: 通らない 0.6"), "{text}");
        assert!(
            text.contains("回した周: 2（上限 2） / 採った周: 1"),
            "{text}"
        );
        assert!(text.contains("元の版"), "{text}");
        assert!(
            text.contains("推論設定の直し方"),
            "基準との距離で止まっている: {text}"
        );
    }

    #[test]
    fn 表示用のプロンプトは元のファイルを読み書き出し先に書かせる() {
        let dir = TempDir::new("polish-print");
        let p = plan(&dir, "通らない 0.5", 1);
        let first = stub_judge(&p.original);
        let got = first_prompt(&p, &first).expect("周回に入る");
        assert!(
            got.contains(&format!("- 読む: {}", p.original.display())),
            "{got}"
        );
        assert!(
            got.contains(&format!("- 書く: {}", p.output.display())),
            "{got}"
        );
        assert!(!p.work.exists(), "作業用のフォルダも作らない");
    }

    #[test]
    fn 上限の周回数は_1_以上の整数である() {
        assert_eq!(parse_rounds("3"), Ok(3));
        assert_eq!(parse_rounds("0"), Err(Exit::Usage));
        assert_eq!(parse_rounds("x"), Err(Exit::Usage));
        assert_eq!(parse_rounds(&MAX_ROUNDS.to_string()), Ok(MAX_ROUNDS));
        assert_eq!(
            parse_rounds(&(MAX_ROUNDS + 1).to_string()),
            Err(Exit::Usage),
            "設定ファイルと同じ上限を掛ける"
        );
    }

    #[test]
    fn 版の番号が数えられる上限に届いていれば続きを振らずに断る() {
        let dir = TempDir::new("polish-overflow");
        let p = plan(&dir, "通らない 0.5", 1);
        std::fs::create_dir_all(&p.work).unwrap();
        std::fs::write(p.work.join(format!("round-{}.md", usize::MAX)), "細工").unwrap();
        assert_eq!(p.next_number(), None);
        let got = run_rounds(&p, &stub_judge, &|_| unreachable!("道具を起動しない"));
        assert_eq!(got.err(), Some(Exit::Usage));
        assert!(!p.version(0).exists(), "元の版を写していない");
    }

    /// 今の版をそのまま書き写す偽の道具。
    fn copying_agent(dir: &TempDir) -> String {
        let script = dir.write(
            "写す.sh",
            "#!/bin/sh\n\
             r=$(grep '^- 読む: ' \"$2\")\n\
             w=$(grep '^- 書く: ' \"$2\")\n\
             cp \"${r#- 読む: }\" \"${w#- 書く: }\"\n",
        );
        format!("sh '{script}' {{mode}} {{prompt_file}}")
    }

    fn cli_args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn どの周でも目盛りの組み立ては周回の最初の_1_度だけである() {
        let dir = TempDir::new("polish-assemble-once");
        let (p, b) = crate::fixture::katashiro_pair(&dir);
        let body = kuchiyose_metrics::humanness::joined(&crate::fixture::document(3, true).prose());
        let draft = dir.write("草稿.md", &body);
        let env = Env::for_test(
            dir.path(),
            &[
                ("XDG_CONFIG_HOME", &dir.join("設定")),
                ("KUCHIYOSE_AGENT_CMD", &copying_agent(&dir)),
            ],
        );
        let calls = std::cell::RefCell::new(0usize);
        let spy = |t: &kuchiyose_scale::stats::KatashiroStats,
                   base: &kuchiyose_scale::stats::KatashiroStats,
                   tuning: assembly::Tuning,
                   by: &dyn Fn(&str) -> bool| {
            *calls.borrow_mut() += 1;
            assembly::assemble_stats(t, base, tuning, by)
        };
        let exit = run_with(
            &cli_args(&[&draft, "--katashiro", &p, "--baseline", &b, "--rounds", "2"]),
            &env,
            &spy,
        );
        assert!(exit.code() < 64, "{exit:?}");
        assert_eq!(*calls.borrow(), 1);
        assert_eq!(
            std::fs::read_to_string(&draft).unwrap(),
            body,
            "元のファイルは上書きしない"
        );
    }

    #[test]
    fn 表示だけなら道具を起動せず周回に入らない草稿ではプロンプトを出さない() {
        let dir = TempDir::new("polish-print-short");
        let (p, b) = crate::fixture::katashiro_pair(&dir);
        let draft = dir.write("草稿.md", "短い草稿。\n");
        let env = Env::for_test(dir.path(), &[("XDG_CONFIG_HOME", &dir.join("設定"))]);
        let exit = crate::run_in(
            &cli_args(&[
                "polish",
                &draft,
                "--katashiro",
                &p,
                "--baseline",
                &b,
                "--print",
            ]),
            &env,
        );
        assert_eq!(exit, Exit::Unknown);
        assert!(!Path::new(&dir.join("草稿.kuchiyose")).exists());
    }

    #[test]
    fn 短すぎる草稿は周回に入らず判定できないで終わる() {
        let dir = TempDir::new("polish-short");
        let (p, b) = crate::fixture::katashiro_pair(&dir);
        let draft = dir.write("草稿.md", "短い草稿。\n");
        let env = Env::for_test(
            dir.path(),
            &[
                ("XDG_CONFIG_HOME", &dir.join("設定")),
                ("KUCHIYOSE_AGENT_CMD", &copying_agent(&dir)),
            ],
        );
        let exit = crate::run_in(
            &cli_args(&["polish", &draft, "--katashiro", &p, "--baseline", &b]),
            &env,
        );
        assert_eq!(exit, Exit::Unknown);
        assert!(Path::new(&dir.join("草稿.kuchiyose/round-0.review.json")).exists());
        assert!(!Path::new(&dir.join("草稿.kuchiyose/round-1.prompt.md")).exists());
    }

    #[test]
    fn 本物の検めで周回を回しても目盛りの組み立ては最初の_1_度だけである() {
        // 試験の素材には前に出す指標が無く、そのままでは周回に入らない。 版を検める
        // 関数は本物のまま、前に出す指標の本数だけを足して周回に入れる。
        let dir = TempDir::new("polish-real-rounds");
        let (p, b) = crate::fixture::katashiro_pair(&dir);
        let body = kuchiyose_metrics::humanness::joined(&crate::fixture::document(3, true).prose());
        let draft = dir.write("草稿.md", &body);
        let calls = std::cell::RefCell::new(0usize);
        let spy = |t: &kuchiyose_scale::stats::KatashiroStats,
                   base: &kuchiyose_scale::stats::KatashiroStats,
                   tuning: assembly::Tuning,
                   by: &dyn Fn(&str) -> bool| {
            *calls.borrow_mut() += 1;
            assembly::assemble_stats(t, base, tuning, by)
        };
        let session = Session::open(p, Some(b), &spy).expect("開ける");
        let judge = |path: &Path| Checked {
            directives: 1,
            ..session.check(&path.display().to_string())
        };
        let plan = Plan::new(PathBuf::from(&draft), None, 2);
        let agent = Agent {
            kind: Kind::Custom(copying_agent(&dir)),
            program: PathBuf::from("/bin/sh"),
        };
        let r = run_rounds(&plan, &judge, &|l| agent::run_batch(&agent, l)).expect("回せる");
        assert_eq!(r.not_entered, None);
        assert_eq!(r.rounds_run, 2);
        assert_eq!(r.adopted, 0, "同じ版は採らない");
        assert_eq!(*calls.borrow(), 1);
        let json = std::fs::read_to_string(plan.work.join("round-2.review.json")).unwrap();
        assert!(
            json.contains("\"verdict\""),
            "review --json と同じ形: {json}"
        );
        let prompt = std::fs::read_to_string(plan.work.join("round-1.prompt.md")).unwrap();
        assert!(
            prompt.contains("判定: 通らない"),
            "review の散文をそのまま入れる: {prompt}"
        );
    }
}
