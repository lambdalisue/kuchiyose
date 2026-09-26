//! `kuchiyose write`。代筆させる（[write](../../../docs/design/200-command.md#write)）。
//!
//! 代筆のプロンプトを組み立て、LLM の道具を対話の画面で起動する。 使う人は道具と
//! 対話して内容を詰め、道具が草稿を保存する。 kuchiyose は対話に関わらない。
//! 対話が終わって草稿があれば、そのまま表現を寄せる周回に入る。

use std::io::Read as _;
use std::path::Path;

use kuchiyose_scale::assembly;

use crate::agent::{self, Failure, Launch, Task};
use crate::env::{Env, Input};
use crate::exit::Exit;
use crate::facts;
use crate::pair::Assemble;
use crate::polish_cmd::{self, Plan};
use crate::remedies::FromDefinitions;
use crate::review::Session;
use crate::{config, katashiros};

/// `write` の help。
pub const HELP: &str = "\
kuchiyose write [<要約>] [-o <草稿>] [--katashiro <形代>] [--baseline <形代>]
                [--agent <道具>] [--rounds <数>] [--no-polish] [--print]
    代筆させる。ペルソナ・文体の事実・要約・保存先からプロンプトを組み立て、LLM の
    道具を対話の画面で起動する。道具と対話して内容を詰めると、道具が草稿を保存する。
    対話が終わって草稿があれば、そのまま polish と同じ周回に入る。
    要約は、引数があれば引数、無くて標準入力が端末でなければ標準入力、どちらでも
    なければ $EDITOR（無ければ vi）で書く。コメントを除いて空なら 64。
    標準入力から要約を読んだときは、道具の入出力を端末につなぎ直す。端末が無ければ 64。
    -o           草稿の保存先。省けば今のディレクトリの draft-<日時>.md。
                 既にファイルがあれば起動せずに 64。
    --no-polish  対話が終わったところで止める。
    --print      道具を起動せず、代筆のプロンプトを出す。周回にも入らない。
    終了コード: 周回に入れば polish と同じ。入らなければ、草稿が無ければ 65、
    --no-polish なら 0。";

/// `$EDITOR` に入れておく見本。Markdown の見出しと区別できるよう、HTML のコメントにする。
const BRIEF_TEMPLATE: &str = "\n<!--\n\
何を書きたいかを書く。保存して閉じると、LLM の道具が起動する。\n\
題材、読み手、伝えたいこと、入れたい例などを短く。\n\
このコメントは消さなくてよい。コメントのほかが空のまま閉じると、何もしない。\n\
-->\n";

/// 引数。
#[derive(Debug, Default)]
struct Args {
    brief: Option<String>,
    output: Option<String>,
    katashiro: Option<String>,
    baseline: Option<String>,
    agent: Option<String>,
    rounds: Option<usize>,
    no_polish: bool,
    print: bool,
}

fn parse_args(args: &[String]) -> Result<Args, Exit> {
    let mut a = Args::default();
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].as_str();
        match flag {
            "--print" => a.print = true,
            "--no-polish" => a.no_polish = true,
            "-o" | "--output" | "--katashiro" | "--baseline" | "--agent" | "--rounds" => {
                let Some(v) = args.get(i + 1).cloned() else {
                    eprintln!("{flag} に値を渡す");
                    return Err(Exit::Usage);
                };
                match flag {
                    "--katashiro" => a.katashiro = Some(v),
                    "--baseline" => a.baseline = Some(v),
                    "--agent" => a.agent = Some(v),
                    "--rounds" => a.rounds = Some(polish_cmd::parse_rounds(&v)?),
                    _ => a.output = Some(v),
                }
                i += 1;
            }
            other if other.starts_with('-') => {
                eprintln!("知らない引数: {other}");
                return Err(Exit::Usage);
            }
            other if a.brief.is_none() => a.brief = Some(other.to_owned()),
            other => {
                eprintln!("要約は 1 つの引数で渡す。空白を含むなら引用符で囲む: {other}");
                return Err(Exit::Usage);
            }
        }
        i += 1;
    }
    Ok(a)
}

/// HTML のコメントを除く。
fn strip_comments(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("<!--") {
        out.push_str(&rest[..i]);
        match rest[i..].find("-->") {
            Some(j) => rest = &rest[i + j + 3..],
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// `$EDITOR` で要約を書かせる。
fn brief_from_editor(env: &Env) -> Result<String, Exit> {
    let file = crate::private::create_file(
        &std::env::temp_dir(),
        "kuchiyose-brief-",
        ".md",
        BRIEF_TEMPLATE.as_bytes(),
    )
    .map_err(|e| {
        eprintln!("断る: 要約を書くファイルが作れない: {e}");
        Exit::Unreadable
    })?;
    let editor = env.var("EDITOR").unwrap_or("vi");
    let status = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("sh")
        .arg(&file)
        .status();
    let body = std::fs::read_to_string(&file).unwrap_or_default();
    std::fs::remove_file(&file).ok();
    match status {
        Ok(s) if s.success() => Ok(body),
        Ok(_) | Err(_) => {
            eprintln!("エディタが失敗した: {editor}");
            Err(Exit::Usage)
        }
    }
}

/// 要約を取る（[要約の渡し方](../../../docs/design/200-command.md#要約の渡し方)）。
fn brief(a: &Args, env: &Env) -> Result<String, Exit> {
    let raw = match (&a.brief, &env.stdin) {
        (Some(b), _) => b.clone(),
        (None, Input::Stream) => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s).map_err(|e| {
                eprintln!("断る: 標準入力が読めない: {e}");
                Exit::Unreadable
            })?;
            s
        }
        #[cfg(test)]
        (None, Input::Text(t)) => t.clone(),
        (None, Input::Terminal) => brief_from_editor(env)?,
    };
    let text = strip_comments(&raw).trim().to_owned();
    if text.is_empty() {
        eprintln!("要約が空なので、何もしない");
        return Err(Exit::Usage);
    }
    Ok(text)
}

/// 今の日時。草稿の既定の名前に使う。手元の時刻で `YYYYMMDD-HHMMSS`。
fn now_stamp() -> String {
    #[repr(C)]
    struct Tm {
        sec: i32,
        min: i32,
        hour: i32,
        mday: i32,
        mon: i32,
        year: i32,
        wday: i32,
        yday: i32,
        isdst: i32,
        gmtoff: i64,
        zone: *const std::ffi::c_char,
    }
    unsafe extern "C" {
        fn localtime_r(t: *const i64, out: *mut Tm) -> *mut Tm;
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
    let mut tm = Tm {
        sec: 0,
        min: 0,
        hour: 0,
        mday: 0,
        mon: 0,
        year: 0,
        wday: 0,
        yday: 0,
        isdst: 0,
        gmtoff: 0,
        zone: std::ptr::null(),
    };
    // SAFETY: 読むのは渡した値だけで、書くのは渡した構造体だけである。
    let ok = unsafe { !localtime_r(&raw const secs, &raw mut tm).is_null() };
    if !ok {
        return secs.to_string();
    }
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        tm.year + 1900,
        tm.mon + 1,
        tm.mday,
        tm.hour,
        tm.min,
        tm.sec
    )
}

/// 代筆のプロンプトを組み立てる。同じ形代・基準・要約・保存先からは同じバイト列が出る。
///
/// ペルソナは `kuchiyose-prompt` にだけ渡す。測る側へは渡さない。
///
/// # Errors
///
/// 形代のペルソナが読めなければ 65 で断る。
pub fn draft_prompt_for(session: &Session, brief: &str, save: &str) -> Result<String, Exit> {
    let target = session.target();
    let persona = match target
        .katashiro
        .persona
        .as_deref()
        .map(kuchiyose_prompt::parse)
    {
        None => None,
        Some(Ok(p)) => Some(p),
        Some(Err(e)) => {
            eprintln!("断る: 形代のペルソナが読めない: {e}");
            eprintln!("katashiro persona で取り込み直す");
            return Err(Exit::Unreadable);
        }
    };
    // 目盛りが組み立てられなくても代筆は止めない。 基準が要る事実を置かずに作る。
    let built = match session.usable() {
        Ok(b) => Some(b),
        Err(why) => {
            eprintln!("{why}");
            eprintln!("基準が要る文体の事実（型、避ける言い回し）を置かずにプロンプトを作る");
            None
        }
    };
    let facts = facts::style_facts(
        &target.stats,
        &target.katashiro.tuning,
        built,
        &FromDefinitions::load(),
    );
    Ok(kuchiyose_prompt::draft_prompt(
        &kuchiyose_prompt::DraftRequest {
            persona: persona.as_ref(),
            facts: &facts,
            brief,
            save_path: save,
        },
    ))
}

/// `kuchiyose write`。
pub fn run(args: &[String], env: &Env) -> Exit {
    run_with(args, env, &assembly::assemble_stats)
}

/// 組み立ての関数を差し替えて書かせる。
#[allow(clippy::too_many_lines)]
pub fn run_with(args: &[String], env: &Env, assemble: &Assemble<'_>) -> Exit {
    let a = match parse_args(args) {
        Ok(a) => a,
        Err(e) => return e,
    };
    let cfg = match config::load(env) {
        Ok(c) => c,
        Err(e) => return e,
    };
    let katashiro = match katashiros::resolve(a.katashiro.as_deref(), env) {
        Ok(k) => k,
        Err(e) => return e,
    };
    let brief = match brief(&a, env) {
        Ok(b) => b,
        Err(e) => return e,
    };
    let save = a.output.as_deref().map_or_else(
        || env.cwd.join(format!("draft-{}.md", now_stamp())),
        |o| env.absolute(o),
    );
    if !a.print && save.exists() {
        eprintln!("断る: 草稿の保存先に既にファイルがある: {}", save.display());
        eprintln!("道具が上書きすれば人のファイルが消える。-o で別の経路を渡す");
        return Exit::Usage;
    }
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
    let save_str = save.display().to_string();
    let prompt = match draft_prompt_for(&session, &brief, &save_str) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let Some(agent) = agent else {
        print!("{prompt}");
        return Exit::Pass;
    };

    let rounds = a
        .rounds
        .or(cfg.rounds)
        .unwrap_or(polish_cmd::DEFAULT_ROUNDS);
    let plan = Plan::new(save.clone(), None, rounds);
    let prompt_file = plan.work.join("write.prompt.md");
    let wrote =
        std::fs::create_dir_all(&plan.work).and_then(|()| std::fs::write(&prompt_file, &prompt));
    if let Err(e) = wrote {
        eprintln!("断る: プロンプトを書けない: {}: {e}", prompt_file.display());
        return Exit::Unreadable;
    }
    let cwd = save
        .parent()
        .map_or_else(|| env.cwd.clone(), Path::to_path_buf);
    let launched = agent::run_interactive(
        &agent,
        &Launch {
            task: Task::Draft,
            prompt: &prompt,
            prompt_file: &prompt_file,
            cwd: &cwd,
        },
        env,
    );
    match launched {
        Err(Failure::NoTerminal(why)) => {
            eprintln!("断る: 対話の画面を起動できない。端末が開けない（{why}）");
            eprintln!("--print でプロンプトだけを出せる");
            return Exit::Usage;
        }
        Err(e @ Failure::NotStarted(_)) => {
            eprintln!("{e}");
            return Exit::Environment;
        }
        // 対話の画面は、使う人が止めても 0 以外で終わる。 草稿があるかで決める。
        Err(Failure::Exited { .. }) | Ok(()) => {}
    }
    if !save.is_file() {
        eprintln!("草稿が無い: {}", save.display());
        eprintln!("道具に、この経路へ書くよう頼む。書いたら polish で周回だけを回せる");
        return Exit::Unreadable;
    }
    if a.no_polish {
        println!("草稿: {}", save.display());
        return Exit::Pass;
    }
    polish_cmd::run_session(&plan, &session, Some(&agent))
}

/// 草稿の保存先。試験で見るために出す。
#[cfg(test)]
fn default_save(env: &Env) -> std::path::PathBuf {
    env.cwd.join(format!("draft-{}.md", now_stamp()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;
    use crate::testdir::TempDir;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn コメントを除いて空の要約では何もしない() {
        assert_eq!(strip_comments(BRIEF_TEMPLATE).trim(), "");
        assert_eq!(strip_comments("前<!-- 見本 -->後\n<!-- 閉じない"), "前後\n");
    }

    #[test]
    fn 草稿の既定の名前は今のディレクトリの日時である() {
        let env = Env::for_test("/w", &[]);
        let p = default_save(&env).display().to_string();
        assert!(p.starts_with("/w/draft-"), "{p}");
        assert_eq!(p.len(), "/w/draft-20260927-153012.md".len(), "{p}");
    }

    /// 本人の形代を作り、既定の形代として使う環境を返す。
    fn setup(dir: &TempDir) -> (String, Env) {
        let person = fixture::write_corpus(dir, "本人", false);
        let k = dir.join("本人.katashiro");
        assert_eq!(
            crate::run(&args(&["katashiro", "build", &person, "-o", &k])),
            Exit::Pass
        );
        let env = Env::for_test(
            dir.path(),
            &[
                ("XDG_CONFIG_HOME", &dir.join("設定")),
                ("KUCHIYOSE_KATASHIRO", &k),
                ("PATH", "/usr/bin:/bin"),
            ],
        );
        (k, env)
    }

    #[test]
    fn 形代を渡さず既定も無ければ断る() {
        let dir = TempDir::new("write-no-katashiro");
        let env = Env::for_test(dir.path(), &[("XDG_CONFIG_HOME", dir.path())]);
        assert_eq!(
            crate::run_in(&args(&["write", "要約", "--print"]), &env),
            Exit::Usage
        );
    }

    #[test]
    fn 保存先に既にファイルがあれば起動せずに断る() {
        let dir = TempDir::new("write-exists");
        let (_, env) = setup(&dir);
        let draft = dir.write("草稿.md", "人のファイル");
        let marker = dir.join("起動した");
        let mut env = env;
        env.vars.insert(
            "KUCHIYOSE_AGENT_CMD".into(),
            format!("touch '{marker}' # {{prompt_file}}"),
        );
        assert_eq!(
            crate::run_in(&args(&["write", "要約", "-o", &draft]), &env),
            Exit::Usage
        );
        assert!(!Path::new(&marker).exists());
        assert_eq!(std::fs::read_to_string(&draft).unwrap(), "人のファイル");
    }

    #[test]
    fn 要約が空なら何もしない() {
        let dir = TempDir::new("write-empty-brief");
        let (_, mut env) = setup(&dir);
        env.stdin = Input::Text("<!-- 見本だけ -->\n".into());
        assert_eq!(
            crate::run_in(&args(&["write", "--print"]), &env),
            Exit::Usage
        );
    }

    #[test]
    fn エディタで書いた要約を使う() {
        let dir = TempDir::new("write-editor");
        let (_, mut env) = setup(&dir);
        let editor = dir.write(
            "編集.sh",
            "#!/bin/sh\necho 'エディタで書いた要約' >> \"$1\"\n",
        );
        env.vars.insert("EDITOR".into(), format!("sh '{editor}'"));
        let out = dir.join("草稿.md");
        let mut v = args(&["write", "--print", "-o"]);
        v.push(out);
        assert_eq!(crate::run_in(&v, &env), Exit::Pass);
        assert_eq!(
            brief(&parse_args(&args(&[])).unwrap(), &env).unwrap(),
            "エディタで書いた要約"
        );
    }

    #[test]
    fn 標準入力の要約を使う() {
        let dir = TempDir::new("write-stdin");
        let (_, mut env) = setup(&dir);
        env.stdin = Input::Text("標準入力の要約\n".into());
        assert_eq!(
            brief(&parse_args(&args(&[])).unwrap(), &env).unwrap(),
            "標準入力の要約"
        );
        let a = parse_args(&args(&["引数の要約"])).unwrap();
        assert_eq!(brief(&a, &env).unwrap(), "引数の要約", "引数が先");
    }

    /// 偽の道具。対話の画面なら保存先の経路を草稿に書き、周回なら何もしない。
    fn fake(dir: &TempDir, body: &str) -> String {
        let script = dir.write(
            "偽の道具.sh",
            format!(
                "#!/bin/sh\n\
                 [ -t 0 ] && echo 端末 > '{d}/標準入力' || echo 端末でない > '{d}/標準入力'\n\
                 [ \"$1\" = interactive ] || exit 0\n\
                 line=$(grep '^- /' \"$2\" | tail -n 1)\n\
                 out=${{line#- }}\n\
                 printf '%s\\n' '{body}' > \"$out\"\n",
                d = dir.path()
            ),
        );
        format!("sh '{script}' {{mode}} {{prompt_file}}")
    }

    #[test]
    fn 道具は決めた保存先に草稿を書き草稿が無ければ_65_で終わる() {
        let dir = TempDir::new("write-save");
        let (_, mut env) = setup(&dir);
        env.vars
            .insert("KUCHIYOSE_AGENT_CMD".into(), fake(&dir, "短い草稿"));
        let out = dir.join("草稿.md");
        let got = crate::run_in(&args(&["write", "要約", "-o", &out, "--no-polish"]), &env);
        assert_eq!(got, Exit::Pass);
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "短い草稿\n");
        assert!(
            Path::new(&dir.join("草稿.kuchiyose/write.prompt.md")).exists(),
            "渡したプロンプトを残す"
        );

        let silent = dir.write("何もしない.sh", "#!/bin/sh\nexit 0\n");
        env.vars.insert(
            "KUCHIYOSE_AGENT_CMD".into(),
            format!("sh '{silent}' {{prompt_file}}"),
        );
        let missing = dir.join("書かれない.md");
        assert_eq!(
            crate::run_in(&args(&["write", "要約", "-o", &missing]), &env),
            Exit::Unreadable
        );
    }

    #[test]
    fn 草稿があれば周回に入り短すぎれば周回に入らず判定できないで終わる() {
        let dir = TempDir::new("write-polish");
        let (_, mut env) = setup(&dir);
        env.vars
            .insert("KUCHIYOSE_AGENT_CMD".into(), fake(&dir, "短い草稿"));
        let out = dir.join("草稿.md");
        let got = crate::run_in(&args(&["write", "要約", "-o", &out]), &env);
        assert_eq!(got, Exit::Unknown);
        assert!(Path::new(&dir.join("草稿.kuchiyose/round-0.md")).exists());
        assert!(!Path::new(&dir.join("草稿.kuchiyose/round-1.prompt.md")).exists());
    }

    #[test]
    fn 標準入力から要約を読んだら道具の標準入力を端末につなぎ直す() {
        // 端末の無い環境では 64 で断る。 どちらになるかは走らせた環境で決まる。
        let dir = TempDir::new("write-tty");
        let (_, mut env) = setup(&dir);
        env.stdin = Input::Text("要約\n".into());
        env.vars
            .insert("KUCHIYOSE_AGENT_CMD".into(), fake(&dir, "短い草稿"));
        let out = dir.join("草稿.md");
        let got = crate::run_in(&args(&["write", "-o", &out, "--no-polish"]), &env);
        let has_tty = std::fs::File::options()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .is_ok();
        if has_tty {
            assert_eq!(got, Exit::Pass);
            assert_eq!(
                std::fs::read_to_string(dir.join("標準入力")).unwrap(),
                "端末\n"
            );
        } else {
            assert_eq!(got, Exit::Usage);
            assert!(!Path::new(&dir.join("標準入力")).exists(), "起動していない");
        }
    }

    fn pair(dir: &TempDir) -> (String, String) {
        fixture::katashiro_pair(dir)
    }

    fn open(p: &str, b: &str) -> Session {
        Session::open(p.to_owned(), Some(b.to_owned()), &assembly::assemble_stats).expect("開ける")
    }

    #[test]
    fn 同じ形代_基準_要約_保存先からは同じバイト列のプロンプトが出る() {
        let dir = TempDir::new("write-deterministic");
        let (p, b) = pair(&dir);
        let q = crate::persona_cmd::tests::quote_from_material();
        let persona = dir.write("p.md", crate::persona_cmd::tests::persona_text(&q));
        crate::persona_cmd::import(&p, &persona, None).expect("取り込める");
        let one = draft_prompt_for(&open(&p, &b), "要約", "/d/草稿.md").unwrap();
        let two = draft_prompt_for(&open(&p, &b), "要約", "/d/草稿.md").unwrap();
        assert_eq!(one, two);
        assert!(one.contains("## ペルソナ"), "{one}");
        assert!(one.contains("## 文体の事実"), "{one}");
    }

    #[test]
    fn 無効にした型と基準の型と基準の語はプロンプトに出ない() {
        let dir = TempDir::new("write-muted");
        let (p, b) = pair(&dir);
        let before = {
            let s = open(&p, &b);
            let built = s.usable().expect("組み立てられる");
            let top = |v: Vec<(String, f64)>| {
                v.into_iter()
                    .max_by(|x, y| x.1.partial_cmp(&y.1).unwrap().then_with(|| y.0.cmp(&x.0)))
            };
            let kata = top(built
                .scale
                .katas
                .iter()
                .map(|k| (k.shown(), k.rate))
                .collect());
            (kata, draft_prompt_for(&s, "要約", "/d/草稿.md").unwrap())
        };
        let (kata, prompt) = before;
        let kata = kata.expect("試験の素材に本人の型がある").0;
        assert!(prompt.contains(&format!("「{kata}」")), "{prompt}");
        assert!(prompt.contains("避ける言い回し"), "{prompt}");

        let mut c = katashiros::read_file(&p).unwrap();
        c.tuning
            .mute
            .get_mut(&kuchiyose_katashiro::MuteKind::Kata)
            .unwrap()
            .insert(kata.clone());
        c.tuning
            .mute_kinds
            .insert(kuchiyose_katashiro::MuteKind::BaselineKata);
        c.tuning
            .mute_kinds
            .insert(kuchiyose_katashiro::MuteKind::BaselineGoi);
        kuchiyose_katashiro::save::save(&p, &c, Some(c.generation)).unwrap();
        let after = draft_prompt_for(&open(&p, &b), "要約", "/d/草稿.md").unwrap();
        assert!(!after.contains(&format!("「{kata}」")), "{after}");
        assert!(!after.contains("避ける言い回し"), "{after}");
    }

    #[test]
    fn ペルソナを足しても外しても組み立てた目盛りと判定は変わらない() {
        // ペルソナが測る側に漏れていないこと。 型では組み立て層で渡す経路を止められない。
        let dir = TempDir::new("write-persona-leak");
        let (p, b) = pair(&dir);
        let draft = dir.write(
            "草稿.md",
            kuchiyose_metrics::humanness::joined(&fixture::document(3, true).prose()),
        );
        let without = open(&p, &b).check(&draft);
        let q = crate::persona_cmd::tests::quote_from_material();
        let persona = dir.write("p.md", crate::persona_cmd::tests::persona_text(&q));
        crate::persona_cmd::import(&p, &persona, None).expect("取り込める");
        let with = open(&p, &b).check(&draft);
        assert_eq!(with.json, without.json);
        assert_eq!(with.prose, without.prose);
        assert_eq!(with.version, without.version);
    }
}
