//! `kuchiyose build`。形代を作る（[build](../../../docs/design/200-command.md#build)）。
//!
//! `katashiro build` と同じ手順で統計値を測り、続けて LLM の道具を対話なしで起動して
//! ペルソナを下書きさせる。作った形代を既定の形代として覚える。

use std::path::{Path, PathBuf};

use kuchiyose_katashiro::store;
use kuchiyose_prompt::shell_quote;

use crate::agent::{self, Launch, Task};
use crate::env::Env;
use crate::exit::Exit;
use crate::{config, katashiro_cmd, persona_cmd};

/// `build` の help。
pub const HELP: &str = "\
kuchiyose build <フォルダ> [-o <形代>] [--scene <場面>] [--no-persona]
                [--agent <道具>] [--print]
    形代を作る。katashiro build と同じ手順で統計値を測り、続けて LLM の道具を対話なしで
    起動してペルソナを下書きさせる。作った形代を既定の形代として設定ファイルに覚える。
    記事が増えたら同じコマンドを打つ。作り直しでは調整もペルソナも引き継ぐ。
    形代が既にペルソナを持っていれば、道具は起動しない。
    -o            形代の経路。省けば $XDG_DATA_HOME/kuchiyose/<フォルダの名前>.katashiro
                  （無ければ ~/.local/share）。そこに別のフォルダから作った形代があれば 64。
    --scene       場面の名前。katashiro build と同じ。
    --no-persona  ペルソナを作らない。統計値だけの形代にする。
    --print       統計値は作る。道具は起動せず、ペルソナを作るプロンプトを出す。
                  下書きの経路は <形代の名前>.persona.md で、取り込みは自分で打つ。
    道具は kuchiyose が作った専用のディレクトリで走らせ、下書きをそこに書かせる。
    道具が打てるのは katashiro persona --check だけで、形代には書けない。道具が
    終わったら、下書きを形代の隣の <形代の名前>.persona.md に置いて取り込む。
    本人が読んで直し、katashiro persona で取り込み直す。
    終了コード: 0 形代を書いた / 64 以上 katashiro build と同じ理由で断った /
    69 形代は書いたが、道具が見つからないか失敗して、ペルソナが無い。";

/// 引数。
#[derive(Debug, Default)]
struct Args {
    dir: Option<String>,
    output: Option<String>,
    scene: Option<String>,
    agent: Option<String>,
    no_persona: bool,
    print: bool,
}

fn parse_args(args: &[String]) -> Result<Args, Exit> {
    let mut a = Args::default();
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].as_str();
        match flag {
            "--print" => a.print = true,
            "--no-persona" => a.no_persona = true,
            "-o" | "--output" | "--scene" | "--agent" => {
                let Some(v) = args.get(i + 1).cloned() else {
                    eprintln!("{flag} に値を渡す");
                    return Err(Exit::Usage);
                };
                match flag {
                    "--scene" => a.scene = Some(v),
                    "--agent" => a.agent = Some(v),
                    _ => a.output = Some(v),
                }
                i += 1;
            }
            other if other.starts_with('-') => {
                eprintln!("知らない引数: {other}");
                return Err(Exit::Usage);
            }
            other if a.dir.is_none() => a.dir = Some(other.to_owned()),
            other => {
                eprintln!("フォルダは 1 つだけ渡す: {other}");
                return Err(Exit::Usage);
            }
        }
        i += 1;
    }
    Ok(a)
}

/// `-o` を省いたときの経路。フォルダの名前だけで決める。
fn default_output(env: &Env, material: &Path) -> PathBuf {
    let name = material.file_name().map_or_else(
        || "katashiro".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    env.data_dir().join(format!("{name}.katashiro"))
}

/// 既定の経路に、別のフォルダから作った形代があるか。
///
/// フォルダの名前だけで経路を決めるので、別の場所にある同じ名前のフォルダが同じ経路に
/// 当たる。 作り直しとして上書きすれば、別の人の統計値に調整とペルソナが引き継がれる。
/// どのフォルダから作ったかが分からない形代（版 5）は、違うとは言えないので断らない。
fn made_elsewhere(path: &Path, material: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let other = store::read(&bytes).ok()?.material?;
    (Path::new(&other) != material).then_some(other)
}

/// 道具が書いた下書きを置く経路。形代の隣の `<形代の名前>.persona.md`。
#[must_use]
pub fn persona_path(katashiro: &Path) -> PathBuf {
    let stem = katashiro
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    katashiro.with_file_name(format!("{stem}.persona.md"))
}

/// `kuchiyose build`。
#[allow(clippy::too_many_lines)]
pub fn run(args: &[String], env: &Env) -> Exit {
    let a = match parse_args(args) {
        Ok(a) => a,
        Err(e) => return e,
    };
    let Some(dir) = &a.dir else {
        eprintln!("素材のフォルダを渡す");
        eprintln!("{HELP}");
        return Exit::Usage;
    };
    let Ok(material) = std::fs::canonicalize(env.absolute(dir)) else {
        eprintln!("フォルダが読めない: {dir}");
        return Exit::Unreadable;
    };
    if !material.is_dir() {
        eprintln!("フォルダが読めない: {dir}");
        return Exit::Unreadable;
    }
    let cfg = match config::load(env) {
        Ok(c) => c,
        Err(e) => return e,
    };
    let output = match &a.output {
        Some(o) => env.absolute(o),
        None => {
            let p = default_output(env, &material);
            if let Some(other) = made_elsewhere(&p, &material) {
                eprintln!(
                    "断る: {} には別のフォルダから作った形代がある（{other}）",
                    p.display()
                );
                eprintln!("既定の経路はフォルダの名前だけで決まる。-o で別の経路を渡す");
                return Exit::Usage;
            }
            if let Err(e) = std::fs::create_dir_all(env.data_dir()) {
                eprintln!(
                    "断る: 形代の置き場が作れない: {}: {e}",
                    env.data_dir().display()
                );
                return Exit::Unreadable;
            }
            p
        }
    };
    let out_str = output.display().to_string();
    let c = match katashiro_cmd::build_into(
        &material.display().to_string(),
        &out_str,
        a.scene.clone(),
        false,
    ) {
        Ok(c) => c,
        Err(e) => return e,
    };
    if let Err(e) = config::remember_katashiro(env, &out_str) {
        return e;
    }
    println!("既定の形代として覚えた: {}", config::path(env).display());
    if env.var("KUCHIYOSE_KATASHIRO").is_some() {
        eprintln!("KUCHIYOSE_KATASHIRO が設定されているので、そちらが優先される");
    }

    if a.no_persona {
        return Exit::Pass;
    }
    if c.persona.is_some() {
        println!("ペルソナを持っているので作らない。作り直すなら katashiro persona --remove で外してから");
        return Exit::Pass;
    }
    let persona = persona_path(&output);
    if a.print {
        // 道具を起動しないので、下書きは本人が直すファイルの経路に書かせる。取り込みは使う人が打つ。
        print!("{}", persona_prompt_for(env, &material, &persona));
        return Exit::Pass;
    }
    // 本人が直したかもしれないファイルを、下書きで上書きさせない。
    if persona.exists() {
        eprintln!(
            "断る: ペルソナの経路に既にファイルがある: {}",
            persona.display()
        );
        eprintln!("取り込むなら katashiro persona、下書きさせ直すなら消すか移してから build する");
        return Exit::Usage;
    }
    let agent = match agent::resolve(a.agent.as_deref(), env, &cfg) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("形代は書いたが、ペルソナが無い");
            return e;
        }
    };
    let work = work_dir(env, &output);
    let prepared =
        std::fs::create_dir_all(&work).and_then(|()| crate::private::create_dir(&work, "persona-"));
    let sandbox = match prepared {
        Ok(d) => d,
        Err(e) => {
            eprintln!(
                "断る: 道具を走らせるディレクトリが作れない: {}: {e}",
                work.display()
            );
            return Exit::Unreadable;
        }
    };
    let draft = sandbox.join(persona.file_name().unwrap_or_default());
    let check = check_command(env, &material, &draft);
    let prompt = kuchiyose_prompt::persona_prompt(
        &material.display().to_string(),
        &draft.display().to_string(),
        &check,
    );
    // 作業用のフォルダは前からあるものを受けるので、決まった名前で書けば置かれた symlink を追う。
    let prompt_file =
        match crate::private::create_file(&work, "persona-", ".prompt.md", prompt.as_bytes()) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("断る: プロンプトを書けない: {}: {e}", work.display());
                std::fs::remove_dir_all(&sandbox).ok();
                return Exit::Unreadable;
            }
        };
    println!("ペルソナを下書きさせている: {}", persona.display());
    let launched = agent::run_batch(
        &agent,
        &Launch {
            task: Task::Persona {
                material: &material,
                write: &draft,
                check: &check,
            },
            prompt: &prompt,
            prompt_file: &prompt_file,
            cwd: &sandbox,
        },
    );
    if let Err(f) = launched {
        std::fs::remove_dir_all(&sandbox).ok();
        eprintln!("{f}");
        eprintln!("形代は書いたが、ペルソナが無い。同じ build を打てば作り直させる");
        return Exit::Environment;
    }
    if let Err(why) = place_draft(&draft, &persona) {
        // 下書きを失わないよう、道具を走らせたディレクトリは残す。
        eprintln!("{why}");
        eprintln!("道具を走らせたディレクトリは残した: {}", sandbox.display());
        eprintln!("形代は書いたが、ペルソナが無い");
        return Exit::Environment;
    }
    std::fs::remove_dir_all(&sandbox).ok();
    match persona_cmd::import(&out_str, &persona.display().to_string(), None) {
        Ok(r) => {
            r.report(&out_str, false);
            println!(
                "本人が読んで直し、katashiro persona で取り込み直す: {}",
                persona.display()
            );
            Exit::Pass
        }
        Err(_) => {
            eprintln!("形代は書いたが、道具の書いたペルソナを取り込めない。直して katashiro persona で取り込む");
            Exit::Environment
        }
    }
}

/// 作業用のフォルダ。形代の隣の `<形代の名前>.kuchiyose/`。
fn work_dir(env: &Env, katashiro: &Path) -> PathBuf {
    let parent = katashiro
        .parent()
        .map_or_else(|| env.cwd.clone(), Path::to_path_buf);
    parent.join(format!(
        "{}.kuchiyose",
        katashiro
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned())
    ))
}

/// 下書きを確かめるコマンド。道具がそのまま打てる形で、許す規則にも同じ文字列を使う。
fn check_command(env: &Env, material: &Path, draft: &Path) -> String {
    format!(
        "{} katashiro persona --check {} --material {}",
        shell_quote(&env.exe.display().to_string()),
        shell_quote(&draft.display().to_string()),
        shell_quote(&material.display().to_string())
    )
}

/// `--print` で出すプロンプト。下書きの経路は本人が直すファイルである。
fn persona_prompt_for(env: &Env, material: &Path, persona: &Path) -> String {
    kuchiyose_prompt::persona_prompt(
        &material.display().to_string(),
        &persona.display().to_string(),
        &check_command(env, material, persona),
    )
}

/// 道具が書いた下書きを、本人が直すファイルの経路に置く。
///
/// ふつうのファイルだけを受ける。 symlink を追えば、道具が指した先のファイルの中身を
/// 形代に取り込むことになる。 置き場に何かあれば上書きしない。
fn place_draft(draft: &Path, persona: &Path) -> Result<(), String> {
    let is_file = std::fs::symlink_metadata(draft).is_ok_and(|m| m.file_type().is_file());
    if !is_file {
        return Err(format!(
            "道具がペルソナの下書きをふつうのファイルとして書かなかった: {}",
            draft.display()
        ));
    }
    let body = read_bounded(draft)
        .map_err(|e| format!("下書きが読めない: {}: {e}", draft.display()))?
        .ok_or_else(|| {
            format!(
                "下書きが大きすぎる（{DRAFT_LIMIT} バイトまで）: {}",
                draft.display()
            )
        })?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(persona)
        .map_err(|e| format!("下書きを置けない: {}: {e}", persona.display()))?;
    if let Err(e) = std::io::Write::write_all(&mut file, &body) {
        // 書きかけを残すと、次の build が「もうある」と断って置き直せない。
        drop(file);
        std::fs::remove_file(persona).ok();
        return Err(format!("下書きを置けない: {}: {e}", persona.display()));
    }
    Ok(())
}

/// ペルソナの下書きとして受ける大きさの上限。見出しと引用の箇条には十分に大きい。
const DRAFT_LIMIT: u64 = 1024 * 1024;

/// 上限までしか読まない。上限を超えていれば `None`。
fn read_bounded(path: &Path) -> std::io::Result<Option<Vec<u8>>> {
    use std::io::Read as _;
    let mut body = Vec::new();
    std::fs::File::open(path)?
        .take(DRAFT_LIMIT + 1)
        .read_to_end(&mut body)?;
    Ok((body.len() as u64 <= DRAFT_LIMIT).then_some(body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persona_cmd::tests::{persona_text, quote_from_material};
    use crate::testdir::TempDir;
    use crate::{fixture, katashiros};

    /// 作業用のフォルダに置いたペルソナ作りのプロンプト。名前は毎回違う。
    fn prompt_file(work: &Path) -> PathBuf {
        let mut found: Vec<PathBuf> = std::fs::read_dir(work)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.to_string_lossy().ends_with(".prompt.md"))
            .collect();
        assert_eq!(found.len(), 1, "{found:?}");
        found.remove(0)
    }

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    fn env(dir: &TempDir, extra: &[(&str, &str)]) -> Env {
        let config = dir.join("設定");
        let data = dir.join("データ");
        let mut vars = vec![
            ("XDG_CONFIG_HOME", config.as_str()),
            ("XDG_DATA_HOME", data.as_str()),
            ("PATH", "/usr/bin:/bin"),
        ];
        vars.extend_from_slice(extra);
        Env::for_test(dir.path(), &vars)
    }

    #[test]
    fn ペルソナの経路は形代の隣である() {
        assert_eq!(
            persona_path(Path::new("/d/本人.katashiro")),
            PathBuf::from("/d/本人.persona.md")
        );
    }

    #[test]
    fn ペルソナを作らずに作った形代を既定として覚える() {
        let dir = TempDir::new("build-remember");
        let person = fixture::write_corpus(&dir, "本人", false);
        let e = env(&dir, &[]);
        assert_eq!(
            crate::run_in(&args(&["build", &person, "--no-persona"]), &e),
            Exit::Pass
        );
        let k = e.data_dir().join("本人.katashiro");
        assert!(k.is_file(), "既定の経路に書く");
        assert_eq!(
            config::load(&e).unwrap().katashiro.as_deref(),
            k.to_str(),
            "既定の形代として覚える"
        );
        assert_eq!(
            katashiros::resolve(None, &e).unwrap(),
            k.display().to_string()
        );
    }

    #[test]
    fn 既定の経路に別のフォルダの形代があれば書かない() {
        let dir = TempDir::new("build-elsewhere");
        let a = fixture::write_corpus(&dir, "甲/本人", false);
        let b = fixture::write_corpus(&dir, "乙/本人", false);
        let e = env(&dir, &[]);
        assert_eq!(
            crate::run_in(&args(&["build", &a, "--no-persona"]), &e),
            Exit::Pass
        );
        let k = e.data_dir().join("本人.katashiro");
        let before = std::fs::read(&k).unwrap();
        assert_eq!(
            crate::run_in(&args(&["build", &b, "--no-persona"]), &e),
            Exit::Usage
        );
        assert_eq!(std::fs::read(&k).unwrap(), before, "元のまま");
        assert_eq!(
            crate::run_in(&args(&["build", &a, "--no-persona"]), &e),
            Exit::Pass,
            "同じフォルダなら作り直す"
        );
    }

    #[test]
    fn 表示だけならペルソナを作るプロンプトを出し統計値は作る() {
        let dir = TempDir::new("build-print");
        let person = fixture::write_corpus(&dir, "本人", false);
        let e = env(&dir, &[]);
        let k = dir.join("本人.katashiro");
        assert_eq!(
            crate::run_in(&args(&["build", &person, "-o", &k, "--print"]), &e),
            Exit::Pass
        );
        assert!(Path::new(&k).is_file());
    }

    #[test]
    fn 道具が見つからなければ形代を残して_69_で終わる() {
        let dir = TempDir::new("build-no-agent");
        let person = fixture::write_corpus(&dir, "本人", false);
        let e = env(&dir, &[("PATH", "/無い")]);
        let k = dir.join("本人.katashiro");
        assert_eq!(
            crate::run_in(&args(&["build", &person, "-o", &k]), &e),
            Exit::Environment
        );
        assert!(katashiros::read_file(&k).is_ok(), "形代は残す");
    }

    /// 偽の道具。下書きの経路に `write` のしかたで書いて終わる。走った場所を `偽/pwd` に、
    /// その場所の権限を `偽/ls` に残す。
    fn fake_with(dir: &TempDir, write: &str) -> String {
        let script = dir.write(
            "偽/道具.sh",
            format!(
                "#!/bin/sh\n\
                 [ \"$1\" = batch ] || exit 9\n\
                 pwd -P > '{d}/偽/pwd'\n\
                 ls -ld . > '{d}/偽/ls'\n\
                 line=$(grep '^- 書く経路: ' \"$2\")\n\
                 out=${{line#- 書く経路: }}\n\
                 {write}\n",
                d = dir.path()
            ),
        );
        format!("sh '{script}' {{mode}} {{prompt_file}}")
    }

    /// 偽の道具。ペルソナを書き、取り込みはせずに終わる。
    fn fake(dir: &TempDir, body: &str) -> String {
        let src = dir.write("偽/ペルソナ.md", body);
        fake_with(dir, &format!("cp '{src}' \"$out\""))
    }

    #[test]
    fn 道具は形代の隣ではなく自分だけが入れる専用のディレクトリで走り下書きはそこに書く() {
        let dir = TempDir::new("build-sandbox");
        let person = fixture::write_corpus(&dir, "本人", false);
        let agent = fake(&dir, &persona_text(&quote_from_material()));
        let e = env(&dir, &[("KUCHIYOSE_AGENT_CMD", &agent)]);
        let k = dir.join("本人.katashiro");
        assert_eq!(
            crate::run_in(&args(&["build", &person, "-o", &k]), &e),
            Exit::Pass
        );
        let work = std::fs::canonicalize(dir.join("本人.kuchiyose")).unwrap();
        let ran = PathBuf::from(std::fs::read_to_string(dir.join("偽/pwd")).unwrap().trim());
        assert_eq!(ran.parent(), Some(work.as_path()), "{}", ran.display());
        assert!(
            std::fs::read_to_string(dir.join("偽/ls"))
                .unwrap()
                .starts_with("drwx------"),
            "自分だけが入れる"
        );
        let prompt = std::fs::read_to_string(prompt_file(&work)).unwrap();
        let written = format!("- 書く経路: {}/persona-", dir.join("本人.kuchiyose"));
        assert!(prompt.contains(&written), "{prompt}");
        assert!(prompt.contains("katashiro persona --check"), "{prompt}");
        assert!(!ran.exists(), "終わったら片づける");
        assert_eq!(
            std::fs::read_to_string(dir.join("本人.persona.md")).unwrap(),
            persona_text(&quote_from_material()),
            "下書きは形代の隣に置く"
        );
        assert!(katashiros::read_file(&k).unwrap().persona.is_some());
    }

    #[test]
    fn 下書きが_symlink_なら指した先を取り込まない() {
        let dir = TempDir::new("build-symlink");
        let person = fixture::write_corpus(&dir, "本人", false);
        let secret = dir.write("秘密.md", persona_text(&quote_from_material()));
        let agent = fake_with(&dir, &format!("ln -s '{secret}' \"$out\""));
        let e = env(&dir, &[("KUCHIYOSE_AGENT_CMD", &agent)]);
        let k = dir.join("本人.katashiro");
        assert_eq!(
            crate::run_in(&args(&["build", &person, "-o", &k]), &e),
            Exit::Environment
        );
        assert_eq!(katashiros::read_file(&k).unwrap().persona, None);
        assert!(!Path::new(&dir.join("本人.persona.md")).exists());
    }

    #[test]
    fn 上限を超える下書きは読まずに断る() {
        let dir = TempDir::new("build-huge");
        let person = fixture::write_corpus(&dir, "本人", false);
        let agent = fake_with(
            &dir,
            &format!("head -c {} /dev/zero > \"$out\"", DRAFT_LIMIT + 1),
        );
        let e = env(&dir, &[("KUCHIYOSE_AGENT_CMD", &agent)]);
        let k = dir.join("本人.katashiro");
        assert_eq!(
            crate::run_in(&args(&["build", &person, "-o", &k]), &e),
            Exit::Environment
        );
        assert_eq!(katashiros::read_file(&k).unwrap().persona, None);
        assert!(!Path::new(&dir.join("本人.persona.md")).exists());
    }

    #[test]
    fn 道具が書いたペルソナを取り込み次の_build_では道具を起動しない() {
        let dir = TempDir::new("build-persona");
        let person = fixture::write_corpus(&dir, "本人", false);
        let agent = fake(&dir, &persona_text(&quote_from_material()));
        let e = env(&dir, &[("KUCHIYOSE_AGENT_CMD", &agent)]);
        let k = dir.join("本人.katashiro");
        assert_eq!(
            crate::run_in(&args(&["build", &person, "-o", &k]), &e),
            Exit::Pass
        );
        let c = katashiros::read_file(&k).unwrap();
        assert_eq!(
            c.persona.as_deref(),
            Some(persona_text(&quote_from_material()).as_str())
        );
        assert!(prompt_file(Path::new(&dir.join("本人.kuchiyose"))).is_file());

        // 本人が直したものを上書きしない。
        std::fs::remove_file(dir.join("本人.persona.md")).unwrap();
        let broken = fake(&dir, "壊れた下書き");
        let e = env(&dir, &[("KUCHIYOSE_AGENT_CMD", &broken)]);
        assert_eq!(
            crate::run_in(&args(&["build", &person, "-o", &k]), &e),
            Exit::Pass
        );
        assert!(
            !Path::new(&dir.join("本人.persona.md")).exists(),
            "道具を起動していない"
        );
    }

    #[test]
    fn 道具が形の合わないペルソナを書けば_69_で終わる() {
        let dir = TempDir::new("build-persona-broken");
        let person = fixture::write_corpus(&dir, "本人", false);
        let agent = fake(&dir, "## 見出しが違う\n");
        let e = env(&dir, &[("KUCHIYOSE_AGENT_CMD", &agent)]);
        let k = dir.join("本人.katashiro");
        assert_eq!(
            crate::run_in(&args(&["build", &person, "-o", &k]), &e),
            Exit::Environment
        );
        assert_eq!(katashiros::read_file(&k).unwrap().persona, None);
    }

    #[test]
    fn ペルソナの経路に既にファイルがあれば道具を起動しない() {
        let dir = TempDir::new("build-persona-exists");
        let person = fixture::write_corpus(&dir, "本人", false);
        let agent = fake(&dir, &persona_text(&quote_from_material()));
        let e = env(&dir, &[("KUCHIYOSE_AGENT_CMD", &agent)]);
        let k = dir.join("本人.katashiro");
        let mine = dir.write("本人.persona.md", "本人が直したもの");
        assert_eq!(
            crate::run_in(&args(&["build", &person, "-o", &k]), &e),
            Exit::Usage
        );
        assert_eq!(std::fs::read_to_string(mine).unwrap(), "本人が直したもの");
    }
}
