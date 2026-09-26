//! `kuchiyose katashiro persona`。ペルソナを入れる・入れ直す・外す・見る・確かめる
//! （[katashiro persona](../../../docs/design/200-command.md#katashiro-persona)）。
//!
//! ペルソナは形代の中の原本なので、調整と同じく `katashiro` の下に置く。 測った値に
//! 触らないので、作り直しは要らない。

use std::collections::BTreeMap;
use std::path::Path;

use kuchiyose_katashiro::json::Value;
use kuchiyose_katashiro::{save, Katashiro};
use kuchiyose_prompt::{Citations, Persona};

use crate::exit::Exit;
use crate::{folder, katashiros};

/// `katashiro persona` の help。
pub const HELP: &str = "\
kuchiyose katashiro persona <形代>
kuchiyose katashiro persona <形代> <ファイル> [--material <フォルダ>] [--json]
kuchiyose katashiro persona <形代> --remove
kuchiyose katashiro persona --check <ファイル> --material <フォルダ> [--json]
    ペルソナを入れる・入れ直す・外す・見る・確かめる。測った値に触らないので作り直しは
    要らない。
    ファイルを渡すと形を確かめて取り込む。見出しは ## で 大事にすること、読み手、
    話の運び方、書かないこと、よく扱う領域 をこの順に置く。欠けた見出しや余計な
    見出しがあれば取り込まない（64）。UTF-8 で読めなければ 65。
    引用が素材のフォルダに実在するかを確かめ、解決しない引用と、解決する引用を
    持たない項目を知らせる。知らせがあっても取り込み、0 で終わる。
    --material  引用を照らす素材のフォルダ。省けば形代が覚えている素材のフォルダ。
                そこにフォルダが無ければ、確かめずに取り込み、確かめていないと言う。
    --remove    ペルソナを外す。形代を人に渡す前に使う。外すと戻らない。
    --check     取り込まずに、形と引用だけを確かめる。形代を取らず、何も書かない。
                --material が要る。知らせが無ければ 0、あれば 1。
    ファイルも --remove も渡さなければ、今のペルソナを出す。持っていなければ 2。";

/// 振り分ける。
pub fn run(args: &[String]) -> Exit {
    let mut positional: Vec<&str> = Vec::new();
    let mut material: Option<&str> = None;
    let mut check: Option<&str> = None;
    let mut json = false;
    let mut remove = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json = true,
            "--remove" => remove = true,
            flag @ ("--material" | "--check") => {
                let Some(v) = args.get(i + 1) else {
                    eprintln!("{flag} に値を渡す");
                    return Exit::Usage;
                };
                if flag == "--check" {
                    check = Some(v);
                } else {
                    material = Some(v);
                }
                i += 1;
            }
            other if other.starts_with('-') => {
                eprintln!("知らない引数: {other}");
                return Exit::Usage;
            }
            other => positional.push(other),
        }
        i += 1;
    }
    if let Some(file) = check {
        return match (positional.as_slice(), remove, material) {
            ([], false, Some(m)) => check_only(file, m, json),
            _ => {
                eprintln!("--check は形代を取らない。ファイルと --material だけを渡す");
                eprintln!("{HELP}");
                Exit::Usage
            }
        };
    }
    match (positional.as_slice(), remove) {
        ([k], false) => show(k),
        ([k], true) => remove_from(k),
        ([k, file], false) => match import(k, file, material) {
            Ok(r) => {
                r.report(k, json);
                Exit::Pass
            }
            Err(e) => e,
        },
        _ => {
            eprintln!("{HELP}");
            Exit::Usage
        }
    }
}

/// 今のペルソナを出す。
fn show(path: &str) -> Exit {
    let c = match katashiros::read_file(path) {
        Ok(c) => c,
        Err(e) => return e,
    };
    match &c.persona {
        Some(p) => {
            print!("{p}");
            Exit::Pass
        }
        None => {
            eprintln!("ペルソナを持っていない: {path}");
            Exit::Unknown
        }
    }
}

/// 外す。
fn remove_from(path: &str) -> Exit {
    let mut c = match katashiros::read_file(path) {
        Ok(c) => c,
        Err(e) => return e,
    };
    if c.persona.is_none() {
        println!("ペルソナを持っていない: {path}");
        return Exit::Pass;
    }
    c.persona = None;
    if let Err(e) = write(path, &c) {
        return e;
    }
    println!("ペルソナを外した: {path}");
    println!("戻すなら、手元のファイルを katashiro persona で取り込み直す");
    Exit::Pass
}

fn write(path: &str, c: &Katashiro) -> Result<(), Exit> {
    save::save(path, c, Some(c.generation)).map_err(|e| {
        eprintln!("断る: 書けない: {e}");
        Exit::Unreadable
    })
}

/// 引用を照らした素材のフォルダ。照らせなかったなら、その理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verified {
    /// 照らした。
    With {
        /// 素材のフォルダ。
        material: String,
        /// 結果。
        citations: Citations,
    },
    /// 照らしていない。
    Not(String),
}

/// 取り込んだ結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imported {
    /// 引用を照らしたか。
    pub verified: Verified,
}

impl Imported {
    /// 知らせを標準出力に出す。取り込みの結果そのものなので stdout である。
    pub fn report(&self, katashiro: &str, json: bool) {
        if json {
            let mut fields = vec![("katashiro".to_owned(), Value::s(katashiro))];
            fields.extend(verified_fields(&self.verified));
            println!("{}", Value::obj(fields).write());
            return;
        }
        println!("ペルソナを取り込んだ: {katashiro}");
        print_verified(&self.verified);
    }
}

/// 知らせが無いか。引用を照らし、どれも解決したときだけである。
fn clean(v: &Verified) -> bool {
    matches!(v, Verified::With { citations, .. }
        if citations.unresolved.is_empty() && citations.unsupported.is_empty())
}

/// 引用を照らした結果を出す。LLM の道具が読んで引用を直すためのものでもある。
fn print_verified(v: &Verified) {
    match v {
        Verified::Not(why) => println!("引用を確かめていない: {why}"),
        Verified::With {
            material,
            citations,
        } => {
            println!("引用を照らした素材のフォルダ: {material}");
            if clean(v) {
                println!("引用はすべて解決した");
            }
            if !citations.unresolved.is_empty() {
                println!("解決しない引用 {} 本", citations.unresolved.len());
                for u in &citations.unresolved {
                    println!(
                        "  - {} / {}: 「{}」（{}）: {}",
                        u.heading,
                        u.item,
                        u.quote.text,
                        u.quote.unit,
                        u.reason.name()
                    );
                }
            }
            if !citations.unsupported.is_empty() {
                println!(
                    "解決する引用を持たない項目 {} 本",
                    citations.unsupported.len()
                );
                for (heading, item) in &citations.unsupported {
                    println!("  - {heading} / {item}");
                }
            }
        }
    }
}

/// 引用を照らした結果の JSON の欄。
fn verified_fields(v: &Verified) -> Vec<(String, Value)> {
    let (material, unresolved, unsupported, why) = match v {
        Verified::With {
            material,
            citations,
        } => (
            Value::s(material),
            Value::Array(
                citations
                    .unresolved
                    .iter()
                    .map(|u| {
                        Value::obj([
                            ("heading".to_owned(), Value::s(&u.heading)),
                            ("item".to_owned(), Value::s(&u.item)),
                            ("quote".to_owned(), Value::s(&u.quote.text)),
                            ("unit".to_owned(), Value::s(&u.quote.unit)),
                            ("reason".to_owned(), Value::s(u.reason.name())),
                        ])
                    })
                    .collect(),
            ),
            Value::Array(
                citations
                    .unsupported
                    .iter()
                    .map(|(h, i)| {
                        Value::obj([
                            ("heading".to_owned(), Value::s(h)),
                            ("item".to_owned(), Value::s(i)),
                        ])
                    })
                    .collect(),
            ),
            Value::Null,
        ),
        Verified::Not(why) => (
            Value::Null,
            Value::Array(vec![]),
            Value::Array(vec![]),
            Value::s(why),
        ),
    };
    vec![
        (
            "verified".to_owned(),
            Value::Bool(matches!(v, Verified::With { .. })),
        ),
        ("not_verified".to_owned(), why),
        ("material".to_owned(), material),
        ("unresolved".to_owned(), unresolved),
        ("unsupported".to_owned(), unsupported),
    ]
}

/// 素材のフォルダの単位ごとに、node ごとの地の文の文字列を集める。
fn units_of(dir: &str) -> Result<BTreeMap<String, Vec<String>>, String> {
    let files = folder::readable_files(dir);
    if files.is_empty() {
        return Err(format!("素材のフォルダに文書が無い（{dir}）"));
    }
    let units = folder::load(&files).map_err(|_| format!("素材のフォルダが読めない（{dir}）"))?;
    Ok(units
        .into_iter()
        .map(|(name, doc)| (name, doc.prose().into_iter().map(|s| s.text).collect()))
        .collect())
}

/// ファイルを読んで形を確かめる。
///
/// # Errors
///
/// 読めなければ 65、見出しが決めたとおりでなければ 64。
fn read_persona(file: &str) -> Result<(String, Persona), Exit> {
    let Ok(raw) = std::fs::read(file) else {
        eprintln!("読めない: {file}");
        return Err(Exit::Unreadable);
    };
    let Ok(text) = String::from_utf8(raw) else {
        eprintln!("断る: UTF-8 として読めない: {file}");
        return Err(Exit::Unreadable);
    };
    let persona: Persona = kuchiyose_prompt::parse(&text).map_err(|e| {
        eprintln!("断る: ペルソナの形が決めたとおりでない: {file}: {e}");
        Exit::Usage
    })?;
    Ok((text, persona))
}

/// 素材のフォルダで引用を照らす。
fn verify(persona: &Persona, dir: Option<String>) -> Verified {
    match dir {
        None => Verified::Not("素材のフォルダが分からない。--material で渡せば確かめる".to_owned()),
        Some(d) if !Path::new(&d).is_dir() => {
            Verified::Not(format!("素材のフォルダが手元に無い（{d}）"))
        }
        Some(d) => match units_of(&d) {
            Ok(units) => Verified::With {
                citations: kuchiyose_prompt::check(
                    persona,
                    &units,
                    &kuchiyose_doc::text::count_japanese,
                ),
                material: d,
            },
            Err(why) => Verified::Not(why),
        },
    }
}

/// 取り込まずに、形と引用だけを確かめる。何も書かない。
///
/// ペルソナを下書きする LLM の道具に打たせるのはこれだけである。 形代を取らないので、
/// 道具に形代を書き換える道を渡さずに済む。
fn check_only(file: &str, material: &str, json: bool) -> Exit {
    let persona = match read_persona(file) {
        Ok((_, p)) => p,
        Err(e) => return e,
    };
    if !Path::new(material).is_dir() {
        eprintln!("断る: --material のフォルダが読めない: {material}");
        return Exit::Usage;
    }
    let verified = verify(&persona, Some(material.to_owned()));
    if let Verified::Not(why) = &verified {
        eprintln!("断る: 引用を照らせない: {why}");
        return Exit::Usage;
    }
    if json {
        let mut fields = vec![
            ("file".to_owned(), Value::s(file)),
            ("clean".to_owned(), Value::Bool(clean(&verified))),
        ];
        fields.extend(verified_fields(&verified));
        println!("{}", Value::obj(fields).write());
    } else {
        println!("形と引用を確かめた（取り込んでいない）: {file}");
        print_verified(&verified);
    }
    if clean(&verified) {
        Exit::Pass
    } else {
        Exit::Fail
    }
}

/// ファイルを読んで形を確かめ、引用を照らし、形代に取り込む。
///
/// # Errors
///
/// 読めなければ 65、見出しが決めたとおりでなければ 64 で、取り込まずに断る。
pub fn import(katashiro: &str, file: &str, material: Option<&str>) -> Result<Imported, Exit> {
    let (text, persona) = read_persona(file)?;
    if let Some(m) = material.filter(|m| !Path::new(m).is_dir()) {
        eprintln!("断る: --material のフォルダが読めない: {m}");
        return Err(Exit::Usage);
    }
    let mut c = katashiros::read_file(katashiro)?;
    let dir = material.map(str::to_owned).or_else(|| c.material.clone());
    let verified = verify(&persona, dir);
    c.persona = Some(text);
    write(katashiro, &c)?;
    Ok(Imported { verified })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::fixture;
    use crate::testdir::TempDir;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    /// 素材の p00 に現れる、8 字以上の文字列。
    pub(crate) fn quote_from_material() -> String {
        let body = kuchiyose_metrics::humanness::joined(&fixture::document(0, false).prose());
        body.lines()
            .next()
            .expect("1 行はある")
            .chars()
            .take(12)
            .collect()
    }

    pub(crate) fn persona_text(quote: &str) -> String {
        format!(
            "# 試験\n\n## 大事にすること\n\n- 同じ言い回しを繰り返す\n  - 「{quote}」（p00）\n\n\
             ## 読み手\n\n- 仲間\n  - 「どこにも無い文字列ですよ」（p01）\n\n## 話の運び方\n\n\
             ## 書かないこと\n\n## よく扱う領域\n"
        )
    }

    /// 素材から形代を作り、経路を返す。
    pub(crate) fn built(dir: &TempDir) -> String {
        let person = fixture::write_corpus(dir, "本人", false);
        let out = dir.join("本人.katashiro");
        assert_eq!(
            crate::run(&args(&["katashiro", "build", &person, "-o", &out])),
            Exit::Pass
        );
        out
    }

    #[test]
    fn 取り込むと形代がペルソナを持ち知らせがあっても_0_で終わる() {
        let dir = TempDir::new("persona-import");
        let k = built(&dir);
        let p = dir.write("本人.persona.md", persona_text(&quote_from_material()));
        let got = import(&k, &p, None).expect("取り込める");
        let Verified::With {
            citations,
            material,
        } = &got.verified
        else {
            panic!("形代が覚えている素材のフォルダで照らす: {got:?}");
        };
        assert!(material.ends_with("本人"), "{material}");
        assert_eq!(citations.unresolved.len(), 1, "{citations:?}");
        assert_eq!(citations.unresolved[0].quote.unit, "p01");
        assert_eq!(
            citations.unsupported,
            vec![("読み手".to_owned(), "仲間".to_owned())]
        );
        let c = katashiros::read_file(&k).unwrap();
        assert_eq!(
            c.persona.as_deref(),
            Some(persona_text(&quote_from_material()).as_str())
        );
        assert_eq!(
            crate::run(&args(&["katashiro", "persona", &k, &p, "--json"])),
            Exit::Pass
        );
    }

    #[test]
    fn 見出しが欠けたペルソナは取り込まない() {
        let dir = TempDir::new("persona-shape");
        let k = built(&dir);
        let p = dir.write(
            "p.md",
            persona_text(&quote_from_material()).replace("## 読み手", "## 読者"),
        );
        assert_eq!(
            crate::run(&args(&["katashiro", "persona", &k, &p])),
            Exit::Usage
        );
        assert_eq!(katashiros::read_file(&k).unwrap().persona, None);
    }

    #[test]
    fn utf8_で読めないファイルは_65_で断る() {
        let dir = TempDir::new("persona-utf8");
        let k = built(&dir);
        let p = dir.write("p.md", [0xff, 0xfe]);
        assert_eq!(
            crate::run(&args(&["katashiro", "persona", &k, &p])),
            Exit::Unreadable
        );
    }

    #[test]
    fn 素材のフォルダが手元に無ければ確かめずに取り込む() {
        let dir = TempDir::new("persona-no-material");
        let k = built(&dir);
        std::fs::remove_dir_all(dir.join("本人")).unwrap();
        let p = dir.write("p.md", persona_text(&quote_from_material()));
        let got = import(&k, &p, None).expect("取り込める");
        assert!(matches!(got.verified, Verified::Not(_)), "{got:?}");
        assert!(katashiros::read_file(&k).unwrap().persona.is_some());
    }

    #[test]
    fn 渡した素材のフォルダで照らす() {
        let dir = TempDir::new("persona-material");
        let k = built(&dir);
        let other = fixture::write_corpus(&dir, "別", true);
        let p = dir.write("p.md", persona_text(&quote_from_material()));
        let got = import(&k, &p, Some(&other)).expect("取り込める");
        let Verified::With { citations, .. } = got.verified else {
            panic!("照らす");
        };
        assert!(
            citations
                .unresolved
                .iter()
                .all(|u| u.reason == kuchiyose_prompt::Reason::NoUnit),
            "別のフォルダには p00 が無い: {citations:?}"
        );
    }

    #[test]
    fn 見て外して見る() {
        let dir = TempDir::new("persona-remove");
        let k = built(&dir);
        assert_eq!(
            crate::run(&args(&["katashiro", "persona", &k])),
            Exit::Unknown
        );
        let p = dir.write("p.md", persona_text(&quote_from_material()));
        assert_eq!(
            crate::run(&args(&["katashiro", "persona", &k, &p])),
            Exit::Pass
        );
        assert_eq!(crate::run(&args(&["katashiro", "persona", &k])), Exit::Pass);
        assert_eq!(
            crate::run(&args(&["katashiro", "persona", &k, "--remove"])),
            Exit::Pass
        );
        assert_eq!(katashiros::read_file(&k).unwrap().persona, None);
        assert_eq!(
            crate::run(&args(&["katashiro", "persona", &k])),
            Exit::Unknown
        );
    }

    /// ディレクトリの下の全部のファイルの経路と中身。
    fn snapshot(dir: &Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    out.push((p.clone(), std::fs::read(&p).unwrap()));
                }
            }
        }
        out.sort();
        out
    }

    #[test]
    fn 確かめるだけなら形代を取らず何も書かず知らせの有無を終了コードで言う() {
        let dir = TempDir::new("persona-check");
        let material = fixture::write_corpus(&dir, "本人", false);
        let clean = dir.write(
            "clean.md",
            format!(
                "## 大事にすること\n\n- 同じ言い回しを繰り返す\n  - 「{}」（p00）\n\n\
                 ## 読み手\n\n## 話の運び方\n\n## 書かないこと\n\n## よく扱う領域\n",
                quote_from_material()
            ),
        );
        let noted = dir.write("noted.md", persona_text(&quote_from_material()));
        let before = snapshot(Path::new(dir.path()));
        let check = |file: &str, extra: &[&str]| {
            let mut v = vec![
                "katashiro",
                "persona",
                "--check",
                file,
                "--material",
                &material,
            ];
            v.extend_from_slice(extra);
            crate::run(&args(&v))
        };
        assert_eq!(check(&clean, &[]), Exit::Pass, "知らせが無い");
        assert_eq!(check(&noted, &[]), Exit::Fail, "知らせがある");
        assert_eq!(check(&noted, &["--json"]), Exit::Fail);
        assert_eq!(snapshot(Path::new(dir.path())), before, "何も書かない");
    }

    #[test]
    fn 確かめるときも見出しが崩れていれば_64_で断る() {
        let dir = TempDir::new("persona-check-shape");
        let material = fixture::write_corpus(&dir, "本人", false);
        let p = dir.write(
            "p.md",
            persona_text(&quote_from_material()).replace("## 読み手", "## 読者"),
        );
        assert_eq!(
            crate::run(&args(&[
                "katashiro",
                "persona",
                "--check",
                &p,
                "--material",
                &material
            ])),
            Exit::Usage
        );
    }

    #[test]
    fn 確かめるときは素材のフォルダが要り形代は渡せない() {
        let dir = TempDir::new("persona-check-args");
        let k = built(&dir);
        let material = dir.join("本人");
        let p = dir.write("p.md", persona_text(&quote_from_material()));
        for v in [
            vec!["katashiro", "persona", "--check", &p],
            vec!["katashiro", "persona", "--check", &p, "--material", "/無い"],
            vec![
                "katashiro",
                "persona",
                &k,
                "--check",
                &p,
                "--material",
                &material,
            ],
            vec![
                "katashiro",
                "persona",
                "--check",
                &p,
                "--material",
                &material,
                "--remove",
            ],
        ] {
            assert_eq!(crate::run(&args(&v)), Exit::Usage, "{v:?}");
        }
        assert_eq!(
            katashiros::read_file(&k).unwrap().persona,
            None,
            "取り込んでいない"
        );
    }

    #[test]
    fn 統計値を作り直してもペルソナは残る() {
        let dir = TempDir::new("persona-rebuild");
        let k = built(&dir);
        let p = dir.write("p.md", persona_text(&quote_from_material()));
        import(&k, &p, None).expect("取り込める");
        let before = katashiros::read_file(&k).unwrap().persona;
        assert_eq!(
            crate::run(&args(&["katashiro", "build", &dir.join("本人"), "-o", &k])),
            Exit::Pass
        );
        assert_eq!(katashiros::read_file(&k).unwrap().persona, before);
    }
}
