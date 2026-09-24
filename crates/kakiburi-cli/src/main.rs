//! kakiburi。人が触る面。
//!
//! <strong>`review` だけが周回に出てくる。</strong> ほかは素材が増えたときにしか動かさない。

mod analyzer;
mod effective_json;
mod exit;
#[cfg(test)]
mod fixture;
mod machine;
mod remedies;
mod scale_json;

use exit::Exit;
use kakiburi_cassette::{
    save, store, Cassette, Common, Decided, Derived, Fingerprint, Inputs, Normalization, Tool,
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
    // <strong>`--help` は引数解析の前に見る。</strong> 位置引数がファイルなので、そのまま渡すと
    // `--help` がファイル名として解釈され、<strong>「読めない（65）」で終わる。</strong>
    if let Some(name) = args.first() {
        if args[1..].iter().any(|a| a == "--help" || a == "-h") {
            return match section(name) {
                Some(text) => {
                    println!("{text}");
                    Exit::Pass
                }
                None => {
                    eprintln!("知らないコマンド: {name}");
                    Exit::Usage
                }
            };
        }
    }
    match args.first().map(String::as_str) {
        // <strong>名乗っているのに動かない解析器で進まない。</strong> 進めば、道具の壊れが
        // <strong>素材が足りないという顔で出る</strong>——足しても直らないものを足させる。
        //
        // <strong>値を出す口だけを止める。</strong>`decide` も `show` も測らないので通す。
        Some(
            name @ ("measure" | "compare" | "build" | "review" | "doctor"),
        ) if analyzer::resolve_or_report().is_err() => {
            let _ = name;
            Exit::Unreadable
        }
        Some("measure") => measure(&args[1..]),
        Some("metrics") => metrics(&args[1..]),
        Some("new") => new_cassette(&args[1..]),
        Some("decide") => decide(&args[1..]),
        Some("show") => show(&args[1..]),
        Some("compare") => compare(&args[1..]),
        Some("doctor") => doctor(&args[1..]),
        Some("build") => build(&args[1..]),
        Some("quick") => quick(&args[1..]),
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

/// 1 つのコマンドの help。<strong>知らない名前なら `None`。</strong>
///
/// 全体の help と同じ文を使う——2 か所に書けば、片方だけが古くなる。
fn section(name: &str) -> Option<&'static str> {
    Some(match name {
        "measure" => MEASURE,
        "new" => NEW,
        "decide" => DECIDE,
        "build" => BUILD,
        "quick" => QUICK,
        "review" => REVIEW,
        "compare" => COMPARE,
        "show" => SHOW,
        "doctor" => DOCTOR,
        "metrics" => METRICS,
        "help" => return None,
        _ => return None,
    })
}

const MEASURE: &str = "\
kakiburi measure <ファイル> --source <取り込み元> [--json]
    1 本を測る。カセットが無くても動く。
    <strong>系統の距離は出ない</strong>——語彙が無いので、その場で選べば違う軸のベクトル
    どうしの距離になる。";

const NEW: &str = "\
kakiburi new <カセット> <場面>
    入れ物を作る。<strong>既に在れば断る。</strong>
    <strong>1 カセットが 1 場面である。</strong> 場面を分けるならファイルを分ける。
    <strong>場面は人が指定する</strong>——文章から当てにいかない。";

const DECIDE: &str = "\
kakiburi decide <カセット> boilerplate <文字列...>
kakiburi decide <カセット> movement <指標> moves|stuck
    <strong>素材から導けないものを書く。</strong> 落とす定型は人が決め、
    指示して動くかは直させてみて初めて分かる。
    <strong>落とす定型は渡した一覧で置き換える</strong>——足していく形にしない。";

const BUILD: &str = "\
kakiburi build <カセット> --person <フォルダ> [--baseline <フォルダ>]
                          [--other <フォルダ>] [--json]
    素材のフォルダを読んで目盛りを作る。<strong>カセットは本文を持たない</strong>ので、
    作り直すたびにフォルダを読む。
    <strong>取り込み元は拡張子から決める</strong>——.html は html、ほかは directive-markdown。
    <strong>作らずに終わる条件を持つ</strong>——止まっても失敗ではない。作れたら割りを出す。

    フォルダは 3 つある。
      --person    本人の文書。照合の相手集合と天井になる
      --baseline  基準。LLM の既定出力。床になる。省くと同梱の池
      --other     <strong>他人の文書。</strong> 人らしさの人の側の較正にだけ効く

    <strong>基準の束ね方は build が決める。</strong> 池の 1 本は地の文 4,500 字あたりで
    頭打ちになるので、本人に長い記事があると長さの範囲の防護柵に当たる。
    届かない分は池の記事を束ねて 1 単位にし、<strong>断られたら束ね方を変えて作り直す。</strong>";

const QUICK: &str = "\
kakiburi quick <本人の記事のフォルダ> [--cassette <カセット>] [--scene <場面>]
                                      [--baseline <基準の池>]
    フォルダを指すだけで目盛りまで作る。new と build をまとめて回す。
    <strong>場面の名前は default</strong>（--scene で変えられる）。検めるたびに打つ名前を
    道具が日本語で付けない。

    基準は<strong>同梱の池から取る</strong>。基準は機械がどう書くかであって書き手ごとに
    変わらないので、あらかじめ作ったものでよい——題材の統制は対ではなく素材全体に
    効かせる。";

const REVIEW: &str = "\
kakiburi review <ファイル> --cassette <カセット> --source <取り込み元> [--json]
    検める。3 値と指摘を返す。
    <strong>どの場面として検めるかはカセットが言う</strong>——1 カセットが 1 場面なので、
    入れ物を選ぶことが場面を選ぶことである。
    <strong>目盛りの無いカセットは判定できない（2）を返す</strong>——素材が足りずに作れな
    かったのは正常な状態である。";

const COMPARE: &str = "\
kakiburi compare <ファイル>... --source <取り込み元>
    並べて比べる。<strong>系統の距離は出ない</strong>——語彙が無いためである。
    <strong>3 本以上を取る</strong>——n 周した草稿を並べて散らばりを見るためである。";

const SHOW: &str = "\
kakiburi show <カセット>
    中身を出す。場面・世代・暫定値・帯・語彙の大きさ・効く指標・言い回しの表。
    <strong>本文は持たないので、素材の分布は出ない</strong>——それは build が出す。";

const DOCTOR: &str = "\
kakiburi doctor <カセット>
    <strong>カセットを検査する。</strong> 指紋が環境と合っているか、派生物が揃っているか、
    帯が全体を覆っていないか、暫定値が立っていないか。
    <strong>素材は要らない。</strong> 本人がいちばん高く出るかは build が測る——
    そちらは素材を持っているときにしか言えない。";

const METRICS: &str = "\
kakiburi metrics
    登録簿を回して一覧を出す。使う側が一覧を持たないことの裏返し。";

/// 環境変数。<strong>help に出さなければ、仕様を読むまで進めない。</strong>
const ENVIRONMENT: &str = "\
環境

  KAKIBURI_UNIDIC          UniDic の展開先。指すと全系統を測れる
  KAKIBURI_UNIDIC_VERSION  指紋に入る版の申告（既定: 版の申告なし）
  KAKIBURI_MECAB           MeCab の実行ファイル（既定: mecab）
  KAKIBURI_BASELINES       quick が使う基準の池（既定: 作業ディレクトリの baselines）

  <strong>UniDic は nixpkgs に無い。</strong> 国語研が配布している:
  https://clrd.ninjal.ac.jp/unidic_archive/cwj/2.1.2/unidic-mecab-2.1.2_bin.zip
  <strong>IPADic は断る</strong>——体系が違えば語彙素で引けない。";

fn print_help() {
    println!("kakiburi — どこがその人と違うかを、言えるようにする");
    println!();
    println!("{QUICK}");
    println!();
    println!("{MEASURE}");
    println!();
    println!("作る——たまに動かす");
    println!();
    println!("  <strong>1 カセットが 1 場面である。</strong> 語彙も重みも帯も 1 場面ぶんで、");
    println!("  場面を分けるならファイルを分ける。");
    println!("  <strong>カセットは本文を持たない。</strong> 素材のフォルダが正本である。");
    for s in [NEW, DECIDE, BUILD] {
        println!();
        println!("{s}");
    }
    println!();
    println!("回す——毎周");
    println!();
    println!("{REVIEW}");
    println!();
    println!("覗く");
    for s in [COMPARE, SHOW, DOCTOR, METRICS] {
        println!();
        println!("{s}");
    }
    println!();
    println!("取り込み元: {}", source_names().join(" / "));
    println!("  <strong>--source に既定は無い。</strong> 取り違えても数が変わるだけで、エラーにならない。");
    println!();
    println!("{ENVIRONMENT}");
    println!();
    println!("<strong>--json は道具向けである。</strong> 人向けの表示は変えない。但し書きは stderr に出る。");
    println!();
    println!("終了コード: 0 通る / 1 通らない / 2 判定できない / 64 以上 使う前の問題");
    println!();
    println!("<コマンド> --help でその節だけを出せる。");
}

/// カセットを作る。
///
/// <strong>1 カセットが 1 場面である。</strong> 場面はここで決まり、以後変わらない——
/// 場面を分けるならファイルを分ける。
fn new_cassette(args: &[String]) -> Exit {
    let (Some(path), Some(scene)) = (args.first(), args.get(1)) else {
        eprintln!("new <カセット> <場面>");
        eprintln!("<strong>場面は人が指定する。</strong> 文章から当てにいかない");
        return Exit::Usage;
    };
    if let Some(other) = args.get(2) {
        eprintln!("知らない引数: {other}");
        return Exit::Usage;
    }
    if !kakiburi_cassette::scene_name_ok(scene) {
        eprintln!("断る: 場面の名前に使えない: {scene}");
        return Exit::Usage;
    }

    let mut c = Cassette {
        version: store::VERSION,
        // <strong>置き換えるたびに増える。</strong> 作った時点では 0 で、書けば 1 になる。
        generation: 0,
        fingerprint: current_fingerprint(),
        // <strong>いまは常に暫定値が立つ。</strong> 閾値がまだ導き直されていない。
        provisional: vec!["除外の既定".into(), "帯の端".into(), "語彙の大きさ".into()],
        scene: scene.clone(),
        decided: Decided::default(),
        derived: Derived::dropped(),
    };
    // <strong>場面は指紋の材料である。</strong> 組み直さなければ、作った直後から
    // 「環境と合わない」になる——<strong>1 度も使えないカセットが出来上がる。</strong>
    refresh(&mut c);

    // <strong>既に在れば断る。</strong> 素材の取り込みには時間が掛かるうえ、原本は作り直せない
    // ——上書きすれば、取り込んだ単位はそこで消える。
    //
    // <strong>ここで `exists()` を見てから書かない。</strong> 見てから書くまでのあいだに割り込ま
    // れれば同じことが起きる。<strong>錠の中で確かめるのは保存の側の仕事である</strong>——
    // `expected` に `None` を渡すことが「作るつもりだ」という申告になる。
    if let Err(e) = save::save(path, &c, None) {
        eprintln!("断る: {e}");
        if matches!(e, save::SaveError::Exists { .. }) {
            eprintln!("<strong>人が決めたことは戻らない</strong>");
            return Exit::Usage;
        }
        return Exit::Unreadable;
    }
    println!("作った: {path}（場面: {scene}）");
    println!("素材のフォルダを指して build する");
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

/// 読んだ取り込み元を指紋に置く。<strong>足すのではなく、入れ替える。</strong>
///
/// 取り込み元が変われば値が変わるので、<strong>指紋が動かなければ照らしても違いが出ない</strong>
/// ——[取り込み元を間違えると 0 が並ぶ](../../../docs/spec/030-normalize.md#取り込み元を間違えると0-が並ぶ)
/// のに、それを検出する唯一の手がかりが動かない。
///
/// <strong>足すだけにしない。</strong> 本文をカセットに溜めていた頃は、最後の 1 本を差し替えた
/// ことを知る手段が無かったので足すしかなかった。<strong>いまは毎回フォルダを全部読む</strong>
/// ので、いま読んだものがそのまま正本である——足すだけにすると、<strong>HTML を外して
/// 作り直しても指紋に HTML が残る。</strong>
fn set_sources(c: &mut Cassette, files: &[String]) {
    let mut sources: Vec<String> = files
        .iter()
        .map(|f| source_of(f).name().to_owned())
        .collect();
    sources.sort_unstable();
    sources.dedup();
    let mut inputs = c.fingerprint.inputs.clone();
    inputs.common.normalization.sources = sources;
    c.fingerprint = Fingerprint::build(inputs);
}

/// 指紋を組み直す。<strong>カセットを変えたら必ず通る。</strong>
///
/// <strong>変えたのに組み直さなければ、次に検めたときに「合っている」と言われる。</strong>
fn refresh(c: &mut Cassette) {
    c.fingerprint = fingerprint_with(c, analyzer::resolved().as_ref());
}

/// カセットの中身を出す。
///
/// <strong>素材の分布は出ない。</strong> カセットは本文を持たないので、ここで出せるのは
/// <strong>作り終えた目盛りそのもの</strong>である——素材がどう散らばっていたかは
/// `build` が読んだその場でしか言えない。
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
    if !c.decided.boilerplate.is_empty() {
        println!("落とす定型: {}", c.decided.boilerplate.join("、"));
    }
    let stuck: Vec<&str> = c
        .decided
        .movement
        .iter()
        .filter(|(_, m)| **m == kakiburi_cassette::Movement::Stuck)
        .map(|(k, _)| k.as_str())
        .collect();
    if !stuck.is_empty() {
        println!("動かないと分かった指標: {}", stuck.join("、"));
    }

    println!();
    let Some(scale) = c.derived.scale.as_deref().and_then(scale_json::read) else {
        println!("目盛り: 無い。素材のフォルダを指して build する");
        return Exit::Pass;
    };
    println!("目盛り: ある");
    for (name, set) in &scale.frozen {
        println!("  {name:<12} {:>5} 次元", set.len());
    }
    println!(
        "照合値の帯: 天井 {:.3}〜{:.3} / 床 {:.3}〜{:.3}",
        scale.band.ceiling.low, scale.band.ceiling.high, scale.band.floor.low, scale.band.floor.high
    );
    println!(
        "人らしさの帯: 人 {:.3}〜{:.3} / 機械 {:.3}〜{:.3}",
        scale.humanness_band.ceiling.low,
        scale.humanness_band.ceiling.high,
        scale.humanness_band.floor.low,
        scale.humanness_band.floor.high
    );
    for line in selection_lines(&scale.selection) {
        println!("{line}");
    }
    if let Some(effective) = c
        .derived
        .spread
        .as_deref()
        .zip(c.derived.effective.as_deref())
        .and_then(|(s, e)| effective_json::read(s, e))
    {
        let works = effective.iter().filter(|e| e.works()).count();
        println!("効く指標: {works} / {} 本", effective.len());
    }
    // <strong>言い回しの表は本文の代わりである。</strong> 何本畳んであるかを出さなければ、
    // 繰り返しの上限が言えるかどうかが読めない。
    println!("言い回しの表: {} 本", phrase_table_len(&c));
    Exit::Pass
}

/// 言い回しの表に入っている本数。
fn phrase_table_len(c: &Cassette) -> usize {
    c.derived
        .phrases
        .as_deref()
        .map_or(0, |s| s.lines().filter(|l| !l.trim().is_empty()).count())
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
    let mut source: Option<Source> = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--source" {
            let Some(s) = args.get(i + 1).and_then(Source::from_name) else {
                eprintln!("対応表に無い取り込み元");
                return Exit::Usage;
            };
            source = Some(s);
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
    let Some(source) = source else {
        return missing_source();
    };

    let mecab = analyzer::resolved();
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

/// カセットを検査する。
///
/// <strong>素材は要らない。</strong> 本人がいちばん高く出るかは [`build`] が測る——
/// そちらは素材を持っているときにしか言えない。ここで見るのは、<strong>カセット単体で
/// 矛盾していないか</strong>である。
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
    println!("場面: {}", c.scene);

    // 1. 指紋が現在の環境と合っているか。<strong>共通部分だけを見る。</strong>
    match check_fingerprint(&c) {
        Ok(()) => println!("指紋: 環境と合っている"),
        Err(diff) => {
            println!("指紋: <strong>合わない</strong>（{}）", diff.join("、"));
            bad += 1;
        }
    }

    // 2. 暫定値が立っていないか。
    if c.provisional.is_empty() {
        println!("暫定値: 立っていない");
    } else {
        println!("暫定値: {}", c.provisional.join("、"));
        println!("  判定に但し書きが付く");
    }

    // 3. 派生物が揃っているか。<strong>片方だけあるのは壊れている。</strong>
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

    let Some(scale) = c.derived.scale.as_deref().and_then(scale_json::read) else {
        println!("目盛りが無い。素材のフォルダを指して build する");
        return if bad == 0 { Exit::Pass } else { Exit::Unknown };
    };

    // 4. 帯が全体を覆っていないか。<strong>覆っていれば「判定できない」しか返らない。</strong>
    //    仮説が成り立っていない書き手・場面では、それがここに出る。
    for (name, band) in [
        ("照合値", scale.band),
        ("人らしさ値", scale.humanness_band),
    ] {
        println!(
            "{name}の帯: 天井 {:.3}〜{:.3} / 床 {:.3}〜{:.3}",
            band.ceiling.low, band.ceiling.high, band.floor.low, band.floor.high
        );
        if band.floor.high >= band.ceiling.low {
            println!("  <strong>帯が重なっている。</strong> 本人と基準が分かれていない");
            bad += 1;
        }
    }

    // 5. 相手集合のベクトルが噛み合っているか。<strong>本数だけでは足りない。</strong>
    if scale.partner_vectors_ok() {
        println!("相手集合のベクトル: {} 本", scale.partner_vectors.len());
    } else {
        println!("相手集合のベクトル: <strong>目盛りと噛み合わない</strong>");
        println!("  照合値が出せない。build し直しが要る");
        bad += 1;
    }

    // 6. 言い回しの表があるか。<strong>本文の代わりなので、無ければ上限を言えない。</strong>
    //
    // <strong>空の表と、表が無いことを分ける。</strong> 2 つ以上の単位に現れる言い回しが
    // 無ければ表は正しく空になる——<strong>それを壊れと言えば、build し直しても直らない
    // ことを要求することになる。</strong>
    match c.derived.phrases.as_deref() {
        None => {
            println!("言い回しの表: 無い");
            println!("  <strong>繰り返しの上限を言えない。</strong> build し直しが要る");
            bad += 1;
        }
        Some(_) => {
            let phrases = phrase_table_len(&c);
            println!("言い回しの表: {phrases} 本");
            if phrases == 0 {
                println!("  <strong>正常な状態である。</strong> 2 つ以上の単位に現れる言い回しが無い");
            }
        }
    }

    if bad == 0 {
        Exit::Pass
    } else {
        Exit::Unknown
    }
}

/// 本人が基準より高く出た対の、通ると言える割合の下限。<strong>暫定値である。</strong>
///
/// <strong>1.0 を求めない。</strong> それは「1 対でも逆に出たら落とす」ということで、
/// [端で見るのと同じく n で漂う](higher_rate)——素材を足すほど落ちやすくなる。
///
/// <strong>導き直していない。</strong> 目盛りが壊れていれば 0.5 付近に落ちるので、そこから
/// 十分に離れた値を置いてある。どこまで緩めてよいかは、複数の書き手で測るまで決まらない。
const HIGHER_RATE_FLOOR: f64 = 0.95;

/// 逆に出た対を、いくつまで名指しするか。
const INVERTED_SHOWN: usize = 5;

/// 本人がいちばん高く出るかを検める。<strong>おかしかった数を返す。</strong>
///
/// <strong>素材を持っている `build` の側でやる。</strong> カセットは本文を持たないので、
/// 作り終えたあとにこれを測り直す道が無い——<strong>測れる唯一の瞬間がここである。</strong>
fn check_person_higher(
    scale: &Scale,
    person: &[Sample<'_>],
    baseline: &[Sample<'_>],
    a: Option<&dyn kakiburi_metrics::morph::Analyzer>,
    say: &dyn Fn(String),
) -> usize {
    // <strong>相手集合そのものは測らない。</strong> 自分との距離を測ることになる。
    let side = |samples: &[Sample<'_>]| -> Vec<(String, f64)> {
        samples
            .iter()
            .filter(|s| !scale.partners().iter().any(|n| n == s.name))
            .filter_map(|s| {
                measure_against(scale, *s, a)
                    .matching
                    .map(|v| (s.name.to_owned(), v))
            })
            .collect()
    };
    let mine = side(person);
    let theirs = side(baseline);
    if mine.is_empty() || theirs.is_empty() {
        say("自己検査: 照合値を出せる単位が足りない".to_owned());
        return 0;
    }
    let (rate, inverted) = higher_rate(&mine, &theirs);
    say(format!(
        "自己検査: 本人が高く出た対 {rate:.3}（{} 対中 {} 対が逆、下限 {HIGHER_RATE_FLOOR:.2} は暫定値）",
        mine.len() * theirs.len(),
        inverted.len()
    ));
    // <strong>逆に出た対を名指しする。</strong> 割合だけでは、目盛り全体が緩んでいるのか
    // 1 本の単位が外れているのかが分からない。
    for (p, pv, b, bv) in inverted.iter().take(INVERTED_SHOWN) {
        say(format!("  {p} ({pv:.3}) ≦ {b} ({bv:.3})"));
    }
    if inverted.len() > INVERTED_SHOWN {
        say(format!("  ほか {} 対", inverted.len() - INVERTED_SHOWN));
    }
    if rate >= HIGHER_RATE_FLOOR {
        0
    } else {
        say("  <strong>基準のほうが高く出る対が多すぎる。</strong> 目盛りを疑う".to_owned());
        say("  測っているのは著者性ではなく指示追従かもしれない".to_owned());
        1
    }
}


/// 本人が基準より高く出た対の割合。<strong>逆に出た対も返す。</strong>
///
/// <strong>最小と最大では見ない。</strong> 端は n とともに外へ広がるので、素材を足すほど
/// 本人の最小は下がり基準の最大は上がる——<strong>目盛りが良くなっても検査が落ちやすくなる</strong>。
/// [帯の端を各側で数を決めて取る](kakiburi_scale::assemble)のと同じ理由である。
///
/// <strong>対ごとの比較は漂わない。</strong> 全部の対で本人が高ければ 1.0 で、これが
/// 「本人がいちばん高く出る」の言い換えになる。
fn higher_rate(
    mine: &[(String, f64)],
    theirs: &[(String, f64)],
) -> (f64, Vec<(String, f64, String, f64)>) {
    let mut win = 0.0f64;
    let mut inverted = Vec::new();
    for (pn, pv) in mine {
        for (bn, bv) in theirs {
            if pv > bv {
                win += 1.0;
            } else {
                // <strong>並んだ対も逆として数える。</strong> 高く出ていないことに変わりはない。
                if pv == bv {
                    win += 0.5;
                }
                inverted.push((pn.clone(), *pv, bn.clone(), *bv));
            }
        }
    }
    // <strong>差の小さい順に並べる。</strong> いちばん惜しい対から見せる。
    inverted.sort_by(|a, b| (b.1 - b.3).total_cmp(&(a.1 - a.3)));
    #[allow(clippy::cast_precision_loss)]
    let n = (mine.len() * theirs.len()) as f64;
    (win / n, inverted)
}

/// 人が決めたことを書く。
///
/// <strong>素材から導けないものだけがここに来る。</strong> 落とす定型は人が決めるもので
/// あり、指示して動くかは直させてみて初めて分かる。
///
/// <strong>場面は取らない。</strong> 1 カセットが 1 場面なので、入れ物を選ぶことが場面を
/// 選ぶことである。
fn decide(args: &[String]) -> Exit {
    let Some(path) = args.first() else {
        eprintln!("カセットの経路を渡す");
        return Exit::Usage;
    };
    let rest = &args[1..];
    match rest.first().map(String::as_str) {
        Some("boilerplate") => decide_boilerplate(path, &rest[1..]),
        Some("movement") => decide_movement(path, &rest[1..]),
        _ => {
            eprintln!(
                "decide <カセット> boilerplate <文字列...>\n\
                 decide <カセット> movement <指標> moves|stuck"
            );
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
    refresh(&mut c);
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
    refresh(&mut c);
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

/// フォルダを指すだけで目盛りまで作る。
///
/// <strong>新しい経路を作らない。</strong>`new` と `build` をそのまま呼ぶ——別の道を作れば、
/// そちらだけが古くなる。
fn quick(args: &[String]) -> Exit {
    let Some(dir) = args.first() else {
        eprintln!("本人の記事が入ったフォルダを渡す");
        return Exit::Usage;
    };
    let mut cassette: Option<String> = None;
    // <strong>言われた場面と、既定の場面を分けて持つ。</strong> 既にあるカセットの場面と
    // 食い違ったときに、<strong>人が名乗ったのか道具が埋めたのかで扱いが違う。</strong>
    let mut scene_name: Option<String> = None;
    let mut pool_dir: Option<String> = None;
    // <strong>基準の来歴はそのまま通す。</strong> 外から池を渡すなら `build` が版を要求する
    // ので、<strong>quick に渡す道が無ければ `--baseline` が必ず落ちる。</strong>
    let mut provenance: Vec<String> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        let Some(v) = args.get(i + 1) else {
            eprintln!("{} に値を渡す", args[i]);
            return Exit::Usage;
        };
        match args[i].as_str() {
            "--cassette" => cassette = Some(v.clone()),
            "--scene" => scene_name = Some(v.clone()),
            "--baseline" => pool_dir = Some(v.clone()),
            flag @ ("--model" | "--version" | "--param" | "--topic") => {
                provenance.extend([flag.to_owned(), v.clone()]);
            }
            other => {
                eprintln!("知らない引数: {other}");
                return Exit::Usage;
            }
        }
        i += 2;
    }
    let cassette = cassette.unwrap_or_else(|| format!("{dir}.kb"));
    if readable_files(dir).is_empty() {
        eprintln!("記事が 1 本も見つからない: {dir}");
        return Exit::Unreadable;
    }

    // <strong>在るものを消さない。</strong> 消してよいかは、読んでみるまで分からない
    // ——カセットでないファイルを指されたら、<strong>それは人の別のファイルである。</strong>
    //
    // <strong>カセットだったとしても消さない。</strong>[人が決めたこと](kakiburi_cassette::Decided)は
    // 作り直せないので、落とす定型も動かない指標もそこで消える。<strong>目盛りは
    // `build` が入れ替える</strong>ので、消す理由がそもそも無い。
    if std::path::Path::new(&cassette).exists() {
        let (existing, _) = match open(&cassette) {
            Ok(v) => v,
            Err(_) => {
                eprintln!("断る: 既に在るが、カセットとして読めない: {cassette}");
                eprintln!("<strong>消さない。</strong> 別の経路を --cassette で渡す");
                return Exit::Usage;
            }
        };
        // <strong>場面の食い違いは断る。</strong> 別の場面の目盛りを、名乗りだけ
        // 据え置いて作り直せば、<strong>中身と名前が合わないカセットになる。</strong>
        if let Some(asked) = &scene_name {
            if asked != &existing.scene {
                eprintln!(
                    "断る: 既に在るカセットの場面が違う（{} / 言われたのは {asked}）",
                    existing.scene
                );
                eprintln!("<strong>場面はファイルで分ける。</strong> --cassette で別の経路を渡す");
                return Exit::Usage;
            }
        }
        say_reuse(&cassette, &existing.scene);
    } else if run(&[
        "new".to_owned(),
        cassette.clone(),
        scene_name.unwrap_or_else(|| DEFAULT_SCENE.to_owned()),
    ]) != Exit::Pass
    {
        return Exit::Usage;
    }

    let mut a = vec![
        "build".to_owned(),
        cassette,
        "--person".to_owned(),
        dir.clone(),
    ];
    // <strong>池は build に探させる。</strong> ここで解決して渡すと、同梱の池まで
    // 「外から渡された基準」になり、版を名乗れと言われる。
    if let Some(pool) = pool_dir {
        a.extend(["--baseline".to_owned(), pool]);
    }
    a.extend(provenance);
    run(&a)
}

/// 作り直すときに、何を引き継ぐかを言う。
fn say_reuse(cassette: &str, scene: &str) {
    println!("既に在るカセットに作り直す: {cassette}（場面: {scene}）");
    println!("人が決めたことは引き継ぐ。目盛りだけ作り直す");
}

/// `quick` が作る場面の名前。
///
/// <strong>半角で名付ける。</strong> 道具が勝手に作る名前を日本語にしない
/// ——<strong>打ちにくい名前は道具の側の落ち度である。</strong>
const DEFAULT_SCENE: &str = "default";

/// この文章が繰り返している言い回しと、その回数。
///
/// <strong>繰り返していないものは見ない。</strong> 1 度きりの言い回しは書きぶりではなく、
/// その文章の題材である。
const DRAFT_REPEAT: usize = 3;

/// <strong>上限を渡す先を、道具が勧めた言い回しに限らない。</strong>
///
/// 受け取った側は勧められていない言い回しでも足す——人らしさを通すために語尾を
/// 揃えるのが、いちばん安い手だからである。<strong>そこを見ていなければ、道具は自分が
/// 誘発した水増しを見逃す。</strong>
fn repeated_in_draft(analyzed: Option<&kakiburi_metrics::morph::Analyzed>) -> Vec<(String, usize)> {
    let mut n: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for (g, _, _) in
        kakiburi_metrics::word::grams_with_position(analyzed, &kakiburi_scale::assemble::KATA_N)
    {
        *n.entry(g).or_insert(0) += 1;
    }
    let mut out: Vec<(String, usize)> = n
        .into_iter()
        .filter(|(text, times)| *times >= DRAFT_REPEAT && text.chars().count() >= 4)
        .collect();
    // <strong>長いほうを先に採る。</strong> 短い並びはその一部なので、両方出すと同じ指摘が重なる。
    out.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| b.0.chars().count().cmp(&a.0.chars().count()))
            .then_with(|| a.0.cmp(&b.0))
    });
    let mut kept: Vec<(String, usize)> = Vec::new();
    for (text, times) in out {
        if kept.iter().any(|(k, _)| k.contains(&text)) {
            continue;
        }
        kept.push((text, times));
    }
    kept
}

/// 本人がその言い回しを使う、日本語 1,000 字あたりの最大。
///
/// <strong>カセットの[言い回しの表](kakiburi_cassette::Derived::phrases)を引く。</strong>
/// <strong>表に無いものは 0 である</strong>——1 つの単位にしか出てこない言い回しは上限が
/// ほぼ 0 なので、持たないのと同じ結論になる。本人が一度も使っていない言い回しを
/// 草稿が繰り返していれば、表に無いことがそのまま指摘になる。
fn person_ceiling(table: &BTreeMap<String, f64>, phrase: &str) -> f64 {
    table.get(phrase).copied().unwrap_or(0.0)
}

/// 言い回しの表を読む。<strong>読めない行は落とす。</strong>
fn read_phrase_table(c: &Cassette) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    let Some(body) = c.derived.phrases.as_deref() else {
        return out;
    };
    for line in body.lines() {
        let Ok(v) = kakiburi_cassette::json::parse(line) else {
            continue;
        };
        let (Some(text), Some(ceiling)) = (
            v.get("text").and_then(kakiburi_cassette::json::Value::as_str),
            v.get("ceiling")
                .and_then(kakiburi_cassette::json::Value::as_f64),
        ) else {
            continue;
        };
        out.insert(text.to_owned(), ceiling);
    }
    out
}

/// 言い回しの表を作る。<strong>本文の代わりである。</strong>
///
/// <strong>2 つ以上の単位に現れるものだけを持つ。</strong> 1 つの単位にしか出てこない
/// 言い回しは、その記事の題材であって書きぶりではない——持っても上限がほぼ 0 で、
/// 持たないのと同じ結論になる。
fn phrase_table(
    units: &[(String, kakiburi_doc::Document)],
    a: Option<&dyn kakiburi_metrics::morph::Analyzer>,
) -> String {
    let mut seen: BTreeMap<String, (usize, f64)> = BTreeMap::new();
    for (_, doc) in units {
        let chars = doc.japanese_chars();
        if chars == 0 {
            continue;
        }
        let analyzed = analyzed_of(&doc.prose(), a);
        let mut here: BTreeMap<String, usize> = BTreeMap::new();
        for (g, _, _) in kakiburi_metrics::word::grams_with_position(
            analyzed.as_ref(),
            &kakiburi_scale::assemble::KATA_N,
        ) {
            if g.chars().count() < PHRASE_CHARS {
                continue;
            }
            *here.entry(g).or_insert(0) += 1;
        }
        for (text, times) in here {
            #[allow(clippy::cast_precision_loss)]
            let rate = 1000.0 * times as f64 / chars as f64;
            let e = seen.entry(text).or_insert((0, 0.0));
            e.0 += 1;
            if rate > e.1 {
                e.1 = rate;
            }
        }
    }
    let mut out = String::new();
    for (text, (units_seen, ceiling)) in seen {
        if units_seen < PHRASE_UNITS {
            continue;
        }
        out.push_str(
            &kakiburi_cassette::json::Value::obj([
                ("text".to_owned(), kakiburi_cassette::json::Value::s(&text)),
                (
                    "ceiling".to_owned(),
                    kakiburi_cassette::json::Value::Number(ceiling),
                ),
            ])
            .write(),
        );
        out.push('\n');
    }
    out
}

/// 言い回しとして持つ最短の長さ。<strong>草稿を見る側と同じ線である。</strong>
const PHRASE_CHARS: usize = 4;

/// 表に載せるのに要る単位の数。
const PHRASE_UNITS: usize = 2;

/// 同梱の池が版を名乗るファイル。
///
/// 池を作り直したら中の版を上げる——<strong>上げなければ、中身が変わったのに
/// 過去の値と比べられてしまう。</strong>
const POOL_MARKER: &str = "POOL";

/// 同梱の池の名前。<strong>指紋に入る。</strong>
const POOL_MODEL: &str = "同梱の池";

/// 池から取る本数。<strong>暫定値である。</strong>
///
/// [下限](kakiburi_scale::split::UNITS_FLOOR)は 10 だが、測れない分が出るので余裕を
/// 持たせる。手作りの基準 21 本で目盛りが作れていたので、そのあたりに置いた。
///
/// <strong>多く取れば題材の遠いものが混ざり、少なく取れば本数が下限を割る。</strong>
const POOL_TAKE: usize = 24;

/// 基準の池の場所。
fn baseline_pool() -> Option<String> {
    if let Ok(p) = std::env::var("KAKIBURI_BASELINES") {
        return Some(p);
    }
    let here = std::path::Path::new("baselines");
    here.is_dir().then(|| "baselines".to_owned())
}

/// 池が名乗っている版。<strong>同梱の池かどうかは、池自身が言う。</strong>
///
/// <strong>場所では決められない。</strong>[環境変数](baseline_pool)は同梱の池を指すのが
/// 普通の使い方だが、どこを指しているかは道具に分からない——<strong>場所で決めると、
/// 別の素材が同梱の池の来歴を名乗る。</strong>
///
/// <strong>だから目印を池の中に置く。</strong> 目印のあるフォルダだけが同梱の池である。
fn pool_version(dir: &str) -> Option<String> {
    let marker = std::path::Path::new(dir).join(POOL_MARKER);
    let text = std::fs::read_to_string(marker).ok()?;
    let v = text.trim();
    (!v.is_empty()).then(|| v.to_owned())
}

/// フォルダの中の、読める文書。<strong>決定的に並べる。</strong>
fn readable_files(dir: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| matches!(x, "md" | "markdown" | "html" | "htm" | "txt"))
        })
        .filter_map(|p| p.to_str().map(str::to_owned))
        .collect();
    out.sort();
    out
}

/// 地の文の日本語の文字数。<strong>読めないものは数えない。</strong>
fn japanese_chars_of(files: &[String]) -> Vec<usize> {
    files
        .iter()
        .filter_map(|f| {
            let body = std::fs::read_to_string(f).ok()?;
            let source = if f.ends_with(".html") || f.ends_with(".htm") {
                Source::Html
            } else {
                Source::DirectiveMarkdown
            };
            normalize(&body, source).ok().map(|d| d.japanese_chars())
        })
        .collect()
}

/// 目盛りに乗りそうな文書だけの、地の文の日本語の文字数。
///
/// <strong>[長さの範囲](kakiburi_scale::length_range_ok)は測れた単位だけで測られる。</strong>
/// 短すぎて落ちる文書まで数えて束ね方を決めると、<strong>実際より低いところから始まる
/// 分布に合わせてしまう。</strong>
///
/// 実測で、362 字の記事まで数えていたために本人の下端が低く見え、束ねた基準が
/// 上へ寄って防護柵に当たった——取り置きの総当たりで 4 分割のうち 1 つが目盛りを
/// 作れず、<strong>取り置いた 12 本が丸ごと判定できない</strong>になっていた。
///
/// <strong>ここで掛けるのは字数と読点だけである。</strong> 延べ語数は解析器が要り、
/// 束ね方を決めるだけのために全文を解析し直すのは高い。
fn measurable_chars_of(files: &[String]) -> Vec<usize> {
    files
        .iter()
        .filter_map(|f| {
            let body = std::fs::read_to_string(f).ok()?;
            let doc = normalize(&body, source_of(f)).ok()?;
            let chars = doc.japanese_chars();
            if chars < kakiburi_metrics::floor::JAPANESE_CHARS {
                return None;
            }
            let commas: usize = doc
                .prose()
                .iter()
                .map(|s| s.text.matches('、').count())
                .sum();
            (commas >= kakiburi_metrics::matching::COMMA_FLOOR).then_some(chars)
        })
        .collect()
}

/// 池の 1 本を何本束ねて 1 単位にするかを決める。
///
/// <strong>池の 1 本では本人の長い記事に届かない。</strong> 生成は地の文 4,500 字あたりで
/// 頭打ちになるので、本人に長い記事があると[長さの範囲](kakiburi_scale::length_range_ok)の
/// 防護柵に当たり、目盛りが作れない。
///
/// <strong>束ねる仕組みは既にある</strong>——短い文書を 1 単位にまとめるためのものを、
/// 長さを届かせるために使う。束ねた結果は 1 単位なので、下限も範囲も単位で数える。
///
/// 池から、本人の題材に近い分を選ぶ。
///
/// <strong>題材を揃えないと、測っているのは題材である。</strong> 統制しないで訓練した文体表現は、
/// 題材を揃えたテストで AUC が .79 から .58 へ落ちる
/// （[Wegmann](../../../docs/references/wegmann-2022.md)）。
///
/// <strong>揃えるのは対ごとではなく素材全体である</strong>——対で絞ると相手の本数が変わる
/// （[統制](../../../docs/spec/200-extract.md)）。だから池から部分集合を選ぶ形にする。
///
/// 近さは<strong>自立語の重なり</strong>で測る。助詞や助動詞は誰が書いても同じで、題材を
/// 分けない。
fn pick_by_topic(person: &[String], pool: &[String], take: usize) -> Vec<usize> {
    let all = || (0..pool.len()).collect();
    if pool.len() <= take {
        return all();
    }
    let Some(analyzer) = analyzer::resolved() else {
        // <strong>解析器が無ければ選ばない。</strong> 題材で絞れないことを、黙って
        // 別の基準で絞ったことにしない。
        return all();
    };
    let words = |files: &[String]| -> std::collections::BTreeSet<String> {
        let mut out = std::collections::BTreeSet::new();
        for f in files {
            let Ok(body) = std::fs::read_to_string(f) else {
                continue;
            };
            let source = source_of(f);
            let Ok(doc) = normalize(&body, source) else {
                continue;
            };
            let Ok(a) = kakiburi_metrics::morph::Analyzed::of(&doc.prose(), &analyzer) else {
                continue;
            };
            out.extend(
                a.all()
                    .filter(|m| !m.is_function_word())
                    .filter(|m| m.surface.chars().count() >= 2)
                    .map(|m| m.surface.clone()),
            );
        }
        out
    };
    let mine = words(person);
    if mine.is_empty() {
        return all();
    }
    let mut scored: Vec<(usize, usize)> = pool
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let theirs = words(std::slice::from_ref(f));
            (theirs.intersection(&mine).count(), i)
        })
        .collect();
    // <strong>重なりの多い順。同点なら池の並び順。</strong> 決めておかないと、同じ素材から
    // 違う目盛りができる。
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let mut picked: Vec<usize> = scored.into_iter().take(take).map(|(_, i)| i).collect();
    picked.sort_unstable();
    picked
}

/// 拡張子から取り込み元を決める。
fn source_of(path: &str) -> Source {
    if path.ends_with(".html") || path.ends_with(".htm") {
        Source::Html
    } else {
        Source::DirectiveMarkdown
    }
}

/// 返すのは<strong>池の何番目をどう束ねるか</strong>である。中の並びは池の添字。
///
/// <strong>小さいほうから積んで本人の上端に届かせ、残りは単独で置く。</strong> こうすると単独の分が
/// 下から中ほどを埋め、束ねた分が上端に届く。
///
/// <strong>一律に束ねない。</strong> 全部を同じ本数で束ねると、大きいものどうしが合わさって
/// 本人の上端を大きく超え、今度は基準側の範囲が広がりすぎる。実測で、一律 3 本に
/// したら本人側 70% / 基準側 42% になった。
///
/// <strong>[防護柵](kakiburi_scale::length_range_ok)は通ればよい門であって、最大化する目的ではない。</strong>
/// この案が通るならそのまま使い、<strong>落ちたときだけ</strong>ほかの束ね方を探す。
///
/// <strong>最大化しにいくと別の場所が壊れる。</strong> 実測で、重なりを最大にする案に置き換え
/// たら目盛りはすべて作れるようになったが、<strong>本人の通過が 15 本から 9 本へ落ちた</strong>
/// ——長さの重なりが最大の案が、値の帯まで良くしてくれるわけではない。
#[cfg(test)]
fn bundle_plan(person: &[usize], pool: &[usize]) -> Vec<Vec<usize>> {
    bundle_plans(person, pool).swap_remove(0)
}

/// 試す順に並べた束ね方。<strong>1 案目が本命で、残りは断られたときの控えである。</strong>
///
/// <strong>通るかどうかは作ってみないと分からない。</strong> 長さの範囲は測れた単位だけで
/// 測られ、どれが測れるかは全文を解析するまで決まらない——ここで計算する重なりは
/// <strong>近似でしかない。</strong>
///
/// だから<strong>選ぶのではなく、順番を付けて渡す。</strong> 決めるのは build である。
fn bundle_plans(person: &[usize], pool: &[usize]) -> Vec<Vec<Vec<usize>>> {
    let single = || -> Vec<Vec<usize>> { (0..pool.len()).map(|i| vec![i]).collect() };
    let (Some(&p_hi), Some(&b_hi)) = (person.iter().max(), pool.iter().max()) else {
        return vec![single()];
    };
    let mut order: Vec<usize> = (0..pool.len()).collect();
    order.sort_by_key(|&i| pool[i]);

    // <strong>本命は今までと同じ案である。</strong> 小さいほうから積んで本人の上端に届かせ、
    // 残りは単独で置く。これが通る素材のほうが多い。
    let mut out: Vec<Vec<Vec<usize>>> = Vec::new();
    if b_hi > 0 && p_hi > b_hi {
        out.push(bundle_to(&order, pool, p_hi, pool.len() / 3));
    }
    out.push(single());

    // <strong>控えは重なりの良い順。</strong> 近似でしかないが、順番を付ける材料はこれしかない。
    let mut rest: Vec<(f64, Vec<Vec<usize>>)> = Vec::new();
    for step in 1..=12u32 {
        let target = p_hi * step as usize / 12;
        for div in [2usize, 3, 4, 6] {
            let plan = bundle_to(&order, pool, target, pool.len() / div);
            if out.contains(&plan) || rest.iter().any(|(_, p)| *p == plan) {
                continue;
            }
            rest.push((overlap_score(person, &lengths_of(&plan, pool)), plan));
        }
    }
    rest.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    out.extend(rest.into_iter().map(|(_, p)| p));
    out
}

/// 目標の上端まで小さいほうから積む。残りは単独で置く。
fn bundle_to(order: &[usize], pool: &[usize], target: usize, budget: usize) -> Vec<Vec<usize>> {
    let mut plan: Vec<Vec<usize>> = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    let mut sum = 0usize;
    for (used, &i) in order.iter().enumerate() {
        if used >= budget {
            break;
        }
        cur.push(i);
        sum += pool[i];
        if sum >= target {
            plan.push(std::mem::take(&mut cur));
            sum = 0;
        }
    }
    if cur.len() > 1 {
        plan.push(cur);
    }
    let bundled: std::collections::BTreeSet<usize> = plan.iter().flatten().copied().collect();
    plan.extend(
        order
            .iter()
            .filter(|i| !bundled.contains(i))
            .map(|&i| vec![i]),
    );
    plan
}

/// 束ねた結果の、単位ごとの長さ。
fn lengths_of(plan: &[Vec<usize>], pool: &[usize]) -> Vec<usize> {
    plan.iter()
        .map(|g| g.iter().map(|&i| pool[i]).sum())
        .collect()
}

/// [防護柵](kakiburi_scale::length_range_ok)の採点。<strong>小さいほうの比を返す。</strong>
///
/// 片方だけ良くても通らないので、最大化するのは<strong>悪いほう</strong>である。
fn overlap_score(person: &[usize], baseline: &[usize]) -> f64 {
    let span = |v: &[usize]| -> Option<(f64, f64)> {
        #[allow(clippy::cast_precision_loss)]
        Some((*v.iter().min()? as f64, *v.iter().max()? as f64))
    };
    let (Some((plo, phi)), Some((blo, bhi))) = (span(person), span(baseline)) else {
        return 0.0;
    };
    let overlap = (phi.min(bhi) - plo.max(blo)).max(0.0);
    (overlap / (phi - plo).max(1.0)).min(overlap / (bhi - blo).max(1.0))
}

/// 目盛りを作る。
///
/// <strong>素材のフォルダを読む。</strong> カセットは本文を持たないので、作り直すたびに
/// フォルダから読み直す（[素材を正本にする](../../../docs/spec/200-extract.md#素材を正本にする)）。
///
/// <strong>作らずに終わる条件を持つ。</strong> 止まっても失敗ではない——目盛りの無いカセットが
/// 出来上がり、`0` で終わる。
fn build(args: &[String]) -> Exit {
    let Some(path) = args.first() else {
        eprintln!("カセットの経路を渡す");
        return Exit::Usage;
    };
    let mut person_dir: Option<String> = None;
    let mut baseline_dir: Option<String> = None;
    let mut other_dir: Option<String> = None;
    // 基準の作り方。<strong>記録の無い基準で作った値は、次に測ったときに比べられない。</strong>
    let mut model: Option<String> = None;
    let mut version: Option<String> = None;
    let mut params: BTreeMap<String, String> = BTreeMap::new();
    let mut topics: Vec<String> = Vec::new();
    let mut json = false;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--json" {
            json = true;
            i += 1;
            continue;
        }
        let Some(v) = args.get(i + 1) else {
            eprintln!("{} に値を渡す", args[i]);
            return Exit::Usage;
        };
        match args[i].as_str() {
            "--person" => person_dir = Some(v.clone()),
            "--baseline" => baseline_dir = Some(v.clone()),
            "--other" => other_dir = Some(v.clone()),
            "--model" => model = Some(v.clone()),
            "--version" => version = Some(v.clone()),
            "--topic" => topics.push(v.clone()),
            "--param" => {
                let Some((k, x)) = v.split_once('=') else {
                    eprintln!("--param は <鍵>=<値>");
                    return Exit::Usage;
                };
                params.insert(k.to_owned(), x.to_owned());
            }
            other => {
                eprintln!("知らない引数: {other}");
                return Exit::Usage;
            }
        }
        i += 2;
    }
    let Some(person_dir) = person_dir else {
        eprintln!("--person に本人の記事が入ったフォルダを渡す");
        eprintln!("<strong>カセットは本文を持たない。</strong> 作り直すたびに素材を読む");
        return Exit::Usage;
    };
    // <strong>作り方を知っているのは同梱の池だけである。</strong> ほかはどこから来たか
    // 分からないので、<strong>版まで名乗らせる</strong>——モデル名だけでは足りない。
    // 版が変われば出力が変わり、[記録の無い基準で作った値は次に測ったときに
    // 比べられない](../../../docs/spec/200-extract.md#版と推論設定まで記録する)。
    //
    // <strong>同梱かどうかは池自身が言う。</strong> 場所では決められない。
    let Some(baseline_dir) = baseline_dir.or_else(baseline_pool) else {
        eprintln!("基準が見つからない。--baseline で渡すか KAKIBURI_BASELINES を指す");
        return Exit::Usage;
    };
    let bundled = pool_version(&baseline_dir);
    let baseline_made = if let (Some(v), None, None) = (&bundled, &model, &version) {
        kakiburi_cassette::Baseline {
            model: POOL_MODEL.to_owned(),
            version: v.clone(),
            params,
            topics,
        }
    } else {
        let (Some(model), Some(version)) = (model, version) else {
            eprintln!("断る: 同梱の池でない基準には --model と --version が要る");
            eprintln!("<strong>モデル名だけでは足りない。</strong> 版が変われば出力が変わる");
            eprintln!("記録の無い基準で作った値は、次に測ったときに比べられない");
            eprintln!("（同梱の池なら {POOL_MARKER} が版を名乗っている）");
            return Exit::Usage;
        };
        kakiburi_cassette::Baseline {
            model,
            version,
            params,
            topics,
        }
    };

    let (mut c, generation) = match open(path) {
        Ok(v) => v,
        Err(e) => return e,
    };
    // <strong>基準の作り方は人が決めたことである。</strong> 派生物ではないので、
    // 目盛りを作り直しても残る。
    c.decided.baseline = baseline_made;

    // <strong>`--json` でも進み方は stderr へ出す。</strong> stdout に混ぜれば、JSON として
    // 読めなくなる。
    let say = |line: String| {
        if json {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    };
    let mecab = analyzer::resolved();
    match &mecab {
        Some(m) => say(format!(
            "形態素解析: MeCab / {} {}",
            m.dict_name, m.dict_version
        )),
        None => {
            say(format!(
                "形態素解析: 無し（{} が未設定）。<strong>形態素を要る系統が測れない</strong>",
                analyzer::DICDIR
            ));
            // <strong>入手先を言う。</strong> 未設定だと言うだけでは、辞書をどこから引くかも
            // どこに置くかも分からず、仕様を読むまで進めない。
            say("  辞書の入手先は kakiburi help の「環境」に書いてある".to_owned());
        }
    }

    let person_files = readable_files(&person_dir);
    if person_files.is_empty() {
        eprintln!("記事が 1 本も見つからない: {person_dir}");
        return Exit::Unreadable;
    }
    let pool_files = readable_files(&baseline_dir);
    if pool_files.is_empty() {
        eprintln!("基準が 1 本も見つからない: {baseline_dir}");
        return Exit::Unreadable;
    }

    let person = match load_units(&person_files, &[]) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let other_files = other_dir.as_deref().map(readable_files).unwrap_or_default();
    let others = match load_units(&other_files, &[]) {
        Ok(v) => v,
        Err(e) => return e,
    };
    if !others.is_empty() {
        say(format!(
            "他人 {} 単位（人らしさの人の側にだけ効く）",
            others.len()
        ));
    }

    // <strong>題材で選んでから、長さで束ねる。</strong> 逆にすると、題材の合わない分を
    // 束ねてから捨てることになる。
    let picked = pick_by_topic(&person_files, &pool_files, POOL_TAKE);
    let pool_files: Vec<String> = picked.iter().map(|&i| pool_files[i].clone()).collect();
    say(format!(
        "基準を {} 本選んだ（題材の近い順）",
        pool_files.len()
    ));

    // <strong>束ね方は作ってみて決める。</strong> 通るかどうかは作るまで分からない——
    // [長さの範囲](kakiburi_scale::length_range_ok)は<strong>測れた単位だけ</strong>で測られ、
    // どれが測れるかは全文を解析するまで分からない。
    //
    // <strong>手前で当てにいくと外れる。</strong> 字数と読点で近似して計画したが、延べ語数と
    // 地の文のバイト数が効いて<strong>本人 37 本のうち使えたのは 17 本</strong>だった。
    // 見ている分布が違えば、束ね方も違う。
    let person_lengths = measurable_chars_of(&person_files);
    let pool_lengths = japanese_chars_of(&pool_files);
    let plans = bundle_plans(&person_lengths, &pool_lengths);
    let last = plans.len() - 1;
    let mut report = kakiburi_cassette::json::Value::Null;
    for (attempt, plan) in plans.into_iter().enumerate() {
        if attempt > 0 {
            say(format!(
                "長さの範囲で断られたので、束ね方を変えて作り直す（{} 回目）",
                attempt + 1
            ));
        }
        let bundled = plan.iter().filter(|g| g.len() > 1).count();
        if bundled > 0 {
            say(format!(
                "基準のうち {bundled} 単位を束ねる。本人の長い記事に、1 本では届かない"
            ));
        }
        let baseline = match load_units(&pool_files, &plan) {
            Ok(v) => v,
            Err(e) => return e,
        };
        // <strong>名前は役を跨いで一意である。</strong> 測るときは名前で引くので、
        // 重なれば<strong>片方の値がもう片方の値で黙って置き換わる</strong>——本人の単位が
        // 基準の数字で測られ、壊れた帯が正常な顔でカセットに入る。
        if let Err(why) = check_names(&person, &baseline, &others) {
            eprintln!("断る: {why}");
            eprintln!("<strong>名前はファイル名である。</strong> 分けたいならファイル名を分ける");
            return Exit::Usage;
        }
        report = build_scene(&mut c, &person, &baseline, &others, mecab.as_ref(), json, &say);
        if attempt == last || c.derived.has_scale() {
            break;
        }
        // <strong>環境が壊れているなら束ね直しても直らない。</strong> 案を全部試せば、
        // 直らないことを何度も確かめるだけで時間が溶ける。
        if report.get("reason").and_then(kakiburi_cassette::json::Value::as_str)
            == Some(BROKEN_ENVIRONMENT)
        {
            break;
        }
    }

    // <strong>読んだ取り込み元を指紋に置く。</strong> 別の取り込み元で読み直したのに指紋が
    // 古いままだと、照らしても違いが出ない。
    //
    // <strong>他人の文書も入れる。</strong> 人らしさの較正に実際に効くので、そこだけ
    // 別の取り込み元で読んでも指紋が動かないと、<strong>較正が変わったことを言えない。</strong>
    let read: Vec<String> = person_files
        .iter()
        .chain(pool_files.iter())
        .chain(other_files.iter())
        .cloned()
        .collect();
    set_sources(&mut c, &read);
    // <strong>指紋を作り直す。</strong> 語彙と z 得点と道具が値を決めるので、目盛りができた
    // 時点で指紋も変わる——変えなければ、次に検めるときに合わないことが分からない。
    refresh(&mut c);
    if let Err(e) = store_back(path, &c, generation) {
        return e;
    }
    if json {
        println!("{}", report.write());
        return Exit::Pass;
    }
    println!();
    println!("入れた: {path}");
    Exit::Pass
}

/// 単位の名前が役を跨いで一意かを検める。
///
/// <strong>役で名前空間を分けない。</strong> 分ければ、本人の `Rust入門` と基準の `Rust入門` が
/// 別物として通る——[測るときは名前で引く](kakiburi_scale::assemble)ので、重なれば
/// <strong>片方の測定値がもう片方で置き換わる。</strong>
///
/// <strong>[題材を揃える](../../../docs/spec/200-extract.md#題材の統制は対ではなく素材に効かせる)ほど
/// 同じ名前が付きやすい</strong>ので、いちばん正しく集めた人がいちばん踏む。
fn check_names(
    person: &[(String, kakiburi_doc::Document)],
    baseline: &[(String, kakiburi_doc::Document)],
    others: &[(String, kakiburi_doc::Document)],
) -> Result<(), String> {
    let mut seen: BTreeMap<&str, &'static str> = BTreeMap::new();
    for (role, units) in [("本人", person), ("基準", baseline), ("他人", others)] {
        for (name, _) in units {
            if let Some(first) = seen.insert(name.as_str(), role) {
                return Err(format!("単位の名前が重なっている: `{name}`（{first} と {role}）"));
            }
        }
    }
    Ok(())
}

/// 素材のファイルを単位にする。
///
/// <strong>1 本でも断ったら何も返さない。</strong> 黙って一部を落として通せば、欠けたまま
/// 目盛りが出来上がる。
///
/// `plan` が空でなければ、<strong>その組ごとに 1 単位へ束ねる</strong>——1 文書では指標が
/// 意味を持たないほど短いものを、何本かでまとめて数えるためである。
fn load_units(
    files: &[String],
    plan: &[Vec<usize>],
) -> Result<Vec<(String, kakiburi_doc::Document)>, Exit> {
    let mut docs: Vec<kakiburi_doc::Document> = Vec::with_capacity(files.len());
    for f in files {
        let Ok(body) = std::fs::read_to_string(f) else {
            eprintln!("読めない: {f}");
            return Err(Exit::Unreadable);
        };
        // <strong>取り込み元は拡張子から決める。</strong> 既定ではなく判別である——
        // 取り違えれば[0 が並ぶ](../../../docs/spec/030-normalize.md#取り込み元を間違えると0-が並ぶ)。
        match normalize(&body, source_of(f)) {
            Ok(d) => docs.push(d),
            Err(e) => {
                eprintln!("断る: {f}: {e}");
                eprintln!("1 本でも断ったら使わない。欠けたまま次へ進まないため");
                return Err(Exit::Unreadable);
            }
        }
    }
    if plan.is_empty() {
        return Ok(files
            .iter()
            .map(|f| stem_of(f))
            .zip(docs)
            .collect());
    }
    // <strong>取り込み元が違うものは束ねない</strong>（[仕様](../../../docs/spec/200-extract.md#短い文書は束ねる)）。
    // [升目は取り込み元ごとに効く](../../../docs/spec/030-normalize.md#対応表は取り込み元ごとに持つ)
    // ので、混ぜると<strong>単位ごとに測れる指標が変わる。</strong>
    for group in plan {
        let mut kinds = group.iter().map(|&i| source_of(&files[i]));
        let first = kinds.next();
        if kinds.any(|k| Some(k) != first) {
            eprintln!("断る: 取り込み元の違うものを束ねようとした");
            eprintln!("<strong>升目は取り込み元ごとに効く。</strong> 混ぜると測れる指標が単位ごとに変わる");
            return Err(Exit::Usage);
        }
    }
    let mut out = Vec::with_capacity(plan.len());
    for group in plan {
        let nodes: Vec<kakiburi_doc::node::Node> = group
            .iter()
            .flat_map(|&i| docs[i].nodes.iter().cloned())
            .collect();
        // <strong>束ねた単位は、中身を名前にする。</strong> 連番にすると
        // [束の構成と並び](../../../docs/spec/200-extract.md#短い文書は束ねる)が
        // どこにも残らず、<strong>割りの表に名前だけが並んで中身が見えない</strong>
        // ——偏っていても気付けない。<strong>並びも名前がそのまま持つ。</strong>
        let name = group
            .iter()
            .map(|&i| stem_of(&files[i]))
            .collect::<Vec<_>>()
            .join("+");
        out.push((name, kakiburi_doc::Document::new(nodes)));
    }
    Ok(out)
}


/// 環境の側で測れないときの理由。<strong>束ね直しでは直らない印である。</strong>
const BROKEN_ENVIRONMENT: &str = "環境の側で測れない指標がある";

/// 目盛りを作る。<strong>作らずに終わる条件を持つ。</strong>
///
/// 何が起きたかを返す——<strong>道具向けの出口が要る</strong>ので、出力を組み立てながら
/// 進み方を捨ててしまわない。
fn build_scene(
    c: &mut Cassette,
    person_units: &[(String, kakiburi_doc::Document)],
    baseline_units: &[(String, kakiburi_doc::Document)],
    other_units: &[(String, kakiburi_doc::Document)],
    mecab: Option<&kakiburi_metrics::mecab::Mecab>,
    json: bool,
    say: &dyn Fn(String),
) -> kakiburi_cassette::json::Value {
    use kakiburi_cassette::json::Value;
    #[allow(clippy::cast_precision_loss)]
    let n = |v: usize| Value::Number(v as f64);
    let stopped = |why: String| {
        Value::obj([
            ("built".to_owned(), Value::Bool(false)),
            ("reason".to_owned(), Value::s(why)),
        ])
    };

    // <strong>定型は測るときだけ落とす。</strong> 人が決め直したら測り直せる形にしておく。
    let person_units = stripped(c, person_units);
    let baseline_units = stripped(c, baseline_units);
    let person = samples(&person_units);
    let baseline = samples(&baseline_units);
    // <strong>他人の文書に落とす定型は掛けない。</strong> 定型は本人の書きぶりについて
    // 人が決めたものである。
    let others = samples(other_units);
    say(format!(
        "本人 {} 単位 / 基準 {} 単位",
        person.len(),
        baseline.len()
    ));
    let boilerplate = c.decided.boilerplate.len();
    if boilerplate > 0 {
        say(format!("落とす定型 {boilerplate} 本"));
    }
    let a = mecab.map(|m| m as &dyn kakiburi_metrics::morph::Analyzer);

    // <strong>環境の側の理由で測れないものがあれば、目盛りを作らない。</strong>
    // 直すのはコーパスではなく環境であり、直せば全部の値が変わる——このまま進めば、
    // 壊れた環境で出た値が正常な顔でカセットに入る。
    let broken = broken_environment(&person, &baseline, a);
    if !broken.is_empty() {
        say("目盛りを作らない: 環境の側で測れない指標がある".to_owned());
        for (name, why) in &broken {
            say(format!("  {name}: {why}"));
        }
        say("<strong>素材ではなく環境を直す。</strong> 足しても直らない".to_owned());
        c.drop_derived();
        return stopped(BROKEN_ENVIRONMENT.to_owned());
    }

    let material = kakiburi_scale::assemble::Material {
        person: &person,
        baseline: &baseline,
        others: &others,
    };
    let scale = match assemble(material, a) {
        Ok(s) => s,
        Err(e) => {
            // <strong>作らずに終わる。</strong> 止まっても失敗ではない。
            say(format!("目盛りを作らない: {e}"));
            // <strong>どの単位のどこで止まったかを言う。</strong>「10 本に届かない」だけでは、
            // 素材を足すべきか、長さを揃えるべきか、辞書を入れるべきかが分からない。
            let mine = kakiburi_scale::inspect(&person, a);
            print_reports("本人", &mine, json);
            print_reports("基準", &kakiburi_scale::inspect(&baseline, a), json);
            // <strong>次の一手を言う。</strong> 止まった理由だけでは、素材を足せばよいのか
            // 長さを揃えればよいのかが分からない——<strong>本人側が短さで落ちているなら、
            // 足しても直らない。</strong>
            for line in next_step(&mine) {
                say(line);
            }
            c.drop_derived();
            return stopped(e.to_string());
        }
    };

    say(String::new());
    // <strong>どう割れたかを出す。</strong> 単位名の昇順で取るので、名前に年や媒体が入って
    // いれば相手集合と測る分がその境目で分かれる——<strong>値は出るし、エラーにもならない。</strong>
    // 帯が「本人 対 本人」ではなく「ある時期 対 別の時期」になっていても、出さなければ
    // 出力から区別が付かない。
    for line in selection_lines(&scale.selection) {
        say(line);
    }

    say(String::new());
    for (name, set) in &scale.frozen {
        say(format!("  {name:<12} {:>5} 次元", set.len()));
    }
    say(format!(
        "照合値の帯: 天井 {:.3}〜{:.3} / 床 {:.3}〜{:.3}",
        scale.band.ceiling.low,
        scale.band.ceiling.high,
        scale.band.floor.low,
        scale.band.floor.high
    ));
    // <strong>向きを支えられなかった次元を数える。</strong> 指標の定義は、先行研究に基づいて
    // どちらが機械の側かを名乗っている。<strong>素材がその向きを否定したなら、目盛りは
    // 作れても人らしさを名乗れない</strong>——黙って出せば、重なった帯が「判定できない」
    // として出るだけで、原因が基準の側にあることが誰にも見えない。
    // <strong>分けていないので落とした次元を先に言う。</strong> 言わなければ、次の行が
    // その次元を「定義と逆に出た」として数えてしまう——落とした理由は向きではない。
    let dropped = scale.humanness.ineffective_dims();
    if !dropped.is_empty() {
        say(format!(
            "人らしさ: {} / {} 次元が本人と機械を分けておらず、合算から外れた（{}）",
            dropped.len(),
            scale.humanness.dims().len(),
            dropped.join("、")
        ));
    }
    let bad: Vec<String> = scale
        .humanness
        .contradicting_dims()
        .into_iter()
        .filter(|n| !dropped.contains(n))
        .collect();
    if !bad.is_empty() {
        say(format!(
            "人らしさ: {} / {} 次元が定義と逆に出た（{}）",
            bad.len(),
            scale.humanness.dims().len(),
            bad.join("、")
        ));
        // <strong>食い違いは止める理由ではない。</strong> 寄せる向きは較正から読むので、
        // 定義と逆でも直し方は渡せる——<strong>逆だと分かったことを言うだけである。</strong>
        let toward = scale.humanness.toward_human();
        say(format!(
            "  寄せる向き（較正が決めた）: {}",
            if toward.is_empty() {
                "無し。<strong>どの指標も次元の向きが割れている</strong>".to_owned()
            } else {
                toward
                    .iter()
                    .map(|(m, up)| format!("{m} を{}", if *up { "上げる" } else { "下げる" }))
                    .collect::<Vec<_>>()
                    .join("、")
            }
        ));
    }
    say(format!(
        "人らしさの帯: 人 {:.3}〜{:.3} / 機械 {:.3}〜{:.3}",
        scale.humanness_band.ceiling.low,
        scale.humanness_band.ceiling.high,
        scale.humanness_band.floor.low,
        scale.humanness_band.floor.high
    ));
    if scale.humanness.evenly_spread() {
        // 語彙の狭さを見る 5 つは同じ現象を別の角度から見ている。<strong>均等に開いたら較正を疑う。</strong>
        eprintln!("但し書き: 人らしさの合算が指標に均等に開いている。較正を疑う");
    }

    // <strong>効くかの判定はここで出す。</strong> 検めが作り直せる形にしておくと、検める文書を
    // 見てから幅や集合を作り直す経路が書けてしまう。
    // **効くかの判定も、下端と同じ分け方に従う。** 密度や個数では 0 が「使わなかった」
    // を意味するので、素の幅で見ると**いちばん指示しやすい指標が落ちる。**
    let defs = remedies::FromDefinitions::load();
    let effective =
        kakiburi_scale::effective::judge(&rows(&person, a), &rows(&baseline, a), &|n| {
            defs.by_appearance(n)
        });
    let works = effective.iter().filter(|e| e.works()).count();
    say(format!("効く指標: {works} / {} 本", effective.len()));

    // <strong>本人がいちばん高く出るかは、ここでしか測れない。</strong> カセットは本文を
    // 持たないので、作り終えたあとに測り直す道が無い。
    say(String::new());
    let suspect = check_person_higher(&scale, &person, &baseline, a, say);

    let report = Value::obj([
        ("built".to_owned(), Value::Bool(true)),
        (
            "selection".to_owned(),
            Value::obj([
                (
                    "person_partners".to_owned(),
                    machine::strings(&scale.selection.person_partners),
                ),
                (
                    "person_points".to_owned(),
                    machine::strings(&scale.selection.person_points),
                ),
                (
                    "baseline_partners".to_owned(),
                    machine::strings(&scale.selection.baseline_partners),
                ),
                (
                    "baseline_points".to_owned(),
                    machine::strings(&scale.selection.baseline_points),
                ),
            ]),
        ),
        ("matching_band".to_owned(), band_json(scale.band)),
        ("humanness_band".to_owned(), band_json(scale.humanness_band)),
        ("effective".to_owned(), n(works)),
        ("metrics".to_owned(), n(effective.len())),
        ("person_higher".to_owned(), Value::Bool(suspect == 0)),
        (
            "evenly_spread".to_owned(),
            Value::Bool(scale.humanness.evenly_spread()),
        ),
    ]);

    // <strong>作り終えた目盛りだけを入れる。</strong> 検めはこれを受け取る。
    //
    // <strong>言い回しの表は本文の代わりである。</strong> どの言い回しを訊かれるかは検める
    // まで決まらないので、ここで畳んでおかなければ繰り返しの上限を言えない。
    c.derived = Derived {
        vocabulary: Some(vocabulary_note(&scale)),
        values: None,
        spread: Some(effective_json::write_spread(&effective)),
        calibration: Some(format!("系統 {} 本の較正と合算", scale.frozen.len())),
        scale: Some(scale_json::write(&scale)),
        effective: Some(effective_json::write_effective(&effective)),
        phrases: Some(phrase_table(&person_units, a)),
    };
    report
}

/// 帯を道具向けにする。
fn band_json(b: kakiburi_scale::Band) -> kakiburi_cassette::json::Value {
    use kakiburi_cassette::json::Value;
    Value::obj([
        (
            "ceiling".to_owned(),
            Value::Array(vec![
                Value::Number(b.ceiling.low),
                Value::Number(b.ceiling.high),
            ]),
        ),
        (
            "floor".to_owned(),
            Value::Array(vec![
                Value::Number(b.floor.low),
                Value::Number(b.floor.high),
            ]),
        ),
    ])
}

/// 割りを読める形にする。<strong>4 つとも出す。</strong>
///
/// <strong>相手集合だけでは、どこで割れたかが読めない。</strong>
fn selection_lines(s: &kakiburi_scale::Selection) -> Vec<String> {
    let mut out = vec!["割り（単位名の昇順）".to_owned()];
    for (label, names) in [
        ("本人の相手集合", &s.person_partners),
        ("本人の測る分  ", &s.person_points),
        ("基準の較正分  ", &s.baseline_partners),
        ("基準の床の点  ", &s.baseline_points),
    ] {
        out.push(format!("  {label} {}", names.join("、")));
    }
    out
}

/// 桁を区切る。<strong>下限は文書でも区切って書いてある。</strong>
fn with_commas(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// 本人側が短さで止まっているときの、次の一手。
///
/// <strong>「10 本に届かない」だけでは足りない。</strong> 素材を足すべきか、長さを揃えるべきか、
/// 辞書を入れるべきかが分からない——<strong>短さで落ちているなら、同じ長さのものを
/// いくら足しても届かない。</strong>
fn next_step(mine: &[kakiburi_scale::Report]) -> Vec<String> {
    let usable = mine.iter().filter(|r| r.usable()).count();
    if usable >= kakiburi_scale::split::UNITS_FLOOR {
        return Vec::new();
    }
    let short = mine
        .iter()
        .filter(|r| !r.usable() && r.chars < kakiburi_metrics::floor::JAPANESE_CHARS)
        .count();
    // <strong>落ちた分の大半が短さなら、足しても直らない。</strong>
    if short * 2 < mine.len() - usable {
        return Vec::new();
    }
    vec![
        String::new(),
        "<strong>短い文書ばかりなので、同じものを足しても届かない。</strong>".to_owned(),
        format!(
            "  1 単位が地の文 {} 字に届く必要がある。いまは {short} 本がそこで落ちている",
            with_commas(kakiburi_metrics::floor::JAPANESE_CHARS)
        ),
        "  <strong>いまの道具は本人側を束ねられない。</strong> 長い文書を素材にするか、".to_owned(),
        "  1 つのファイルにまとめてから渡す".to_owned(),
    ]
}

/// 単位ごとの内訳を出す。<strong>止まった理由を単位まで下ろす。</strong>
fn print_reports(side: &str, reports: &[kakiburi_scale::Report], json: bool) {
    let say = |line: String| {
        if json {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    };
    let usable = reports.iter().filter(|r| r.usable()).count();
    say(String::new());
    say(format!(
        "{side}: 使える単位 {usable} / {} 本",
        reports.len()
    ));
    // <strong>短さでまとめて言う。</strong> どの指標が落ちたかを 1 本ずつ並べても、
    // <strong>読み手が知りたいのは「なぜ」と「次に何をするか」である</strong>
    // ——同じ理由の単位が 15 本並ぶのは、情報ではなく雑音である。
    let short: Vec<&kakiburi_scale::Report> = reports
        .iter()
        .filter(|r| !r.usable() && r.chars < kakiburi_metrics::floor::JAPANESE_CHARS)
        .collect();
    if !short.is_empty() {
        let longest = short.iter().map(|r| r.chars).max().unwrap_or(0);
        say(format!(
            "  <strong>{} 本が短すぎる。</strong> 地の文の日本語が {} 字に届かない（いちばん長いもので {longest} 字）",
            short.len(),
            with_commas(kakiburi_metrics::floor::JAPANESE_CHARS)
        ));
        say("  <strong>短い文書は測れない。</strong> 分布と呼べる形にならないものを 0 で埋めない".to_owned());
    }
    // 残りは 1 本ずつ言う。<strong>長さで説明が付かないものは、理由が違う。</strong>
    for r in reports {
        if r.usable() || r.chars < kakiburi_metrics::floor::JAPANESE_CHARS {
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
        say(format!(
            "  {} ({} 字): {}",
            r.name,
            r.chars,
            why.join(" / ")
        ));
    }
}

/// 単位から<strong>定型を落とした写し</strong>を作る。
///
/// <strong>素材は変えない。</strong> 定型は[人が決めたこと](../../../docs/spec/200-extract.md#定型を落とす)
/// であって本文ではないので、決め直したら測り直せる形にしておく。
fn stripped(
    c: &Cassette,
    units: &[(String, kakiburi_doc::Document)],
) -> Vec<(String, kakiburi_doc::Document)> {
    units
        .iter()
        .map(|(name, doc)| {
            (
                name.clone(),
                doc.without_boilerplate(&c.decided.boilerplate),
            )
        })
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
        for (name, m) in humanness_of(s.document, analyzer).flat() {
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

/// 人らしさを測る。<strong>識別子を伏せてから測る。</strong>
///
/// [目盛りを作る側](kakiburi_scale)が同じ前処理を掛けている。掛けないと、`measure` が
/// 出す値と `review` が判定に使う値が食い違う——<strong>同じ文書で違う数を 2 つ出す道具</strong>に
/// なる。実際そうなっていて、生の圧縮率がほぼ同じ本人と基準の記事で、寄与が
/// <strong>+2.604 と −0.539</strong> に割れていた。
///
/// <strong>掛けるのは人らしさと照合だけである。</strong> 指示できる指標は和欧間スペースや
/// 半角英字そのものを測るので、生の文から測り続ける。
fn humanness_of(
    doc: &kakiburi_doc::Document,
    analyzer: Option<&dyn kakiburi_metrics::morph::Analyzer>,
) -> kakiburi_metrics::Humanness {
    let prose = kakiburi_doc::prose::mask_identifiers(&doc.prose());
    let analyzed = analyzed_of(&prose, analyzer);
    kakiburi_metrics::Humanness::measure(&prose, analyzed.as_ref())
}

/// 解析し終えた形。<strong>解析器が無ければ `None`。</strong>
fn analyzed_of(
    prose: &[kakiburi_doc::prose::Segment],
    analyzer: Option<&dyn kakiburi_metrics::morph::Analyzer>,
) -> Option<kakiburi_metrics::morph::Analyzed> {
    analyzer.and_then(|a| kakiburi_metrics::morph::Analyzed::of(prose, a).ok())
}

/// 判定できないで終える。
///
/// <strong>`--json` でも必ず JSON を出す。</strong> 途中で抜ける道だけ人向けの文にすると、
/// 道具の側は<strong>「出力が無い」を自分で場合分けする</strong>ことになる——そこは
/// 判定できないと同じ側であって、壊れたわけではない。
fn unknown(json: bool, scene: &str, source: Source, c: &Cassette, reason: &str) -> Exit {
    let outcome = judge(&[], None, None, &[]);
    if json {
        use kakiburi_cassette::json::Value;
        println!(
            "{}",
            Value::obj([
                ("scene".to_owned(), Value::s(scene)),
                ("source".to_owned(), Value::s(source.name())),
                (
                    "verdict".to_owned(),
                    Value::s(verdict_name(outcome.verdict))
                ),
                ("stage".to_owned(), Value::s(outcome.stage.name())),
                ("reason".to_owned(), Value::s(reason)),
                ("humanness".to_owned(), Value::Null),
                ("missing_humanness".to_owned(), Value::Array(vec![])),
                ("matching".to_owned(), Value::Null),
                ("missing_systems".to_owned(), Value::Array(vec![])),
                ("directives".to_owned(), Value::Number(0.0)),
                ("points".to_owned(), Value::Array(vec![])),
                // <strong>早く抜けても欄は同じである。</strong> 欄が消えれば、読む側は
                // 「出なかった」と「そもそも無い」を区別できない。
                ("humanness_points".to_owned(), Value::Array(vec![])),
                ("humanness_by_metric".to_owned(), Value::obj([])),
                ("matching_points".to_owned(), Value::Array(vec![])),
                ("matching_by_dim".to_owned(), Value::Array(vec![])),
                ("provisional".to_owned(), machine::strings(&c.provisional)),
            ])
            .write()
        );
        return Exit::from_verdict(outcome.verdict);
    }
    println!("判定: 判定できない");
    println!("止まった段: {}", outcome.stage.name());
    println!("理由: {reason}");
    Exit::from_verdict(outcome.verdict)
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
    let mut source: Option<Source> = None;
    let mut json = false;
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
                source = Some(s);
                i += 2;
            }
            "--json" => {
                json = true;
                i += 1;
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
    let Some(source) = source else {
        return missing_source();
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

    // <strong>場面はカセットが言う。</strong>[1 カセットが 1 場面](../../../docs/spec/010-strategy.md#場面ごとに閉じる)
    // なので、入れ物を選ぶことが場面を選ぶことである——選び間違いは、検める前に
    // <strong>どのファイルを渡すかとして現れる。</strong>
    let scene = c.scene.clone();

    // <strong>指紋を先に照らす。</strong> 合わないカセットで測れば、比べたものに意味が無い。
    // 判定できないではなく <strong>使う前の問題</strong>である——64 以上で返す。
    if let Err(diff) = check_fingerprint(&c) {
        eprintln!("指紋が環境と合わない: {}", diff.join("、"));
        eprintln!("測り直しが要る。過去の値とは比べられない。");
        return Exit::FingerprintMismatch;
    }

    if !c.provisional.is_empty() {
        eprintln!(
            "但し書き: 暫定値が立っている（{}）",
            c.provisional.join("、")
        );
    }

    // <strong>目盛りが無ければ判定できない。</strong> 素材が足りずに作れなかったのは正常な
    // 状態であり、仕様がそのために判定できないを置いている。
    let Some(scale) = c.derived.scale.as_deref().and_then(scale_json::read) else {
        return unknown(
            json,
            &scene,
            source,
            &c,
            "目盛りが無い。素材が足りずに作れなかった",
        );
    };

    // <strong>検める側にも同じ定型を掛ける。</strong> 片方だけに掛ければ、落とした分だけ値が
    // ずれたものを比べることになる（[同じ測り方で測る](../../../docs/spec/300-revise.md#同じ測り方で測る)）。
    let doc = doc.without_boilerplate(&c.decided.boilerplate);

    // <strong>相手集合は目盛りが持っている。</strong> カセットは本文を持たないので、
    // 検める側が本文から選び直す経路そのものが無い。
    //
    // <strong>本数だけでは足りない。</strong> 次元がずれていても距離は短いほうまでで
    // 計算されるので、<strong>エラーにならずに違う照合値が出る。</strong>
    if !scale.partner_vectors_ok() {
        eprintln!("相手集合が目盛りと噛み合わない。目盛りが壊れている");
        eprintln!("素材のフォルダを指して build し直す");
        return Exit::FingerprintMismatch;
    }

    let mecab = analyzer::resolved();
    let got = measure_against(
        &scale,
        Sample {
            name: path,
            document: &doc,
        },
        mecab
            .as_ref()
            .map(|m| m as &dyn kakiburi_metrics::morph::Analyzer),
    );

    // <strong>照合値のどこが違うのかを言えるようにする。</strong> 1 つの数のままでは、帯の中で
    // 止まったときに受け取った側が動きようがない。
    //
    // <strong>次元が語として読める系統だけを見る。</strong> 品詞 bigram の「名詞-助詞」を
    // 増やせとは言えない。
    let readable = [
        kakiburi_metrics::System::FunctionWord,
        kakiburi_metrics::System::Comma,
        kakiburi_metrics::System::CharType,
    ];
    // <strong>畳む前の距離を出す。</strong> 照合値は 5 つの距離を重みで畳んだものなので、
    // 畳んだあとだけではどこが動いたか分からない。
    let distances = kakiburi_scale::assemble::distances_against(
        &scale,
        Sample {
            name: path,
            document: &doc,
        },
        mecab
            .as_ref()
            .map(|m| m as &dyn kakiburi_metrics::morph::Analyzer),
    );
    let diverging = kakiburi_scale::diverging(
        &scale,
        Sample {
            name: path,
            document: &doc,
        },
        mecab
            .as_ref()
            .map(|m| m as &dyn kakiburi_metrics::morph::Analyzer),
        &readable,
        12,
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
        return unknown(
            json,
            &scene,
            source,
            &c,
            "効くかの判定が入っていない。build し直しが要る",
        );
    };
    let defs = remedies::FromDefinitions::load();
    if defs.is_empty() {
        // <strong>黙って指摘を落とさない。</strong> 直し方の出どころが無ければ、判定は出ても
        // 指摘が 1 本も出ない——それを「幅の中だった」と読まれてはいけない。
        eprintln!("但し書き: 定義ファイルが見つからない。指摘の文を引けない");
    } else if json {
        // <strong>どこから引いたかは結果ではない。</strong> stdout に混ぜれば JSON が読めない。
        eprintln!("直し方の出どころ: 定義ファイル {} 本", defs.len());
    } else {
        println!("直し方の出どころ: 定義ファイル {} 本", defs.len());
    }
    // <strong>前に出す指標。</strong> 効くと判定されたものから、層 3 と動かないものを除く
    // （[3 段](../../../docs/spec/300-revise.md#種別を合わせて通るを出す)）。
    // <strong>判定も指摘も、この同じ集合から取る。</strong>
    // <strong>検める側も同じ解析器で測る。</strong> 片方だけ違えば、比べたものに意味が無い。
    // <strong>カセットが持つ辞書で割る。</strong> 作ったときと違う割り方をすれば、
    // 比べたものに意味が無い。
    let analyzed_now = mecab.as_ref().and_then(|m| {
        kakiburi_metrics::morph::Analyzed::with_lexicon(
            &doc.prose(),
            m as &dyn kakiburi_metrics::morph::Analyzer,
            &scale.lexicon,
        )
        .ok()
    });
    let measured_now = measured_with(&doc, analyzed_now.as_ref());
    // 0 段目。書き方。
    //
    // カセットを見ない。日本語として成立しているかは書き手の性質ではないので、
    // 線は定義ファイルが持つ。
    let inspections: Vec<kakiburi_review::Inspected> = defs
        .inspections()
        .into_iter()
        .map(|(name, upper, limit, remedy)| {
            let value = measured_now
                .iter()
                .find(|(n, _)| *n == name)
                .and_then(|(_, m)| m.value());
            // <strong>どこが壊れているかを渡す。</strong> 「読点を外せ」と言うだけでは、
            // 受け取った側は文書ぜんぶを読み直すことになる。
            let where_ = match name.as_str() {
                "語を割る読点" => analyzed_now
                    .as_ref()
                    .map(|a| a.split_commas().join("」「"))
                    .filter(|s| !s.is_empty())
                    .map(|s| format!("「{s}」。")),
                _ => None,
            };
            kakiburi_review::Inspected {
                broken: match value {
                    Some(v) => format!(
                        "{name}が {} で、線の {limit} を超えている。{}{remedy}",
                        number(v),
                        where_.unwrap_or_default()
                    ),
                    None => format!("{name}が線の {limit} を超えている。{remedy}"),
                },
                name,
                value,
                limit,
                upper,
            }
        })
        .collect();
    let directives: Vec<Observed> = effective
        .iter()
        // 条件 1 と 2。
        .filter(|e| e.works())
        // 検査は幅で見ない。**比べる先が本人ではない。**
        .filter(|e| !defs.is_inspection(&e.name))
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
    // <strong>一貫しているだけの軸も見る。判定はしない、指摘にだけ出す。</strong>
    //
    // 条件 2 は「基準と本人が違うか」で軸を選ぶので、<strong>基準と本人が一致している軸は
    // 捨てられる</strong>——そこから草稿が外れても何も言われない。
    let habits: Vec<Observed> = effective
        .iter()
        .filter(|e| e.narrow_only())
        .filter(|e| !c.is_stuck(&e.name))
        .filter(|e| !defs.is_layer_three(&e.name))
        .filter(|e| !defs.is_inspection(&e.name))
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
    // <strong>人らしさの直し方は、指標ごとの値から組む。</strong> 合算した 1 つの値では
    // 「機械の側にある」としか言えず、直し方を渡せない。
    let by_metric: Vec<kakiburi_review::HumannessObserved> = got
        .humanness_by_metric
        .iter()
        .map(|m| kakiburi_review::HumannessObserved {
            name: m.name.clone(),
            value: m.value,
            raise: m.raise,
            // <strong>長い繰り返しにだけ添える。</strong> ほかの指標に言い回しを付けても、
            // どう使えばよいかを言えない。
            phrases: if m.name == "長い繰り返し" {
                scale.phrases.clone()
            } else {
                Vec::new()
            },
            overused: m.overused.clone(),
            once_only: m.once_only.clone(),
            effect: m.effect,
            target: m.target,
        })
        .collect();
    let apart: Vec<kakiburi_review::MatchingObserved> = diverging
        .iter()
        .map(|d| kakiburi_review::MatchingObserved {
            system: d.system.clone(),
            dim: d.dim.clone(),
            mine: d.mine,
            theirs: d.theirs,
            effect: d.effect(),
            examples: d.examples.clone(),
            spots: d.spots.clone(),
        })
        .collect();
    // <strong>型が使われているか。</strong> 地の文から探す——記法の外にある並びは型ではない。
    let joined = kakiburi_metrics::humanness::joined(&doc.prose());
    let ja = doc.japanese_chars();
    // <strong>出ている箇所と回数を数える。</strong>「言い換えろ」と言うなら、どこを言い換えるのかを言う。
    // <strong>回数も要る</strong>——繰り返し出ているものほど、その機械の癖である。
    const SPOTS_SHOWN: usize = 2;
    let spots_of = |needle: &str| -> (Vec<String>, usize) {
        const AROUND: usize = 12;
        let (mut out, mut times) = (Vec::new(), 0usize);
        let pat: Vec<char> = needle.chars().collect();
        if pat.is_empty() {
            return (out, 0);
        }
        for line in joined.split('\n') {
            let chars: Vec<char> = line.chars().collect();
            let mut i = 0usize;
            while i + pat.len() <= chars.len() {
                if chars[i..i + pat.len()] != pat[..] {
                    i += 1;
                    continue;
                }
                times += 1;
                if out.len() < SPOTS_SHOWN {
                    let from = i.saturating_sub(AROUND);
                    let to = (i + pat.len() + AROUND).min(chars.len());
                    out.push(chars[from..to].iter().collect::<String>());
                }
                i += pat.len();
            }
        }
        (out, times)
    };
    let seen = |ks: &[kakiburi_scale::assemble::Kata]| -> Vec<kakiburi_review::Kata> {
        ks.iter()
            .map(|k| {
                let (spots, times) = spots_of(&k.text);
                #[allow(clippy::cast_precision_loss)]
                let density = if ja == 0 {
                    0.0
                } else {
                    1000.0 * times as f64 / ja as f64
                };
                kakiburi_review::Kata {
                    spots,
                    times,
                    density,
                    ceiling: k.ceiling,
                    base: k.base,
                    // <strong>穴あきは、固定部が 2 つともこの順で同じ段落にあれば使われている。</strong>
                    // 間は書き手が埋めるので、そこは見ない。
                    used: match &k.tail {
                        Some(t) => joined.split('\n').any(|line| {
                            line.find(&k.text)
                                .and_then(|i| line[i + k.text.len()..].find(t.as_str()))
                                .is_some()
                        }),
                        None => joined.contains(&k.text),
                    },
                    text: k.shown(),
                    rate: k.rate,
                    at: k.at,
                }
            })
            .collect()
    };
    // <strong>直し方に載せた言い回しも、同じ見方で数える。</strong> 使いすぎを止めるためである。
    let phrases: Vec<kakiburi_review::Kata> = scale
        .phrase_ceilings
        .iter()
        .map(|(text, ceiling)| {
            let (spots, times) = spots_of(text);
            #[allow(clippy::cast_precision_loss)]
            let density = if ja == 0 {
                0.0
            } else {
                1000.0 * times as f64 / ja as f64
            };
            kakiburi_review::Kata {
                text: text.clone(),
                rate: 0.0,
                // <strong>本人の側は分からない。</strong> ここは草稿から作った行なので、
                // 本人の割合を持たない——言い切らせないために 0 を渡さない筋も無い。
                base: 0.0,
                at: 0.0,
                used: times > 0,
                spots,
                times,
                density,
                ceiling: *ceiling,
            }
        })
        .collect();
    // <strong>この文章が繰り返している言い回しも、同じ物差しに乗せる。</strong>
    //
    // <strong>上限を持っていたのは道具が「繰り返せ」と言った分だけだった。</strong> 受け取った
    // 側が自分で足した言い回しは誰も見ていない——実測で、人らしさを通すために
    // <strong>本人が 49 本で 6 本しか使わない `ことになります` を 1 本に 6 回</strong>入れても、
    // 道具は何も言わなかった。
    //
    // <strong>機械の型では拾えない。</strong> あれは基準の 10% 以上が使う並びしか名指しできず、
    // この言い回しは池 44 本のうち 1 本にしか出てこない。
    //
    // <strong>本人の上限は畳んだ表から引く。</strong> どの言い回しを訊かれるかは検めるまで
    // 決まらないので、<strong>`build` が 2 つ以上の単位に現れるものを全部表にしてある</strong>
    // ——[表に無いものは上限 0](../../../docs/design/100-cassette.md#phrasesjsonl--本文の代わり)
    // で、本人が一度も使っていないことがそのまま指摘になる。
    let ceilings = read_phrase_table(&c);
    let phrases: Vec<kakiburi_review::Kata> = phrases
        .into_iter()
        .chain(
            repeated_in_draft(analyzed_now.as_ref())
                .into_iter()
                .filter(|(text, _)| {
                    !scale
                        .phrase_ceilings
                        .iter()
                        .any(|(p, _)| p == text || text.contains(p) || p.contains(text))
                })
                .map(|(text, times)| {
                    let (spots, _) = spots_of(&text);
                    #[allow(clippy::cast_precision_loss)]
                    let density = if ja == 0 {
                        0.0
                    } else {
                        1000.0 * times as f64 / ja as f64
                    };
                    let ceiling = person_ceiling(&ceilings, &text);
                    kakiburi_review::Kata {
                        text,
                        rate: 0.0,
                        base: 0.0,
                        at: 0.0,
                        used: true,
                        spots,
                        times,
                        density,
                        ceiling,
                    }
                }),
        )
        .collect();
    let katas = seen(&scale.katas);
    // <strong>役を入れ替えた側も同じ見方で拾う。</strong>
    let machine_katas = seen(&scale.machine_katas);
    // <strong>語は文字列ではなく語彙素で照らす。</strong> そのまま探すと `地味` が `地味な` の
    // 前半に当たるだけで、`地味だ` と `地味に` を別のものとして数えてしまう。
    let here = kakiburi_metrics::word::goi(analyzed_now.as_ref());
    let machine_gois: Vec<kakiburi_review::Goi> = scale
        .machine_gois
        .iter()
        .map(|g| kakiburi_review::Goi {
            times: here.get(&g.text).map_or(0, |(_, n)| *n),
            text: g.text.clone(),
            rate: g.rate,
            base: g.base,
            theirs: g.theirs.clone(),
        })
        .collect();
    let result = kakiburi_review::review(
        &kakiburi_review::Observations {
            inspections: &inspections,
            humanness,
            matching,
            directives: &directives,
            habits: &habits,
            humanness_by_metric: &by_metric,
            diverging: &apart,
            katas: &katas,
            machine_katas: &machine_katas,
            machine_gois: &machine_gois,
            phrases: &phrases,
        },
        &defs,
    );

    if json {
        // <strong>人向けの表示は変えない。</strong> 出すのは同じ値の生の形である。
        use kakiburi_cassette::json::Value;
        #[allow(clippy::cast_precision_loss)]
        let n = |v: usize| Value::Number(v as f64);
        println!(
            "{}",
            Value::obj([
                ("scene".to_owned(), Value::s(&scene)),
                ("source".to_owned(), Value::s(source.name())),
                (
                    "verdict".to_owned(),
                    Value::s(verdict_name(result.outcome.verdict))
                ),
                ("stage".to_owned(), Value::s(result.outcome.stage.name())),
                ("reason".to_owned(), Value::s(&result.outcome.reason)),
                ("humanness".to_owned(), machine::number(got.humanness)),
                (
                    "missing_humanness".to_owned(),
                    machine::strings(&got.missing_humanness),
                ),
                (
                    // <strong>指標ごとの観測。</strong> 合算した 1 つの値だけでは、どの指標が
                    // 隔たりを担っているかを言えない——直し方を選ぶ根拠が消える。
                    //
                    // <strong>向きも出す。</strong> 落とせば、向きを知りたい読み手は
                    // `humanness_points` の散文を切り出すしかなくなる——
                    // [道具向けの出力](machine)が避けようとした経路そのものである。
                    "humanness_by_metric".to_owned(),
                    Value::obj(got.humanness_by_metric.iter().map(|m| {
                        (
                            m.name.clone(),
                            Value::obj([
                                ("value".to_owned(), Value::Number(m.value)),
                                ("raise".to_owned(), Value::Bool(m.raise)),
                            ]),
                        )
                    })),
                ),
                ("matching".to_owned(), machine::number(got.matching)),
                (
                    "missing_systems".to_owned(),
                    machine::strings(&got.missing_systems),
                ),
                ("directives".to_owned(), n(directives.len())),
                (
                    "distances".to_owned(),
                    Value::obj(
                        distances
                            .iter()
                            .map(|(k, v)| (k.clone(), Value::Number(*v)))
                            .collect::<Vec<_>>(),
                    ),
                ),
                (
                    "katas".to_owned(),
                    Value::Array(result.katas.iter().map(Value::s).collect()),
                ),
                // <strong>人向けに出しているものは、道具向けにも出す。</strong> 片方にしか
                // 無ければ、<strong>同じ検めでも読む口で結論が変わる</strong>——
                // `--json` は表示を変えないという約束が、そこで崩れる。
                (
                    "machine_katas".to_owned(),
                    Value::Array(result.machine_katas.iter().map(Value::s).collect()),
                ),
                (
                    "machine_gois".to_owned(),
                    Value::Array(result.machine_gois.iter().map(Value::s).collect()),
                ),
                (
                    // <strong>指摘は結果であって断り書きではない。</strong> ここに入れる。
                    "points".to_owned(),
                    Value::Array(result.points.iter().map(|p| Value::s(p.prose())).collect()),
                ),
                (
                    // <strong>書きぶりの枠と混ぜない。</strong> 別の欄に出す——混ぜれば、
                    // 機械臭さを消す指示と、その人へ寄せる指示が席を取り合う。
                    "humanness_points".to_owned(),
                    machine::strings(&result.humanness),
                ),
                (
                    "matching_points".to_owned(),
                    machine::strings(&result.matching),
                ),
                (
                    // <strong>散文だけでは検証できない。</strong> 予測した効きが当たったかを
                    // 確かめるには、次元と値がそのまま要る。
                    "matching_by_dim".to_owned(),
                    Value::Array(
                        diverging
                            .iter()
                            .map(|d| {
                                Value::obj([
                                    ("system".to_owned(), Value::s(&d.system)),
                                    ("dim".to_owned(), Value::s(&d.dim)),
                                    ("mine".to_owned(), Value::Number(d.mine)),
                                    ("theirs".to_owned(), Value::Number(d.theirs)),
                                    ("outside".to_owned(), Value::Number(d.outside())),
                                    ("effect".to_owned(), Value::Number(d.effect())),
                                ])
                            })
                            .collect(),
                    ),
                ),
                ("provisional".to_owned(), machine::strings(&c.provisional)),
            ])
            .write()
        );
        return Exit::from_verdict(result.outcome.verdict);
    }

    println!("前に出す指標: {} 本", directives.len());
    println!();
    // <strong>長さで黙るなら、長さで黙ると言う。</strong> 直すと短くなり、下限を割って測れなく
    // なる——<strong>直した側には、道具が壊れたのか自分が削りすぎたのかが分からない。</strong>
    let tokens = mecab
        .as_ref()
        .and_then(|m| {
            analyzed_of(
                &doc.prose(),
                Some(m as &dyn kakiburi_metrics::morph::Analyzer),
            )
        })
        .map(|a| a.tokens());
    if let Some(n) = tokens {
        let floor = kakiburi_metrics::floor::TOKENS;
        if n < floor {
            println!(
                "延べ {n} 語。<strong>下限 {floor} 語に届かないので測れない</strong>——{} 語ぶん足りない",
                floor - n
            );
        } else if n < floor + floor / 5 {
            println!(
                "延べ {n} 語。<strong>下限 {floor} 語に近い</strong>——これ以上削ると測れなくなる"
            );
        }
    }
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
    if !result.matching.is_empty() {
        println!();
        println!("照合の直し方 {} 本", result.matching.len());
        for m in &result.matching {
            println!("  - {m}");
        }
    }
    if !result.humanness.is_empty() {
        println!();
        println!("人らしさの直し方 {} 本", result.humanness.len());
        for h in &result.humanness {
            println!("  - {h}");
        }
    }
    if !result.habits.is_empty() {
        println!();
        println!("本人の癖から外れているところ {} 本", result.habits.len());
        for h in &result.habits {
            println!("  - {}", h.prose());
        }
    }
    if !result.katas.is_empty() {
        println!();
        println!("使われていない型 {} 本", result.katas.len());
        for k in &result.katas {
            println!("  - {k}");
        }
    }
    if !result.machine_katas.is_empty() {
        println!();
        println!("残っている機械の型 {} 本", result.machine_katas.len());
        for k in &result.machine_katas {
            println!("  - {k}");
        }
    }
    if !result.machine_gois.is_empty() {
        println!();
        println!("残っている機械の語 {} 本", result.machine_gois.len());
        for g in &result.machine_gois {
            println!("  - {g}");
        }
    }
    if !result.overused_katas.is_empty() {
        println!();
        println!("使いすぎている言い回し {} 本", result.overused_katas.len());
        for k in &result.overused_katas {
            println!("  - {k}");
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
    Fingerprint::build(base_inputs(None))
}

/// カセットに入れる指紋。<strong>目盛りができた時点で変わる。</strong>
///
/// 語彙と z 得点と道具が値を決めるので、目盛りを入れたら指紋も入れ替える——
/// <strong>入れ替えなければ、次に検めるときに合わないことが分からない。</strong>
///
/// <strong>場面ごとの部分は、その場面の目盛りから読む。</strong> 1 つの場面を `build` した
/// だけで、ほかの場面の語彙が消えてはいけない。
fn fingerprint_with(c: &Cassette, mecab: Option<&kakiburi_metrics::mecab::Mecab>) -> Fingerprint {
    let mut inputs = base_inputs(mecab);
    // カセットが決めたことは引き継ぐ。取り込み元は環境の側で作り直せない。
    inputs.common.normalization.sources = c.fingerprint.inputs.common.normalization.sources.clone();
    let scale = c.derived.scale.as_deref().and_then(scale_json::read);
    inputs.scene = kakiburi_cassette::SceneInputs {
        vocabulary: vocabulary_of(scale.as_ref()),
        z_scores: z_scores_of(scale.as_ref()),
        selection: selection_of(scale.as_ref()),
        // <strong>基準の作り方は `decided` が正本である。</strong> 指紋に写した値ではなく、
        // 人が決めたほうを読む——写しを読むと、決め直したのに指紋が動かない。
        baseline: c.decided.baseline.clone(),
        decided: decided_inputs(&c.scene, &c.decided),
    };
    Fingerprint::build(inputs)
}

/// 固定した語彙。<strong>次元の並びが変われば値が変わる。</strong>
fn vocabulary_of(scale: Option<&Scale>) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    let Some(s) = scale else { return out };
    for (name, set) in &s.frozen {
        let mut dims = Vec::new();
        for (i, part) in set.parts().iter().enumerate() {
            for d in part.dims() {
                dims.push(format!("{i}:{d}"));
            }
        }
        out.insert(name.clone(), dims);
    }
    out
}

/// 単位の割り。<strong>束の構成と並びもここに出る。</strong>
///
/// <strong>[束ねた単位は中身を名前にする](../../../docs/spec/200-extract.md#短い文書は束ねる)</strong>
/// ので、どの文書をどの順で束ねたかがそのまま並びに入る。
fn selection_of(scale: Option<&Scale>) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    let Some(s) = scale else { return out };
    for (name, names) in [
        ("本人の相手集合", &s.selection.person_partners),
        ("本人の測る分", &s.selection.person_points),
        ("基準の較正分", &s.selection.baseline_partners),
        ("基準の床の点", &s.selection.baseline_points),
    ] {
        out.insert(name.to_owned(), names.clone());
    }
    out
}

/// 固定した z 得点の平均と標準偏差。
fn z_scores_of(scale: Option<&Scale>) -> BTreeMap<String, Vec<(f64, f64)>> {
    let mut out = BTreeMap::new();
    let Some(s) = scale else { return out };
    for (name, set) in &s.frozen {
        let mut zs = Vec::new();
        for part in set.parts() {
            for j in 0..part.dims().len() {
                zs.push((part.mean()[j], part.sd()[j]));
            }
        }
        out.insert(name.clone(), zs);
    }
    out
}

/// 取り込み元を渡していない。
///
/// <strong>取り込み元に既定を置かない。</strong> 役に既定を置かないのと同じ理由である——
/// <strong>取り違えても、エラーは出ない。</strong> HTML を `github-markdown` として読めば、
/// 見出しも箇条書きも記法として認識されず、節も項目も文も違う数になる。
/// 値だけが静かに変わるので、<strong>出力を見ても間違いに気付けない。</strong>
fn missing_source() -> Exit {
    eprintln!("--source を渡す（{}）", source_names().join(" / "));
    eprintln!("<strong>既定を置かない。</strong> 取り違えても数が変わるだけで、エラーにならない");
    Exit::Usage
}

/// 使える取り込み元の名前。
fn source_names() -> Vec<&'static str> {
    Source::all().iter().map(|s| s.name()).collect()
}

/// 人が決めたこと。<strong>指紋に入る。</strong>
///
/// <strong>場面がいちばん効く。</strong> どのカセットのファイルを渡すかが場面の指定になっている
/// のに、入れなければ <strong>取り違えても指紋が通る</strong>——1 場面 1 ファイルという切り方が、
/// それ自体の守りを持たないことになる。
///
/// <strong>空にしない。</strong> 空対空の比較は必ず一致するので、差が出る道が閉じる。
fn decided_inputs(scene: &str, d: &Decided) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    // <strong>場面の名前も入れる。</strong> 材料は場面ごとに分かれたが、名前そのものが
    // 中身に写っていなければ、名前だけを変えたことが差にならない。
    out.insert("場面".to_owned(), scene.to_owned());
    // <strong>落とす定型は本文を変える。</strong> 変えたのに指紋が動かなければ、落とす前の値と
    // 落としたあとの値が同じ顔で並ぶ。
    out.insert("落とす定型".to_owned(), d.boilerplate.join("\u{1F}"));
    // 基準の題材は `SceneInputs::baseline` が持つので、ここには入れない——同じものを
    // 2 か所に入れると、変わったときに違う名前で 2 回言うことになる。
    //
    // <strong>指示して動くか。</strong> 前に出す指標が変われば、判定も指摘も変わる。
    out.insert(
        "動かないと分かった指標".to_owned(),
        d.movement
            .iter()
            .filter(|(_, m)| **m == kakiburi_cassette::Movement::Stuck)
            .map(|(k, _)| k.clone())
            .collect::<Vec<_>>()
            .join("、"),
    );
    out
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

/// 指紋の材料。<strong>共通部分だけを環境から作る。</strong>
///
/// 場面ごとの部分はカセットの側にしか無い。
fn base_inputs(mecab: Option<&kakiburi_metrics::mecab::Mecab>) -> Inputs {
    Inputs {
        common: Common {
            // <strong>本数を指紋にしない。</strong> 同じ本数のまま数え方・除外・直し方を変えれば、
            // 値の意味が変わったのに指紋が動かず、古い派生値が使い回される。
            metric_definitions: remedies::FromDefinitions::load().digest(),
            unit_definitions: format!(
                "kakiburi-doc {} / Unicode {}",
                env!("CARGO_PKG_VERSION"),
                kakiburi_doc::text::UNICODE_VERSION,
            ),
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
        },
        scene: kakiburi_cassette::SceneInputs::default(),
    }
}

/// カセットの指紋を、いまの環境と照らす。
///
/// <strong>合わなければ何が違うかを言う。</strong> ハッシュだけでは、変わったことは分かっても
/// 何が変わったかが分からない。
///
/// <strong>照らす相手は、道具と実装と定義の側である。</strong> 辞書を入れ替えた、圧縮器が
/// 変わった、指標が増えた——そこが変われば過去の値と比べられない。取り込み元と語彙は
/// カセットが決めたことなので、カセットのものを引き継いで照らす。
fn check_fingerprint(c: &Cassette) -> Result<(), Vec<String>> {
    let mecab = analyzer::resolved();
    let here = fingerprint_with(c, mecab.as_ref());
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
    let mut source: Option<Source> = None;
    let mut json = false;
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
            source = Some(s);
            i += 2;
            continue;
        }
        if args[i] == "--json" {
            json = true;
            i += 1;
            continue;
        }
        eprintln!("知らない引数: {}", args[i]);
        return Exit::Usage;
    }
    let Some(source) = source else {
        return missing_source();
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

    // <strong>解析は 1 度だけ行い、両方の出し方が同じ値を使う。</strong>
    //
    // 別々に解析すると<strong>片方だけが解析器を捨てる</strong>。実際そうなっていた——
    // 人向けは `語を割る読点 0.000`、`--json` は `道具が無い` を返していた。
    // <strong>同じ入力に対して、測れたか測れていないかが出し方で変わってはいけない。</strong>
    let mecab = analyzer::resolved();
    let analyzed = analyzed_of(
        &doc.prose(),
        mecab
            .as_ref()
            .map(|m| m as &dyn kakiburi_metrics::morph::Analyzer),
    );
    #[allow(clippy::cast_precision_loss)]
    let tokens = analyzed.as_ref().map(|a| a.tokens() as f64);
    let values = measured_with(&doc, analyzed.as_ref());

    if json {
        // <strong>人向けの表示は変えない。</strong> 出すのは同じ値の生の形である。
        println!(
            "{}",
            kakiburi_cassette::json::Value::obj([
                (
                    "source".to_owned(),
                    kakiburi_cassette::json::Value::s(source.name())
                ),
                (
                    "japanese_chars".to_owned(),
                    #[allow(clippy::cast_precision_loss)]
                    kakiburi_cassette::json::Value::Number(doc.japanese_chars() as f64),
                ),
                ("tokens".to_owned(), machine::number(tokens)),
                ("structure".to_owned(), machine::structure(&doc)),
                ("directives".to_owned(), machine::metrics(&values)),
                (
                    // <strong>人らしさの生の値も出す。</strong> カセットが無くても測れる値であり、
                    // 出さなければ<strong>素材が向きを支えているかを外から確かめられない</strong>。
                    "humanness".to_owned(),
                    machine::metrics(
                        &humanness_of(
                            &doc,
                            mecab
                                .as_ref()
                                .map(|m| m as &dyn kakiburi_metrics::morph::Analyzer)
                        )
                        .flat()
                    ),
                ),
            ])
            .write()
        );
        return Exit::Pass;
    }

    println!("取り込み元 {}", source.name());
    println!("日本語 {} 字", doc.japanese_chars());
    // <strong>形態素の数も出す。</strong> 字数で足りていても語で足りないことがあり、
    // そのとき何が測れないかが字数からは分からない。
    if let (Some(n), Some(m)) = (tokens, mecab.as_ref()) {
        println!("延べ {n} 語（{} {}）", m.dict_name, m.dict_version);
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
    for (name, m) in values {
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

/// 数を散文に載せる。<strong>個数を `1.0000` と書かない。</strong>
fn number(v: f64) -> String {
    if (v - v.round()).abs() < f64::EPSILON {
        format!("{v:.0}")
    } else {
        format!("{v:.4}")
    }
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
    out.push((
        "語を割る読点".to_owned(),
        kakiburi_metrics::word::splitting_commas(analyzed),
    ));

    // <strong>文末の軸は node の種類ごとに出す。</strong> 1 つの定義が種類の数だけ軸を作るので、
    // 種類が増えても指標の側を書き足さなくてよい。
    out.extend(structure::register_rates(&p, analyzed));

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
        ("和文間スペース", symbol::wabun_space(p)),
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
    fn 人らしさは識別子を伏せてから測る() {
        // <strong>目盛りを作る側が同じ前処理を掛けている。</strong> 掛けないと、`measure` が出す値と
        // `review` が判定に使う値が食い違う——同じ文書で違う数を 2 つ出す道具になる。
        // <strong>圧縮率の下限を越える長さが要る。</strong> 越えないとどちらも「測っていない」に
        // なり、同じ値として通ってしまう。
        let body = "ここでは denops.vim という名前のプラグインを書いていきます。\n\n".repeat(200);
        let doc = normalize(&body, Source::PlainMarkdown).expect("正規化できる");
        let masked = humanness_of(&doc, None);
        let raw = kakiburi_metrics::Humanness::measure(&doc.prose(), None);
        assert_ne!(
            masked, raw,
            "識別子だらけの文章で、伏せた値と伏せない値が同じになっている"
        );
    }

    #[test]
    fn 測る値は出し方で変わらない() {
        // <strong>片方だけが解析器を捨てる</strong>という壊れ方をした。人向けは
        // `語を割る読点 0.000`、`--json` は `道具が無い` を返していた。
        //
        // <strong>解析器の有無で測れる軸は変わってよい。</strong> 変わってはいけないのは、
        // 同じ解析器を渡したのに出し方で結果が違うことである。
        let doc = normalize(
            "あらためて取得し直す。\n\n次の段落。",
            Source::PlainMarkdown,
        )
        .expect("正規化できる");
        let a = analyzed_of(&doc.prose(), None);
        assert_eq!(
            measured_with(&doc, a.as_ref()),
            measured_with(&doc, a.as_ref()),
            "同じ入力と同じ解析器からは同じ値が出る"
        );
        // 解析器を捨てれば、要る軸は<strong>0 ではなく「道具が無い」</strong>になる。
        let without: Vec<Measured> = measured_with(&doc, None)
            .into_iter()
            .filter(|(n, _)| n == "語を割る読点")
            .map(|(_, m)| m)
            .collect();
        assert_eq!(without, vec![Measured::ToolMissing]);
    }

    #[test]
    fn 池が本人の長さに届かなければ束ねる() {
        // <strong>池の 1 本は地の文 4,500 字あたりで頭打ちになる。</strong> 本人に長い記事が
        // あると長さの範囲の防護柵に当たり、目盛りが作れない。実測では、池を
        // 題材でも長さでも広げたのに本人の範囲との重なりが 28% から 43% までしか
        // 伸びず、束ねて初めて通った。
        let pool = vec![2200, 2900, 3000, 4400, 2400, 2600, 3200, 3800, 2300];
        let plan = bundle_plan(&[1500, 3000, 4000], &pool);
        assert!(
            plan.iter().all(|g| g.len() == 1),
            "届くなら束ねない: {plan:?}"
        );

        let plan = bundle_plan(&[1500, 7000], &pool);
        assert!(
            plan.iter().any(|g| g.len() > 1),
            "届かないなら束ねる: {plan:?}"
        );
        assert!(
            plan.iter().filter(|g| g.len() == 1).count() >= pool.len() / 2,
            "単独の分を残す——全部束ねると単位の本数が下限を割る: {plan:?}"
        );
        let used: Vec<usize> = plan.iter().flatten().copied().collect();
        assert_eq!(used.len(), pool.len(), "池を余さず使う");
    }

    #[test]
    fn 束ね方は防護柵の式で選ぶ() {
        // <strong>勘で決めた 1 案では落ちる素材がある。</strong> 取り置きの総当たりで、
        // 4 分割のうち 1 つが長さの範囲で目盛りを作れず、取り置いた 12 本が
        // 丸ごと判定できないになっていた。
        //
        // <strong>候補を作って、通さなければならない式そのもので採点する。</strong>
        let pool = vec![2200, 2900, 3000, 4400, 2400, 2600, 3200, 3800, 2300];
        for person in [
            vec![1500, 7000],
            vec![900, 12000],
            vec![3000, 3200, 18000],
            vec![2500, 2600, 2700],
        ] {
            let plan = bundle_plan(&person, &pool);
            let got = overlap_score(&person, &lengths_of(&plan, &pool));
            let flat: Vec<Vec<usize>> = (0..pool.len()).map(|i| vec![i]).collect();
            let flat_score = overlap_score(&person, &lengths_of(&flat, &pool));
            assert!(
                got >= flat_score,
                "束ねて悪くなる案は採らない: {person:?} で {got} < {flat_score}"
            );
            let used: Vec<usize> = plan.iter().flatten().copied().collect();
            assert_eq!(used.len(), pool.len(), "池を余さず使う: {person:?}");
        }
    }

    #[test]
    fn 採点は悪いほうの比を返す() {
        // <strong>片方だけ良くても通らない。</strong> 防護柵は両方に 0.5 を要求する。
        // 基準が本人の範囲に完全に含まれていると、本人側の比だけが効く。
        let wide = overlap_score(&[1000, 5000], &[2000, 3000]);
        assert!(wide < 0.5, "基準が狭すぎれば落ちる: {wide}");
        let same = overlap_score(&[1000, 5000], &[1000, 5000]);
        assert!((same - 1.0).abs() < 1e-9, "同じ範囲なら 1.0: {same}");
    }

    #[test]
    fn 池が空でも落ちない() {
        assert!(bundle_plan(&[1500], &[]).is_empty());
        assert_eq!(bundle_plan(&[], &[2000]).len(), 1);
    }

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
        let args = [
            "measure".to_owned(),
            "/存在しない経路/x.md".to_owned(),
            "--source".to_owned(),
            "plain-markdown".to_owned(),
        ];
        assert_eq!(run(&args), Exit::Unreadable);
    }

    #[test]
    fn 割りは_4_つとも読める形で出る() {
        // 昇順で取るので、単位名に年や媒体が入っていれば割りがその境目で分かれる。
        // <strong>値は出るしエラーにもならない</strong>ので、出さなければ帯が「ある時期 対
        // 別の時期」になっていることに気付けない。
        let s = fixture::scale();
        let lines = selection_lines(&s.selection);
        assert_eq!(lines.len(), 5, "見出し 1 行と 4 つの割り");
        for (line, names) in lines[1..].iter().zip([
            &s.selection.person_partners,
            &s.selection.person_points,
            &s.selection.baseline_partners,
            &s.selection.baseline_points,
        ]) {
            assert!(!names.is_empty(), "割りが空になっている");
            for n in names {
                assert!(line.contains(n.as_str()), "{line} に {n} が無い");
            }
        }
    }

    #[test]
    fn 取り込み元を省いたら断る() {
        // HTML を github-markdown として読めば、節も項目も文も違う数になる——
        // <strong>エラーは出ず、値だけが静かに変わる。</strong>
        let dir = temp_dir("no-source");
        let f = a_document(&dir, "x");
        let c = empty_cassette(&dir);
        assert_eq!(
            run(&["measure".to_owned(), f.clone()]),
            Exit::Usage,
            "measure"
        );
        assert_eq!(
            run(&["compare".to_owned(), f.clone(), f.clone()]),
            Exit::Usage,
            "compare"
        );
        assert_eq!(
            run(&["review".to_owned(), f, "--cassette".to_owned(), c.clone(),]),
            Exit::Usage,
            "review"
        );
        // 断ったのだから、目盛りもできていない。
        let (got, _) = open(&c).expect("読める");
        assert!(!got.derived.has_scale());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 指標の一覧が出る() {
        assert_eq!(run(&["metrics".to_owned()]), Exit::Pass);
        // <strong>定義と軸は 1 対 1 ではない。</strong> ほとんどの定義は軸 1 本を作るが、
        // 接続詞直後の読点は語彙素 × 位置に、文末の軸は node の種類に展開される。
        //
        // <strong>展開の分は数え上げない。</strong> 展開する側から取る——ここに数を書くと、
        // 種類や語彙素を足すたびに 2 か所を直すことになる。
        let expanded = kakiburi_metrics::word::conjunction_comma_names().len()
            + kakiburi_metrics::structure::register_names().len();
        assert_eq!(measured_names().len(), 40 + expanded);
        assert_eq!(
            kakiburi_metrics::word::conjunction_comma_names().len(),
            24,
            "展開後の軸"
        );
        // <strong>地の文に入る種類ごとに、敬体率と体言止め率が 1 本ずつ。</strong>
        assert_eq!(
            kakiburi_metrics::structure::register_names().len(),
            kakiburi_doc::node::Kind::PROSE.len() * 2,
            "文末の軸"
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
    /// 目盛りの入ったカセットを置く。<strong>作るのは目盛りの側である。</strong>
    fn cassette_with_scale(dir: &std::path::Path) -> String {
        let scale = fixture::scale();
        let (person, baseline) = fixture::corpus();
        let mut c = Cassette {
            version: store::VERSION,
            generation: 0,
            fingerprint: current_fingerprint(),
            provisional: vec![],
            scene: "試験".into(),
            decided: Decided::default(),
            derived: Derived::dropped(),
        };
        c.derived.scale = Some(scale_json::write(&scale));
        // <strong>効くかの判定も入れる。</strong> 入れなければ、検めはそこで判定できないを返す
        // ——別の理由で止まるので、通したい経路が通っていないことに気付けない。
        let effective = kakiburi_scale::effective::judge(
            &rows(&fixture::samples(&person), Some(&fixture::Chars)),
            &rows(&fixture::samples(&baseline), Some(&fixture::Chars)),
            &|n| remedies::FromDefinitions::load().by_appearance(n),
        );
        c.derived.spread = Some(effective_json::write_spread(&effective));
        c.derived.effective = Some(effective_json::write_effective(&effective));
        c.derived.phrases = Some(phrase_table(&person, Some(&fixture::Chars)));
        // <strong>指紋も目盛りに合わせる。</strong> 合わせなければ、検めが使う前に断る。
        refresh(&mut c);
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
            "--source".to_owned(),
            "plain-markdown".to_owned(),
        ];
        // 短い 1 本なので除外に掛かる。<strong>0 ではなく「測れていない」が返る</strong>ので、
        // 1 段目で止まって判定できないになる。
        assert_eq!(run(&args), Exit::Unknown);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 検めは場面を訊かない() {
        // <strong>1 カセットが 1 場面である。</strong> 入れ物を選ぶことが場面を選ぶこと
        // なので、引数で二重に言わせない——言わせれば、食い違ったときに正本が
        // 決まらない。
        let dir = temp_dir("review-no-scene");
        let cassette = cassette_with_scale(&dir);
        let target = dir.join("検める.md");
        std::fs::write(&target, "これは、そうだ、と思う。\n").expect("書ける");
        assert_eq!(
            run(&[
                "review".to_owned(),
                target.to_string_lossy().into_owned(),
                "--cassette".to_owned(),
                cassette,
                "--source".to_owned(),
                "plain-markdown".to_owned(),
                "--scene".to_owned(),
                "試験".to_owned(),
            ]),
            Exit::Usage,
            "--scene はもう無い"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 空のカセットを 1 つ作る。
    fn empty_cassette(dir: &std::path::Path) -> String {
        let p = dir.join("c.kbc").to_string_lossy().into_owned();
        assert_eq!(
            run(&["new".to_owned(), p.clone(), "試験".to_owned()]),
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
    fn 場面を名乗らなければ作らない() {
        // <strong>場面は人が指定する。</strong> 文章から当てにいかない。
        let dir = temp_dir("new-needs-scene");
        let p = dir.join("c.kbc").to_string_lossy().into_owned();
        assert_eq!(run(&["new".to_owned(), p.clone()]), Exit::Usage);
        assert_eq!(
            run(&["new".to_owned(), p, "   ".to_owned()]),
            Exit::Usage,
            "空白だけの名前も断る"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 中身を出す() {
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
    fn 検査は素材を要らない() {
        // <strong>カセット単体で矛盾していないかを見る。</strong> 本人がいちばん高く出るかは
        // build が測る——そちらは素材を持っているときにしか言えない。
        let dir = temp_dir("doctor-no-material");
        let cassette = cassette_with_scale(&dir);
        // 素材のフォルダをどこにも渡していないのに、最後まで走る。
        assert!(matches!(
            run(&["doctor".to_owned(), cassette]),
            Exit::Pass | Exit::Unknown
        ));
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
    fn 既に在るところには作らない() {
        // 人が決めたことは作り直せない。上書きすれば、そこで消える。
        let dir = temp_dir("new-exists");
        let c = empty_cassette(&dir);
        assert_eq!(
            run(&["new".to_owned(), c.clone(), "別の場面".to_owned()]),
            Exit::Usage,
            "既に在れば断る"
        );
        let (got, _) = open(&c).expect("読める");
        assert_eq!(got.scene, "試験", "断ったのだから中身は変わらない");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 場面が違えば指紋が違う() {
        // <strong>場面は指紋の材料である。</strong> 入れなければ、どの場面として検めても
        // 指紋が通ってしまう。
        let dir = temp_dir("scene-fingerprint");
        let mut made = Vec::new();
        for (name, scene) in [("a", "技術記事"), ("b", "議事録")] {
            let p = dir.join(name).to_string_lossy().into_owned();
            assert_eq!(
                run(&["new".to_owned(), p.clone(), scene.to_owned()]),
                Exit::Pass
            );
            made.push(open(&p).expect("読める").0);
        }
        assert_eq!(
            made[0].fingerprint.differences(&made[1].fingerprint),
            vec!["人が決めたこと"]
        );
        // <strong>道具は同じである。</strong> 場面が違うだけで「辞書が変わった」と言わない。
        assert!(made[0]
            .fingerprint
            .common_differences(&made[1].fingerprint)
            .is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 各コマンドが自分の節を出す() {
        // 位置引数がファイルなので、そのまま渡すと `--help` がファイル名として
        // 解釈され、<strong>「読めない（65）」で終わる。</strong>
        for name in [
            "measure", "new", "decide", "build", "quick", "review", "compare", "show", "doctor",
            "metrics",
        ] {
            assert_eq!(
                run(&[name.to_owned(), "--help".to_owned()]),
                Exit::Pass,
                "{name}"
            );
            assert!(section(name).is_some(), "{name}");
        }
        // <strong>カセットを渡したあとでも効く。</strong> 実際の打ち方はこちらである。
        assert_eq!(
            run(&[
                "build".to_owned(),
                "/存在しない経路/x.kbc".to_owned(),
                "--help".to_owned(),
            ]),
            Exit::Pass
        );
    }

    #[test]
    fn 消したコマンドは知らないコマンドである() {
        // <strong>黙って別の意味で受けない。</strong> 素材はフォルダを指して渡すように
        // なったので、単位を 1 本ずつ入れる道はもう無い。
        for name in ["scene", "add", "replace"] {
            assert_eq!(run(&[name.to_owned()]), Exit::Usage, "{name}");
            assert!(section(name).is_none(), "{name}");
        }
    }

    #[test]
    fn 全体の_help_に環境変数と取り込み元が出る() {
        // 未設定だと言うだけでは、辞書をどこから引くかが分からない。
        for name in [analyzer::DICDIR, analyzer::VERSION, analyzer::PROGRAM] {
            assert!(ENVIRONMENT.contains(name), "{name}");
        }
        assert!(ENVIRONMENT.contains("unidic-mecab-2.1.2_bin.zip"));
        // 一覧を 2 か所に書かない。
        for s in source_names() {
            assert!(Source::from_name(s).is_some(), "{s}");
        }
    }

    #[test]
    fn 道具向けの出口は_json_だけを出す() {
        // 但し書きと本文が混ざれば、JSON として読めなくなる。
        let dir = temp_dir("json");
        let f = a_document(&dir, "x");
        assert_eq!(
            run(&[
                "measure".to_owned(),
                f,
                "--source".to_owned(),
                "plain-markdown".to_owned(),
                "--json".to_owned(),
            ]),
            Exit::Pass
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 作った直後の指紋は環境と合う() {
        let dir = temp_dir("new-fingerprint");
        let c = empty_cassette(&dir);
        let (got, _) = open(&c).expect("読める");
        assert_eq!(check_fingerprint(&got), Ok(()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 落とす定型と動かない指標も指紋に入る() {
        // どちらも派生物を変える。指紋が動かなければ、変える前の値と変えたあとの
        // 値が同じ顔で並ぶ。
        // <strong>解析器は環境から取る。</strong> カセットに入る指紋は解析器を含むので、
        // ここで `None` を渡すと、辞書を持っている環境でだけ「形態素解析器」が
        // 差として出て落ちる——見たいのは決めたことが指紋に入るかである。
        let dir = temp_dir("decided-fingerprint");
        let c = empty_cassette(&dir);
        let mecab = analyzer::resolved();
        let mecab = mecab.as_ref();
        let before = open(&c).expect("読める").0.fingerprint;

        assert_eq!(
            run(&[
                "decide".to_owned(),
                c.clone(),
                "boilerplate".to_owned(),
                "お世話になっており".to_owned(),
            ]),
            Exit::Pass
        );
        let after = fingerprint_with(&open(&c).expect("読める").0, mecab);
        assert_eq!(before.differences(&after), vec!["人が決めたこと"]);

        let metric = measured_names().first().expect("指標が要る").clone();
        assert_eq!(
            run(&[
                "decide".to_owned(),
                c.clone(),
                "movement".to_owned(),
                metric,
                "stuck".to_owned(),
            ]),
            Exit::Pass
        );
        let stuck = fingerprint_with(&open(&c).expect("読める").0, mecab);
        assert_eq!(after.differences(&stuck), vec!["人が決めたこと"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 素材から作り直すと同じものが出る() {
        // <strong>[素材を正本にする](../../../docs/spec/200-extract.md#素材を正本にする)が
        // 本当かを確かめる。</strong> 通らなければ、派生物のどこかに原本が混ざっている
        // ——気付かないまま運用すると、測り直した瞬間に人が決めたことが消える。
        //
        // <strong>実際に消して、実際に build する。</strong> 控えを戻す形では build を 1 度も
        // 呼ばずに通ってしまう。
        // <strong>辞書が無ければ目盛りは作れない。</strong> そのまま走らせると、捨てた派生物と
        // 作り直した派生物がどちらも空で一致し、<strong>何も確かめずに緑になる。</strong>
        let Some(_) = analyzer::resolved() else {
            eprintln!("辞書が無いので飛ばす（{} を指す）", analyzer::DICDIR);
            return;
        };
        let dir = temp_dir("rebuild");
        let person = dir.join("person");
        let baseline = dir.join("baseline");
        std::fs::create_dir_all(&person).expect("作れる");
        std::fs::create_dir_all(&baseline).expect("作れる");
        // <strong>素材は目盛りが作れる形で置く。</strong> 作れなければ、捨てた派生物と
        // 作り直した派生物がどちらも空で一致し、何も確かめていないことになる。
        let (p, b) = fixture::corpus();
        for (side, units) in [(&person, &p), (&baseline, &b)] {
            for (name, doc) in units {
                let body = kakiburi_metrics::humanness::joined(&doc.prose());
                std::fs::write(side.join(format!("{name}.md")), body).expect("書ける");
            }
        }
        let c = dir.join("c.kb").to_string_lossy().into_owned();
        let build = vec![
            "build".to_owned(),
            c.clone(),
            "--person".to_owned(),
            person.to_string_lossy().into_owned(),
            "--baseline".to_owned(),
            baseline.to_string_lossy().into_owned(),
            // <strong>同梱の池ではないので、作り方を名乗る。</strong>
            "--model".to_owned(),
            "m".to_owned(),
            "--version".to_owned(),
            "v1".to_owned(),
        ];
        assert_eq!(
            run(&["new".to_owned(), c.clone(), "試験".to_owned()]),
            Exit::Pass
        );
        assert_eq!(run(&build), Exit::Pass);
        let before = open(&c).expect("読める").0;
        assert!(before.derived.has_scale(), "目盛りができている");

        let mut dropped = before.clone();
        dropped.drop_derived();
        assert!(!dropped.derived.has_scale(), "捨てられている");
        std::fs::write(&c, store::write(&dropped)).expect("書ける");

        assert_eq!(run(&build), Exit::Pass, "作り直せる");
        let after = open(&c).expect("読める").0;
        assert_eq!(after.derived, before.derived, "同じ派生物が出る");
        assert_eq!(after.decided, before.decided, "決めたことも変わらない");
        assert!(
            after.fingerprint.matches(&before.fingerprint),
            "指紋も同じ"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn quick_はカセットでないファイルを消さない() {
        // <strong>在るものを消さない。</strong> 消してよいかは読んでみるまで分からない
        // ——カセットでないファイルは、人の別のファイルである。
        let dir = temp_dir("quick-not-a-cassette");
        let person = dir.join("person");
        std::fs::create_dir_all(&person).expect("作れる");
        std::fs::write(person.join("a.md"), "これは、そうだ。\n").expect("書ける");
        let victim = dir.join("大事.txt");
        std::fs::write(&victim, "消えてはいけない").expect("書ける");
        assert_eq!(
            run(&[
                "quick".to_owned(),
                person.to_string_lossy().into_owned(),
                "--cassette".to_owned(),
                victim.to_string_lossy().into_owned(),
            ]),
            Exit::Usage
        );
        assert_eq!(
            std::fs::read_to_string(&victim).expect("在る"),
            "消えてはいけない",
            "消されている"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn quick_は人が決めたことを引き継ぐ() {
        // <strong>カセットだったとしても消さない。</strong> 人が決めたことは作り直せない
        // ので、消せば落とす定型も動かない指標もそこで消える。
        let dir = temp_dir("quick-keeps-decided");
        let person = dir.join("person");
        std::fs::create_dir_all(&person).expect("作れる");
        std::fs::write(person.join("a.md"), "これは、そうだ。\n").expect("書ける");
        let c = dir.join("c.kb").to_string_lossy().into_owned();
        assert_eq!(
            run(&["new".to_owned(), c.clone(), "default".to_owned()]),
            Exit::Pass
        );
        assert_eq!(
            run(&[
                "decide".to_owned(),
                c.clone(),
                "boilerplate".to_owned(),
                "お世話になっており".to_owned(),
            ]),
            Exit::Pass
        );
        // 素材が足りないので目盛りは作れないが、<strong>作れなくても消してはいけない。</strong>
        run(&[
            "quick".to_owned(),
            person.to_string_lossy().into_owned(),
            "--cassette".to_owned(),
            c.clone(),
        ]);
        let (got, _) = open(&c).expect("読める");
        assert_eq!(
            got.decided.boilerplate,
            vec!["お世話になっており"],
            "人が決めたことが消えている"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 役を跨いで名前が重なれば断る() {
        // <strong>測るときは名前で引く。</strong> 重なれば片方の測定値がもう片方で
        // 置き換わり、<strong>本人の単位が基準の数字で測られる</strong>——壊れた帯が
        // 正常な顔でカセットに入る。
        let dir = temp_dir("dup-name");
        let person = dir.join("person");
        let baseline = dir.join("baseline");
        std::fs::create_dir_all(&person).expect("作れる");
        std::fs::create_dir_all(&baseline).expect("作れる");
        // <strong>同じ題材を揃えるほど同じ名前が付きやすい。</strong>
        for side in [&person, &baseline] {
            std::fs::write(side.join("Rust入門.md"), "これは、そうだ。\n").expect("書ける");
        }
        let c = dir.join("c.kb").to_string_lossy().into_owned();
        assert_eq!(
            run(&["new".to_owned(), c.clone(), "試験".to_owned()]),
            Exit::Pass
        );
        assert_eq!(
            run(&[
                "build".to_owned(),
                c,
                "--person".to_owned(),
                person.to_string_lossy().into_owned(),
                "--baseline".to_owned(),
                baseline.to_string_lossy().into_owned(),
                "--model".to_owned(),
                "m".to_owned(),
                "--version".to_owned(),
                "v1".to_owned(),
            ]),
            Exit::Usage
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 同梱の池は自分で版を名乗る() {
        // <strong>場所では決められない。</strong> 環境変数が同梱の池を指すのは普通の
        // 使い方だが、どこを指しているかは道具に分からない——目印を池の中に置く。
        let dir = temp_dir("pool-marker");
        let pool = dir.join("pool");
        std::fs::create_dir_all(&pool).expect("作れる");
        assert_eq!(pool_version(&pool.to_string_lossy()), None, "目印が無い");
        std::fs::write(pool.join(POOL_MARKER), "2026-09\n").expect("書ける");
        assert_eq!(
            pool_version(&pool.to_string_lossy()),
            Some("2026-09".to_owned())
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 同梱でない基準には版が要る() {
        // <strong>記録の無い基準で作った値は、次に測ったときに比べられない。</strong>
        let dir = temp_dir("baseline-version");
        let person = dir.join("person");
        let baseline = dir.join("baseline");
        std::fs::create_dir_all(&person).expect("作れる");
        std::fs::create_dir_all(&baseline).expect("作れる");
        std::fs::write(person.join("p.md"), "これは、そうだ。\n").expect("書ける");
        std::fs::write(baseline.join("b.md"), "これは、そうだ。\n").expect("書ける");
        let c = dir.join("c.kb").to_string_lossy().into_owned();
        assert_eq!(
            run(&["new".to_owned(), c.clone(), "試験".to_owned()]),
            Exit::Pass
        );
        let base = vec![
            "build".to_owned(),
            c,
            "--person".to_owned(),
            person.to_string_lossy().into_owned(),
            "--baseline".to_owned(),
            baseline.to_string_lossy().into_owned(),
        ];
        assert_eq!(run(&base), Exit::Usage, "版が無い");
        let mut with = base.clone();
        with.extend([
            "--model".to_owned(),
            "m".to_owned(),
            "--version".to_owned(),
            "v1".to_owned(),
        ]);
        assert_ne!(run(&with), Exit::Usage, "名乗れば通る");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 束ねた単位は中身を名前にする() {
        // 連番にすると束の構成がどこにも残らず、割りの表に名前だけが並ぶ。
        let dir = temp_dir("bundle-name-members");
        let files = vec![a_document(&dir, "b001"), a_document(&dir, "b002")];
        let got = load_units(&files, &[vec![0, 1]]).expect("読める");
        assert_eq!(got[0].0, "b001+b002");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 束ねると_1_単位になる() {
        // 10 単位の下限は単位で数える。ファイルで数えれば、束ねた分だけ多く見える。
        let dir = temp_dir("bundle");
        let files = vec![a_document(&dir, "a"), a_document(&dir, "b")];
        let alone = load_units(&files, &[]).expect("読める");
        assert_eq!(alone.len(), 2, "束ねなければ 2 単位");
        let bundled = load_units(&files, &[vec![0, 1]]).expect("読める");
        assert_eq!(bundled.len(), 1, "測る単位は 1 つ");
        assert_eq!(bundled[0].1.nodes.len(), 2, "node の境界は残す");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 束ねない_1_本はファイル名を名乗る() {
        // <strong>束ねた単位だけが自分の名前を名乗る。</strong> 1 本しか入っていない組に
        // 別の名前を付けると、割りの表からどのファイルか引けなくなる。
        let dir = temp_dir("bundle-name");
        let files = vec![a_document(&dir, "記事")];
        let got = load_units(&files, &[vec![0]]).expect("読める");
        assert_eq!(got[0].0, "記事");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 読めない_1_本があれば何も返さない() {
        // 黙って一部を落として通せば、欠けたまま目盛りが出来上がる。
        let dir = temp_dir("load-partial");
        let files = vec![
            a_document(&dir, "a"),
            dir.join("無い.md").to_string_lossy().into_owned(),
        ];
        assert!(load_units(&files, &[]).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 書くたびに世代が進む() {
        // 世代が進まなければ、同時に書いた片方の変更が正常終了のまま消える。
        let dir = temp_dir("generation");
        let c = empty_cassette(&dir);
        assert_eq!(save::generation_of(&c), 1, "作った時点で 1");
        assert_eq!(
            run(&[
                "decide".to_owned(),
                c.clone(),
                "boilerplate".to_owned(),
                "お世話になっており".to_owned(),
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
    fn 言い回しの表は_2_つ以上の単位に出るものだけを持つ() {
        // 1 つの単位にしか出てこない言い回しは、その記事の題材であって書きぶり
        // ではない——<strong>持っても上限がほぼ 0 で、持たないのと同じ結論になる。</strong>
        let (person, _) = fixture::corpus();
        let table = phrase_table(&person, Some(&fixture::Chars));
        let read = Cassette {
            derived: Derived {
                phrases: Some(table),
                ..Derived::dropped()
            },
            ..fixture_cassette()
        };
        let got = read_phrase_table(&read);
        assert!(!got.is_empty(), "共通の言い回しが拾えている");
        assert!(got.values().all(|v| *v > 0.0), "上限は正である");
        // <strong>表に無いものは上限 0 である。</strong> 本人が一度も使っていない言い回しを
        // 草稿が繰り返していれば、それがそのまま指摘になる。
        assert_eq!(person_ceiling(&got, "誰も書かない並び"), 0.0);
    }

    /// 言い回しの表だけを見るための、中身の無いカセット。
    fn fixture_cassette() -> Cassette {
        Cassette {
            version: store::VERSION,
            generation: 0,
            fingerprint: current_fingerprint(),
            provisional: vec![],
            scene: "試験".into(),
            decided: Decided::default(),
            derived: Derived::dropped(),
        }
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
            "--source".to_owned(),
            "plain-markdown".to_owned(),
        ];
        assert_eq!(run(&args), Exit::Unknown);
        // <strong>途中で抜ける道でも JSON を出す。</strong> 出さなければ、道具の側が
        // 「出力が無い」を自分で場合分けすることになる。
        let mut with = args.to_vec();
        with.push("--json".to_owned());
        assert_eq!(run(&with), Exit::Unknown);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 相手集合のベクトルが欠ければ使う前の問題である() {
        // <strong>カセットは本文を持たない。</strong> 相手集合のベクトルが目盛りの中で欠けて
        // いれば、照合値はもう出せない——<strong>目盛りがあるのに判定できないが返る</strong>
        // のは、壊れていることが正常に見えるということである。
        let dir = temp_dir("partners");
        let cassette = cassette_with_scale(&dir);
        let raw = std::fs::read(&cassette).expect("読める");
        let mut c = store::read(&raw).expect("読める");
        let mut scale = scale_json::read(c.derived.scale.as_deref().expect("在る")).expect("読める");
        scale.partner_vectors.pop();
        c.derived.scale = Some(scale_json::write(&scale));
        std::fs::write(&cassette, store::write(&c)).expect("書ける");

        let target = dir.join("検める.md");
        std::fs::write(&target, "これは、そうだ、と思う。\n").expect("書ける");
        let args = [
            "review".to_owned(),
            target.to_string_lossy().into_owned(),
            "--cassette".to_owned(),
            cassette,
            "--source".to_owned(),
            "plain-markdown".to_owned(),
        ];
        assert_eq!(run(&args), Exit::FingerprintMismatch);
        std::fs::remove_dir_all(&dir).ok();
    }

    fn pairs(v: &[f64]) -> Vec<(String, f64)> {
        v.iter()
            .enumerate()
            .map(|(i, x)| (format!("u{i}"), *x))
            .collect()
    }

    #[test]
    fn 全部の対で本人が高ければ_1_である() {
        let (rate, inverted) = higher_rate(&pairs(&[2.0, 3.0]), &pairs(&[0.0, 1.0]));
        assert!((rate - 1.0).abs() < f64::EPSILON, "{rate}");
        assert!(inverted.is_empty());
    }

    #[test]
    fn 逆に出た対を名指しできる() {
        let (rate, inverted) = higher_rate(&pairs(&[1.0, 3.0]), &pairs(&[0.0, 2.0]));
        assert!((rate - 0.75).abs() < f64::EPSILON, "{rate}");
        assert_eq!(inverted.len(), 1);
        assert_eq!(inverted[0].0, "u0");
        assert_eq!(inverted[0].2, "u1");
    }

    #[test]
    fn 並んだ対は高く出たことにしない() {
        // 半分だけ数えるが、逆に出た対としては残す——高く出ていないことに変わりはない。
        let (rate, inverted) = higher_rate(&pairs(&[1.0]), &pairs(&[1.0]));
        assert!((rate - 0.5).abs() < f64::EPSILON, "{rate}");
        assert_eq!(inverted.len(), 1);
    }

    #[test]
    fn 外れた_1_本を足しても割合はほとんど動かない() {
        // <strong>これが最小・最大との違いである。</strong> 端で見れば、この 1 本だけで
        // 通っていたものが落ちる。
        let mine: Vec<f64> = (0..20).map(|i| 2.0 + f64::from(i)).collect();
        let theirs: Vec<f64> = (0..20).map(|i| -20.0 + f64::from(i)).collect();
        let (before, _) = higher_rate(&pairs(&mine), &pairs(&theirs));
        assert!((before - 1.0).abs() < f64::EPSILON, "{before}");

        let mut theirs = theirs;
        theirs.push(100.0);
        let (after, inverted) = higher_rate(&pairs(&mine), &pairs(&theirs));
        assert_eq!(inverted.len(), 20, "外れた 1 本は全部の対で逆に出る");
        assert!(after > HIGHER_RATE_FLOOR, "{after}");
    }
}
