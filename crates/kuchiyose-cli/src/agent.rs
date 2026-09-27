//! LLM の道具の起動（[設計](../../../docs/design/400-agent.md)）。
//!
//! kuchiyose は LLM の API を呼ばない。 使う人が手元に入れてある道具を子プロセスとして
//! 起動する。道具ごとの違いは[引数の組](argv)という値にして持ち、起動する関数は
//! 対話の画面と対話なしの 2 つだけにする。起動は入出力と子プロセスの扱いで、判断を持たない。

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config::Config;
use crate::env::{Env, Input};
use crate::exit::Exit;

/// 同梱している道具の定義と、自前の道具。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// Claude Code。
    Claude,
    /// Codex CLI。
    Codex,
    /// 自前の道具。起動のしかたのひな形を持つ。
    Custom(String),
}

impl Kind {
    /// 名前。
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Kind::Claude => "claude",
            Kind::Codex => "codex",
            Kind::Custom(_) => "custom",
        }
    }
}

/// 起動する道具。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    /// どの道具か。
    pub kind: Kind,
    /// 実行ファイル。自前の道具では `/bin/sh` である。
    pub program: PathBuf,
}

/// 起動の 2 通り。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 対話の画面で始める。端末につなぎ、使う人が道具と話す。
    Interactive,
    /// 対話なしで最後まで走らせる。
    Batch,
}

impl Mode {
    /// ひな形の `{mode}` に入る名前。
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Mode::Interactive => "interactive",
            Mode::Batch => "batch",
        }
    }
}

/// 何をさせるか。許す操作の範囲がここで決まる（[権限は必要な分だけ](../../../docs/design/400-agent.md#権限は必要な分だけ)）。
#[derive(Debug, Clone, Copy)]
pub enum Task<'a> {
    /// 内容を詰めて書かせる。対話の画面で始め、あらかじめ許すものを足さない。
    Draft,
    /// ペルソナを下書きさせる。素材を読み、下書きの経路だけに書き、確かめるコマンドだけを打てる。
    ///
    /// 道具は kuchiyose が作った専用のディレクトリで走らせ、下書きの経路はその中に置く。
    Persona {
        /// 素材のフォルダ。
        material: &'a Path,
        /// 下書きを書く経路。
        write: &'a Path,
        /// 確かめるコマンド。プロンプトに書いたとおりの文字列である。
        check: &'a str,
    },
    /// 表現を直させる。今の版を読み、その周の版の経路だけに書き、その版を検めるコマンドだけを打てる。
    Polish {
        /// 今の版。
        read: &'a Path,
        /// その周の版を書く経路。
        write: &'a Path,
        /// その周の版を検めるコマンド。プロンプトに書いたとおりの文字列である。空なら何も実行させない。
        checks: &'a [String],
    },
}

impl Task<'_> {
    /// 起動のしかた。
    #[must_use]
    pub fn mode(&self) -> Mode {
        match self {
            Task::Draft => Mode::Interactive,
            Task::Persona { .. } | Task::Polish { .. } => Mode::Batch,
        }
    }
}

/// 引数に載せるプロンプトの上限（バイト）。
///
/// 引数 1 つの長さには OS の上限がある（Linux は 128 KiB）。 超えるなら、引数には
/// ファイルを読めと言う短い文だけを渡し、中身はファイルで渡す。
pub const ARG_LIMIT: usize = 100_000;

/// 1 回の起動。
#[derive(Debug, Clone, Copy)]
pub struct Launch<'a> {
    /// 何をさせるか。
    pub task: Task<'a>,
    /// プロンプト。
    pub prompt: &'a str,
    /// プロンプトを書いたファイル。起動する前に書いておく。
    pub prompt_file: &'a Path,
    /// 道具を走らせるディレクトリ。
    pub cwd: &'a Path,
}

impl Launch<'_> {
    /// 引数に載せるプロンプト。長すぎればファイルを読めと言う。
    fn prompt_arg(&self) -> String {
        if self.prompt.len() > ARG_LIMIT {
            format!(
                "次のファイルを読んで、その指示に従う: {}",
                self.prompt_file.display()
            )
        } else {
            self.prompt.to_owned()
        }
    }
}

/// Claude Code の権限の規則で、絶対経路を指す形。先頭を `//` にする。
fn claude_path(p: &Path) -> String {
    format!("/{}", p.display())
}

/// 起動の引数。先頭が実行ファイルである。
///
/// 同梱の道具の引数は、それぞれの `--help` で確かめた形である
/// （[道具ごとに違うこと](../../../docs/design/400-agent.md#道具ごとに違うこと)）。
#[must_use]
pub fn argv(agent: &Agent, launch: &Launch<'_>) -> Vec<String> {
    let program = agent.program.display().to_string();
    let prompt = launch.prompt_arg();
    let s = |v: &str| v.to_owned();
    match (&agent.kind, launch.task) {
        (Kind::Custom(template), task) => vec![
            program,
            s("-c"),
            fill(template, launch.prompt_file, task.mode()),
        ],
        (Kind::Claude, Task::Draft) | (Kind::Codex, Task::Draft) => vec![program, prompt],
        (
            Kind::Claude,
            Task::Persona {
                material,
                write,
                check,
            },
        ) => vec![
            program,
            s("-p"),
            prompt,
            s("--permission-mode"),
            s("dontAsk"),
            s("--add-dir"),
            material.display().to_string(),
            s("--tools"),
            s("Read,Glob,Grep,Write,Edit,Bash"),
            // 可変個の引数を取るので最後に置く。
            s("--allowedTools"),
            format!("Read({}/**)", claude_path(material)),
            format!("Edit({})", claude_path(write)),
            // 前置きで許す形（`:*`）にしない。 ほかの形代への取り込みや `--remove`、
            // 後ろに足したシェルの続きまで通る。
            format!("Bash({check})"),
        ],
        (
            Kind::Claude,
            Task::Polish {
                read,
                write,
                checks,
            },
        ) => {
            let tools = if checks.is_empty() {
                "Read,Write,Edit"
            } else {
                "Read,Write,Edit,Bash"
            };
            let mut v = vec![
                program,
                s("-p"),
                prompt,
                s("--permission-mode"),
                s("dontAsk"),
                s("--tools"),
                s(tools),
                // 可変個の引数を取るので最後に置く。
                s("--allowedTools"),
                format!("Read({})", claude_path(read)),
                format!("Edit({})", claude_path(write)),
            ];
            // 前置きで許す形（`:*`）にしない。 ほかの版や別の形代で検める道、後ろに
            // 足したシェルの続きまで通る。
            v.extend(checks.iter().map(|c| format!("Bash({c})")));
            v
        }
        // codex は書ける場所と実行できるものを細かく名指せない。 絞れる範囲で最も狭い
        // 段階を使う。作業のディレクトリの外には書けない。 ペルソナ作りの作業の
        // ディレクトリは、kuchiyose が作った専用のものである。 サンドボックスの中では
        // コマンドを名指さずに打てるので、周回で版を検めるコマンドもそのまま打てる。
        (Kind::Codex, Task::Persona { .. } | Task::Polish { .. }) => vec![
            program,
            s("exec"),
            s("--sandbox"),
            s("workspace-write"),
            s("--skip-git-repo-check"),
            s("--cd"),
            launch.cwd.display().to_string(),
            prompt,
        ],
    }
}

/// ひな形の置き場を、シェルの引用符で囲んだ値に置き換える。
fn fill(template: &str, prompt_file: &Path, mode: Mode) -> String {
    template
        .replace(
            "{prompt_file}",
            &kuchiyose_prompt::shell_quote(&prompt_file.display().to_string()),
        )
        .replace("{mode}", &kuchiyose_prompt::shell_quote(mode.name()))
}

/// 自前の道具のひな形。環境変数が設定ファイルより先である。
fn template(env: &Env, cfg: &Config) -> Option<String> {
    env.var("KUCHIYOSE_AGENT_CMD")
        .map(str::to_owned)
        .or_else(|| cfg.agent_cmd.clone())
}

/// どの道具を使うかを決める（[どの道具を使うか](../../../docs/design/400-agent.md#どの道具を使うか)）。
///
/// # Errors
///
/// 知らない名前、ひな形の無い `custom`、`{prompt_file}` の無いひな形は 64、道具が
/// 見つからなければ 69 で断る。
pub fn resolve(flag: Option<&str>, env: &Env, cfg: &Config) -> Result<Agent, Exit> {
    let named = flag
        .map(str::to_owned)
        .or_else(|| env.var("KUCHIYOSE_AGENT").map(str::to_owned))
        .or_else(|| cfg.agent.clone());
    let kind = match named.as_deref() {
        Some("claude") => Kind::Claude,
        Some("codex") => Kind::Codex,
        Some("custom") => Kind::Custom(template(env, cfg).ok_or_else(|| {
            eprintln!("断る: custom を選んだが、ひな形が無い");
            eprintln!("KUCHIYOSE_AGENT_CMD か設定ファイルの agent_cmd に書く");
            Exit::Usage
        })?),
        Some(other) => {
            eprintln!("断る: 知らない道具: {other}。claude、codex、custom のどれか");
            return Err(Exit::Usage);
        }
        None => match template(env, cfg) {
            Some(t) => Kind::Custom(t),
            None => return found_on_path(env),
        },
    };
    match kind {
        Kind::Custom(t) => {
            // 長いプロンプトが引数の上限に当たらないよう、ファイルで渡させる。
            if !t.contains("{prompt_file}") {
                eprintln!("断る: 自前の道具のひな形に {{prompt_file}} が無い: {t}");
                return Err(Exit::Usage);
            }
            Ok(Agent {
                kind: Kind::Custom(t),
                program: PathBuf::from("/bin/sh"),
            })
        }
        k => {
            let program = env.find_program(k.name()).ok_or_else(|| {
                eprintln!("{} が PATH に見つからない", k.name());
                guide();
                Exit::Environment
            })?;
            Ok(Agent { kind: k, program })
        }
    }
}

/// どれも決まっていなければ、`PATH` にある道具を使う。どれを使ったかを 1 行出す。
fn found_on_path(env: &Env) -> Result<Agent, Exit> {
    for kind in [Kind::Claude, Kind::Codex] {
        if let Some(program) = env.find_program(kind.name()) {
            eprintln!(
                "LLM の道具: {}（PATH で見つけた。--agent か KUCHIYOSE_AGENT で選べる）",
                program.display()
            );
            return Ok(Agent { kind, program });
        }
    }
    eprintln!("LLM の道具が見つからない。claude も codex も PATH に無い");
    guide();
    Err(Exit::Environment)
}

fn guide() {
    eprintln!("--agent claude|codex|custom で選ぶか、--print でプロンプトだけを出す");
}

/// 道具が失敗した。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// 起動できない。
    NotStarted(String),
    /// 0 以外で終わった。標準エラーの末尾を持つ。
    Exited {
        /// 終了コード。シグナルで終わったなら `None`。
        code: Option<i32>,
        /// 標準エラーの末尾。
        tail: String,
    },
    /// 端末が無く、対話の画面を起動できない。
    NoTerminal(String),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::NotStarted(why) => write!(f, "道具を起動できない: {why}"),
            Failure::Exited { code, tail } => {
                match code {
                    Some(c) => write!(f, "道具が {c} で終わった")?,
                    None => write!(f, "道具がシグナルで終わった")?,
                }
                if !tail.is_empty() {
                    write!(f, "。標準エラーの末尾:\n{tail}")?;
                }
                Ok(())
            }
            Failure::NoTerminal(why) => write!(f, "端末が開けない（{why}）"),
        }
    }
}

/// 標準エラーの末尾として見せる行数。
const TAIL_LINES: usize = 20;

fn tail_of(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(TAIL_LINES)..].join("\n")
}

fn command(agent: &Agent, launch: &Launch<'_>) -> Command {
    let v = argv(agent, launch);
    let mut c = Command::new(&v[0]);
    c.args(&v[1..]).current_dir(launch.cwd);
    c
}

/// 対話なしで最後まで走らせる。標準入力は `/dev/null`、出力は受けて、失敗したときに見せる。
///
/// # Errors
///
/// 起動できない、0 以外で終わったときに失敗を返す。
pub fn run_batch(agent: &Agent, launch: &Launch<'_>) -> Result<(), Failure> {
    let out = command(agent, launch)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| Failure::NotStarted(e.to_string()))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(Failure::Exited {
            code: out.status.code(),
            tail: tail_of(&out.stderr),
        })
    }
}

/// 対話の画面で始める。
///
/// 標準入力が端末でなければ（要約を標準入力から読んだあと）、端末につなぎ直す。
/// そのまま渡せば、道具は入力の終わりを受けて閉じる。 標準出力と標準エラーも、端末で
/// なければ端末につなぐ——エディタから流したとき、画面がエディタのバッファに入る。
///
/// # Errors
///
/// 端末が開けない、起動できない、0 以外で終わったときに失敗を返す。
pub fn run_interactive(agent: &Agent, launch: &Launch<'_>, env: &Env) -> Result<(), Failure> {
    let mut c = command(agent, launch);
    let stdin_terminal = env.stdin == Input::Terminal;
    if !stdin_terminal || !env.output_terminal {
        let tty = File::options()
            .read(true)
            .write(true)
            .open(&env.tty)
            .map_err(|e| Failure::NoTerminal(format!("{}: {e}", env.tty.display())))?;
        let clone = |f: &File| {
            f.try_clone()
                .map_err(|e| Failure::NoTerminal(e.to_string()))
        };
        if !stdin_terminal {
            c.stdin(clone(&tty)?);
        }
        if !env.output_terminal {
            c.stdout(clone(&tty)?);
            c.stderr(tty);
        }
    }
    let status = c.status().map_err(|e| Failure::NotStarted(e.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(Failure::Exited {
            code: status.code(),
            tail: String::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdir::TempDir;

    fn agent(kind: Kind) -> Agent {
        let program = PathBuf::from(match kind {
            Kind::Claude => "/bin/claude",
            Kind::Codex => "/bin/codex",
            Kind::Custom(_) => "/bin/sh",
        });
        Agent { kind, program }
    }

    fn launch<'a>(task: Task<'a>) -> Launch<'a> {
        Launch {
            task,
            prompt: "依頼",
            prompt_file: Path::new("/w/p.prompt.md"),
            cwd: Path::new("/w"),
        }
    }

    fn persona() -> Task<'static> {
        Task::Persona {
            material: Path::new("/m"),
            write: Path::new("/w/a.persona.md"),
            check: "'/bin/kuchiyose' katashiro persona --check '/w/a.persona.md' --material '/m'",
        }
    }

    fn polish() -> Task<'static> {
        polish_checking(&[])
    }

    fn polish_checking(checks: &[String]) -> Task<'_> {
        Task::Polish {
            read: Path::new("/w/round-0.md"),
            write: Path::new("/w/round-1.md"),
            checks,
        }
    }

    #[test]
    fn 対話の画面ではプロンプトを渡すだけで許すものを足さない() {
        for (kind, bin) in [(Kind::Claude, "/bin/claude"), (Kind::Codex, "/bin/codex")] {
            assert_eq!(argv(&agent(kind), &launch(Task::Draft)), vec![bin, "依頼"]);
        }
    }

    #[test]
    fn claude_のペルソナ作りは素材を読み下書きだけに書き確かめるコマンドだけをそのとおりに打てる() {
        assert_eq!(
            argv(&agent(Kind::Claude), &launch(persona())),
            vec![
                "/bin/claude",
                "-p",
                "依頼",
                "--permission-mode",
                "dontAsk",
                "--add-dir",
                "/m",
                "--tools",
                "Read,Glob,Grep,Write,Edit,Bash",
                "--allowedTools",
                "Read(//m/**)",
                "Edit(//w/a.persona.md)",
                "Bash('/bin/kuchiyose' katashiro persona --check '/w/a.persona.md' --material '/m')",
            ]
        );
    }

    #[test]
    fn claude_の周回は検めるコマンドが無ければ今の版を読みその周の版だけに書き何も実行しない() {
        let v = argv(&agent(Kind::Claude), &launch(polish()));
        assert_eq!(
            v,
            vec![
                "/bin/claude",
                "-p",
                "依頼",
                "--permission-mode",
                "dontAsk",
                "--tools",
                "Read,Write,Edit",
                "--allowedTools",
                "Read(//w/round-0.md)",
                "Edit(//w/round-1.md)",
            ]
        );
        assert!(!v.iter().any(|a| a.contains("Bash")), "{v:?}");
    }

    #[test]
    fn claude_の周回で検めるコマンドを渡せばそのとおりの文字列だけを打てる() {
        let checks = vec![
            "'/bin/kuchiyose' review '/w/round-1.md' --katashiro '/k/a.katashiro'".to_owned(),
            "'/bin/kuchiyose' review '/w/round-1.md' --katashiro '/k/a.katashiro' --values"
                .to_owned(),
        ];
        assert_eq!(
            argv(&agent(Kind::Claude), &launch(polish_checking(&checks))),
            vec![
                "/bin/claude",
                "-p",
                "依頼",
                "--permission-mode",
                "dontAsk",
                "--tools",
                "Read,Write,Edit,Bash",
                "--allowedTools",
                "Read(//w/round-0.md)",
                "Edit(//w/round-1.md)",
                "Bash('/bin/kuchiyose' review '/w/round-1.md' --katashiro '/k/a.katashiro')",
                "Bash('/bin/kuchiyose' review '/w/round-1.md' --katashiro '/k/a.katashiro' --values)",
            ]
        );
    }

    #[test]
    fn codex_は作業のディレクトリにだけ書けるサンドボックスで走らせる() {
        for task in [persona(), polish()] {
            assert_eq!(
                argv(&agent(Kind::Codex), &launch(task)),
                vec![
                    "/bin/codex",
                    "exec",
                    "--sandbox",
                    "workspace-write",
                    "--skip-git-repo-check",
                    "--cd",
                    "/w",
                    "依頼",
                ]
            );
        }
    }

    #[test]
    fn 自前の道具はひな形の置き場を引用符で囲んで_sh_で走らせる() {
        let a = agent(Kind::Custom("my-llm --file {prompt_file} --{mode}".into()));
        let mut l = launch(polish());
        l.prompt_file = Path::new("/w/a b's.md");
        assert_eq!(
            argv(&a, &l),
            vec!["/bin/sh", "-c", r"my-llm --file '/w/a b'\''s.md' --'batch'"]
        );
        assert_eq!(
            argv(&a, &launch(Task::Draft))[2],
            "my-llm --file '/w/p.prompt.md' --'interactive'"
        );
    }

    #[test]
    fn 長すぎるプロンプトはファイルで渡す() {
        let long = "あ".repeat(ARG_LIMIT);
        let mut l = launch(Task::Draft);
        l.prompt = &long;
        let v = argv(&agent(Kind::Claude), &l);
        assert_eq!(v[1], "次のファイルを読んで、その指示に従う: /w/p.prompt.md");
    }

    fn with_programs(dir: &TempDir, names: &[&str]) -> String {
        for n in names {
            let p = dir.write(&format!("bin/{n}"), "#!/bin/sh\n");
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        dir.join("bin")
    }

    #[test]
    fn 引数_環境変数_設定ファイル_ひな形_path_の順に決める() {
        let dir = TempDir::new("agent-resolve");
        let path = with_programs(&dir, &["claude", "codex"]);
        let cfg = Config {
            agent: Some("claude".into()),
            ..Config::default()
        };
        let env = Env::for_test("/w", &[("PATH", &path), ("KUCHIYOSE_AGENT", "codex")]);
        assert_eq!(
            resolve(Some("claude"), &env, &cfg).unwrap().kind,
            Kind::Claude
        );
        assert_eq!(resolve(None, &env, &cfg).unwrap().kind, Kind::Codex);
        let env = Env::for_test("/w", &[("PATH", &path)]);
        assert_eq!(resolve(None, &env, &cfg).unwrap().kind, Kind::Claude);
        let env = Env::for_test(
            "/w",
            &[("PATH", &path), ("KUCHIYOSE_AGENT_CMD", "x {prompt_file}")],
        );
        assert_eq!(
            resolve(None, &env, &Config::default()).unwrap().kind,
            Kind::Custom("x {prompt_file}".into())
        );
        let env = Env::for_test("/w", &[("PATH", &path)]);
        let got = resolve(None, &env, &Config::default()).unwrap();
        assert_eq!(got.kind, Kind::Claude, "PATH では claude が先");
        assert_eq!(got.program, PathBuf::from(dir.join("bin/claude")));
    }

    #[test]
    fn path_に_claude_が無ければ_codex_を使う() {
        let dir = TempDir::new("agent-codex");
        let path = with_programs(&dir, &["codex"]);
        let env = Env::for_test("/w", &[("PATH", &path)]);
        assert_eq!(
            resolve(None, &env, &Config::default()).unwrap().kind,
            Kind::Codex
        );
    }

    #[test]
    fn どれも無ければ環境の側の理由で断る() {
        let env = Env::for_test("/w", &[("PATH", "/無い")]);
        assert_eq!(
            resolve(None, &env, &Config::default()),
            Err(Exit::Environment)
        );
        assert_eq!(
            resolve(Some("claude"), &env, &Config::default()),
            Err(Exit::Environment),
            "選んだ道具が見つからない"
        );
    }

    #[test]
    fn ひな形の無い_custom_と_prompt_file_の無いひな形と知らない名前は使い方の誤りである() {
        let env = Env::for_test("/w", &[]);
        assert_eq!(
            resolve(Some("custom"), &env, &Config::default()),
            Err(Exit::Usage)
        );
        let env = Env::for_test("/w", &[("KUCHIYOSE_AGENT_CMD", "my-llm")]);
        assert_eq!(resolve(None, &env, &Config::default()), Err(Exit::Usage));
        assert_eq!(
            resolve(Some("gemini"), &env, &Config::default()),
            Err(Exit::Usage)
        );
    }

    #[test]
    fn 環境変数のひな形が設定ファイルのひな形より先である() {
        let cfg = Config {
            agent_cmd: Some("cfg {prompt_file}".into()),
            ..Config::default()
        };
        let env = Env::for_test("/w", &[("KUCHIYOSE_AGENT_CMD", "env {prompt_file}")]);
        assert_eq!(
            resolve(Some("custom"), &env, &cfg).unwrap().kind,
            Kind::Custom("env {prompt_file}".into())
        );
    }

    #[test]
    fn 対話なしの起動は失敗を標準エラーの末尾と一緒に返す() {
        let dir = TempDir::new("agent-batch");
        let a = agent(Kind::Custom(
            "echo 出力; echo 失敗の理由 >&2; exit 3 # {prompt_file}".into(),
        ));
        let cwd = PathBuf::from(dir.path());
        let mut l = launch(polish());
        l.cwd = &cwd;
        let got = run_batch(&a, &l).unwrap_err();
        assert_eq!(
            got,
            Failure::Exited {
                code: Some(3),
                tail: "失敗の理由".into()
            }
        );
        let ok = agent(Kind::Custom("test -z \"$(cat)\" # {prompt_file}".into()));
        assert_eq!(run_batch(&ok, &l), Ok(()), "標準入力は空である");
    }

    #[test]
    fn 端末が開けなければ対話の画面を起動しない() {
        let dir = TempDir::new("agent-no-tty");
        let mut env = Env::for_test(dir.path(), &[]);
        env.stdin = Input::Text("要約".into());
        env.tty = PathBuf::from(dir.join("無い端末"));
        let marker = dir.join("起動した");
        let a = agent(Kind::Custom(format!("touch '{marker}' # {{prompt_file}}")));
        let cwd = PathBuf::from(dir.path());
        let mut l = launch(Task::Draft);
        l.cwd = &cwd;
        assert!(matches!(
            run_interactive(&a, &l, &env),
            Err(Failure::NoTerminal(_))
        ));
        assert!(!Path::new(&marker).exists(), "起動していない");
    }

    #[test]
    fn 標準入力が端末でなければ端末につなぎ直す() {
        // 端末の代わりにファイルを渡し、道具の標準入力がそこにつながったことを見る。
        let dir = TempDir::new("agent-reconnect");
        let tty = dir.write("端末", "端末からの入力\n");
        let mut env = Env::for_test(dir.path(), &[]);
        env.stdin = Input::Text("要約".into());
        env.tty = PathBuf::from(&tty);
        let seen = dir.join("見た");
        let a = agent(Kind::Custom(format!("cat > '{seen}' # {{prompt_file}}")));
        let cwd = PathBuf::from(dir.path());
        let mut l = launch(Task::Draft);
        l.cwd = &cwd;
        run_interactive(&a, &l, &env).expect("起動できる");
        assert_eq!(std::fs::read_to_string(&seen).unwrap(), "端末からの入力\n");
    }
}
