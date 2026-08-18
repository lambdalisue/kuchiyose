//! kakiburi。人が触る面。
//!
//! <strong>`review` だけが周回に出てくる。</strong> ほかは素材が増えたときにしか動かさない。

mod analyzer;
mod effective_json;
mod exit;
#[cfg(test)]
mod fixture;
mod remedies;
mod scale_json;

use exit::Exit;
use kakiburi_cassette::{
    save, store, Baseline, Cassette, Corpus, Decided, Derived, Fingerprint, Inputs, Normalization,
    Role, Tool, Unit,
};
use kakiburi_metrics::{phrase, structure, symbol, Measured};
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
        Some("replace") => replace(&args[1..]),
        Some("decide") => decide(&args[1..]),
        Some("show") => show(&args[1..]),
        Some("compare") => compare(&args[1..]),
        Some("doctor") => doctor(&args[1..]),
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

  kakiburi add <カセット> <ファイル...> --as person|baseline|other
                                   [--source <取り込み元>] [--id <名前>] [--unit <名前>]
      正規化して入れる。<strong>1 本でも断ったら何も入れない。</strong>
      名前はカセット全体で一意である。<strong>衝突したら断る</strong>——黙って上書きしない。
      <strong>--unit で束ねる</strong>——短い文書を何本かで 1 単位にする。
      <strong>--as baseline には --model と --version が要る</strong>（--param / --topic も取る）。
      本文が変わるので派生物を捨てる。

  kakiburi replace <カセット> <ファイル> --id <名前> [--source <取り込み元>]
      既にある 1 本を差し替える。<strong>無い名前を渡したら断る。</strong>
      足すことと差し替えることを分けるのは、上書きを事故ではなく意思にするため。

  kakiburi decide <カセット> boilerplate <文字列...>
  kakiburi decide <カセット> movement <指標> moves|stuck
      <strong>コーパスから導けないものを書く。</strong> 落とす定型は場面ごとに人が決め、
      指示して動くかは直させてみて初めて分かる。

  kakiburi build <カセット>
      目盛りを作る。<strong>作らずに終わる条件を持つ</strong>——止まっても失敗ではない。

回す——毎周

  kakiburi review <ファイル> --cassette <カセット> [--source <取り込み元>]
      検める。3 値と指摘を返す。
      <strong>目盛りの無いカセットは判定できない（2）を返す</strong>——素材が足りずに作れな
      かったのは正常な状態である。

覗く

  kakiburi compare <ファイル>... [--source <取り込み元>]
      並べて比べる。<strong>系統の距離は出ない</strong>——語彙が無いためである。

  kakiburi show <カセット>
      コーパス全体の分布を、役ごとに出す。

  kakiburi doctor <カセット>
      <strong>自分を検査する。</strong> 本人がいちばん高く出ることが、目盛りが壊れていない
      ことの最低条件である。指紋・派生物・暫定値もあわせて確かめる。

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
        // <strong>置き換えるたびに増える。</strong> 作った時点では 0 で、書けば 1 になる。
        generation: 0,
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
    if let Err(e) = save::save(path, &c, None) {
        eprintln!("書けない: {e}");
        return Exit::Unreadable;
    }
    println!("作った: {path}");
    println!("場面: {}", c.scene);
    Exit::Pass
}

/// カセットを読む。<strong>残った一時ファイルを片づけてから開く。</strong>
///
/// 置き換えの途中まで進んだ zip を放っておくと、ディレクトリに溜まる。
///
/// 読めた世代を一緒に返す——<strong>置き換える直前に、いまの世代と照らす</strong>ためである。
fn open(path: &str) -> Result<(Cassette, u64), Exit> {
    save::sweep(path);
    let Ok(raw) = std::fs::read(path) else {
        eprintln!("カセットが読めない: {path}");
        return Err(Exit::Unreadable);
    };
    match store::read(&raw) {
        Ok(c) => {
            let generation = c.generation;
            Ok((c, generation))
        }
        Err(e) => {
            eprintln!("カセットが読めない: {e}");
            Err(Exit::Unreadable)
        }
    }
}

/// 置き換える。<strong>断られたら、元のカセットは無傷である。</strong>
fn store_back(path: &str, c: &Cassette, generation: u64) -> Result<(), Exit> {
    match save::save(path, c, Some(generation)) {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("書けない: {e}");
            Err(Exit::Unreadable)
        }
    }
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
    // <strong>役に既定を置かない。</strong> 取り違えると、対照にしたはずの文書が書き手の帯に
    // 残る——いちばん高くつく取り違えに既定を与えない。
    let mut role: Option<Role> = None;
    let mut id: Option<String> = None;
    let mut unit: Option<String> = None;
    // 基準の作り方。<strong>`--as baseline` では欠かせない。</strong>
    let mut model: Option<String> = None;
    let mut version: Option<String> = None;
    let mut params: BTreeMap<String, String> = BTreeMap::new();
    let mut topics: Vec<String> = Vec::new();
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
                    Some("person") => Some(Role::Person),
                    Some("baseline") => Some(Role::BaselineOutput),
                    Some("other") => Some(Role::Other),
                    _ => {
                        eprintln!("--as は person / baseline / other");
                        return Exit::Usage;
                    }
                };
                i += 2;
            }
            "--id" => {
                let Some(v) = args.get(i + 1) else {
                    eprintln!("--id に名前を渡す");
                    return Exit::Usage;
                };
                id = Some(v.clone());
                i += 2;
            }
            "--unit" => {
                let Some(v) = args.get(i + 1) else {
                    eprintln!("--unit に名前を渡す");
                    return Exit::Usage;
                };
                unit = Some(v.clone());
                i += 2;
            }
            "--model" => {
                model = args.get(i + 1).cloned();
                i += 2;
            }
            "--version" => {
                version = args.get(i + 1).cloned();
                i += 2;
            }
            "--param" => {
                let Some((k, v)) = args.get(i + 1).and_then(|s| s.split_once('=')) else {
                    eprintln!("--param は <鍵>=<値>");
                    return Exit::Usage;
                };
                params.insert(k.to_owned(), v.to_owned());
                i += 2;
            }
            "--topic" => {
                let Some(v) = args.get(i + 1) else {
                    eprintln!("--topic に題材を渡す");
                    return Exit::Usage;
                };
                topics.push(v.clone());
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
    let Some(role) = role else {
        eprintln!("--as を渡す（person / baseline / other）");
        eprintln!("<strong>既定を置かない。</strong> 役を取り違えると、対照が書き手の帯に残る");
        return Exit::Usage;
    };
    // <strong>`--id` は 1 本だけ渡すときにしか書けない。</strong> 複数に同じ名前は付けられない。
    if id.is_some() && files.len() > 1 {
        eprintln!("--id は 1 本だけ渡すときに使う");
        return Exit::Usage;
    }
    // <strong>基準は版と推論設定まで記録する。</strong> 外で作ったものでも同じである——
    // 記録の無い基準で作った値は、次に測ったときに比べられない。
    if role == Role::BaselineOutput && (model.is_none() || version.is_none()) {
        eprintln!("--as baseline では --model と --version が要る");
        eprintln!("<strong>モデル名だけでは足りない。</strong> 版が変われば出力が変わる");
        return Exit::Usage;
    }

    let (mut c, generation) = match open(path) {
        Ok(v) => v,
        Err(e) => return e,
    };

    if role == Role::BaselineOutput {
        let next = Baseline {
            model: model.unwrap_or_default(),
            version: version.unwrap_or_default(),
            params,
            // <strong>題材は外さない。</strong> 言葉づかいだけで帯が動く。
            topics: if topics.is_empty() {
                c.decided.baseline.topics.clone()
            } else {
                topics
            },
        };
        // <strong>作り方が変われば、前に入れた基準と混ぜられない。</strong> 黙って上書きしない。
        let already = &c.decided.baseline;
        if !already.model.is_empty()
            && (already.model != next.model || already.version != next.version)
        {
            eprintln!(
                "断る: 基準の作り方が違う（{} {} → {} {}）",
                already.model, already.version, next.model, next.version
            );
            eprintln!("<strong>違う作り方の基準を混ぜない。</strong> 別のカセットにする");
            return Exit::Usage;
        }
        c.decided.baseline = next;
    }

    // <strong>まず全部を読む。</strong> 1 本でも断られたら何も入れない——
    // 途中まで入った状態を残すと、欠けたまま次へ進む。
    let mut units = Vec::new();
    for f in &files {
        let name = id.clone().unwrap_or_else(|| stem_of(f));
        let Ok(body) = std::fs::read_to_string(f) else {
            eprintln!("読めない: {f}");
            return Exit::Unreadable;
        };
        match normalize(&body, source) {
            Ok(d) => units.push(Unit {
                // <strong>束ねなければ、測る単位は取り込んだ 1 本と同じである。</strong>
                unit: unit.clone().unwrap_or_else(|| name.clone()),
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

    // <strong>名前の衝突は、書く前に断る。</strong> 黙って上書きすれば、原本が 1 本消えたことは
    // 値が変わるまで誰も気付かない。
    if let Err(why) = check_names(&c, &units) {
        eprintln!("断る: {why}");
        eprintln!("差し替えたいなら replace を使う");
        return Exit::Usage;
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

    if let Err(e) = store_back(path, &c, generation) {
        return e;
    }
    println!("入れた {} 本。役: {}", files.len(), role.dir());
    println!("派生物を捨てた。build し直しが要る");
    Exit::Pass
}

/// 文書を差し替える。
///
/// <strong>`add` と分けるのは、上書きを事故ではなく意思にするためである。</strong> `add` が上書きも
/// できると、同じ名前のファイルを 2 度取り込んだだけで<strong>原本が 1 本消える</strong>。
///
/// <strong>置き換える先をファイル名から当てにいかない。</strong> 取り込み直すときにファイル名が
/// 変わっていることは普通にある——`--id` で名指しする。
fn replace(args: &[String]) -> Exit {
    let Some(path) = args.first() else {
        eprintln!("カセットの経路を渡す");
        return Exit::Usage;
    };
    let mut file: Option<String> = None;
    let mut source = Source::GithubMarkdown;
    let mut role: Option<Role> = None;
    let mut id: Option<String> = None;
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
                    Some("person") => Some(Role::Person),
                    Some("baseline") => Some(Role::BaselineOutput),
                    Some("other") => Some(Role::Other),
                    _ => {
                        eprintln!("--as は person / baseline / other");
                        return Exit::Usage;
                    }
                };
                i += 2;
            }
            "--id" => {
                id = args.get(i + 1).cloned();
                i += 2;
            }
            f => {
                if file.is_some() {
                    eprintln!("1 度に 1 本だけ差し替える");
                    return Exit::Usage;
                }
                file = Some(f.to_owned());
                i += 1;
            }
        }
    }
    let (Some(file), Some(id)) = (file, id) else {
        eprintln!("差し替えるファイルと --id を渡す");
        return Exit::Usage;
    };

    let (mut c, generation) = match open(path) {
        Ok(v) => v,
        Err(e) => return e,
    };

    // <strong>無い `id` を渡したら断る。</strong> `add` の綴り間違いで差し替えたことにしない。
    let Some(old) = c.corpus.units.iter().find(|u| u.name == id) else {
        eprintln!("断る: `{id}` はカセットに無い");
        eprintln!("足すなら add を使う");
        return Exit::Usage;
    };
    // <strong>役を省いたら、いまの役をそのまま使う。</strong> 差し替えは中身の入れ替えであって、
    // 役の変更ではない。
    let role = role.unwrap_or(old.role);

    let Ok(body) = std::fs::read_to_string(&file) else {
        eprintln!("読めない: {file}");
        return Exit::Unreadable;
    };
    let document = match normalize(&body, source) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("断る: {file}: {e}");
            return Exit::Unreadable;
        }
    };

    let units: Vec<Unit> = c
        .corpus
        .units
        .iter()
        .map(|u| {
            if u.name == id {
                Unit {
                    name: id.clone(),
                    // <strong>束は変えない。</strong> 差し替えは中身の入れ替えであって、
                    // どの単位に属するかの変更ではない。
                    unit: u.unit.clone(),
                    role,
                    document: document.clone(),
                }
            } else {
                u.clone()
            }
        })
        .collect();
    c.corpus = Corpus::new(units);
    // <strong>本文が変われば派生物は古い。</strong> 捨てる——指紋は測った条件を表すものなので、
    // 本文を差し替えても変わらない。捨てなければ古い値が有効な顔で読まれる。
    c.drop_derived();
    if let Err(e) = store_back(path, &c, generation) {
        return e;
    }
    println!("差し替えた: {id}（役: {}）", role.dir());
    println!("派生物を捨てた。build し直しが要る");
    Exit::Pass
}

/// コーパス全体の分布を出す。
///
/// <strong>役ごとに分けて出す。</strong> 混ぜれば、本人と基準の差がそこで潰れる。
fn show(args: &[String]) -> Exit {
    let Some(path) = args.first() else {
        eprintln!("カセットの経路を渡す");
        return Exit::Usage;
    };
    let (c, _) = match open(path) {
        Ok(v) => v,
        Err(e) => return e,
    };
    println!("場面: {}", c.scene);
    println!("世代: {}", c.generation);
    if !c.provisional.is_empty() {
        println!("暫定値: {}", c.provisional.join("、"));
    }
    let mecab = analyzer::resolve();
    let a = mecab
        .as_ref()
        .map(|m| m as &dyn kakiburi_metrics::morph::Analyzer);
    for role in [Role::Person, Role::BaselineOutput, Role::Other] {
        let units = stripped(&c, role);
        if units.is_empty() {
            continue;
        }
        println!();
        println!("{} — {} 単位", role.dir(), units.len());
        print_distribution(&rows(&samples(&units), a));
    }
    Exit::Pass
}

/// 指標ごとの分布を出す。<strong>測れた単位の数も添える。</strong>
///
/// 数を出さなければ、幅が狭いのか素材が少ないのかが読めない。
fn print_distribution(rows: &[kakiburi_scale::effective::Row]) {
    let mut seen: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for row in rows {
        for (name, v) in row {
            if let Some(v) = v {
                seen.entry(name.clone()).or_default().push(*v);
            }
        }
    }
    println!("  {:<28} {:>9} {:>9} {:>6}", "指標", "最小", "最大", "単位");
    for (name, vs) in seen {
        let low = vs.iter().copied().fold(f64::INFINITY, f64::min);
        let high = vs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        println!("  {name:<28} {low:>9.3} {high:>9.3} {:>6}", vs.len());
    }
}

/// 並べて比べる。
///
/// <strong>周回ごとの散らばりを見るために、3 本以上を取る。</strong> n 周した A・B・C を並べて
/// 渡す（[周回のあいだの観測](../../../docs/design/100-cassette.md#周回のあいだの観測は外でやる)）。
///
/// <strong>系統の距離は出さない。</strong> カセットが無ければ語彙が決まらず、渡された 2 本から
/// その場で選べば違う軸のベクトルどうしの距離になる。
fn compare(args: &[String]) -> Exit {
    let mut files: Vec<String> = Vec::new();
    let mut source = Source::GithubMarkdown;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--source" {
            let Some(s) = args.get(i + 1).and_then(Source::from_name) else {
                eprintln!("対応表に無い取り込み元");
                return Exit::Usage;
            };
            source = s;
            i += 2;
            continue;
        }
        files.push(args[i].clone());
        i += 1;
    }
    if files.len() < 2 {
        eprintln!("比べるファイルを 2 本以上渡す");
        return Exit::Usage;
    }

    let mecab = analyzer::resolve();
    let a = mecab
        .as_ref()
        .map(|m| m as &dyn kakiburi_metrics::morph::Analyzer);
    let mut columns: Vec<(String, Vec<(String, Measured)>)> = Vec::new();
    for f in &files {
        let Ok(body) = std::fs::read_to_string(f) else {
            eprintln!("読めない: {f}");
            return Exit::Unreadable;
        };
        let doc = match normalize(&body, source) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("断る: {f}: {e}");
                return Exit::Unreadable;
            }
        };
        let prose = doc.prose();
        let analyzed = analyzed_of(&prose, a);
        columns.push((stem_of(f), measured_with(&doc, analyzed.as_ref())));
    }

    print!("{:<28}", "指標");
    for (name, _) in &columns {
        print!("{:>12}", cut(name, 11));
    }
    println!();
    println!("{}", "-".repeat(28 + 12 * columns.len()));
    for (name, _) in &columns[0].1.clone() {
        print!("{name:<28}");
        for (_, ms) in &columns {
            let m = ms.iter().find(|(n, _)| n == name).map(|(_, m)| *m);
            match m.and_then(Measured::value) {
                Some(v) => print!("{v:>12.3}"),
                None => print!("{:>12}", "—"),
            }
        }
        println!();
    }
    println!("{}", "-".repeat(28 + 12 * columns.len()));
    println!("`—` は測っていない。<strong>0 ではない。</strong>");
    println!(
        "<strong>系統の距離は出していない。</strong> カセットが無いと語彙が決まらないためである"
    );
    Exit::Pass
}

/// 表示のために縮める。
fn cut(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// 自分を検査する。
///
/// <strong>本人がいちばん高く出ることが、目盛りが壊れていないことの最低条件である。</strong>
/// 生成文より低く出る指標があれば、測っているのは著者性ではなく指示追従である。
fn doctor(args: &[String]) -> Exit {
    let Some(path) = args.first() else {
        eprintln!("カセットの経路を渡す");
        return Exit::Usage;
    };
    let (c, _) = match open(path) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let mut bad = 0usize;

    // 1. 指紋が現在の環境と合っているか。
    match check_fingerprint(&c) {
        Ok(()) => println!("指紋: 環境と合っている"),
        Err(diff) => {
            println!("指紋: <strong>合わない</strong>（{}）", diff.join("、"));
            bad += 1;
        }
    }

    // 2. 派生物が原本と整合しているか。
    let has_scale = c.derived.scale.is_some();
    let has_effective = c.derived.effective.is_some() && c.derived.spread.is_some();
    println!(
        "派生物: 目盛り {} / 効くかの判定 {}",
        if has_scale { "あり" } else { "無し" },
        if has_effective { "あり" } else { "無し" }
    );
    if has_scale != has_effective {
        println!("  <strong>片方だけある。</strong> build し直しが要る");
        bad += 1;
    }

    // 3. 暫定値が立っていないか。
    if c.provisional.is_empty() {
        println!("暫定値: 立っていない");
    } else {
        println!("暫定値: {}", c.provisional.join("、"));
        println!("  判定に但し書きが付く");
    }

    // 4. 本人がいちばん高く出るか。
    let Some(scale) = c.derived.scale.as_deref().and_then(scale_json::read) else {
        println!();
        println!("目盛りが無いので、本人の側が高く出るかは確かめられない");
        return if bad == 0 { Exit::Pass } else { Exit::Unknown };
    };
    let person = stripped(&c, Role::Person);
    let person = samples(&person);
    let partners: Vec<Sample<'_>> = person
        .iter()
        .filter(|s| scale.partners.iter().any(|n| n == s.name))
        .copied()
        .collect();
    let mecab = analyzer::resolve();
    let a = mecab
        .as_ref()
        .map(|m| m as &dyn kakiburi_metrics::morph::Analyzer);
    let side = |samples: &[Sample<'_>]| -> Vec<f64> {
        samples
            .iter()
            .filter(|s| !partners.iter().any(|p| p.name == s.name))
            .filter_map(|s| measure_against(&scale, *s, &partners, a).matching)
            .collect()
    };
    let baseline_units = stripped(&c, Role::BaselineOutput);
    let mine = side(&person);
    let theirs = side(&samples(&baseline_units));
    println!();
    if mine.is_empty() || theirs.is_empty() {
        println!("照合値を出せる単位が足りない");
        return if bad == 0 { Exit::Pass } else { Exit::Unknown };
    }
    let lowest = mine.iter().copied().fold(f64::INFINITY, f64::min);
    let highest = theirs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    println!("本人の最小: {lowest:.3} / 基準の最大: {highest:.3}");
    if lowest > highest {
        println!("<strong>本人がいちばん高く出ている。</strong> 目盛りは壊れていない");
    } else {
        println!("<strong>本人より高く出る基準がある。</strong> 目盛りを疑う");
        println!("  測っているのは著者性ではなく指示追従かもしれない");
        bad += 1;
    }
    if bad == 0 {
        Exit::Pass
    } else {
        Exit::Unknown
    }
}

/// 人が決めたことを書く。
///
/// <strong>コーパスから導けないものだけがここに来る。</strong> 落とす定型は場面ごとに人が決める
/// ものであり、指示して動くかは直させてみて初めて分かる。
fn decide(args: &[String]) -> Exit {
    let Some(path) = args.first() else {
        eprintln!("カセットの経路を渡す");
        return Exit::Usage;
    };
    match args.get(1).map(String::as_str) {
        Some("boilerplate") => decide_boilerplate(path, &args[2..]),
        Some("movement") => decide_movement(path, &args[2..]),
        _ => {
            eprintln!("decide boilerplate <文字列...> / decide movement <指標> moves|stuck");
            Exit::Usage
        }
    }
}

/// 落とす定型を決める。<strong>渡した一覧で置き換える。</strong>
///
/// 足すのではなく置き換えるのは、<strong>いま何を落としているかが 1 度で読める</strong>ようにする
/// ためである。積み上げると、消すのに別の操作が要る。
fn decide_boilerplate(path: &str, words: &[String]) -> Exit {
    let (mut c, generation) = match open(path) {
        Ok(v) => v,
        Err(e) => return e,
    };
    c.decided.boilerplate = words.to_vec();
    // <strong>落とす範囲が変われば値が変わる。</strong> 派生物を捨てる。
    c.drop_derived();
    if let Err(e) = store_back(path, &c, generation) {
        return e;
    }
    if words.is_empty() {
        println!("落とす定型を空にした");
    } else {
        println!("落とす定型 {} 本にした", words.len());
        for w in words {
            println!("  {w}");
        }
    }
    println!("派生物を捨てた。build し直しが要る");
    Exit::Pass
}

/// 指示して動くかを決める。<strong>[戻る線 1 本目](../../../docs/spec/010-strategy.md#運用に入ると戻る線が-2-本できる)の入口である。</strong>
///
/// <strong>照合値が動かなかったことを根拠にしない。</strong> 照合値は 1 つの切り口には鈍く、
/// 指摘が正しく通じても動かないことがある。混ぜれば、効いている指標を `stuck` にして
/// 捨てる。
fn decide_movement(path: &str, args: &[String]) -> Exit {
    let (Some(metric), Some(state)) = (args.first(), args.get(1)) else {
        eprintln!("decide movement <指標> moves|stuck");
        return Exit::Usage;
    };
    let state = match state.as_str() {
        "moves" => kakiburi_cassette::Movement::Moves,
        "stuck" => kakiburi_cassette::Movement::Stuck,
        _ => {
            eprintln!("moves か stuck を渡す");
            return Exit::Usage;
        }
    };

    // <strong>登録簿に無い名前は受けない。</strong> 綴りを間違えたまま書けば、直したつもりの
    // 指標がいつまでも指摘に出続ける——エラーにならないので気付けない。
    if !measured_names().iter().any(|n| n == metric) {
        eprintln!("知らない指標: {metric}");
        eprintln!("metrics で一覧を出せる");
        return Exit::Usage;
    }

    let (mut c, generation) = match open(path) {
        Ok(v) => v,
        Err(e) => return e,
    };
    c.decided.movement.insert(metric.clone(), state);

    // <strong>前に出す指標は movement から導く派生物である。</strong> 書き換えたのに作り直さな
    // ければ、`stuck` にした指標が指摘に出続ける。
    //
    // 値も目盛りも movement では変わらないので、<strong>作り直すのはここだけである。</strong>
    let dropped = c.derived.effective.is_some();
    c.derived.effective = None;
    c.derived.spread = None;
    if let Err(e) = store_back(path, &c, generation) {
        return e;
    }
    println!(
        "{metric}: {}",
        match state {
            kakiburi_cassette::Movement::Moves => "動く",
            kakiburi_cassette::Movement::Stuck => "動かない",
        }
    );
    if dropped {
        println!("効くかの判定を捨てた。build し直しが要る");
    }
    Exit::Pass
}

/// ファイル名から拡張子を外したもの。<strong>`--id` を省いたときの名前である。</strong>
fn stem_of(f: &str) -> String {
    std::path::Path::new(f)
        .file_stem()
        .map_or_else(|| f.to_owned(), |s| s.to_string_lossy().into_owned())
}

/// 入れようとしている単位の名前を検める。
///
/// <strong>名前はカセット全体で一意である。</strong> 役で名前空間を分けない——分ければ、
/// 本人の `Rust入門` と基準の `Rust入門` が別物として通る。
/// [題材を揃える](../../../docs/spec/200-extract.md#題材の統制は対ではなく素材に効かせる)ほど
/// 同じ名前が付きやすいので、<strong>いちばん正しく集めた人がいちばん踏む。</strong>
fn check_names(c: &Cassette, adding: &[Unit]) -> Result<(), String> {
    let mut ids: BTreeMap<&str, Role> = c
        .corpus
        .units
        .iter()
        .map(|u| (u.name.as_str(), u.role))
        .collect();
    for u in adding {
        if let Some(role) = ids.insert(u.name.as_str(), u.role) {
            return Err(if role == u.role {
                format!("`{}` は既にある", u.name)
            } else {
                format!("`{}` は役 {} で既にある", u.name, role.dir())
            });
        }
    }

    // <strong>束ねた文書は同じ `unit` を共有する。</strong> だから単純な重複拒否にはできない。
    // <strong>だが役を跨いだ共有は断る</strong>——1 つの単位が本人でも基準でもあることになる。
    let mut units: BTreeMap<&str, Role> = BTreeMap::new();
    for u in c.corpus.units.iter().chain(adding) {
        if let Some(role) = units.insert(u.unit.as_str(), u.role) {
            if role != u.role {
                return Err(format!(
                    "単位 `{}` が役 {} と {} に跨っている",
                    u.unit,
                    role.dir(),
                    u.role.dir()
                ));
            }
        }
    }
    Ok(())
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
    let (mut c, generation) = match open(path) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let person_units = stripped(&c, Role::Person);
    let baseline_units = stripped(&c, Role::BaselineOutput);
    let person = samples(&person_units);
    let baseline = samples(&baseline_units);
    println!("本人 {} 単位 / 基準 {} 単位", person.len(), baseline.len());
    if !c.decided.boilerplate.is_empty() {
        println!("落とす定型 {} 本", c.decided.boilerplate.len());
    }

    let mecab = analyzer::resolve();
    match &mecab {
        Some(m) => println!("形態素解析: MeCab / {} {}", m.dict_name, m.dict_version),
        None => println!(
            "形態素解析: 無し（{} が未設定）。<strong>5 系統のうち 2 つが測れない</strong>",
            analyzer::DICDIR
        ),
    }

    // <strong>環境の側の理由で測れないものがあれば、目盛りを作らない。</strong>
    // 直すのはコーパスではなく環境であり、直せば全部の値が変わる——このまま進めば、
    // 壊れた環境で出た値が正常な顔でカセットに入る。
    let broken = broken_environment(
        &person,
        &baseline,
        mecab
            .as_ref()
            .map(|m| m as &dyn kakiburi_metrics::morph::Analyzer),
    );
    if !broken.is_empty() {
        println!("目盛りを作らない: 環境の側で測れない指標がある");
        for (name, why) in &broken {
            println!("  {name}: {why}");
        }
        println!("<strong>素材ではなく環境を直す。</strong> 足しても直らない");
        let fingerprint = fingerprint_with(&c, None, mecab.as_ref());
        stop_without_scale(path, &mut c, fingerprint, generation);
        return Exit::Pass;
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
            stop_without_scale(path, &mut c, fingerprint, generation);
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

    // <strong>効くかの判定はここで出す。</strong> 検めが作り直せる形にしておくと、検める文書を
    // 見てから幅や集合を作り直す経路が書けてしまう。
    let a = mecab
        .as_ref()
        .map(|m| m as &dyn kakiburi_metrics::morph::Analyzer);
    let effective = kakiburi_scale::effective::judge(&rows(&person, a), &rows(&baseline, a));
    let works = effective.iter().filter(|e| e.works()).count();
    println!("効く指標: {works} / {} 本", effective.len());

    // <strong>作り終えた目盛りだけを入れる。</strong> 検めはこれを受け取る。
    c.derived = Derived {
        vocabulary: Some(vocabulary_note(&scale)),
        values: None,
        spread: Some(effective_json::write_spread(&effective)),
        calibration: Some(format!("系統 {} 本の較正と合算", scale.frozen.len())),
        scale: Some(scale_json::write(&scale)),
        effective: Some(effective_json::write_effective(&effective)),
    };
    // <strong>指紋を作り直す。</strong> 語彙と z 得点と道具が値を決めるので、目盛りができた時点で
    // 指紋も変わる——変えなければ、次に検めるときに合わないことが分からない。
    c.fingerprint = fingerprint_with(&c, Some(&scale), mecab.as_ref());
    if let Err(e) = store_back(path, &c, generation) {
        return e;
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

/// カセットの単位から<strong>定型を落とした写し</strong>を作る。
///
/// <strong>原本は変えない。</strong> 定型は[人が決めたこと](../../../docs/spec/200-extract.md#定型を落とす)
/// であって本文ではないので、決め直したら測り直せる形にしておく。カセットへ
/// 書き戻すのは落とす前の本文である。
fn stripped(c: &Cassette, role: Role) -> Vec<(String, kakiburi_doc::Document)> {
    // <strong>束ねてから落とす。</strong> 測るのは単位であって、取り込んだ 1 本ではない。
    c.bundles(role)
        .into_iter()
        .map(|(unit, doc)| (unit, doc.without_boilerplate(&c.decided.boilerplate)))
        .collect()
}

/// 素材の形にする。
fn samples(units: &[(String, kakiburi_doc::Document)]) -> Vec<Sample<'_>> {
    units
        .iter()
        .map(|(name, document)| Sample { name, document })
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

/// 環境の側の理由で測れない指標。<strong>あれば目盛りを作らない。</strong>
///
/// コーパスの性質（下限未満・分母 0・書けない記法）は素材や取り込み元を替えれば
/// 直るが、<strong>道具が無い・道具が失敗したは環境の壊れである</strong>。分母から外して済ませると、
/// 辞書を入れ忘れたまま <strong>まともな値が出ているように見える</strong>。
///
/// 名前ごとに 1 度だけ挙げる。全単位で同じ理由が並ぶので、繰り返しても読めない。
fn broken_environment(
    person: &[Sample<'_>],
    baseline: &[Sample<'_>],
    analyzer: Option<&dyn kakiburi_metrics::morph::Analyzer>,
) -> Vec<(String, String)> {
    let mut out: BTreeMap<String, kakiburi_metrics::Unmeasured> = BTreeMap::new();
    let mut note = |name: &str, m: kakiburi_metrics::Measured| {
        let Some(u) = m.unmeasured().filter(|u| !u.is_corpus()) else {
            return;
        };
        // <strong>いちばん手の限られている理由を残す。</strong>
        out.entry(name.to_owned())
            .and_modify(|e| *e = (*e).min(u))
            .or_insert(u);
    };
    for s in person.iter().chain(baseline) {
        // <strong>人らしさの側も見る。</strong> 解析器と圧縮器を使うのはこちらなので、
        // 環境の壊れはここに出る。
        let prose = s.document.prose();
        // <strong>解析に失敗したら、解析器が無いのと同じにしない。</strong> `None` を渡すと
        // 「道具が無い」になるが、実際は道具が返さなかった——別の理由である。
        let analyzed = analyzer.and_then(|a| kakiburi_metrics::morph::Analyzed::of(&prose, a).ok());
        if analyzer.is_some() && analyzed.is_none() {
            note("形態素解析", kakiburi_metrics::Measured::ToolFailed);
        }
        for (name, m) in kakiburi_metrics::Humanness::measure(&prose, analyzed.as_ref()).flat() {
            note(&name, m);
        }
        for (name, m) in measured_with(s.document, analyzed.as_ref()) {
            note(&name, m);
        }
    }
    out.into_iter()
        .map(|(name, u)| (name, u.name().to_owned()))
        .collect()
}

/// 単位ごとの、指示できる指標の値。<strong>効くかの判定に渡す形である。</strong>
///
/// <strong>相手集合の 5 本ではなく、その役の全単位から取る。</strong> 帯に使わない単位も値と幅には
/// 使う。幅そのものの決め方は[目盛りの側](kakiburi_scale::effective)が持つ——
/// <strong>ここで作れば、検めも作れることになる。</strong>
fn rows(
    samples: &[Sample<'_>],
    analyzer: Option<&dyn kakiburi_metrics::morph::Analyzer>,
) -> Vec<kakiburi_scale::effective::Row> {
    samples
        .iter()
        .map(|s| {
            let prose = s.document.prose();
            let analyzed = analyzed_of(&prose, analyzer);
            measured_with(s.document, analyzed.as_ref())
                .into_iter()
                .map(|(name, m)| (name, m.value()))
                .collect()
        })
        .collect()
}

/// 解析し終えた形。<strong>解析器が無ければ `None`。</strong>
fn analyzed_of(
    prose: &[kakiburi_doc::prose::Segment],
    analyzer: Option<&dyn kakiburi_metrics::morph::Analyzer>,
) -> Option<kakiburi_metrics::morph::Analyzed> {
    analyzer.and_then(|a| kakiburi_metrics::morph::Analyzed::of(prose, a).ok())
}

/// 目盛りを作らずに終える。<strong>失敗ではない。</strong>
fn stop_without_scale(path: &str, c: &mut Cassette, fingerprint: Fingerprint, generation: u64) {
    c.drop_derived();
    c.fingerprint = fingerprint;
    if let Err(e) = save::save(path, c, Some(generation)) {
        eprintln!("書けない: {e}");
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

    // <strong>検める側にも同じ定型を掛ける。</strong> 片方だけに掛ければ、落とした分だけ値が
    // ずれたものを比べることになる（[同じ測り方で測る](../../../docs/spec/300-revise.md#同じ測り方で測る)）。
    let doc = doc.without_boilerplate(&c.decided.boilerplate);

    // <strong>相手集合は目盛りが名指ししたものである。</strong> 検める側が選び直さない。
    let person_units = stripped(&c, Role::Person);
    let person = samples(&person_units);
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

    // <strong>幅も効くかの判定も、目盛りが持っているものを読むだけである。</strong>
    // 検める時点で作り直さない——作り直せるなら、検める文書を見てから作り直す
    // 経路が書ける（[分ける基準](../../../docs/design/000-architecture.md#分ける基準)）。
    let Some(effective) = c
        .derived
        .spread
        .as_deref()
        .zip(c.derived.effective.as_deref())
        .and_then(|(s, e)| effective_json::read(s, e))
    else {
        // <strong>空と欠けを分ける。</strong> 判定がまだ行われていないカセットで「効く指標が
        // 1 本も無い」と読んではいけない。
        let outcome = judge(None, None, &[]);
        println!("判定: 判定できない");
        println!("止まった段: {}", outcome.stage.name());
        println!("理由: 効くかの判定が入っていない。build し直しが要る");
        return Exit::from_verdict(outcome.verdict);
    };
    let defs = remedies::FromDefinitions::load();
    if defs.is_empty() {
        // <strong>黙って指摘を落とさない。</strong> 直し方の出どころが無ければ、判定は出ても
        // 指摘が 1 本も出ない——それを「幅の中だった」と読まれてはいけない。
        println!("但し書き: 定義ファイルが見つからない。指摘の文を引けない");
    } else {
        println!("直し方の出どころ: 定義ファイル {} 本", defs.len());
    }
    // <strong>前に出す指標。</strong> 効くと判定されたものから、層 3 と動かないものを除く
    // （[3 段](../../../docs/spec/300-revise.md#3-種類を合わせて通るを出す)）。
    // <strong>判定も指摘も、この同じ集合から取る。</strong>
    // <strong>検める側も同じ解析器で測る。</strong> 片方だけ違えば、比べたものに意味が無い。
    let measured_now = measured_with(
        &doc,
        analyzed_of(
            &doc.prose(),
            mecab
                .as_ref()
                .map(|m| m as &dyn kakiburi_metrics::morph::Analyzer),
        )
        .as_ref(),
    );
    let directives: Vec<Observed> = effective
        .iter()
        // 条件 1 と 2。
        .filter(|e| e.works())
        // 条件 3。<strong>動かないと分かった指標は前に出さない。</strong>
        .filter(|e| !c.is_stuck(&e.name))
        // <strong>層 3 は指摘にも判定にも使わない。</strong> 止めた理由を言えないものは止めない。
        .filter(|e| !defs.is_layer_three(&e.name))
        .filter_map(|e| {
            let (_, m) = measured_now.iter().find(|(n, _)| *n == e.name)?;
            Some(Observed {
                name: e.name.clone(),
                value: m.value(),
                range: Range {
                    low: e.low,
                    high: e.high,
                    units: e.units,
                },
                lower: defs.lower_rule(&e.name, e.rate),
            })
        })
        .collect();
    println!("前に出す指標: {} 本", directives.len());

    let result = kakiburi_review::review(humanness, matching, &directives, &defs);
    println!();
    println!("人らしさ値: {}", shown(got.humanness));
    if !got.missing_humanness.is_empty() {
        println!("  測れていない次元: {}", got.missing_humanness.join("、"));
    }
    println!("照合値: {}", shown(got.matching));
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
fn shown(v: Option<f64>) -> String {
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
    // <strong>基準の作り方は `decided` が正本である。</strong> 指紋に写した値ではなく、人が決めた
    // ほうを読む——写しを読むと、決め直したのに指紋が動かない。
    inputs.baseline = c.decided.baseline.clone();
    inputs.decided = c.fingerprint.inputs.decided.clone();
    Fingerprint::build(inputs)
}

/// 外部の表の版。<strong>名前だけでは足りない。</strong>
///
/// 版が上がれば区画や推奨列が増え、同じ本文から違う値が出る。
fn external_tables() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    out.insert(
        "Unicode".to_owned(),
        kakiburi_doc::text::UNICODE_VERSION.to_owned(),
    );
    // <strong>暫定の表も、暫定と書いて残す。</strong> 正規の表に替えたら値が変わる。
    out.insert(
        "絵文字の表".to_owned(),
        kakiburi_metrics::symbol::EMOJI_RANGES_VERSION.to_owned(),
    );
    // <strong>使わない表も、使わないと書いて残す。</strong> 空にすると、あとで足したときに
    // 「もともと無かった」のか「混ぜ忘れた」のかが分からない。
    out.insert("語の文体値の表".to_owned(), "使わない".to_owned());
    out.insert("文末表現の辞書".to_owned(), "使わない".to_owned());
    out
}

/// 適用した対応表。<strong>版だけでは足りない。</strong>
///
/// 升目の中身を変えても版を上げ忘れれば、指紋が同じまま別の木が出る。
fn normalization_mapping() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for source in kakiburi_normalize::Source::all() {
        out.insert(source.name().to_owned(), source.mapping_digest());
    }
    out
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
        // <strong>本数を指紋にしない。</strong> 同じ本数のまま数え方・除外・直し方を変えれば、
        // 値の意味が変わったのに指紋が動かず、古い派生値が使い回される。
        metric_definitions: remedies::FromDefinitions::load().digest(),
        unit_definitions: format!(
            "kakiburi-doc {} / Unicode {}",
            env!("CARGO_PKG_VERSION"),
            kakiburi_doc::text::UNICODE_VERSION,
        ),
        vocabulary,
        z_scores,
        morphology: analyzer::tool(mecab),
        // <strong>まだ使わないものも、使わないと書いて渡す。</strong>
        dependency: Tool::unused(),
        compressor: analyzer::compressor(),
        external_tables: external_tables(),
        normalization: Normalization {
            sources: vec![],
            implementation: "kakiburi-normalize".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            mapping: normalization_mapping(),
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
        match m.unmeasured() {
            None => println!("  {name:<28} {:>10.3}", m.value().unwrap_or_default()),
            // <strong>理由をそのまま出す。</strong> まとめて「測れない」と出せば、辞書を入れ忘れた
            // 環境が「素材が足りない」という顔で回り続ける。
            Some(u) => println!("  {name:<28} {:>10}  {}", "—", u.name()),
        }
    }
    println!("{}", "-".repeat(46));
    println!("`—` は測っていない。<strong>0 ではない。</strong>");
    println!("理由が「道具が無い」「道具が失敗した」なら、<strong>直すのは素材ではなく環境である。</strong>");
    println!();
    println!("系統の距離は出していない。カセットが無いと語彙が決まらないためである。");
    Exit::Pass
}

/// 測れる指標。<strong>使う側は一覧を持たない</strong>ので、ここに置くのは呼び出しの束である。
fn measured(doc: &kakiburi_doc::Document) -> Vec<(String, Measured)> {
    measured_with(doc, None)
}

/// 測れる指標。<strong>解析器を要るものも含める。</strong>
///
/// 解析器が無ければ、要る軸は[道具が無い](kakiburi_metrics::Measured::ToolMissing)になる
/// ——<strong>0 を返さない</strong>し、名前も落とさない。落とせば、書き手ごとに軸の数が変わる。
fn measured_with(
    doc: &kakiburi_doc::Document,
    analyzed: Option<&kakiburi_metrics::morph::Analyzed>,
) -> Vec<(String, Measured)> {
    let p = doc.prose();
    let mut out: Vec<(String, Measured)> = fixed(doc, &p)
        .into_iter()
        .map(|(n, m)| (n.to_owned(), m))
        .collect();

    // <strong>1 つの定義が 24 本の軸に展開される。</strong> 名前は定義が作る——実装が作れば、
    // 名前が 2 か所に現れる。
    match analyzed.map(kakiburi_metrics::word::conjunction_comma) {
        Some(got) => out.extend(got),
        None => out.extend(
            kakiburi_metrics::word::conjunction_comma_names()
                .into_iter()
                .map(|n| (n, Measured::ToolMissing)),
        ),
    }
    out
}

/// 解析器を要らない指標。
fn fixed(
    doc: &kakiburi_doc::Document,
    p: &[kakiburi_doc::prose::Segment],
) -> Vec<(&'static str, Measured)> {
    vec![
        ("全角括弧", symbol::full_width_paren(p)),
        ("半角括弧", symbol::half_width_paren(p)),
        ("鉤括弧", symbol::corner_bracket(p)),
        ("感嘆符", symbol::exclamation(p)),
        ("疑問符", symbol::question(p)),
        ("三点リーダ", symbol::ellipsis(p)),
        ("三点リーダの字数", symbol::ellipsis_doubled(p)),
        ("中黒", symbol::middle_dot(p)),
        ("波ダッシュ", symbol::wave_dash(p)),
        ("数字の字幅", symbol::digit_width(p)),
        ("感嘆符の字幅", symbol::exclamation_width(p)),
        ("疑問符の字幅", symbol::question_width(p)),
        ("和欧間スペース欠落", symbol::missing_space(p)),
        ("笑い", symbol::laughter(p)),
        ("絵文字", symbol::emoji(p)),
        ("em dash", symbol::em_dash(p)),
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
        // 手で選んだ語句で数えるもの。<strong>形態素解析を要らない。</strong>
        ("非断定の密度", phrase::hedging(p)),
        ("対比構文", phrase::contrast(p)),
        ("自己否定の密度", phrase::self_negation(p)),
        ("進行の実況", phrase::narration(p)),
        ("脱線", phrase::digression(p)),
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

fn measured_names() -> Vec<String> {
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
        // <strong>定義と軸は 1 対 1 ではない。</strong> 定義 39 本のうち 38 本が軸 1 本を作り、
        // 接続詞直後の読点だけが語彙素 12 × 位置 2 の 24 本に展開される。
        assert_eq!(measured_names().len(), 38 + 24);
        assert_eq!(
            kakiburi_metrics::word::conjunction_comma_names().len(),
            24,
            "展開後の軸"
        );
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
                unit: n.clone(),
                role: Role::Person,
                document: d.clone(),
            })
            .chain(baseline.iter().map(|(n, d)| Unit {
                name: n.clone(),
                unit: n.clone(),
                role: Role::BaselineOutput,
                document: d.clone(),
            }))
            .collect();
        let mut c = Cassette {
            version: 1,
            generation: 0,
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
        // <strong>効くかの判定も入れる。</strong> 入れなければ、検めはそこで判定できないを返す
        // ——別の理由で止まるので、通したい経路が通っていないことに気付けない。
        let effective = kakiburi_scale::effective::judge(
            &rows(&fixture::samples(&person), Some(&fixture::Chars)),
            &rows(&fixture::samples(&baseline), Some(&fixture::Chars)),
        );
        c.derived.spread = Some(effective_json::write_spread(&effective));
        c.derived.effective = Some(effective_json::write_effective(&effective));
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

    /// 空のカセットを 1 つ作る。
    fn empty_cassette(dir: &std::path::Path) -> String {
        let p = dir.join("c.kbc").to_string_lossy().into_owned();
        assert_eq!(
            run(&[
                "new".to_owned(),
                p.clone(),
                "--scene".to_owned(),
                "試験".to_owned(),
            ]),
            Exit::Pass
        );
        p
    }

    /// 入れる 1 本を書く。
    fn a_document(dir: &std::path::Path, name: &str) -> String {
        let p = dir.join(format!("{name}.md"));
        std::fs::write(&p, "これは、そうだ。\n").expect("書ける");
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn 役を渡さなければ入れない() {
        // 取り違えると、対照にしたはずの文書が書き手の帯に残る。
        let dir = temp_dir("as-required");
        let c = empty_cassette(&dir);
        let f = a_document(&dir, "x");
        assert_eq!(run(&["add".to_owned(), c, f]), Exit::Usage);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 同じ名前は入れない() {
        // 黙って上書きすれば、原本が 1 本消えたことは値が変わるまで気付けない。
        let dir = temp_dir("dup-id");
        let c = empty_cassette(&dir);
        let f = a_document(&dir, "x");
        let args = |c: &str, f: &str| {
            vec![
                "add".to_owned(),
                c.to_owned(),
                f.to_owned(),
                "--as".to_owned(),
                "person".to_owned(),
                "--source".to_owned(),
                "plain-markdown".to_owned(),
            ]
        };
        assert_eq!(run(&args(&c, &f)), Exit::Pass);
        assert_eq!(run(&args(&c, &f)), Exit::Usage, "2 度目は断る");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 役を跨いでも名前は一意である() {
        // 役で名前空間を分ければ、本人と基準の同題材が別物として通る。
        let dir = temp_dir("dup-role");
        let c = empty_cassette(&dir);
        let f = a_document(&dir, "x");
        let args = |role: &str| {
            vec![
                "add".to_owned(),
                c.clone(),
                f.clone(),
                "--as".to_owned(),
                role.to_owned(),
                "--source".to_owned(),
                "plain-markdown".to_owned(),
            ]
        };
        assert_eq!(run(&args("person")), Exit::Pass);
        assert_eq!(run(&args("baseline")), Exit::Usage, "役が違っても断る");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 無い名前は差し替えない() {
        // add の綴り間違いで差し替えたことにしない。
        let dir = temp_dir("replace-missing");
        let c = empty_cassette(&dir);
        let f = a_document(&dir, "x");
        let args = vec![
            "replace".to_owned(),
            c,
            f,
            "--id".to_owned(),
            "無い".to_owned(),
            "--source".to_owned(),
            "plain-markdown".to_owned(),
        ];
        assert_eq!(run(&args), Exit::Usage);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 差し替えは名前で指す() {
        // 取り込み直すときにファイル名が変わっていることは普通にある。
        let dir = temp_dir("replace-ok");
        let c = empty_cassette(&dir);
        let f = a_document(&dir, "もとの名前");
        assert_eq!(
            run(&[
                "add".to_owned(),
                c.clone(),
                f,
                "--as".to_owned(),
                "person".to_owned(),
                "--source".to_owned(),
                "plain-markdown".to_owned(),
            ]),
            Exit::Pass
        );
        let f2 = a_document(&dir, "違う名前");
        assert_eq!(
            run(&[
                "replace".to_owned(),
                c.clone(),
                f2,
                "--id".to_owned(),
                "もとの名前".to_owned(),
                "--source".to_owned(),
                "plain-markdown".to_owned(),
            ]),
            Exit::Pass
        );
        let (got, _) = open(&c).expect("読める");
        assert_eq!(got.corpus.units.len(), 1, "増えない");
        assert_eq!(got.corpus.units[0].name, "もとの名前");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 分布を出す() {
        let dir = temp_dir("show");
        let cassette = cassette_with_scale(&dir);
        assert_eq!(run(&["show".to_owned(), cassette]), Exit::Pass);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 並べて比べる() {
        let dir = temp_dir("compare");
        let a = a_document(&dir, "a");
        let b = a_document(&dir, "b");
        let args = [
            "compare".to_owned(),
            a,
            b,
            "--source".to_owned(),
            "plain-markdown".to_owned(),
        ];
        assert_eq!(run(&args), Exit::Pass);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 比べるには_2_本要る() {
        let dir = temp_dir("compare-one");
        let a = a_document(&dir, "a");
        assert_eq!(run(&["compare".to_owned(), a]), Exit::Usage);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 自分を検査する() {
        // 目盛りの入ったカセットで、検査が最後まで走る。
        let dir = temp_dir("doctor");
        let cassette = cassette_with_scale(&dir);
        let got = run(&["doctor".to_owned(), cassette]);
        assert!(matches!(got, Exit::Pass | Exit::Unknown), "{got:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 落とす定型を決める() {
        let dir = temp_dir("decide-bp");
        let c = empty_cassette(&dir);
        assert_eq!(
            run(&[
                "decide".to_owned(),
                c.clone(),
                "boilerplate".to_owned(),
                "お世話になっており".to_owned(),
            ]),
            Exit::Pass
        );
        let (got, _) = open(&c).expect("読める");
        assert_eq!(got.decided.boilerplate, vec!["お世話になっており"]);
        // <strong>置き換える。</strong> 足していく形にしない。
        assert_eq!(
            run(&["decide".to_owned(), c.clone(), "boilerplate".to_owned()]),
            Exit::Pass
        );
        let (got, _) = open(&c).expect("読める");
        assert!(got.decided.boilerplate.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 動くかを決める() {
        let dir = temp_dir("decide-mv");
        let c = empty_cassette(&dir);
        assert_eq!(
            run(&[
                "decide".to_owned(),
                c.clone(),
                "movement".to_owned(),
                "絵文字".to_owned(),
                "stuck".to_owned(),
            ]),
            Exit::Pass
        );
        let (got, _) = open(&c).expect("読める");
        assert!(got.is_stuck("絵文字"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 知らない指標は決められない() {
        // 綴りを間違えたまま書けば、直したつもりの指標が指摘に出続ける。
        let dir = temp_dir("decide-unknown");
        let c = empty_cassette(&dir);
        assert_eq!(
            run(&[
                "decide".to_owned(),
                c,
                "movement".to_owned(),
                "絵文字もどき".to_owned(),
                "stuck".to_owned(),
            ]),
            Exit::Usage
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 動くかを決めたら効くかの判定を捨てる() {
        // 捨てなければ、stuck にした指標が指摘に出続ける。
        let dir = temp_dir("decide-drop");
        let cassette = cassette_with_scale(&dir);
        let (before, _) = open(&cassette).expect("読める");
        assert!(before.derived.effective.is_some());
        assert_eq!(
            run(&[
                "decide".to_owned(),
                cassette.clone(),
                "movement".to_owned(),
                "絵文字".to_owned(),
                "stuck".to_owned(),
            ]),
            Exit::Pass
        );
        let (after, _) = open(&cassette).expect("読める");
        assert!(after.derived.effective.is_none(), "捨てている");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 基準は版まで書かないと入れない() {
        // 記録の無い基準で作った値は、次に測ったときに比べられない。
        let dir = temp_dir("baseline-version");
        let c = empty_cassette(&dir);
        let f = a_document(&dir, "b");
        let base = vec![
            "add".to_owned(),
            c.clone(),
            f,
            "--as".to_owned(),
            "baseline".to_owned(),
            "--source".to_owned(),
            "plain-markdown".to_owned(),
        ];
        assert_eq!(run(&base), Exit::Usage, "版が無い");
        let mut with = base.clone();
        with.extend([
            "--model".to_owned(),
            "m".to_owned(),
            "--version".to_owned(),
            "v1".to_owned(),
        ]);
        assert_eq!(run(&with), Exit::Pass);
        let (got, _) = open(&c).expect("読める");
        assert_eq!(got.decided.baseline.model, "m");
        assert_eq!(got.decided.baseline.version, "v1");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 作り方の違う基準は混ぜない() {
        // 版が変われば出力が変わる。混ぜれば、測っているのが版の差になる。
        let dir = temp_dir("baseline-mix");
        let c = empty_cassette(&dir);
        let args = |name: &str, version: &str| {
            vec![
                "add".to_owned(),
                c.clone(),
                a_document(&dir, name),
                "--as".to_owned(),
                "baseline".to_owned(),
                "--source".to_owned(),
                "plain-markdown".to_owned(),
                "--model".to_owned(),
                "m".to_owned(),
                "--version".to_owned(),
                version.to_owned(),
            ]
        };
        assert_eq!(run(&args("b1", "v1")), Exit::Pass);
        assert_eq!(run(&args("b2", "v1")), Exit::Pass, "同じ版なら入る");
        assert_eq!(run(&args("b3", "v2")), Exit::Usage, "違う版は断る");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 束ねると_1_単位になる() {
        // 10 単位の下限は単位で数える。ファイルで数えれば、束ねた分だけ多く見える。
        let dir = temp_dir("bundle");
        let c = empty_cassette(&dir);
        for name in ["a", "b"] {
            let f = a_document(&dir, name);
            assert_eq!(
                run(&[
                    "add".to_owned(),
                    c.clone(),
                    f,
                    "--as".to_owned(),
                    "person".to_owned(),
                    "--source".to_owned(),
                    "plain-markdown".to_owned(),
                    "--unit".to_owned(),
                    "束".to_owned(),
                ]),
                Exit::Pass
            );
        }
        let (got, _) = open(&c).expect("読める");
        assert_eq!(got.corpus.units.len(), 2, "取り込んだのは 2 本");
        let bundles = got.bundles(Role::Person);
        assert_eq!(bundles.len(), 1, "測る単位は 1 つ");
        assert_eq!(bundles[0].0, "束");
        assert_eq!(bundles[0].1.nodes.len(), 2, "node の境界は残す");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 単位は役を跨げない() {
        // 1 つの単位が本人でも基準でもあることになる。
        let dir = temp_dir("bundle-role");
        let c = empty_cassette(&dir);
        let args = |name: &str, role: &str| {
            vec![
                "add".to_owned(),
                c.clone(),
                a_document(&dir, name),
                "--as".to_owned(),
                role.to_owned(),
                "--source".to_owned(),
                "plain-markdown".to_owned(),
                "--unit".to_owned(),
                "束".to_owned(),
            ]
        };
        assert_eq!(run(&args("a", "person")), Exit::Pass);
        assert_eq!(run(&args("b", "baseline")), Exit::Usage);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 書くたびに世代が進む() {
        // 世代が進まなければ、同時に書いた片方の変更が正常終了のまま消える。
        let dir = temp_dir("generation");
        let c = empty_cassette(&dir);
        assert_eq!(save::generation_of(&c), 1, "作った時点で 1");
        let f = a_document(&dir, "x");
        assert_eq!(
            run(&[
                "add".to_owned(),
                c.clone(),
                f,
                "--as".to_owned(),
                "person".to_owned(),
                "--source".to_owned(),
                "plain-markdown".to_owned(),
            ]),
            Exit::Pass
        );
        assert_eq!(save::generation_of(&c), 2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 解析器が無ければ環境の壊れとして止める() {
        // <strong>「素材が足りない」と混ぜない。</strong> 混ぜれば、辞書を入れ忘れた環境が
        // 素材不足の顔で回り続け、素材をいくら足しても直らない。
        let (person, baseline) = fixture::corpus();
        let broken = broken_environment(
            &fixture::samples(&person),
            &fixture::samples(&baseline),
            None,
        );
        assert!(!broken.is_empty(), "解析器を要る指標が挙がる");
        assert!(
            broken.iter().all(|(_, why)| why == "道具が無い"),
            "{broken:?}"
        );
    }

    #[test]
    fn 解析器があれば環境の壊れは出ない() {
        let (person, baseline) = fixture::corpus();
        let broken = broken_environment(
            &fixture::samples(&person),
            &fixture::samples(&baseline),
            Some(&fixture::Chars),
        );
        assert!(broken.is_empty(), "{broken:?}");
    }

    #[test]
    fn 効くかの判定が無ければ判定できない() {
        // <strong>空と欠けを分ける。</strong> 判定がまだ入っていないカセットを「効く指標が 1 本も
        // 無い」と読んではいけない——読めば、指摘の出ない通るが返る。
        let dir = temp_dir("no-effective");
        let cassette = cassette_with_scale(&dir);
        let raw = std::fs::read(&cassette).expect("読める");
        let mut c = store::read(&raw).expect("読める");
        c.derived.effective = None;
        std::fs::write(&cassette, store::write(&c)).expect("書ける");

        let target = dir.join("検める.md");
        std::fs::write(&target, "これは、そうだ、と思う。\n").expect("書ける");
        let args = [
            "review".to_owned(),
            target.to_string_lossy().into_owned(),
            "--cassette".to_owned(),
            cassette,
        ];
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
