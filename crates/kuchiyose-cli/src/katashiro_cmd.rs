//! `kuchiyose katashiro …`。形代を作る・見る（[katashiro](../../../docs/design/200-command.md#katashiro)）。
//!
//! 形代が操作の対象そのものなので、位置引数で渡す。

use std::collections::BTreeMap;

use kuchiyose_katashiro::json::Value;
use kuchiyose_katashiro::{save, store, Katashiro, MuteKind, Tuning};
use kuchiyose_metrics::morph::Analyzer;
use kuchiyose_metrics::{Humanness, Unmeasured};
use kuchiyose_scale::select::RegisterCount;
use kuchiyose_scale::stats::KatashiroStats;
use kuchiyose_scale::Sample;

use crate::environment;
use crate::exit::Exit;
use crate::folder;
use crate::katashiros::{self, Origin};
use crate::remedies::FromDefinitions;
use crate::{analyzer, directive_names, stats_json, with_commas};

/// `katashiro` の下のコマンドを振り分ける。
pub fn run(args: &[String]) -> Exit {
    match args.first().map(String::as_str) {
        Some("build") => build(&args[1..]),
        Some("show") => show(&args[1..]),
        Some("diff") => crate::diff_cmd::run(&args[1..]),
        Some("list") => crate::tuning_cmd::list(&args[1..]),
        Some("mute") => crate::tuning_cmd::mute(&args[1..], true),
        Some("unmute") => crate::tuning_cmd::mute(&args[1..], false),
        Some("first-person") => crate::tuning_cmd::first_person(&args[1..]),
        Some("register") => crate::tuning_cmd::register(&args[1..]),
        Some("edit") => crate::tuning_cmd::edit(&args[1..]),
        Some("persona") => crate::persona_cmd::run(&args[1..]),
        Some(other) => {
            eprintln!("知らないコマンド: katashiro {other}");
            eprintln!("{}", crate::KATASHIRO);
            Exit::Usage
        }
        None => {
            eprintln!("{}", crate::KATASHIRO);
            Exit::Usage
        }
    }
}

/// フォルダを測った統計値と、読んだ取り込み元。
pub struct Measured {
    /// 統計値。
    pub stats: KatashiroStats,
    /// 読んだ取り込み元の種類。名前の昇順。
    pub sources: Vec<String>,
}

/// フォルダの文書を全部測る。1 本でも読めなければ何も返さない。
///
/// # Errors
///
/// 文書が 1 本も無い、1 本でも読めない、単位の名前が重なるときに断る。
pub fn measure_folder(dir: &str, analyzer: &dyn Analyzer) -> Result<Measured, Exit> {
    let files = folder::readable_files(dir);
    if files.is_empty() {
        eprintln!("文書が 1 本も見つからない: {dir}");
        eprintln!("{}", folder::EXTENSIONS);
        return Err(Exit::Unreadable);
    }
    let units = folder::load(&files)?;
    let samples: Vec<Sample<'_>> = units
        .iter()
        .map(|(name, document)| Sample { name, document })
        .collect();
    let stats = KatashiroStats::measure(&samples, Some(analyzer)).map_err(|e| {
        eprintln!("断る: {e}");
        Exit::Usage
    })?;
    let mut sources: Vec<String> = files
        .iter()
        .filter_map(|f| folder::source_of(f).map(|s| s.name().to_owned()))
        .collect();
    sources.sort_unstable();
    sources.dedup();
    Ok(Measured { stats, sources })
}

/// 測った統計値から形代を組む。世代は 0 で、書けば 1 になる。
#[must_use]
pub fn assemble_katashiro(
    scene: &str,
    measured: &Measured,
    tuning: Tuning,
    defs: &FromDefinitions,
) -> Katashiro {
    Katashiro::new(
        scene,
        environment::inputs(defs, measured.sources.clone()),
        stats_json::write(&measured.stats),
        tuning,
    )
}

/// 既定の書き出し先。`<フォルダ>.katashiro`。
fn default_output(dir: &str) -> String {
    format!("{}.katashiro", dir.trim_end_matches('/'))
}

/// 既定の場面。フォルダの名前。
fn default_scene(dir: &str) -> String {
    std::path::Path::new(dir.trim_end_matches('/'))
        .file_name()
        .map_or_else(|| dir.to_owned(), |n| n.to_string_lossy().into_owned())
}

/// 書き出し先を確かめる。在れば読んで、引き継ぐものを返す。
///
/// 在るものを消さない。 形代として読めないものを指されたら、それは人の別の
/// ファイルである。版が違って読めない形代も断る——調整を引き継げないまま
/// 上書きすれば、そこで消える。
///
/// 統計値は読まない。 素材から作り直せるので、`stats/` を失った形代からも
/// 調整を引き継ぐ。
fn previous(path: &str) -> Result<Option<store::Originals>, Exit> {
    if !std::path::Path::new(path).exists() {
        return Ok(None);
    }
    save::sweep(path);
    let raw = std::fs::read(path).map_err(|e| {
        eprintln!("断る: 書き出し先が読めない: {path}: {e}");
        Exit::Unreadable
    })?;
    match store::read_originals(&raw) {
        Ok(o) => Ok(Some(o)),
        Err(e @ store::StoreError::UnknownVersion { .. }) => {
            eprintln!("断る: {path}: {e}");
            eprintln!("版の違う形代からは調整を引き継げない。上書きすれば調整が消えるので、消さずに止める");
            eprintln!("調整を捨ててよいなら、ファイルを消してから作る");
            Err(Exit::Unreadable)
        }
        Err(e) => {
            eprintln!("断る: {path} は形代として読めない（{e}）");
            eprintln!("人の別のファイルかもしれない。消さずに止める。-o で別の経路を渡す");
            Err(Exit::Unreadable)
        }
    }
}

/// 測れたかの内訳。
struct Measurability {
    /// 下限に届かない文書。名前と字数。
    below_floor: Vec<(String, usize)>,
    /// 理由 → 測れなかった系統や指標 → 文書の本数。
    unmeasured: BTreeMap<Unmeasured, BTreeMap<String, usize>>,
    /// 環境の側の理由で測れなかったもの。文書と指標と理由。
    broken: Vec<(String, String, Unmeasured)>,
}

/// 統計値が測れたかをまとめる。目盛りが作れるかは言わない。
fn measurability(stats: &KatashiroStats) -> Measurability {
    let floor = kuchiyose_metrics::floor::JAPANESE_CHARS;
    let mut out = Measurability {
        below_floor: Vec::new(),
        unmeasured: BTreeMap::new(),
        broken: Vec::new(),
    };
    for d in &stats.documents {
        if d.chars < floor {
            out.below_floor.push((d.name.clone(), d.chars));
        }
        let mut note = |name: String, why: Unmeasured| {
            *out.unmeasured
                .entry(why)
                .or_default()
                .entry(name)
                .or_default() += 1;
        };
        for name in d.report().missing_systems {
            // 数え上げを持っているなら、足りなかったのは量である。
            let counted = kuchiyose_metrics::System::from_name(&name)
                .is_some_and(|s| d.systems.contains_key(&s));
            let why = if counted {
                Unmeasured::BelowFloor
            } else {
                Unmeasured::ToolMissing
            };
            note(format!("系統 {name}"), why);
        }
        for (name, m) in Humanness::from_windows(&d.windows).flat() {
            if let Some(why) = m.unmeasured() {
                note(name, why);
            }
        }
        for (name, c) in &d.directives {
            if let Some(why) = c.measured().unmeasured() {
                note(name.clone(), why);
            }
        }
        out.broken.extend(
            d.broken_environment()
                .into_iter()
                .map(|(name, why)| (d.name.clone(), name, why)),
        );
    }
    out
}

/// フォルダを測り、形代に書く。
///
/// 目盛りは作らない。比べる相手は見ない（[文書を測る](../../../docs/spec/200-extract.md#文書を測る)）。
/// 言うのは統計値が測れたかどうかだけである。
fn build(args: &[String]) -> Exit {
    let mut dir: Option<String> = None;
    let mut output: Option<String> = None;
    let mut scene: Option<String> = None;
    let mut json = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => {
                json = true;
                i += 1;
            }
            flag @ ("-o" | "--output" | "--scene") => {
                let Some(v) = args.get(i + 1) else {
                    eprintln!("{flag} に値を渡す");
                    return Exit::Usage;
                };
                if flag == "--scene" {
                    scene = Some(v.clone());
                } else {
                    output = Some(v.clone());
                }
                i += 2;
            }
            other if other.starts_with('-') => {
                eprintln!("知らない引数: {other}");
                return Exit::Usage;
            }
            other if dir.is_none() => {
                dir = Some(other.to_owned());
                i += 1;
            }
            other => {
                eprintln!("フォルダは 1 つだけ渡す: {other}");
                eprintln!("1 形代が 1 つのフォルダである。 混ぜるならフォルダをまとめる");
                return Exit::Usage;
            }
        }
    }
    let Some(dir) = dir else {
        eprintln!("素材のフォルダを渡す");
        eprintln!("{}", crate::KATASHIRO_BUILD);
        return Exit::Usage;
    };
    let path = output.unwrap_or_else(|| default_output(&dir));
    match build_into(&dir, &path, scene, json) {
        Ok(_) => Exit::Pass,
        Err(e) => e,
    }
}

/// フォルダを測り、`path` に形代を書く。書いた形代を返す。
///
/// 在れば作り直す。 調整とペルソナは作り直せない原本なので引き継ぐ。素材の
/// フォルダの絶対経路を覚える。
///
/// # Errors
///
/// フォルダが読めない、書き出し先が形代でない、測れない、書けないときに断る。
pub fn build_into(
    dir: &str,
    path: &str,
    scene: Option<String>,
    json: bool,
) -> Result<Katashiro, Exit> {
    let Ok(material) = std::fs::canonicalize(dir).map(|p| p.to_string_lossy().into_owned()) else {
        eprintln!("フォルダが読めない: {dir}");
        return Err(Exit::Unreadable);
    };
    if !std::path::Path::new(&material).is_dir() {
        eprintln!("フォルダが読めない: {dir}");
        return Err(Exit::Unreadable);
    }
    let previous = previous(path)?;
    let scene = scene
        .or_else(|| previous.as_ref().map(|p| p.scene.clone()))
        .unwrap_or_else(|| default_scene(dir));
    if !kuchiyose_katashiro::scene_name_ok(&scene) {
        eprintln!("断る: 場面の名前に使えない: {scene:?}");
        return Err(Exit::Usage);
    }

    // `--json` でも進み方は stderr へ出す。 stdout に混ぜれば JSON として読めない。
    let say = |line: String| {
        if json {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    };
    say(format!(
        "形態素解析: {} / {} {}（同梱）",
        kuchiyose_metrics::lindera::ENGINE,
        kuchiyose_metrics::lindera::DICT_NAME,
        kuchiyose_metrics::lindera::DICT_VERSION
    ));
    let lindera = analyzer::resolve();
    let measured = measure_folder(dir, &lindera)?;
    let m = measurability(&measured.stats);
    // 環境の側の理由で測れなかった文書があれば書かない。 直すのはコーパスではなく
    // 環境であり、直せば全部の値が変わる。
    if !m.broken.is_empty() {
        eprintln!("断る: 環境の側の理由で測れなかった文書がある。形代を書かない");
        for (unit, name, why) in &m.broken {
            eprintln!("  {unit}: {name}: {}", why.name());
        }
        eprintln!("素材ではなく環境を直す。 足しても直らない");
        return Err(Exit::Environment);
    }

    // 調整とペルソナは作り直せない原本である。 引き継ぎ、今の登録簿に無い指標の
    // 名前だけを捨てる。
    let (tuning, dropped) = match &previous {
        Some(p) => {
            let mut t = p.tuning.clone();
            let known = directive_names();
            let dropped = t.drop_unknown_metrics(&|n| known.iter().any(|k| k == n));
            (t, dropped)
        }
        None => (Tuning::default(), Vec::new()),
    };
    let defs = FromDefinitions::load();
    let mut c = assemble_katashiro(&scene, &measured, tuning, &defs);
    c.persona = previous.as_ref().and_then(|p| p.persona.clone());
    c.material = Some(material);
    let expected = previous.as_ref().map(|p| p.generation);
    if let Some(g) = expected {
        c.generation = g;
    }
    if let Err(e) = save::save(path, &c, expected) {
        eprintln!("断る: 書けない: {e}");
        return Err(Exit::Unreadable);
    }
    c.generation = expected.unwrap_or(0) + 1;

    let hash = c.stats.content_hash();
    if json {
        println!(
            "{}",
            build_json(path, &c, &measured.stats, &m, previous.is_some(), &dropped).write()
        );
        return Ok(c);
    }
    println!(
        "{}: {path}（場面: {scene}）",
        if previous.is_some() {
            "作り直した"
        } else {
            "作った"
        }
    );
    println!("測った文書: {} 本", measured.stats.documents.len());
    print_measurability(&m);
    println!("語のまとめ方: {} 語", measured.stats.lexicon.len());
    if previous.is_some() {
        println!("調整を引き継いだ");
        print_tuning(&c.tuning);
    }
    if c.persona.is_some() {
        println!("ペルソナを引き継いだ");
    }
    for name in &dropped {
        println!("無効にしていた指標 `{name}` は今の登録簿に無いので捨てた");
    }
    println!("中身のハッシュ: {hash}");
    Ok(c)
}

fn print_measurability(m: &Measurability) {
    let floor = kuchiyose_metrics::floor::JAPANESE_CHARS;
    if !m.below_floor.is_empty() {
        println!(
            "下限に届かない文書 {} 本（地の文の日本語 {} 字）",
            m.below_floor.len(),
            with_commas(floor)
        );
        for (name, chars) in &m.below_floor {
            println!("  {name}: {} 字", with_commas(*chars));
        }
    }
    if !m.unmeasured.is_empty() {
        println!("測れなかった系統と指標（理由ごと。括弧は文書の本数）");
        for (why, names) in &m.unmeasured {
            let list: Vec<String> = names
                .iter()
                .map(|(name, n)| format!("{name}（{n}）"))
                .collect();
            println!("  {}: {}", why.name(), list.join("、"));
        }
    }
}

fn print_tuning(t: &Tuning) {
    let mut any = false;
    for kind in MuteKind::ALL {
        let muted = t.muted(kind);
        if t.mute_kinds.contains(&kind) {
            println!("  {}: 種類ごと無効にしている", kind.name());
            any = true;
        }
        if !muted.is_empty() {
            println!("  無効にしている{}: {}", kind.name(), muted.join("、"));
            any = true;
        }
    }
    if let Some(f) = &t.first_person {
        println!("  一人称の申告: {f}");
        any = true;
    }
    if let Some(r) = t.register {
        println!("  文体の申告: {}", r.name());
        any = true;
    }
    if !any {
        println!("  調整は無い");
    }
}

#[allow(clippy::cast_precision_loss)]
fn n(v: usize) -> Value {
    Value::Number(v as f64)
}

fn measurability_json(m: &Measurability) -> [(String, Value); 2] {
    [
        (
            "below_floor".to_owned(),
            Value::Array(
                m.below_floor
                    .iter()
                    .map(|(name, chars)| {
                        Value::obj([
                            ("unit".to_owned(), Value::s(name)),
                            ("chars".to_owned(), n(*chars)),
                            (
                                "floor".to_owned(),
                                n(kuchiyose_metrics::floor::JAPANESE_CHARS),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "unmeasured".to_owned(),
            Value::obj(m.unmeasured.iter().map(|(why, names)| {
                (
                    why.name().to_owned(),
                    Value::obj(names.iter().map(|(name, k)| (name.clone(), n(*k)))),
                )
            })),
        ),
    ]
}

fn build_json(
    path: &str,
    c: &Katashiro,
    stats: &KatashiroStats,
    m: &Measurability,
    rebuilt: bool,
    dropped: &[String],
) -> Value {
    let [below, unmeasured] = measurability_json(m);
    Value::obj([
        ("katashiro".to_owned(), Value::s(path)),
        ("scene".to_owned(), Value::s(&c.scene)),
        ("rebuilt".to_owned(), Value::Bool(rebuilt)),
        ("documents".to_owned(), n(stats.documents.len())),
        below,
        unmeasured,
        ("lexicon".to_owned(), n(stats.lexicon.len())),
        ("tuning".to_owned(), c.tuning.to_json()),
        (
            "dropped_mutes".to_owned(),
            Value::Array(dropped.iter().map(Value::s).collect()),
        ),
        ("content_hash".to_owned(), Value::s(c.stats.content_hash())),
        ("fingerprint".to_owned(), Value::s(c.fingerprint().digest())),
        ("persona".to_owned(), Value::Bool(c.persona.is_some())),
        ("material".to_owned(), material_json(c)),
    ])
}

fn material_json(c: &Katashiro) -> Value {
    c.material.as_ref().map_or(Value::Null, Value::s)
}

/// ペルソナを持っているかの 1 行。持っていることを忘れたまま人に渡さないために必ず出す。
fn persona_line(c: &Katashiro) -> String {
    match &c.persona {
        Some(p) => format!(
            "ペルソナ: 持っている（{} 字）。人に渡す前に katashiro persona --remove で外せる",
            p.chars().count()
        ),
        None => "ペルソナ: 持っていない".to_owned(),
    }
}

/// 形代の中身を出す。素材は要らない。
fn show(args: &[String]) -> Exit {
    let mut path: Option<&str> = None;
    let mut json = false;
    for a in args {
        match a.as_str() {
            "--json" => json = true,
            other if other.starts_with('-') => {
                eprintln!("知らない引数: {other}");
                return Exit::Usage;
            }
            other if path.is_none() => path = Some(other),
            other => {
                eprintln!("形代は 1 つだけ渡す: {other}");
                return Exit::Usage;
            }
        }
    }
    let Some(path) = path else {
        eprintln!("形代を渡す");
        eprintln!("{}", crate::KATASHIRO_SHOW);
        return Exit::Usage;
    };
    let origin = Origin::File(path.to_owned());
    let c = match katashiros::read_file(path) {
        Ok(c) => c,
        Err(e) => return e,
    };
    let stats = match katashiros::decode(&c, &origin) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let defs = FromDefinitions::load();
    let fit = environment::check(&c, &defs);
    let m = measurability(&stats);
    let chars: Vec<usize> = stats.documents.iter().map(|d| d.chars).collect();
    let register = RegisterCount::of(stats.documents.iter().map(|d| d.polite_share));
    let unsplit = stats
        .documents
        .iter()
        .filter(|d| d.polite_share.is_none())
        .count();
    let exit = if fit.is_ok() {
        Exit::Pass
    } else {
        Exit::FingerprintMismatch
    };
    let f = c.fingerprint();

    if json {
        let [below, unmeasured] = measurability_json(&m);
        println!(
            "{}",
            Value::obj([
                ("scene".to_owned(), Value::s(&c.scene)),
                ("version".to_owned(), Value::Number(f64::from(c.version))),
                ("generation".to_owned(), {
                    #[allow(clippy::cast_precision_loss)]
                    let g = c.generation as f64;
                    Value::Number(g)
                }),
                ("content_hash".to_owned(), Value::s(&f.content_hash)),
                ("fingerprint".to_owned(), Value::s(f.digest())),
                (
                    "fingerprint_differences".to_owned(),
                    Value::Array(
                        fit.as_ref()
                            .err()
                            .map(|d| d.iter().map(Value::s).collect())
                            .unwrap_or_default(),
                    ),
                ),
                ("documents".to_owned(), n(stats.documents.len())),
                (
                    "chars".to_owned(),
                    Value::obj([
                        (
                            "min".to_owned(),
                            chars.iter().min().map_or(Value::Null, |v| n(*v)),
                        ),
                        (
                            "max".to_owned(),
                            chars.iter().max().map_or(Value::Null, |v| n(*v)),
                        ),
                    ]),
                ),
                (
                    "register".to_owned(),
                    Value::obj([
                        ("polite".to_owned(), n(register.polite)),
                        ("plain".to_owned(), n(register.plain)),
                        ("unsplit".to_owned(), n(unsplit)),
                    ]),
                ),
                below,
                unmeasured,
                (
                    "lexicon".to_owned(),
                    Value::Array(
                        stats
                            .lexicon
                            .pairs()
                            .into_iter()
                            .map(|(a, b)| Value::s(format!("{a}{b}")))
                            .collect(),
                    ),
                ),
                ("tuning".to_owned(), c.tuning.to_json()),
                ("persona".to_owned(), Value::Bool(c.persona.is_some())),
                ("material".to_owned(), material_json(&c)),
            ])
            .write()
        );
        if let Err(diff) = &fit {
            eprintln!("指紋が今の道具と合わない: {}", diff.join("、"));
        }
        return exit;
    }

    println!("場面: {}", c.scene);
    println!("版 {} / 世代 {}", c.version, c.generation);
    println!("中身のハッシュ: {}", f.content_hash);
    println!("指紋: {}", f.digest());
    match &fit {
        Ok(()) => println!("  今の道具と合っている"),
        Err(diff) => {
            println!("  今の道具と合わない: {}", diff.join("、"));
            println!("  素材のフォルダから katashiro build で作り直すか、道具の版を合わせる");
        }
    }
    println!("文書: {} 本", stats.documents.len());
    if let (Some(lo), Some(hi)) = (chars.iter().min(), chars.iter().max()) {
        println!(
            "  長さ: 地の文の日本語 {}〜{} 字",
            with_commas(*lo),
            with_commas(*hi)
        );
    }
    println!(
        "  文体: 敬体 {} 本 / 常体 {} 本 / 分けられない {unsplit} 本",
        register.polite, register.plain
    );
    print_measurability(&m);
    let words: Vec<String> = stats
        .lexicon
        .pairs()
        .into_iter()
        .map(|(a, b)| format!("{a}{b}"))
        .collect();
    println!("語のまとめ方: {} 語", words.len());
    if !words.is_empty() {
        println!("  {}", words.join("、"));
    }
    println!("調整");
    print_tuning(&c.tuning);
    println!(
        "素材のフォルダ: {}",
        c.material.as_deref().unwrap_or("分からない")
    );
    println!("{}", persona_line(&c));
    exit
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
    fn 書き出し先の既定はフォルダの名前に拡張子を付けたもの() {
        assert_eq!(default_output("a/本人"), "a/本人.katashiro");
        assert_eq!(default_output("a/本人/"), "a/本人.katashiro");
        assert_eq!(default_scene("a/本人/"), "本人");
    }

    #[test]
    fn フォルダを測って形代を書く() {
        let dir = TempDir::new("build");
        let person = fixture::write_corpus(&dir, "本人", false);
        let out = dir.join("本人.katashiro");
        assert_eq!(
            crate::run(&args(&["katashiro", "build", &person])),
            Exit::Pass
        );
        let c = katashiros::read_file(&out).expect("読める");
        assert_eq!(c.scene, "本人", "場面の既定はフォルダの名前");
        assert_eq!(c.generation, 1);
        let stats = stats_json::read(&c.stats).expect("読み戻せる");
        assert_eq!(stats.documents.len(), 10);
        assert_eq!(
            environment::check(&c, &FromDefinitions::load()),
            Ok(()),
            "作った直後の指紋は今の道具と合う"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ファイル名に改行がある素材からは形代を書かない() {
        // 書けば、show と review が読み戻す口で断る。
        let dir = TempDir::new("build-control-name");
        let person = fixture::write_corpus(&dir, "本人", false);
        dir.write("本人/改\n行.md", "これも、そうだ。");
        let out = dir.join("本人.katashiro");
        assert_eq!(
            crate::run(&args(&["katashiro", "build", &person])),
            Exit::Usage
        );
        assert!(!std::path::Path::new(&out).exists(), "書かない");
    }

    #[test]
    fn 同じフォルダを_2_度測ると同じ統計値が出る() {
        // 決定性。 世代だけが進む。
        let dir = TempDir::new("build-twice");
        let person = fixture::write_corpus(&dir, "本人", false);
        let a = dir.join("a.katashiro");
        let b = dir.join("b.katashiro");
        for out in [&a, &b] {
            assert_eq!(
                crate::run(&args(&["katashiro", "build", &person, "-o", out])),
                Exit::Pass
            );
        }
        assert_eq!(
            std::fs::read(&a).expect("読める"),
            std::fs::read(&b).expect("読める"),
            "同じバイト列"
        );
    }

    #[test]
    fn 統計値を消して作り直すと同じ統計値が出て調整は残る() {
        // 実際に消して、実際に build する。 控えを戻す形では build を 1 度も
        // 呼ばずに通ってしまう。
        let dir = TempDir::new("rebuild");
        let person = fixture::write_corpus(&dir, "本人", false);
        let out = dir.join("c.katashiro");
        let build = args(&["katashiro", "build", &person, "-o", &out, "--scene", "試験"]);
        assert_eq!(crate::run(&build), Exit::Pass);
        let mut before = katashiros::read_file(&out).expect("読める");
        let name = directive_names().into_iter().next().expect("指標がある");
        before
            .tuning
            .mute
            .get_mut(&MuteKind::Metric)
            .unwrap()
            .extend([name.clone(), "消えた指標".to_owned()]);
        before.tuning.first_person = Some("僕".into());
        let stats_before = before.stats.clone();
        save::save(&out, &before, Some(before.generation)).expect("書ける");
        // stats/ を丸ごと消す。
        let mut e = kuchiyose_katashiro::zip::read(&std::fs::read(&out).unwrap()).unwrap();
        e.remove(store::DOCUMENTS);
        e.remove(store::LEXICON);
        std::fs::write(&out, kuchiyose_katashiro::zip::write(&e)).unwrap();
        assert!(katashiros::read_file(&out).is_err(), "消えている");

        assert_eq!(crate::run(&build), Exit::Pass, "作り直せる");
        let after = katashiros::read_file(&out).expect("読める");
        assert_eq!(after.stats, stats_before, "同じ統計値が出る");
        assert_eq!(after.tuning.muted(MuteKind::Metric), vec![name.as_str()]);
        assert_eq!(
            after.tuning.first_person.as_deref(),
            Some("僕"),
            "調整は残る"
        );
        assert_eq!(after.scene, "試験");
    }

    #[test]
    fn 形代でないファイルへは書かない() {
        // 在るものを消さない。
        let dir = TempDir::new("not-a-katashiro");
        let person = fixture::write_corpus(&dir, "本人", false);
        let victim = dir.write("大事.katashiro", "消えてはいけない");
        assert_eq!(
            crate::run(&args(&["katashiro", "build", &person, "-o", &victim])),
            Exit::Unreadable
        );
        assert_eq!(
            std::fs::read_to_string(&victim).expect("読める"),
            "消えてはいけない"
        );
    }

    #[test]
    fn 版の違う形代へは書かない() {
        // 調整を引き継げないまま上書きすれば、そこで消える。
        let dir = TempDir::new("old-version");
        let person = fixture::write_corpus(&dir, "本人", false);
        let out = dir.join("c.katashiro");
        assert_eq!(
            crate::run(&args(&["katashiro", "build", &person, "-o", &out])),
            Exit::Pass
        );
        let mut e = kuchiyose_katashiro::zip::read(&std::fs::read(&out).unwrap()).unwrap();
        let m = String::from_utf8(e[store::MANIFEST].clone())
            .unwrap()
            .replace(&format!("\"version\":{}", store::VERSION), "\"version\":4");
        e.insert(store::MANIFEST.into(), m.into_bytes());
        let old = kuchiyose_katashiro::zip::write(&e);
        std::fs::write(&out, &old).unwrap();
        assert_eq!(
            crate::run(&args(&["katashiro", "build", &person, "-o", &out])),
            Exit::Unreadable
        );
        assert_eq!(std::fs::read(&out).unwrap(), old, "元のまま");
    }

    #[test]
    fn 版_5_の形代は調整を引き継いで版_6_に作り直す() {
        // 版 5 は読む。 読めば調整を打ち直さずに済む。
        let dir = TempDir::new("v5-rebuild");
        let person = fixture::write_corpus(&dir, "本人", false);
        let out = dir.join("c.katashiro");
        assert_eq!(
            crate::run(&args(&["katashiro", "build", &person, "-o", &out])),
            Exit::Pass
        );
        let mut c = katashiros::read_file(&out).unwrap();
        c.tuning.first_person = Some("僕".into());
        save::save(&out, &c, Some(c.generation)).unwrap();
        let mut e = kuchiyose_katashiro::zip::read(&std::fs::read(&out).unwrap()).unwrap();
        let m = String::from_utf8(e[store::MANIFEST].clone())
            .unwrap()
            .replace(&format!("\"version\":{}", store::VERSION), "\"version\":5");
        let m = m.replace(
            &format!(
                "\"material\":{},",
                Value::s(c.material.clone().unwrap()).write()
            ),
            "",
        );
        e.insert(store::MANIFEST.into(), m.into_bytes());
        std::fs::write(&out, kuchiyose_katashiro::zip::write(&e)).unwrap();
        assert_eq!(katashiros::read_file(&out).unwrap().version, 5);

        assert_eq!(
            crate::run(&args(&["katashiro", "build", &person, "-o", &out])),
            Exit::Pass
        );
        let after = katashiros::read_file(&out).unwrap();
        assert_eq!(after.version, store::VERSION);
        assert_eq!(after.tuning.first_person.as_deref(), Some("僕"));
    }

    #[test]
    fn 素材のフォルダの絶対経路を覚える() {
        let dir = TempDir::new("material");
        let person = fixture::write_corpus(&dir, "本人", false);
        let out = dir.join("c.katashiro");
        assert_eq!(
            crate::run(&args(&["katashiro", "build", &person, "-o", &out])),
            Exit::Pass
        );
        let c = katashiros::read_file(&out).unwrap();
        let want = std::fs::canonicalize(&person).unwrap();
        assert_eq!(c.material.as_deref(), want.to_str());
    }

    #[test]
    fn ペルソナを持っているかを必ず言う() {
        let dir = TempDir::new("show-persona");
        let person = fixture::write_corpus(&dir, "本人", false);
        let out = dir.join("c.katashiro");
        assert_eq!(
            crate::run(&args(&["katashiro", "build", &person, "-o", &out])),
            Exit::Pass
        );
        let mut c = katashiros::read_file(&out).unwrap();
        assert_eq!(persona_line(&c), "ペルソナ: 持っていない");
        c.persona = Some("## 大事にすること\n".into());
        assert!(
            persona_line(&c).contains("持っている"),
            "{}",
            persona_line(&c)
        );
        assert!(
            persona_line(&c).contains("--remove"),
            "{}",
            persona_line(&c)
        );
    }

    #[test]
    fn 文書が無いフォルダでは形代を作らない() {
        let dir = TempDir::new("empty");
        dir.write("空/README", "説明");
        let out = dir.join("c.katashiro");
        assert_eq!(
            crate::run(&args(&["katashiro", "build", &dir.join("空"), "-o", &out])),
            Exit::Unreadable
        );
        assert!(!std::path::Path::new(&out).exists());
    }

    #[test]
    fn 読めない_1_本があればフォルダごと使わない() {
        let dir = TempDir::new("one-bad");
        let person = fixture::write_corpus(&dir, "本人", false);
        dir.write("本人/壊れた.html", [0xff, 0xfe, 0x00]);
        let out = dir.join("c.katashiro");
        assert_ne!(
            crate::run(&args(&["katashiro", "build", &person, "-o", &out])),
            Exit::Pass
        );
        assert!(!std::path::Path::new(&out).exists());
    }

    #[test]
    fn 解析器が無ければ環境の壊れとして数える() {
        // 素材を足しても直らない。 コーパスの性質と同じ顔で通さない。
        let docs = fixture::rich_corpus(2);
        let without = KatashiroStats::measure(&fixture::samples(&docs), None).expect("測れる");
        assert!(!measurability(&without).broken.is_empty());
        let with = KatashiroStats::measure(&fixture::samples(&docs), Some(&fixture::Chars))
            .expect("測れる");
        assert!(measurability(&with).broken.is_empty());
    }

    #[test]
    fn 中身を見る() {
        let dir = TempDir::new("show");
        let person = fixture::write_corpus(&dir, "本人", false);
        let out = dir.join("c.katashiro");
        assert_eq!(
            crate::run(&args(&["katashiro", "build", &person, "-o", &out])),
            Exit::Pass
        );
        assert_eq!(crate::run(&args(&["katashiro", "show", &out])), Exit::Pass);
        assert_eq!(
            crate::run(&args(&["katashiro", "show", &out, "--json"])),
            Exit::Pass
        );
    }

    #[test]
    fn 指紋が合わない形代は中身を見せたうえで_66_で終わる() {
        let dir = TempDir::new("show-mismatch");
        let person = fixture::write_corpus(&dir, "本人", false);
        let out = dir.join("c.katashiro");
        assert_eq!(
            crate::run(&args(&["katashiro", "build", &person, "-o", &out])),
            Exit::Pass
        );
        let mut c = katashiros::read_file(&out).unwrap();
        c.inputs.morphology.version = "0.0".into();
        save::save(&out, &c, Some(c.generation)).unwrap();
        assert_eq!(
            crate::run(&args(&["katashiro", "show", &out])),
            Exit::FingerprintMismatch
        );
    }

    #[test]
    fn 壊れた形代は_65_で断る() {
        let dir = TempDir::new("show-broken");
        let broken = dir.write("c.katashiro", "PK");
        assert_eq!(
            crate::run(&args(&["katashiro", "show", &broken])),
            Exit::Unreadable
        );
    }
}
