//! kakiburi。人が触る面。
//!
//! <strong>`review` だけが周回に出てくる。</strong> ほかは素材が増えたときにしか動かさない。

mod analyzer;
mod exit;
#[cfg(test)]
mod fixture;
mod remedies;
mod scale_json;

use exit::Exit;
use kakiburi_cassette::{
    store, Baseline, Cassette, Corpus, Decided, Derived, Fingerprint, Inputs, Normalization, Role,
    Tool, Unit,
};
use kakiburi_metrics::{structure, symbol, Measured};
use kakiburi_normalize::{normalize, Source};
use kakiburi_review::{judge, Observed, Range, Side};
use kakiburi_scale::{assemble, measure_against, Sample, Scale};
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = run(&args);
    std::process::exit(code.code());
}

fn run(args: &[String]) -> Exit {
    match args.first().map(String::as_str) {
        Some("measure") => measure(&args[1..]),
        Some("metrics") => metrics(&args[1..]),
        Some("new") => new_cassette(&args[1..]),
        Some("add") => add(&args[1..]),
        Some("build") => build(&args[1..]),
        Some("review") => review(&args[1..]),
        Some("help" | "--help" | "-h") | None => {
            print_help();
            Exit::Pass
        }
        Some(other) => {
            eprintln!("知らないコマンド: {other}");
            print_help();
            Exit::Usage
        }
    }
}

fn print_help() {
    println!(
        "\
kakiburi — どこがその人と違うかを、言えるようにする

  kakiburi measure <ファイル> [--source <取り込み元>]
      1 本を測る。カセットが無くても動く。
      <strong>系統の距離は出ない</strong>——語彙が無いので、その場で選べば違う軸のベクトル
      どうしの距離になる。

作る——たまに動かす

  kakiburi new <カセット> --scene <場面>
      <strong>場面は人が指定する。</strong> 文章から当てにいかない。1 カセット 1 場面。

  kakiburi add <カセット> <ファイル...> [--source <取り込み元>] [--as person|baseline|other]
      正規化して入れる。<strong>1 本でも断ったら何も入れない。</strong>
      本文が変わるので派生物を捨てる。

  kakiburi build <カセット>
      目盛りを作る。<strong>作らずに終わる条件を持つ</strong>——止まっても失敗ではない。

回す——毎周

  kakiburi review <ファイル> --cassette <カセット> [--source <取り込み元>]
      検める。3 値と指摘を返す。
      <strong>目盛りの無いカセットは判定できない（2）を返す</strong>——素材が足りずに作れな
      かったのは正常な状態である。

覗く

  kakiburi metrics
      登録簿を回して一覧を出す。使う側が一覧を持たないことの裏返し。

取り込み元: github-markdown / directive-markdown / html / plain-markdown

終了コード: 0 通る / 1 通らない / 2 判定できない / 64 以上 使う前の問題"
    );
}

/// カセットを作る。
///
/// <strong>場面は人が指定する。</strong> 文章から当てにいかないので、ここで必ず訊く。
/// <strong>1 カセット 1 場面である。</strong>
fn new_cassette(args: &[String]) -> Exit {
    let Some(path) = args.first() else {
        eprintln!("カセットの経路を渡す");
        return Exit::Usage;
    };
    let mut scene = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--scene" {
            scene = args.get(i + 1).cloned();
            i += 2;
            continue;
        }
        eprintln!("知らない引数: {}", args[i]);
        return Exit::Usage;
    }
    let Some(scene) = scene else {
        eprintln!("--scene が要る。場面は人が指定する");
        return Exit::Usage;
    };

    let c = Cassette {
        version: 1,
        scene: scene.clone(),
        fingerprint: current_fingerprint(),
        // <strong>いまは常に暫定値が立つ。</strong> 12 か所の閾値がまだ導き直されていない。
        provisional: vec!["除外の既定".into(), "帯の端".into(), "語彙の大きさ".into()],
        decided: Decided {
            scene,
            boilerplate: vec![],
            baseline: Baseline {
                model: String::new(),
                version: String::new(),
                params: BTreeMap::new(),
                topics: vec![],
            },
            movement: BTreeMap::new(),
        },
        corpus: Corpus::new(vec![]),
        derived: Derived::dropped(),
    };
    if std::fs::write(path, store::write(&c)).is_err() {
        eprintln!("書けない: {path}");
        return Exit::Unreadable;
    }
    println!("作った: {path}");
    println!("場面: {}", c.scene);
    Exit::Pass
}

/// 文書を入れる。
///
/// <strong>落ちない入力は断る。</strong> 黙って一部を落として通さない——<strong>1 本でも断ったら
/// 失敗する</strong>。成功したことにすると、欠けたまま次へ進む。
fn add(args: &[String]) -> Exit {
    let Some(path) = args.first() else {
        eprintln!("カセットの経路を渡す");
        return Exit::Usage;
    };
    let mut files: Vec<String> = Vec::new();
    let mut source = Source::GithubMarkdown;
    let mut role = Role::Person;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--source" => {
                let Some(name) = args.get(i + 1).and_then(Source::from_name) else {
                    eprintln!("対応表に無い取り込み元");
                    return Exit::Usage;
                };
                source = name;
                i += 2;
            }
            "--as" => {
                role = match args.get(i + 1).map(String::as_str) {
                    Some("person") => Role::Person,
                    Some("baseline") => Role::BaselineOutput,
                    Some("other") => Role::Other,
                    _ => {
                        eprintln!("--as は person / baseline / other");
                        return Exit::Usage;
                    }
                };
                i += 2;
            }
            f => {
                files.push(f.to_owned());
                i += 1;
            }
        }
    }
    if files.is_empty() {
        eprintln!("入れるファイルを渡す");
        return Exit::Usage;
    }

    let Ok(raw) = std::fs::read(path) else {
        eprintln!("カセットが読めない: {path}");
        return Exit::Unreadable;
    };
    let mut c = match store::read(&raw) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("カセットが読めない: {e}");
            return Exit::Unreadable;
        }
    };

    // <strong>まず全部を読む。</strong> 1 本でも断られたら何も入れない——
    // 途中まで入った状態を残すと、欠けたまま次へ進む。
    let mut units = Vec::new();
    for f in &files {
        let name = std::path::Path::new(f)
            .file_stem()
            .map_or_else(|| f.clone(), |s| s.to_string_lossy().into_owned());
        let Ok(body) = std::fs::read_to_string(f) else {
            eprintln!("読めない: {f}");
            return Exit::Unreadable;
        };
        match normalize(&body, source) {
            Ok(d) => units.push(Unit {
                name,
                role,
                document: d,
            }),
            Err(e) => {
                eprintln!("断る: {f}: {e}");
                eprintln!("1 本でも断ったら入れない。欠けたまま次へ進まないため");
                return Exit::Unreadable;
            }
        }
    }

    let mut all = c.corpus.units.clone();
    all.extend(units);
    c.corpus = Corpus::new(all);
    // <strong>本文が変われば派生物は古い。</strong> 捨てる。
    c.drop_derived();
    // 取り込み元を指紋に足す。
    let mut sources = c.fingerprint.inputs.normalization.sources.clone();
    if !sources.iter().any(|s| s == source.name()) {
        sources.push(source.name().to_owned());
        sources.sort_unstable();
    }
    let mut inputs = c.fingerprint.inputs.clone();
    inputs.normalization.sources = sources;
    c.fingerprint = Fingerprint::build(inputs);

    if std::fs::write(path, store::write(&c)).is_err() {
        eprintln!("書けない: {path}");
        return Exit::Unreadable;
    }
    println!("入れた {} 本。役: {}", files.len(), role.dir());
    println!("派生物を捨てた。build し直しが要る");
    Exit::Pass
}

/// 目盛りを作る。
///
/// <strong>作らずに終わる条件を持つ。</strong> 止まっても失敗ではない——目盛りの無いカセットが
/// 出来上がり、`0` で終わる。
fn build(args: &[String]) -> Exit {
    let Some(path) = args.first() else {
        eprintln!("カセットの経路を渡す");
        return Exit::Usage;
    };
    let Ok(raw) = std::fs::read(path) else {
        eprintln!("カセットが読めない: {path}");
        return Exit::Unreadable;
    };
    let mut c = match store::read(&raw) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("カセットが読めない: {e}");
            return Exit::Unreadable;
        }
    };

    let person = samples(&c, Role::Person);
    let baseline = samples(&c, Role::BaselineOutput);
    println!("本人 {} 単位 / 基準 {} 単位", person.len(), baseline.len());

    let mecab = analyzer::resolve();
    match &mecab {
        Some(m) => println!("形態素解析: MeCab / {} {}", m.dict_name, m.dict_version),
        None => println!(
            "形態素解析: 無し（{} が未設定）。<strong>5 系統のうち 2 つが測れない</strong>",
            analyzer::DICDIR
        ),
    }

    let scale = match assemble(
        &person,
        &baseline,
        mecab
            .as_ref()
            .map(|m| m as &dyn kakiburi_metrics::morph::Analyzer),
    ) {
        Ok(s) => s,
        Err(e) => {
            // <strong>作らずに終わる。</strong> 止まっても失敗ではない。
            println!("目盛りを作らない: {e}");
            // <strong>どの単位のどこで止まったかを言う。</strong>「10 本に届かない」だけでは、
            // 素材を足すべきか、長さを揃えるべきか、辞書を入れるべきかが分からない。
            let a = mecab
                .as_ref()
                .map(|m| m as &dyn kakiburi_metrics::morph::Analyzer);
            print_reports("本人", &kakiburi_scale::inspect(&person, a));
            print_reports("基準", &kakiburi_scale::inspect(&baseline, a));
            // <strong>試した環境を指紋に残す。</strong> 残さなければ、次に検めるときに
            // 「道具が違う」と言われる——道具は同じで、目盛りが無いだけである。
            let fingerprint = fingerprint_with(&c, None, mecab.as_ref());
            stop_without_scale(path, &mut c, fingerprint);
            return Exit::Pass;
        }
    };

    println!();
    for (name, set) in &scale.frozen {
        println!("  {name:<12} {:>5} 次元", set.len());
    }
    println!(
        "照合値の帯: 天井 {:.3}〜{:.3} / 床 {:.3}〜{:.3}",
        scale.band.ceiling.low,
        scale.band.ceiling.high,
        scale.band.floor.low,
        scale.band.floor.high
    );
    println!(
        "人らしさの帯: 人 {:.3}〜{:.3} / 機械 {:.3}〜{:.3}",
        scale.humanness_band.ceiling.low,
        scale.humanness_band.ceiling.high,
        scale.humanness_band.floor.low,
        scale.humanness_band.floor.high
    );
    if scale.humanness.evenly_spread() {
        // 4 つは同じ現象を別の角度から見ている。<strong>均等に開いたら較正を疑う。</strong>
        println!("但し書き: 人らしさの合算が 4 指標に均等に開いている。較正を疑う");
    }

    // <strong>作り終えた目盛りだけを入れる。</strong> 検めはこれを受け取る。
    c.derived = Derived {
        vocabulary: Some(vocabulary_note(&scale)),
        values: None,
        spread: Some(spread_note(&person)),
        calibration: Some(format!("系統 {} 本の較正と合算", scale.frozen.len())),
        scale: Some(scale_json::write(&scale)),
        effective: None,
    };
    // <strong>指紋を作り直す。</strong> 語彙と z 得点と道具が値を決めるので、目盛りができた時点で
    // 指紋も変わる——変えなければ、次に検めるときに合わないことが分からない。
    c.fingerprint = fingerprint_with(&c, Some(&scale), mecab.as_ref());
    if std::fs::write(path, store::write(&c)).is_err() {
        eprintln!("書けない: {path}");
        return Exit::Unreadable;
    }
    println!();
    println!("目盛りを入れた: {path}");
    Exit::Pass
}

/// 単位ごとの内訳を出す。<strong>止まった理由を単位まで下ろす。</strong>
fn print_reports(side: &str, reports: &[kakiburi_scale::Report]) {
    let usable = reports.iter().filter(|r| r.usable()).count();
    println!();
    println!("{side}: 使える単位 {usable} / {} 本", reports.len());
    for r in reports {
        if r.usable() {
            continue;
        }
        let mut why = Vec::new();
        if !r.missing_systems.is_empty() {
            why.push(format!("系統 {}", r.missing_systems.join("・")));
        }
        if !r.missing_humanness.is_empty() {
            why.push(format!(
                "人らしさ {} 次元（{}）",
                r.missing_humanness.len(),
                r.missing_humanness.join("・")
            ));
        }
        println!("  {} ({} 字): {}", r.name, r.chars, why.join(" / "));
    }
}

/// カセットの単位を素材の形にする。
fn samples(c: &Cassette, role: Role) -> Vec<Sample<'_>> {
    c.units(role)
        .into_iter()
        .map(|u| Sample {
            name: &u.name,
            document: &u.document,
        })
        .collect()
}

/// 語彙の覚え書き。<strong>次元の並びそのものは目盛りの中にある。</strong>
fn vocabulary_note(scale: &Scale) -> String {
    scale
        .frozen
        .iter()
        .map(|(n, s)| format!("{n} {} 次元", s.len()))
        .collect::<Vec<_>>()
        .join(" / ")
}

/// 幅の覚え書き。<strong>本人の単位から取った、指示できる指標の幅である。</strong>
fn spread_note(person: &[Sample<'_>]) -> String {
    format!("指示できる指標 {} 本の幅", person_ranges(person).len())
}

/// 本人の幅。<strong>測れた単位での最小値から最大値まで。</strong>
///
/// <strong>百分位で刈らない。</strong> 刈れば、その人が現に 1 度書いた書き方が幅の外に出る。
/// <strong>外れた 1 本で幅が広がるのは、この設計では正しい向きである。</strong>
///
/// <strong>相手集合の 5 本ではなく、本人の全単位から取る。</strong> 帯に使わない単位も値と幅には
/// 使う。
fn person_ranges(person: &[Sample<'_>]) -> BTreeMap<String, (Range, f64)> {
    let mut seen: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for s in person {
        for (name, m) in measured(s.document) {
            let entry = seen.entry(name.to_owned()).or_default();
            if let Some(v) = m.value() {
                entry.push(v);
            }
        }
    }
    seen.into_iter()
        .filter(|(_, vs)| !vs.is_empty())
        .map(|(name, vs)| {
            let low = vs.iter().copied().fold(f64::INFINITY, f64::min);
            let high = vs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            // <strong>出現割合は、現れた文書の割合である。</strong> 測れた本数ではない。
            #[allow(clippy::cast_precision_loss)]
            let rate = vs.iter().filter(|v| **v > 0.0).count() as f64 / vs.len() as f64;
            (
                name,
                (
                    Range {
                        low,
                        high,
                        units: vs.len(),
                    },
                    rate,
                ),
            )
        })
        .collect()
}

/// 目盛りを作らずに終える。<strong>失敗ではない。</strong>
fn stop_without_scale(path: &str, c: &mut Cassette, fingerprint: Fingerprint) {
    c.drop_derived();
    c.fingerprint = fingerprint;
    if std::fs::write(path, store::write(c)).is_err() {
        eprintln!("書けない: {path}");
        return;
    }
    println!("目盛りの無いカセットが出来上がった。review は判定できないを返す");
}

/// 検める。
///
/// <strong>目盛りを作らない。</strong> カセットから受け取るだけである——検める文書を見てから
/// 重みや語彙を作り直す経路を作らない。
fn review(args: &[String]) -> Exit {
    let Some(path) = args.first() else {
        eprintln!("ファイルを渡す");
        return Exit::Usage;
    };
    let mut cassette = None;
    let mut source = Source::GithubMarkdown;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--cassette" => {
                let Some(v) = args.get(i + 1) else {
                    eprintln!("--cassette にカセットが要る");
                    return Exit::Usage;
                };
                cassette = Some(v.clone());
                i += 2;
            }
            "--source" => {
                let Some(name) = args.get(i + 1) else {
                    eprintln!("--source に取り込み元が要る");
                    return Exit::Usage;
                };
                let Some(s) = Source::from_name(name) else {
                    eprintln!("対応表に無い取り込み元: {name}");
                    return Exit::Usage;
                };
                source = s;
                i += 2;
            }
            other => {
                eprintln!("知らない引数: {other}");
                return Exit::Usage;
            }
        }
    }
    let Some(cassette) = cassette else {
        eprintln!("--cassette が要る");
        return Exit::Usage;
    };

    let Ok(body) = std::fs::read_to_string(path) else {
        eprintln!("読めない: {path}");
        return Exit::Unreadable;
    };
    let doc = match normalize(&body, source) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("断る: {e}");
            return Exit::Unreadable;
        }
    };

    let Ok(raw) = std::fs::read(&cassette) else {
        eprintln!("カセットが読めない: {cassette}");
        return Exit::Unreadable;
    };
    let c = match store::read(&raw) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("カセットが読めない: {e}");
            return Exit::Unreadable;
        }
    };

    // <strong>指紋を先に照らす。</strong> 合わないカセットで測れば、比べたものに意味が無い。
    // 判定できないではなく <strong>使う前の問題</strong>である——64 以上で返す。
    if let Err(diff) = check_fingerprint(&c) {
        eprintln!("指紋が環境と合わない: {}", diff.join("、"));
        eprintln!("測り直しが要る。過去の値とは比べられない。");
        return Exit::FingerprintMismatch;
    }

    if !c.provisional.is_empty() {
        println!(
            "但し書き: 暫定値が立っている（{}）",
            c.provisional.join("、")
        );
    }

    // <strong>目盛りが無ければ判定できない。</strong> 素材が足りずに作れなかったのは正常な
    // 状態であり、仕様がそのために判定できないを置いている。
    let Some(scale) = c.derived.scale.as_deref().and_then(scale_json::read) else {
        let outcome = judge(None, None, &[]);
        println!("判定: 判定できない");
        println!("止まった段: {}", outcome.stage.name());
        println!("理由: 目盛りが無い。素材が足りずに作れなかった");
        return Exit::from_verdict(outcome.verdict);
    };

    // <strong>相手集合は目盛りが名指ししたものである。</strong> 検める側が選び直さない。
    let person = samples(&c, Role::Person);
    let partners: Vec<Sample<'_>> = person
        .iter()
        .filter(|s| scale.partners.iter().any(|n| n == s.name))
        .copied()
        .collect();
    if partners.len() != scale.partners.len() {
        eprintln!(
            "相手集合が揃わない（{} / {} 本）。カセットの本文が入れ替わっている",
            partners.len(),
            scale.partners.len()
        );
        return Exit::FingerprintMismatch;
    }

    let mecab = analyzer::resolve();
    let got = measure_against(
        &scale,
        Sample {
            name: path,
            document: &doc,
        },
        &partners,
        mecab
            .as_ref()
            .map(|m| m as &dyn kakiburi_metrics::morph::Analyzer),
    );

    // 帯に照らして 3 値のどちら側かにする。
    let side = |band: kakiburi_scale::Band, value: Option<f64>| -> Option<Side> {
        value.map(|v| match band.judge(v) {
            kakiburi_scale::Verdict::Pass => Side::Human,
            kakiburi_scale::Verdict::Fail => Side::Machine,
            kakiburi_scale::Verdict::Unknown => Side::InBand,
        })
    };
    let humanness = side(scale.humanness_band, got.humanness);
    let matching = side(scale.band, got.matching);

    // <strong>幅は本人の単位から取る。</strong> 検める文書は入れない。
    let ranges = person_ranges(&person);
    let defs = remedies::FromDefinitions::load();
    if defs.is_empty() {
        // <strong>黙って指摘を落とさない。</strong> 直し方の出どころが無ければ、判定は出ても
        // 指摘が 1 本も出ない——それを「幅の中だった」と読まれてはいけない。
        println!("但し書き: 定義ファイルが見つからない。指摘の文を引けない");
    } else {
        println!("直し方の出どころ: 定義ファイル {} 本", defs.len());
    }
    let directives: Vec<Observed> = measured(&doc)
        .into_iter()
        // <strong>動かないと分かった指標は前に出さない。</strong>
        .filter(|(name, _)| !c.is_stuck(name))
        .filter_map(|(name, m)| {
            let (range, rate) = ranges.get(name)?;
            Some(Observed {
                name: name.to_owned(),
                value: m.value(),
                range: *range,
                lower: defs.lower_rule(name, *rate),
            })
        })
        .collect();

    let result = kakiburi_review::review(humanness, matching, &directives, &defs);
    println!();
    println!("人らしさ値: {}", show(got.humanness));
    if !got.missing_humanness.is_empty() {
        println!("  測れていない次元: {}", got.missing_humanness.join("、"));
    }
    println!("照合値: {}", show(got.matching));
    if !got.missing_systems.is_empty() {
        println!("  測れていない系統: {}", got.missing_systems.join("、"));
    }
    println!();
    println!("判定: {}", verdict_name(result.outcome.verdict));
    println!("止まった段: {}", result.outcome.stage.name());
    println!("理由: {}", result.outcome.reason);
    if !result.points.is_empty() {
        println!();
        println!("指摘 {} 本", result.points.len());
        for p in &result.points {
            println!("  - {}", p.prose());
        }
    }
    Exit::from_verdict(result.outcome.verdict)
}

/// 値か、出ていないことを書く。<strong>0 と混ぜない。</strong>
fn show(v: Option<f64>) -> String {
    v.map_or_else(|| "出ていない".to_owned(), |x| format!("{x:.3}"))
}

/// 3 値の名前。
fn verdict_name(v: kakiburi_review::Verdict) -> &'static str {
    match v {
        kakiburi_review::Verdict::Pass => "通る",
        kakiburi_review::Verdict::Fail => "通らない",
        kakiburi_review::Verdict::Unknown => "判定できない",
    }
}

/// いまの環境の指紋を組み立てる。
///
/// <strong>材料をすべて渡さないと組み立てられない。</strong> 混ぜ忘れは型が止める。
fn current_fingerprint() -> Fingerprint {
    Fingerprint::build(base_inputs(None, None))
}

/// カセットに入れる指紋。<strong>目盛りができた時点で変わる。</strong>
///
/// 語彙と z 得点と道具が値を決めるので、目盛りを入れたら指紋も入れ替える——
/// <strong>入れ替えなければ、次に検めるときに合わないことが分からない。</strong>
fn fingerprint_with(
    c: &Cassette,
    scale: Option<&Scale>,
    mecab: Option<&kakiburi_metrics::mecab::Mecab>,
) -> Fingerprint {
    let mut inputs = base_inputs(scale, mecab);
    // カセットが決めたことは引き継ぐ。
    inputs.normalization.sources = c.fingerprint.inputs.normalization.sources.clone();
    inputs.baseline = c.fingerprint.inputs.baseline.clone();
    inputs.decided = c.fingerprint.inputs.decided.clone();
    Fingerprint::build(inputs)
}

/// 指紋の材料。
fn base_inputs(scale: Option<&Scale>, mecab: Option<&kakiburi_metrics::mecab::Mecab>) -> Inputs {
    // <strong>語彙と z 得点は系統ごとに入れる。</strong> 次元の並びが変われば値が変わる。
    let mut vocabulary = BTreeMap::new();
    let mut z_scores = BTreeMap::new();
    if let Some(s) = scale {
        for (name, set) in &s.frozen {
            let mut dims = Vec::new();
            let mut zs = Vec::new();
            for (i, part) in set.parts().iter().enumerate() {
                for (j, d) in part.dims().iter().enumerate() {
                    dims.push(format!("{i}:{d}"));
                    zs.push((part.mean()[j], part.sd()[j]));
                }
            }
            vocabulary.insert(name.clone(), dims);
            z_scores.insert(name.clone(), zs);
        }
    }
    Inputs {
        metric_definitions: format!("実装済み {} 本", measured_names().len()),
        unit_definitions: format!("kakiburi-doc {}", env!("CARGO_PKG_VERSION")),
        vocabulary,
        z_scores,
        morphology: analyzer::tool(mecab),
        // <strong>まだ使わないものも、使わないと書いて渡す。</strong>
        dependency: Tool::unused(),
        compressor: analyzer::compressor(),
        external_tables: BTreeMap::new(),
        normalization: Normalization {
            sources: vec![],
            implementation: "kakiburi-normalize".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            mapping: BTreeMap::new(),
        },
        baseline: Baseline {
            model: String::new(),
            version: String::new(),
            params: BTreeMap::new(),
            topics: vec![],
        },
        decided: BTreeMap::new(),
    }
}

/// カセットの指紋を、いまの環境と照らす。
///
/// <strong>合わなければ何が違うかを言う。</strong> ハッシュだけでは、変わったことは分かっても
/// 何が変わったかが分からない。
fn check_fingerprint(c: &Cassette) -> Result<(), Vec<&'static str>> {
    // <strong>取り込み元と語彙はカセットが決めたことである。</strong> 環境の側で作り直せないので、
    // カセットのものを引き継いで照らす——引き継がなければ、正しいカセットが
    // つねに「合わない」になる。
    //
    // <strong>照らす相手は、道具と実装と定義の側である。</strong> 辞書を入れ替えた、圧縮器が
    // 変わった、指標が増えた——そこが変われば過去の値と比べられない。
    let scale = c.derived.scale.as_deref().and_then(scale_json::read);
    let mecab = analyzer::resolve();
    let here = fingerprint_with(c, scale.as_ref(), mecab.as_ref());
    let diff = c.fingerprint.differences(&here);
    if diff.is_empty() {
        Ok(())
    } else {
        Err(diff)
    }
}

/// 1 本を測る。
///
/// <strong>カセットが無くても動く。ただし出るものが違う。</strong> 系統の距離は出ない——
/// 頻度で次元を選ぶ系統は、渡された 2 本からその場で選べば違う軸になる。
///
/// <strong>黙ってスカラーだけ出さない。</strong> 系統を出せないことを言う。
fn measure(args: &[String]) -> Exit {
    let Some(path) = args.first() else {
        eprintln!("ファイルを渡す");
        return Exit::Usage;
    };
    let mut source = Source::GithubMarkdown;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--source" {
            let Some(name) = args.get(i + 1) else {
                eprintln!("--source に取り込み元が要る");
                return Exit::Usage;
            };
            let Some(s) = Source::from_name(name) else {
                eprintln!("対応表に無い取り込み元: {name}");
                return Exit::Usage;
            };
            source = s;
            i += 2;
            continue;
        }
        eprintln!("知らない引数: {}", args[i]);
        return Exit::Usage;
    }

    let Ok(body) = std::fs::read_to_string(path) else {
        eprintln!("読めない: {path}");
        return Exit::Unreadable;
    };
    let doc = match normalize(&body, source) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("断る: {e}");
            return Exit::Unreadable;
        }
    };

    println!("取り込み元 {}", source.name());
    println!("日本語 {} 字", doc.japanese_chars());
    // <strong>形態素の数も出す。</strong> 字数で足りていても語で足りないことがあり、
    // そのとき何が測れないかが字数からは分からない。
    if let Some(m) = analyzer::resolve() {
        match kakiburi_metrics::morph::Analyzed::of(&doc.prose(), &m) {
            Ok(a) => println!(
                "延べ {} 語（{} {}）",
                a.tokens(),
                m.dict_name,
                m.dict_version
            ),
            Err(e) => println!("形態素解析できない: {e}"),
        }
    }
    println!(
        "段落 {} / 文 {} / 節 {} / 項目 {}",
        doc.paragraphs().len(),
        doc.sentences().len(),
        doc.sections().len(),
        doc.items().len()
    );
    println!();
    println!("指示できる指標");
    println!("{}", "-".repeat(46));
    for (name, m) in measured(&doc) {
        match m {
            Measured::Value(v) => println!("  {name:<28} {v:>10.3}"),
            Measured::BelowFloor => println!("  {name:<28} {:>10}  下限を下回った", "—"),
            Measured::NotWritable => println!("  {name:<28} {:>10}  記法が書けない", "×"),
        }
    }
    println!("{}", "-".repeat(46));
    println!("`—` と `×` は測っていない。<strong>0 ではない。</strong>");
    println!();
    println!("系統の距離は出していない。カセットが無いと語彙が決まらないためである。");
    Exit::Pass
}

/// 測れる指標。<strong>使う側は一覧を持たない</strong>ので、ここに置くのは呼び出しの束である。
fn measured(doc: &kakiburi_doc::Document) -> Vec<(&'static str, Measured)> {
    let p = doc.prose();
    vec![
        ("全角括弧", symbol::full_width_paren(&p)),
        ("半角括弧", symbol::half_width_paren(&p)),
        ("鉤括弧", symbol::corner_bracket(&p)),
        ("感嘆符", symbol::exclamation(&p)),
        ("疑問符", symbol::question(&p)),
        ("三点リーダ", symbol::ellipsis(&p)),
        ("三点リーダの字数", symbol::ellipsis_doubled(&p)),
        ("中黒", symbol::middle_dot(&p)),
        ("波ダッシュ", symbol::wave_dash(&p)),
        ("数字の字幅", symbol::digit_width(&p)),
        ("感嘆符の字幅", symbol::exclamation_width(&p)),
        ("疑問符の字幅", symbol::question_width(&p)),
        ("和欧間スペース欠落", symbol::missing_space(&p)),
        ("笑い", symbol::laughter(&p)),
        ("絵文字", symbol::emoji(&p)),
        ("em dash", symbol::em_dash(&p)),
        ("見出し", structure::headings(doc)),
        ("深い見出し", structure::deep_headings(doc)),
        ("箇条書き", structure::bullets(doc)),
        ("番号リスト", structure::ordered_lists(doc)),
        ("表", structure::tables(doc)),
        ("引用", structure::quotes(doc)),
        ("補足", structure::notes(doc)),
        ("警告", structure::warnings(doc)),
        ("折りたたみ", structure::details(doc)),
        ("強調", structure::emphasis(doc)),
        ("コードブロック", structure::code_blocks(doc)),
        (
            "1 文だけの段落の割合",
            structure::single_sentence_paragraphs(doc),
        ),
        ("段落あたりの文数", structure::sentences_per_paragraph(doc)),
        ("太字始まりの項目", structure::bold_leading_items(doc)),
        ("段落長の変動係数", structure::paragraph_length_cv(doc)),
        ("箇条書き項目長の変動係数", structure::item_length_cv(doc)),
        ("節の長さの変動係数", structure::section_length_cv(doc)),
    ]
}

/// 登録簿の一覧。
fn metrics(_args: &[String]) -> Exit {
    println!("実装した指示できる指標 {} 本", measured_names().len());
    for n in measured_names() {
        println!("  {n}");
    }
    println!();
    println!(
        "照合の系統 {} 本",
        kakiburi_metrics::matching::FOR_VERDICT.len()
    );
    for s in kakiburi_metrics::matching::FOR_VERDICT {
        // <strong>部分ベクトルごとに書く。</strong> 足して 1 つにすると、部分ごとに割っている
        // ことが見えなくなる。
        let parts: Vec<String> = kakiburi_metrics::matching::limits(s)
            .iter()
            .map(|l| l.map_or_else(|| "素材で決まる".to_owned(), |n| format!("上限 {n}")))
            .collect();
        println!("  {:<12} {}", s.name(), parts.join(" + "));
    }
    println!();
    println!(
        "人らしさ {} 指標 / {} 次元",
        kakiburi_metrics::humanness::Metric::ALL.len(),
        kakiburi_scale::humanness::dims().len()
    );
    for m in kakiburi_metrics::humanness::Metric::ALL {
        println!("  {:<12} {} 次元", m.name(), m.dims().len());
    }
    println!();
    println!("登録簿の全体は docs/spec/metrics/ にある。");
    println!("<strong>形態素解析を要る系統と指標は、UniDic を指したときだけ測る</strong>——");
    println!(
        "{} に辞書の経路を渡す。指さなければ測らない（0 を返さない）。",
        analyzer::DICDIR
    );
    Exit::Pass
}

fn measured_names() -> Vec<&'static str> {
    let empty = kakiburi_doc::Document::new(vec![]);
    measured(&empty).into_iter().map(|(n, _)| n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 引数が無ければ助けを出す() {
        assert_eq!(run(&[]), Exit::Pass);
    }

    #[test]
    fn 知らないコマンドは使い方の誤りである() {
        assert_eq!(run(&["ためす".to_owned()]), Exit::Usage);
    }

    #[test]
    fn ファイルを渡さなければ使い方の誤りである() {
        assert_eq!(run(&["measure".to_owned()]), Exit::Usage);
    }

    #[test]
    fn 対応表に無い取り込み元は使い方の誤りである() {
        let args = [
            "measure".to_owned(),
            "/dev/null".to_owned(),
            "--source".to_owned(),
            "rst".to_owned(),
        ];
        assert_eq!(run(&args), Exit::Usage);
    }

    #[test]
    fn 読めないファイルは_65_である() {
        let args = ["measure".to_owned(), "/存在しない経路/x.md".to_owned()];
        assert_eq!(run(&args), Exit::Unreadable);
    }

    #[test]
    fn 指標の一覧が出る() {
        assert_eq!(run(&["metrics".to_owned()]), Exit::Pass);
        assert_eq!(measured_names().len(), 33);
    }

    #[test]
    fn 一覧に重なりが無い() {
        // 使う側が一覧を持たないので、ここが 2 度出れば 2 度測ることになる。
        let mut names = measured_names();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), before);
    }

    /// 目盛りの入ったカセットを置く。<strong>作るのは目盛りの側である。</strong>
    fn cassette_with_scale(dir: &std::path::Path) -> String {
        let scale = fixture::scale();
        let (person, baseline) = fixture::corpus();
        let units: Vec<Unit> = person
            .iter()
            .map(|(n, d)| Unit {
                name: n.clone(),
                role: Role::Person,
                document: d.clone(),
            })
            .chain(baseline.iter().map(|(n, d)| Unit {
                name: n.clone(),
                role: Role::BaselineOutput,
                document: d.clone(),
            }))
            .collect();
        let mut c = Cassette {
            version: 1,
            scene: "試験".into(),
            fingerprint: current_fingerprint(),
            provisional: vec![],
            decided: Decided {
                scene: "試験".into(),
                boilerplate: vec![],
                baseline: Baseline {
                    model: String::new(),
                    version: String::new(),
                    params: BTreeMap::new(),
                    topics: vec![],
                },
                movement: BTreeMap::new(),
            },
            corpus: Corpus::new(units),
            derived: Derived::dropped(),
        };
        c.derived.scale = Some(scale_json::write(&scale));
        // <strong>指紋も目盛りに合わせる。</strong> 合わせなければ、検めが使う前に断る。
        c.fingerprint = fingerprint_with(&c, Some(&scale), analyzer::resolve().as_ref());
        let path = dir.join("scale.kbc");
        std::fs::write(&path, store::write(&c)).expect("書ける");
        path.to_string_lossy().into_owned()
    }

    /// 試験のための一時の置き場。
    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("kakiburi-試験-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("作れる");
        dir
    }

    #[test]
    fn 目盛りを読んで検める() {
        // <strong>検めが目盛りを受け取るところまで通す。</strong> 読み戻し・相手集合の解決・
        // 測り・判定・指摘が、実際の経路で繋がっていることを確かめる。
        let dir = temp_dir("review");
        let cassette = cassette_with_scale(&dir);
        let target = dir.join("検める.md");
        std::fs::write(&target, "これは、そうだ、と思う。\n").expect("書ける");

        let args = [
            "review".to_owned(),
            target.to_string_lossy().into_owned(),
            "--cassette".to_owned(),
            cassette,
        ];
        // 短い 1 本なので除外に掛かる。<strong>0 ではなく「測れていない」が返る</strong>ので、
        // 1 段目で止まって判定できないになる。
        assert_eq!(run(&args), Exit::Unknown);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 相手集合が揃わなければ使う前の問題である() {
        // 本文が入れ替わったカセットで測れば、比べたものに意味が無い。
        //
        // <strong>指紋は corpus を含まないので、ここが変わるのは相手集合の照合だけである。</strong>
        // 同じカセットが[落とす前は判定できないを返す](目盛りを読んで検める)ことと
        // 合わせて、通った経路が特定できる。
        let dir = temp_dir("partners");
        let cassette = cassette_with_scale(&dir);
        let raw = std::fs::read(&cassette).expect("読める");
        let mut c = store::read(&raw).expect("読める");
        // 相手集合の 1 本を落とす。
        let mut units = c.corpus.units.clone();
        units.retain(|u| u.name != "p00");
        c.corpus = Corpus::new(units);
        std::fs::write(&cassette, store::write(&c)).expect("書ける");

        let target = dir.join("検める.md");
        std::fs::write(&target, "これは、そうだ、と思う。\n").expect("書ける");
        let args = [
            "review".to_owned(),
            target.to_string_lossy().into_owned(),
            "--cassette".to_owned(),
            cassette,
        ];
        assert_eq!(run(&args), Exit::FingerprintMismatch);
        std::fs::remove_dir_all(&dir).ok();
    }
}
