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
    save, store, Baseline, Cassette, Common, Corpus, Decided, Derived, Fingerprint, Inputs,
    Normalization, Role, Tool, Unit,
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
        Some("measure") => measure(&args[1..]),
        Some("metrics") => metrics(&args[1..]),
        Some("new") => new_cassette(&args[1..]),
        Some("scene") => scene(&args[1..]),
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

/// 1 つのコマンドの help。<strong>知らない名前なら `None`。</strong>
///
/// 全体の help と同じ文を使う——2 か所に書けば、片方だけが古くなる。
fn section(name: &str) -> Option<&'static str> {
    Some(match name {
        "measure" => MEASURE,
        "new" => NEW,
        "scene" => SCENE,
        "add" => ADD,
        "replace" => REPLACE,
        "decide" => DECIDE,
        "build" => BUILD,
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
kakiburi new <カセット>
    1 人分の入れ物を作る。<strong>既に在れば断る。</strong>
    場面はここでは取らない——scene で作る。";

const SCENE: &str = "\
kakiburi scene <カセット> <場面>
    場面を作る。<strong>場面は人が指定する。</strong> 文章から当てにいかない。
    名前に `/` と空は使えない。保存の中では階層の名前になるためである。";

const ADD: &str = "\
kakiburi add <カセット> <ファイル...> --as person|baseline --scene <場面>
                                 --source <取り込み元> [--id <名前>] [--unit <名前>]
                                 [--model <名前> --version <版> --param <鍵>=<値> --topic <題材>]
kakiburi add <カセット> <ファイル...> --as other --source <取り込み元>
    正規化して入れる。<strong>1 本でも断ったら何も入れない。</strong>
    名前はカセット全体で一意である。<strong>衝突したら断る</strong>——黙って上書きしない。
    <strong>--unit で束ねる</strong>——短い文書を何本かで 1 単位にする。
    <strong>--as baseline には --model と --version が要る</strong>（--param / --topic も取る）。
    本文が変わるので派生物を捨てる。

    役は 3 つある。
      person    本人の文書。照合の相手集合と天井になる
      baseline  基準。LLM の既定出力。床になる
      other     <strong>他人の文書。</strong> 人らしさの人の側の較正にだけ効く

    <strong>--as other だけが --scene を取らない。</strong> この軸が見ているのは機械の書きぶりが
    残っていないかであって、その人らしさではない。繰り返しの少なさは書き手も
    場面も問わず同じ向きに出るので、この用途に限って両方を跨げる。
    <strong>目的の側——照合——には一切効かない。</strong>";

const REPLACE: &str = "\
kakiburi replace <カセット> <ファイル> --id <名前> --source <取り込み元>
    既にある 1 本を差し替える。<strong>無い名前を渡したら断る。</strong>
    足すことと差し替えることを分けるのは、上書きを事故ではなく意思にするため。
    <strong>役も場面も束も変えない</strong>——変えるなら消して add し直す。";

const DECIDE: &str = "\
kakiburi decide <カセット> --scene <場面> boilerplate <文字列...>
kakiburi decide <カセット> --scene <場面> movement <指標> moves|stuck
    <strong>コーパスから導けないものを書く。</strong> 落とす定型は場面ごとに人が決め、
    指示して動くかは直させてみて初めて分かる。
    <strong>落とす定型は渡した一覧で置き換える</strong>——足していく形にしない。";

const BUILD: &str = "\
kakiburi build <カセット> [--scene <場面>] [--json]
    目盛りを作る。<strong>省くと全場面。</strong>
    <strong>作らずに終わる条件を持つ</strong>——止まっても失敗ではない。
    作れたら割りを出す。場面が 2 つ以上あれば、分かれ方も出す。";

const REVIEW: &str = "\
kakiburi review <ファイル> --cassette <カセット> --scene <場面> --source <取り込み元>
                           [--json]
    検める。3 値と指摘を返す。
    <strong>目盛りの無い場面は判定できない（2）を返す</strong>——素材が足りずに作れな
    かったのは正常な状態である。";

const COMPARE: &str = "\
kakiburi compare <ファイル>... --source <取り込み元>
    並べて比べる。<strong>系統の距離は出ない</strong>——語彙が無いためである。
    <strong>3 本以上を取る</strong>——n 周した草稿を並べて散らばりを見るためである。";

const SHOW: &str = "\
kakiburi show <カセット>
    コーパス全体の分布を、場面ごと・役ごとに出す。
    <strong>場面を持たない other は場面の外に並べる。</strong>";

const DOCTOR: &str = "\
kakiburi doctor <カセット>
    <strong>自分を検査する。</strong> 本人がいちばん高く出ることが、目盛りが壊れていない
    ことの最低条件である。指紋・派生物・暫定値もあわせて確かめる。
    <strong>場面ごとに検める</strong>——まとめれば、1 つの場面の壊れがほかで薄まる。";

const METRICS: &str = "\
kakiburi metrics
    登録簿を回して一覧を出す。使う側が一覧を持たないことの裏返し。";

/// 環境変数。<strong>help に出さなければ、仕様を読むまで進めない。</strong>
const ENVIRONMENT: &str = "\
環境

  KAKIBURI_UNIDIC          UniDic の展開先。指すと全系統を測れる
  KAKIBURI_UNIDIC_VERSION  指紋に入る版の申告（既定: 版の申告なし）
  KAKIBURI_MECAB           MeCab の実行ファイル（既定: mecab）

  <strong>UniDic は nixpkgs に無い。</strong> 国語研が配布している:
  https://clrd.ninjal.ac.jp/unidic_archive/cwj/2.1.2/unidic-mecab-2.1.2_bin.zip
  <strong>IPADic は断る</strong>——体系が違えば語彙素で引けない。";

fn print_help() {
    println!("kakiburi — どこがその人と違うかを、言えるようにする");
    println!();
    println!("{MEASURE}");
    println!();
    println!("作る——たまに動かす");
    println!();
    println!("  <strong>1 カセットが 1 人である。</strong> 場面ごとの束（トラック）を中に持ち、");
    println!("  語彙も重みも帯も場面ごとに作る。");
    for s in [NEW, SCENE, ADD, REPLACE, DECIDE, BUILD] {
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
/// <strong>1 カセットが 1 人である。</strong> 場面はここでは取らない——場面ごとの束は
/// [`scene`] が作る。
fn new_cassette(args: &[String]) -> Exit {
    let Some(path) = args.first() else {
        eprintln!("カセットの経路を渡す");
        return Exit::Usage;
    };
    if let Some(other) = args.get(1) {
        eprintln!("知らない引数: {other}");
        eprintln!("<strong>場面は new では取らない。</strong> scene で作る");
        return Exit::Usage;
    }

    let c = Cassette {
        version: store::VERSION,
        // <strong>置き換えるたびに増える。</strong> 作った時点では 0 で、書けば 1 になる。
        generation: 0,
        fingerprint: current_fingerprint(),
        // <strong>いまは常に暫定値が立つ。</strong> 閾値がまだ導き直されていない。
        provisional: vec!["除外の既定".into(), "帯の端".into(), "語彙の大きさ".into()],
        corpus: Corpus::new(vec![]),
        tracks: BTreeMap::new(),
    };

    // <strong>既に在れば断る。</strong> 素材の取り込みには時間が掛かるうえ、原本は作り直せない
    // ——上書きすれば、取り込んだ単位はそこで消える。
    //
    // <strong>ここで `exists()` を見てから書かない。</strong> 見てから書くまでのあいだに割り込ま
    // れれば同じことが起きる。<strong>錠の中で確かめるのは保存の側の仕事である</strong>——
    // `expected` に `None` を渡すことが「作るつもりだ」という申告になる。
    if let Err(e) = save::save(path, &c, None) {
        eprintln!("断る: {e}");
        if matches!(e, save::SaveError::Exists { .. }) {
            eprintln!("<strong>取り込んだ単位は戻らない</strong>");
            return Exit::Usage;
        }
        return Exit::Unreadable;
    }
    println!("作った: {path}");
    println!("場面はまだ無い。scene で作る");
    Exit::Pass
}

/// 場面を作る。
///
/// <strong>場面は人が指定する。</strong> 文章から当てにいかないので、必ず訊く。
///
/// <strong>作るだけの操作を分けるのは、`add` で綴りを間違えたときに断れるようにするため
/// である。</strong>`add` が場面を作れると、`技術記事` を `技術記時` と打っただけで
/// <strong>誰もいない場面に素材が入り、エラーも出ない。</strong>
fn scene(args: &[String]) -> Exit {
    let (Some(path), Some(name)) = (args.first(), args.get(1)) else {
        eprintln!("scene <カセット> <場面>");
        return Exit::Usage;
    };
    if let Some(other) = args.get(2) {
        eprintln!("知らない引数: {other}");
        return Exit::Usage;
    }
    if !kakiburi_cassette::scene_name_ok(name) {
        eprintln!("断る: 場面の名前に使えない: {name}");
        eprintln!("保存の中では階層の名前になる。`/` と空は使えない");
        return Exit::Usage;
    }
    let (mut c, generation) = match open(path) {
        Ok(v) => v,
        Err(e) => return e,
    };
    if c.tracks.contains_key(name) {
        eprintln!("断る: 既に在る場面: {name}");
        return Exit::Usage;
    }
    c.track_mut(name);
    refresh(&mut c);
    if let Err(e) = store_back(path, &c, generation) {
        return e;
    }
    println!("場面を作った: {name}");
    println!("場面 {} 個: {}", c.scenes().len(), c.scenes().join("、"));
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
    let mut source: Option<Source> = None;
    // <strong>役に既定を置かない。</strong> 取り違えると、対照にしたはずの文書が書き手の帯に
    // 残る——いちばん高くつく取り違えに既定を与えない。
    let mut role: Option<Role> = None;
    let mut scene: Option<String> = None;
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
                source = Some(name);
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
            "--scene" => {
                let Some(v) = args.get(i + 1) else {
                    eprintln!("--scene に場面を渡す");
                    return Exit::Usage;
                };
                scene = Some(v.clone());
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
    let Some(source) = source else {
        return missing_source();
    };
    // <strong>`--as other` だけが `--scene` を取らない。</strong> 仕様の「越境はこの用途に限る」を、
    // 引数の形でそのまま言う——注釈でしか言えていなければ、越境は注意深さで守られる。
    let belongs = match (role, scene) {
        (Role::Other, None) => kakiburi_cassette::Belongs::Other,
        (Role::Other, Some(_)) => {
            eprintln!("断る: --as other は --scene を取らない");
            eprintln!("他人の文書が効くのは人らしさの人の側だけで、そこは場面を跨ぐ");
            return Exit::Usage;
        }
        (_, None) => {
            eprintln!("--scene を渡す（--as other 以外では要る）");
            eprintln!("<strong>場面は人が指定する。</strong> 文章から当てにいかない");
            return Exit::Usage;
        }
        (Role::Person, Some(s)) => kakiburi_cassette::Belongs::Person { scene: s },
        (Role::BaselineOutput, Some(s)) => kakiburi_cassette::Belongs::Baseline { scene: s },
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

    // <strong>知らない場面には入れない。</strong> `scene` で作っていない場面を受けると、
    // 綴りを間違えただけで誰もいない場面に素材が入り、エラーも出ない。
    if let Some(s) = belongs.scene() {
        if !c.tracks.contains_key(s) {
            eprintln!("断る: 知らない場面: {s}");
            eprintln!("先に scene で作る。いまある場面: {}", scenes_or_none(&c));
            return Exit::Usage;
        }
    }

    if role == Role::BaselineOutput {
        let scene = belongs.scene().unwrap_or_default().to_owned();
        let next = Baseline {
            model: model.unwrap_or_default(),
            version: version.unwrap_or_default(),
            // <strong>推論設定も外さない。</strong> 省いたときに空で上書きすると、記録してあった
            // 設定が黙って消える——題材と同じ扱いにする。
            params: if params.is_empty() {
                c.track_mut(&scene).decided.baseline.params.clone()
            } else {
                params
            },
            // <strong>題材は外さない。</strong> 言葉づかいだけで帯が動く。
            topics: if topics.is_empty() {
                c.track_mut(&scene).decided.baseline.topics.clone()
            } else {
                topics
            },
        };
        // <strong>作り方が変われば、前に入れた基準と混ぜられない。</strong> 黙って上書きしない。
        //
        // <strong>見るのはその場面の基準である。</strong> 場面ごとに帯を作るので、別の場面が
        // 別のモデルで作られていても混ざらない。
        //
        // <strong>推論設定まで見る。</strong> 版が同じでも温度が違えば別の出力になる——
        // 仕様が「版と推論設定まで記録する」と言うのは、そこまでが作り方だからである。
        let already = &c.track_mut(&scene).decided.baseline;
        if !already.model.is_empty()
            && (already.model != next.model
                || already.version != next.version
                || already.params != next.params)
        {
            eprintln!(
                "断る: 基準の作り方が違う（{} {} {:?} → {} {} {:?}）",
                already.model,
                already.version,
                already.params,
                next.model,
                next.version,
                next.params
            );
            eprintln!("<strong>違う作り方の基準を混ぜない。</strong> 別の場面か別のカセットにする");
            return Exit::Usage;
        }
        c.track_mut(&scene).decided.baseline = next;
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
                belongs: belongs.clone(),
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

    for u in units {
        c.corpus.push(u);
    }
    // <strong>本文が変われば派生物は古い。</strong> 捨てる。
    //
    // <strong>他人の文書はどの場面の人らしさにも効くので、全部の場面を捨てる。</strong>
    // 場面ごとに捨て分けると、越境する素材を足したときだけ捨て漏れる。
    c.drop_all_derived();
    note_source(&mut c, source);
    // <strong>指紋を組み直す。</strong> 派生物を捨てたのだから、語彙と z 得点も空に戻る
    // ——残せば、次に検めたときに「合っている」と言われる。
    refresh(&mut c);

    if let Err(e) = store_back(path, &c, generation) {
        return e;
    }
    match belongs.scene() {
        Some(s) => println!("入れた {} 本。役: {} / 場面: {s}", files.len(), role.dir()),
        None => println!(
            "入れた {} 本。役: {}（場面を持たない）",
            files.len(),
            role.dir()
        ),
    }
    println!("派生物を捨てた。build し直しが要る");
    Exit::Pass
}

/// 取り込み元を指紋に足す。
///
/// <strong>本文を入れる道は全部ここを通す。</strong>`add` だけが足して `replace` が足さないと、
/// 別の取り込み元で差し替えても指紋が古いままになり、<strong>照らしても違いが出ない</strong>
/// ——[取り込み元を間違えると 0 が並ぶ](../../../docs/spec/030-normalize.md#取り込み元を間違えると0-が並ぶ)
/// のに、それを検出する唯一の手がかりが動かない。
///
/// <strong>減らす道は持たない。</strong> 単位ごとに取り込み元を持っていないので、最後の 1 本を
/// 差し替えたことを知る手段が無い。<strong>足したことだけを残す</strong>——多めに名乗るのは
/// 「この中のどれかで読んだ」であって、嘘ではない。
fn note_source(c: &mut Cassette, source: Source) {
    let mut inputs = c.fingerprint.inputs.clone();
    let sources = &mut inputs.common.normalization.sources;
    if sources.iter().any(|s| s == source.name()) {
        return;
    }
    sources.push(source.name().to_owned());
    sources.sort_unstable();
    c.fingerprint = Fingerprint::build(inputs);
}

/// 指紋を組み直す。<strong>カセットを変えたら必ず通る。</strong>
///
/// <strong>変えたのに組み直さなければ、次に検めたときに「合っている」と言われる。</strong>
/// 逆に、変わっていない場面まで巻き添えで「合わない」にもしない——場面ごとの材料が
/// 分かれているので、動いた場面の欄だけが動く。
fn refresh(c: &mut Cassette) {
    c.fingerprint = fingerprint_with(c, analyzer::resolve().as_ref());
}

/// いまある場面。<strong>1 つも無ければそう言う。</strong>
fn scenes_or_none(c: &Cassette) -> String {
    if c.tracks.is_empty() {
        "（まだ無い）".to_owned()
    } else {
        c.scenes().join("、")
    }
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
    let mut source: Option<Source> = None;
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
                source = Some(name);
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
    let Some(source) = source else {
        return missing_source();
    };

    let (mut c, generation) = match open(path) {
        Ok(v) => v,
        Err(e) => return e,
    };

    // <strong>無い `id` を渡したら断る。</strong> `add` の綴り間違いで差し替えたことにしない。
    let Some(old) = c.corpus.find(&id) else {
        eprintln!("断る: `{id}` はカセットに無い");
        eprintln!("足すなら add を使う");
        return Exit::Usage;
    };
    // <strong>所属は変えない。</strong> 差し替えは中身の入れ替えであって、役や場面の
    // 変更ではない——変えたいなら消して入れ直す。
    if let Some(r) = role {
        if r != old.role() {
            eprintln!(
                "断る: replace で役は変えられない（いま {}）",
                old.role().dir()
            );
            eprintln!("役や場面を変えるなら、消して add し直す");
            return Exit::Usage;
        }
    }
    let belongs = old.belongs.clone();
    let unit = old.unit.clone();

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

    let role = belongs.role();
    c.corpus.replace(
        &id,
        Unit {
            name: id.clone(),
            // <strong>束は変えない。</strong> 差し替えは中身の入れ替えであって、
            // どの単位に属するかの変更ではない。
            unit,
            belongs,
            document,
        },
    );
    // <strong>本文が変われば派生物は古い。</strong> 捨てる——指紋は測った条件を表すものなので、
    // 本文を差し替えても変わらない。捨てなければ古い値が有効な顔で読まれる。
    c.drop_all_derived();
    // <strong>差し替えでも取り込み元は記録する。</strong>`add` だけが足すと、別の取り込み元で
    // 差し替えたことが指紋から読めない。
    note_source(&mut c, source);
    refresh(&mut c);
    if let Err(e) = store_back(path, &c, generation) {
        return e;
    }
    println!("差し替えた: {id}（役: {}）", role.dir());
    println!("派生物を捨てた。build し直しが要る");
    Exit::Pass
}

/// コーパス全体の分布を出す。
///
/// <strong>場面ごと、役ごとに分けて出す。</strong> 混ぜれば、本人と基準の差がそこで潰れる。
/// <strong>場面を混ぜても同じことが起きる</strong>——場面ごとに閉じている以上、まとめて出した
/// 分布は誰のものでもない。
fn show(args: &[String]) -> Exit {
    let Some(path) = args.first() else {
        eprintln!("カセットの経路を渡す");
        return Exit::Usage;
    };
    let (c, _) = match open(path) {
        Ok(v) => v,
        Err(e) => return e,
    };
    println!("場面: {}", scenes_or_none(&c));
    println!("世代: {}", c.generation);
    if !c.provisional.is_empty() {
        println!("暫定値: {}", c.provisional.join("、"));
    }
    let mecab = analyzer::resolve();
    let a = mecab
        .as_ref()
        .map(|m| m as &dyn kakiburi_metrics::morph::Analyzer);
    for scene in c.scenes() {
        println!();
        println!("== {scene} ==");
        let has_scale = c.track(scene).is_some_and(|t| t.derived.has_scale());
        println!("目盛り: {}", if has_scale { "ある" } else { "無い" });
        for role in [Role::Person, Role::BaselineOutput] {
            let units = stripped(&c, scene, role);
            if units.is_empty() {
                continue;
            }
            println!();
            println!("{} — {} 単位", role.dir(), units.len());
            print_distribution(&rows(&samples(&units), a));
        }
    }
    // <strong>他人の文書は場面の外に出す。</strong> どれか 1 つの場面の下に並べれば、
    // その場面のものだと読める。
    let others = stripped_others(&c);
    if !others.is_empty() {
        println!();
        println!("== 場面を持たない ==");
        println!(
            "other — {} 単位（人らしさの人の側にだけ効く）",
            others.len()
        );
        print_distribution(&rows(&samples(&others), a));
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

    if c.tracks.is_empty() {
        println!();
        println!("場面がまだ無い。scene で作る");
        return if bad == 0 { Exit::Pass } else { Exit::Unknown };
    }

    // 3. 場面ごとに検める。<strong>まとめて 1 つの判定にしない</strong>——1 つの場面が
    //    壊れていることが、ほかの場面の健全さで薄まる。
    for scene in c.scenes() {
        println!();
        println!("== {scene} ==");
        bad += doctor_scene(&c, scene);
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

/// 1 つの場面を検める。<strong>おかしかった数を返す。</strong>
fn doctor_scene(c: &Cassette, scene: &str) -> usize {
    let mut bad = 0usize;
    let derived = c.track(scene).map(|t| &t.derived);
    let has_scale = derived.is_some_and(|d| d.scale.is_some());
    let has_effective = derived.is_some_and(|d| d.effective.is_some() && d.spread.is_some());
    println!(
        "派生物: 目盛り {} / 効くかの判定 {}",
        if has_scale { "あり" } else { "無し" },
        if has_effective { "あり" } else { "無し" }
    );
    if has_scale != has_effective {
        println!("  <strong>片方だけある。</strong> build し直しが要る");
        bad += 1;
    }

    let Some(scale) = derived
        .and_then(|d| d.scale.as_deref())
        .and_then(scale_json::read)
    else {
        println!("目盛りが無いので、本人の側が高く出るかは確かめられない");
        return bad;
    };
    let person = stripped(c, scene, Role::Person);
    let person = samples(&person);
    let partners: Vec<Sample<'_>> = person
        .iter()
        .filter(|s| scale.partners().iter().any(|n| n == s.name))
        .copied()
        .collect();
    let mecab = analyzer::resolve();
    let a = mecab
        .as_ref()
        .map(|m| m as &dyn kakiburi_metrics::morph::Analyzer);
    let side = |samples: &[Sample<'_>]| -> Vec<(String, f64)> {
        samples
            .iter()
            .filter(|s| !partners.iter().any(|p| p.name == s.name))
            .filter_map(|s| {
                measure_against(&scale, *s, &partners, a)
                    .matching
                    .map(|v| (s.name.to_owned(), v))
            })
            .collect()
    };
    let baseline_units = stripped(c, scene, Role::BaselineOutput);
    let mine = side(&person);
    let theirs = side(&samples(&baseline_units));
    if mine.is_empty() || theirs.is_empty() {
        println!("照合値を出せる単位が足りない");
        return bad;
    }
    let (rate, inverted) = higher_rate(&mine, &theirs);
    let lowest = mine.iter().map(|(_, v)| *v).fold(f64::INFINITY, f64::min);
    let highest = theirs
        .iter()
        .map(|(_, v)| *v)
        .fold(f64::NEG_INFINITY, f64::max);
    println!("本人の最小: {lowest:.3} / 基準の最大: {highest:.3}");
    println!(
        "本人が高く出た対: {rate:.3}（{} 対中 {} 対が逆、下限 {HIGHER_RATE_FLOOR:.2} は暫定値）",
        mine.len() * theirs.len(),
        inverted.len()
    );
    // <strong>逆に出た対を名指しする。</strong> 割合だけでは、目盛り全体が緩んでいるのか
    // 1 本の単位が外れているのかが分からない。
    for (p, pv, b, bv) in inverted.iter().take(INVERTED_SHOWN) {
        println!("  {p} ({pv:.3}) ≦ {b} ({bv:.3})");
    }
    if inverted.len() > INVERTED_SHOWN {
        println!("  ほか {} 対", inverted.len() - INVERTED_SHOWN);
    }
    if rate >= HIGHER_RATE_FLOOR {
        println!("<strong>本人が高く出ている。</strong> 目盛りは壊れていない");
    } else {
        println!("<strong>基準のほうが高く出る対が多すぎる。</strong> 目盛りを疑う");
        println!("  測っているのは著者性ではなく指示追従かもしれない");
        bad += 1;
    }
    bad
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
/// <strong>コーパスから導けないものだけがここに来る。</strong> 落とす定型は場面ごとに人が決める
/// ものであり、指示して動くかは直させてみて初めて分かる。
fn decide(args: &[String]) -> Exit {
    let Some(path) = args.first() else {
        eprintln!("カセットの経路を渡す");
        return Exit::Usage;
    };
    // <strong>決めることはすべて場面ごとである。</strong> 場面を省いた `decide` は、どの場面の
    // ことかが決まらない——既定を置けば、いちばん打つ回数の多い操作が静かに別の場面へ
    // 掛かる。
    let mut rest: Vec<String> = Vec::new();
    let mut scene: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--scene" {
            let Some(v) = args.get(i + 1) else {
                eprintln!("--scene に場面を渡す");
                return Exit::Usage;
            };
            scene = Some(v.clone());
            i += 2;
            continue;
        }
        rest.push(args[i].clone());
        i += 1;
    }
    let Some(scene) = scene else {
        eprintln!("--scene を渡す。決めることは場面ごとである");
        return Exit::Usage;
    };
    match rest.first().map(String::as_str) {
        Some("boilerplate") => decide_boilerplate(path, &scene, &rest[1..]),
        Some("movement") => decide_movement(path, &scene, &rest[1..]),
        _ => {
            eprintln!(
                "decide <カセット> --scene <場面> boilerplate <文字列...>\n\
                 decide <カセット> --scene <場面> movement <指標> moves|stuck"
            );
            Exit::Usage
        }
    }
}

/// その場面がカセットにあるか。<strong>無ければ断る。</strong>
fn known_scene(c: &Cassette, scene: &str) -> Result<(), Exit> {
    if c.tracks.contains_key(scene) {
        return Ok(());
    }
    eprintln!("断る: 知らない場面: {scene}");
    eprintln!("いまある場面: {}", scenes_or_none(c));
    Err(Exit::Usage)
}

/// 落とす定型を決める。<strong>渡した一覧で置き換える。</strong>
///
/// 足すのではなく置き換えるのは、<strong>いま何を落としているかが 1 度で読める</strong>ようにする
/// ためである。積み上げると、消すのに別の操作が要る。
fn decide_boilerplate(path: &str, scene: &str, words: &[String]) -> Exit {
    let (mut c, generation) = match open(path) {
        Ok(v) => v,
        Err(e) => return e,
    };
    if let Err(e) = known_scene(&c, scene) {
        return e;
    }
    c.track_mut(scene).decided.boilerplate = words.to_vec();
    // <strong>落とす範囲が変われば値が変わる。</strong> 派生物を捨てる。
    //
    // <strong>捨てるのはその場面だけである。</strong> 落とす定型は場面ごとに人が決めたもので、
    // ほかの場面の値には掛かっていない。
    c.drop_derived(scene);
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
fn decide_movement(path: &str, scene: &str, args: &[String]) -> Exit {
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
    if let Err(e) = known_scene(&c, scene) {
        return e;
    }
    c.track_mut(scene)
        .decided
        .movement
        .insert(metric.clone(), state);

    // <strong>前に出す指標は movement から導く派生物である。</strong> 書き換えたのに作り直さな
    // ければ、`stuck` にした指標が指摘に出続ける。
    //
    // 値も目盛りも movement では変わらないので、<strong>作り直すのはここだけである。</strong>
    let dropped = c.track_mut(scene).derived.effective.is_some();
    c.track_mut(scene).derived.effective = None;
    c.track_mut(scene).derived.spread = None;
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

/// 入れようとしている単位の名前を検める。
///
/// <strong>名前はカセット全体で一意である。</strong> 役で名前空間を分けない——分ければ、
/// 本人の `Rust入門` と基準の `Rust入門` が別物として通る。
/// [題材を揃える](../../../docs/spec/200-extract.md#題材の統制は対ではなく素材に効かせる)ほど
/// 同じ名前が付きやすいので、<strong>いちばん正しく集めた人がいちばん踏む。</strong>
fn check_names(c: &Cassette, adding: &[Unit]) -> Result<(), String> {
    let mut names: std::collections::BTreeSet<&str> = c.corpus.names().collect();
    for u in adding {
        if !names.insert(u.name.as_str()) {
            return Err(format!("`{}` は既にある", u.name));
        }
    }

    // <strong>束ねた文書は同じ `unit` を共有する。</strong> だから単純な重複拒否にはできない。
    // <strong>だが所属を跨いだ共有は断る</strong>——1 つの単位が本人でも基準でも、
    // 技術記事でもチャットでもあることになる。
    let mut units: BTreeMap<&str, &kakiburi_cassette::Belongs> = BTreeMap::new();
    for u in adding {
        if let Some(other) = units.insert(u.unit.as_str(), &u.belongs) {
            if *other != u.belongs {
                return Err(format!("単位 `{}` が所属を跨いでいる", u.unit));
            }
        }
    }
    // カセットに入っている側とも照らす。<strong>束は追加で伸びる。</strong>
    for scene in c.scenes() {
        for role in [Role::Person, Role::BaselineOutput] {
            for u in c.corpus.in_scene(scene, role) {
                if let Some(b) = units.get(u.unit.as_str()) {
                    if **b != u.belongs {
                        return Err(format!("単位 `{}` が所属を跨いでいる", u.unit));
                    }
                }
            }
        }
    }
    for u in c.corpus.for_humanness() {
        if let Some(b) = units.get(u.unit.as_str()) {
            if **b != u.belongs {
                return Err(format!("単位 `{}` が所属を跨いでいる", u.unit));
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
    let mut only: Option<String> = None;
    let mut json = false;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--scene" {
            let Some(v) = args.get(i + 1) else {
                eprintln!("--scene に場面を渡す");
                return Exit::Usage;
            };
            only = Some(v.clone());
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
    let (mut c, generation) = match open(path) {
        Ok(v) => v,
        Err(e) => return e,
    };
    if let Some(s) = &only {
        if let Err(e) = known_scene(&c, s) {
            return e;
        }
    }
    if c.tracks.is_empty() {
        eprintln!("場面がまだ無い。scene で作る");
        return Exit::Usage;
    }

    // <strong>`--json` でも進み方は stderr へ出す。</strong> stdout に混ぜれば、JSON として
    // 読めなくなる。
    let mecab = analyzer::resolve();
    let say = |line: String| {
        if json {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    };
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
    // <strong>道具は共有である。</strong> 場面ごとにファイルが別だった頃は、片方を辞書ありで、
    // もう片方を無しで作れてしまい、しかも気付けなかった。
    let others = stripped_others(&c);
    if !others.is_empty() {
        say(format!(
            "他人 {} 単位（人らしさの人の側にだけ効く）",
            others.len()
        ));
    }

    let scenes: Vec<String> = match &only {
        Some(s) => vec![s.clone()],
        None => c.scenes().into_iter().map(str::to_owned).collect(),
    };
    let mut built = Vec::new();
    for scene in &scenes {
        say(String::new());
        say(format!("== {scene} =="));
        built.push((
            scene.clone(),
            build_scene(&mut c, scene, &others, mecab.as_ref(), json),
        ));
    }

    // <strong>場面が本当に分かれているかを見る。</strong> 同じ入れ物に複数の場面が入って
    // 初めて測れるようになった検査である。
    if scenes.len() > 1 || c.scenes().len() > 1 {
        eprintln!();
        print_scene_separation(&c, mecab.as_ref());
    }

    // <strong>指紋を作り直す。</strong> 語彙と z 得点と道具が値を決めるので、目盛りができた
    // 時点で指紋も変わる——変えなければ、次に検めるときに合わないことが分からない。
    refresh(&mut c);
    if let Err(e) = store_back(path, &c, generation) {
        return e;
    }
    if json {
        println!(
            "{}",
            kakiburi_cassette::json::Value::obj([(
                "scenes".to_owned(),
                kakiburi_cassette::json::Value::obj(built),
            )])
            .write()
        );
        return Exit::Pass;
    }
    println!();
    println!("入れた: {path}");
    Exit::Pass
}

/// 1 つの場面の目盛りを作る。<strong>作らずに終わる条件を持つ。</strong>
///
/// 何が起きたかを返す——<strong>道具向けの出口が要る</strong>ので、出力を組み立てながら
/// 進み方を捨ててしまわない。
fn build_scene(
    c: &mut Cassette,
    scene: &str,
    others: &[(String, kakiburi_doc::Document)],
    mecab: Option<&kakiburi_metrics::mecab::Mecab>,
    json: bool,
) -> kakiburi_cassette::json::Value {
    use kakiburi_cassette::json::Value;
    // <strong>`--json` でも進み方は stderr へ出す。</strong> stdout に混ぜれば読めなくなる。
    let say = |line: String| {
        if json {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    };
    #[allow(clippy::cast_precision_loss)]
    let n = |v: usize| Value::Number(v as f64);
    let stopped = |why: String| {
        Value::obj([
            ("built".to_owned(), Value::Bool(false)),
            ("reason".to_owned(), Value::s(why)),
        ])
    };

    let person_units = stripped(c, scene, Role::Person);
    let baseline_units = stripped(c, scene, Role::BaselineOutput);
    let person = samples(&person_units);
    let baseline = samples(&baseline_units);
    let others = samples(others);
    say(format!(
        "本人 {} 単位 / 基準 {} 単位",
        person.len(),
        baseline.len()
    ));
    let boilerplate = c
        .track(scene)
        .map(|t| t.decided.boilerplate.len())
        .unwrap_or(0);
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
        c.drop_derived(scene);
        return stopped("環境の側で測れない指標がある".to_owned());
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
            print_reports("本人", &kakiburi_scale::inspect(&person, a), json);
            print_reports("基準", &kakiburi_scale::inspect(&baseline, a), json);
            c.drop_derived(scene);
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
    let bad = scale.humanness.contradicting_dims();
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
        (
            "evenly_spread".to_owned(),
            Value::Bool(scale.humanness.evenly_spread()),
        ),
    ]);

    // <strong>作り終えた目盛りだけを入れる。</strong> 検めはこれを受け取る。
    c.track_mut(scene).derived = Derived {
        vocabulary: Some(vocabulary_note(&scale)),
        values: None,
        spread: Some(effective_json::write_spread(&effective)),
        calibration: Some(format!("系統 {} 本の較正と合算", scale.frozen.len())),
        scale: Some(scale_json::write(&scale)),
        effective: Some(effective_json::write_effective(&effective)),
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

/// 場面が本当に分かれているかを見る。
///
/// <strong>1 カセットに複数の場面が入って初めて測れる検査である。</strong> 別のファイルだった
/// 頃は比べる経路そのものが無かった。
///
/// <strong>場面が分かれていないなら、場面ごとに閉じている意味が無い。</strong> 逆に、
/// 場面の中が場面の間より散らばっているなら、その「1 つの場面」は 1 つではない。
fn print_scene_separation(c: &Cassette, mecab: Option<&kakiburi_metrics::mecab::Mecab>) {
    let a = mecab.map(|m| m as &dyn kakiburi_metrics::morph::Analyzer);
    // 場面ごとに、本人の単位を目盛り抜きの生の値で並べる。<strong>目盛りは場面ごとに
    // 違うので、目盛りに載せた値どうしは比べられない。</strong>
    let mut by_scene: Vec<(String, Vec<kakiburi_scale::effective::Row>)> = Vec::new();
    for scene in c.scenes() {
        let units = stripped(c, scene, Role::Person);
        if units.len() < 2 {
            continue;
        }
        by_scene.push((scene.to_owned(), rows(&samples(&units), a)));
    }
    if by_scene.len() < 2 {
        return;
    }
    println!("場面の分かれ方（本人の単位、目盛りに載せる前の値）");
    println!("  {:<20} {:>10} {:>10}", "場面", "中の散らばり", "外との差");
    let mut suspicious = Vec::new();
    for (i, (scene, mine)) in by_scene.iter().enumerate() {
        let within = mean_spread(mine);
        let others: Vec<Vec<(String, Option<f64>)>> = by_scene
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .flat_map(|(_, (_, rows))| rows.clone())
            .collect();
        let between = mean_gap(mine, &others);
        println!("  {scene:<20} {within:>10.3} {between:>10.3}");
        if between <= within {
            suspicious.push(scene.clone());
        }
    }
    if suspicious.is_empty() {
        println!("  <strong>どの場面も、中より外のほうが離れている。</strong> 分かれている");
    } else {
        eprintln!(
            "  但し書き: <strong>中のほうが散らばっている場面がある</strong>（{}）",
            suspicious.join("、")
        );
        eprintln!("  1 つの場面として扱えているかを疑う");
    }
}

/// 指標ごとの散らばりの平均。<strong>測れた指標だけで取る。</strong>
fn mean_spread(rows: &[kakiburi_scale::effective::Row]) -> f64 {
    let mut totals = Vec::new();
    let mut by_metric: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    for row in rows {
        for (name, v) in row {
            if let Some(v) = v {
                by_metric.entry(name.as_str()).or_default().push(*v);
            }
        }
    }
    for (_, vs) in by_metric {
        if vs.len() < 2 {
            continue;
        }
        let m = vs.iter().sum::<f64>() / vs.len() as f64;
        // <strong>指標ごとに大きさが違うので、平均で割る。</strong> 割らなければ、
        // 値の大きい指標だけが全体を決める。
        let sd = (vs.iter().map(|v| (v - m).powi(2)).sum::<f64>() / vs.len() as f64).sqrt();
        if m.abs() > f64::EPSILON {
            totals.push(sd / m.abs());
        }
    }
    if totals.is_empty() {
        return 0.0;
    }
    totals.iter().sum::<f64>() / totals.len() as f64
}

/// 2 つの集合の、指標ごとの中心の隔たりの平均。
fn mean_gap(a: &[kakiburi_scale::effective::Row], b: &[kakiburi_scale::effective::Row]) -> f64 {
    let center = |rows: &[kakiburi_scale::effective::Row]| -> BTreeMap<String, f64> {
        let mut by: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for row in rows {
            for (name, v) in row {
                if let Some(v) = v {
                    by.entry(name.clone()).or_default().push(*v);
                }
            }
        }
        by.into_iter()
            .map(|(k, vs)| {
                let n = vs.len() as f64;
                (k, vs.iter().sum::<f64>() / n)
            })
            .collect()
    };
    let (ca, cb) = (center(a), center(b));
    let mut gaps = Vec::new();
    for (name, va) in &ca {
        let Some(vb) = cb.get(name) else { continue };
        let scale = va.abs().max(vb.abs());
        if scale > f64::EPSILON {
            gaps.push((va - vb).abs() / scale);
        }
    }
    if gaps.is_empty() {
        return 0.0;
    }
    gaps.iter().sum::<f64>() / gaps.len() as f64
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
        say(format!(
            "  {} ({} 字): {}",
            r.name,
            r.chars,
            why.join(" / ")
        ));
    }
}

/// カセットの単位から<strong>定型を落とした写し</strong>を作る。
///
/// <strong>原本は変えない。</strong> 定型は[人が決めたこと](../../../docs/spec/200-extract.md#定型を落とす)
/// であって本文ではないので、決め直したら測り直せる形にしておく。カセットへ
/// 書き戻すのは落とす前の本文である。
fn stripped(c: &Cassette, scene: &str, role: Role) -> Vec<(String, kakiburi_doc::Document)> {
    let boilerplate = c
        .track(scene)
        .map(|t| t.decided.boilerplate.clone())
        .unwrap_or_default();
    // <strong>束ねてから落とす。</strong> 測るのは単位であって、取り込んだ 1 本ではない。
    c.bundles(scene, role)
        .into_iter()
        .map(|(unit, doc)| (unit, doc.without_boilerplate(&boilerplate)))
        .collect()
}

/// 他人の文書。<strong>落とす定型は掛けない。</strong>
///
/// <strong>落とす定型は場面ごとに人が決めたものである。</strong> 場面を持たない文書に、
/// どれか 1 つの場面の定型を当てる筋は無い——当てれば、どの場面で `build` したかで
/// 人らしさの較正が変わる。
fn stripped_others(c: &Cassette) -> Vec<(String, kakiburi_doc::Document)> {
    c.other_bundles()
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
    let mut scene: Option<String> = None;
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
            "--scene" => {
                let Some(v) = args.get(i + 1) else {
                    eprintln!("--scene に場面が要る");
                    return Exit::Usage;
                };
                scene = Some(v.clone());
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
    // <strong>場面は明示する。</strong> ファイルを選ぶことが暗黙の場面指定だった頃は、
    // 選び間違えても止まらなかった——<strong>1 手増えるが、正しい向きの摩擦である。</strong>
    let Some(scene) = scene else {
        eprintln!("--scene が要る。どの場面として検めるかは人が指定する");
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

    // <strong>知らない場面では検めない。</strong> 綴りを間違えたまま通れば、目盛りが無い
    // 場面として「判定できない」が返り、間違いに見えない。
    let Some(track) = c.track(&scene) else {
        eprintln!("断る: 知らない場面: {scene}");
        eprintln!("このカセットの場面: {}", scenes_or_none(&c));
        return Exit::Usage;
    };

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
    let Some(scale) = track.derived.scale.as_deref().and_then(scale_json::read) else {
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
    let doc = doc.without_boilerplate(&track.decided.boilerplate);

    // <strong>相手集合は目盛りが名指ししたものである。</strong> 検める側が選び直さない。
    let person_units = stripped(&c, &scene, Role::Person);
    let person = samples(&person_units);
    let partners: Vec<Sample<'_>> = person
        .iter()
        .filter(|s| scale.partners().iter().any(|n| n == s.name))
        .copied()
        .collect();
    if partners.len() != scale.partners().len() {
        eprintln!(
            "相手集合が揃わない（{} / {} 本）。カセットの本文が入れ替わっている",
            partners.len(),
            scale.partners().len()
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
        &partners,
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
        &partners,
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
    let Some(effective) = track
        .derived
        .spread
        .as_deref()
        .zip(track.derived.effective.as_deref())
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
        .filter(|e| !c.is_stuck(&scene, &e.name))
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
        .filter(|e| !c.is_stuck(&scene, &e.name))
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
                at: 0.0,
                used: times > 0,
                spots,
                times,
                density,
                ceiling: *ceiling,
            }
        })
        .collect();
    let katas = seen(&scale.katas);
    // <strong>役を入れ替えた側も同じ見方で拾う。</strong>
    let machine_katas = seen(&scale.machine_katas);
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
    for (scene, t) in &c.tracks {
        let scale = t.derived.scale.as_deref().and_then(scale_json::read);
        inputs.scenes.insert(
            scene.clone(),
            kakiburi_cassette::SceneInputs {
                vocabulary: vocabulary_of(scale.as_ref()),
                z_scores: z_scores_of(scale.as_ref()),
                // <strong>基準の作り方は `decided` が正本である。</strong> 指紋に写した値ではなく、
                // 人が決めたほうを読む——写しを読むと、決め直したのに指紋が動かない。
                baseline: t.decided.baseline.clone(),
                decided: decided_inputs(scene, &t.decided),
            },
        );
    }
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
        scenes: BTreeMap::new(),
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
    let mecab = analyzer::resolve();
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

    if json {
        // <strong>人向けの表示は変えない。</strong> 出すのは同じ値の生の形である。
        let mecab = analyzer::resolve();
        #[allow(clippy::cast_precision_loss)]
        let tokens = mecab
            .as_ref()
            .and_then(|m| kakiburi_metrics::morph::Analyzed::of(&doc.prose(), m).ok())
            .map(|a| a.tokens() as f64);
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
                ("directives".to_owned(), machine::metrics(&measured(&doc))),
                (
                    // <strong>人らしさの生の値も出す。</strong> カセットが無くても測れる値であり、
                    // 出さなければ<strong>素材が向きを支えているかを外から確かめられない</strong>。
                    "humanness".to_owned(),
                    machine::metrics(
                        &kakiburi_metrics::humanness::Humanness::measure(
                            &doc.prose(),
                            analyzed_of(
                                &doc.prose(),
                                mecab
                                    .as_ref()
                                    .map(|m| m as &dyn kakiburi_metrics::morph::Analyzer),
                            )
                            .as_ref(),
                        )
                        .flat(),
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
    // <strong>解析できたものは測る側にも渡す。</strong> ここで捨てると、辞書を指しているのに
    // 解析器を要る軸が「道具が無い」で並ぶ。
    let mut analyzed = None;
    if let Some(m) = analyzer::resolve() {
        match kakiburi_metrics::morph::Analyzed::of(&doc.prose(), &m) {
            Ok(a) => {
                println!(
                    "延べ {} 語（{} {}）",
                    a.tokens(),
                    m.dict_name,
                    m.dict_version
                );
                analyzed = Some(a);
            }
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
    for (name, m) in measured_with(&doc, analyzed.as_ref()) {
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
            assert_eq!(names.len(), kakiburi_scale::split::PER_SIDE);
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
            run(&[
                "add".to_owned(),
                c.clone(),
                f.clone(),
                "--as".to_owned(),
                "person".to_owned(),
            ]),
            Exit::Usage,
            "add"
        );
        assert_eq!(
            run(&["review".to_owned(), f, "--cassette".to_owned(), c.clone(),]),
            Exit::Usage,
            "review"
        );
        // 断ったのだから、何も入っていない。
        let (got, _) = open(&c).expect("読める");
        assert!(got.corpus.is_empty());
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
    fn cassette_with_scale(dir: &std::path::Path) -> String {
        let scale = fixture::scale();
        let (person, baseline) = fixture::corpus();
        let units: Vec<Unit> = person
            .iter()
            .map(|(n, d)| Unit {
                name: n.clone(),
                unit: n.clone(),
                belongs: kakiburi_cassette::Belongs::Person {
                    scene: "試験".into(),
                },
                document: d.clone(),
            })
            .chain(baseline.iter().map(|(n, d)| Unit {
                name: n.clone(),
                unit: n.clone(),
                belongs: kakiburi_cassette::Belongs::Baseline {
                    scene: "試験".into(),
                },
                document: d.clone(),
            }))
            .collect();
        let mut c = Cassette {
            version: store::VERSION,
            generation: 0,
            fingerprint: current_fingerprint(),
            provisional: vec![],
            corpus: Corpus::new(units),
            tracks: BTreeMap::new(),
        };
        c.track_mut("試験").derived.scale = Some(scale_json::write(&scale));
        // <strong>効くかの判定も入れる。</strong> 入れなければ、検めはそこで判定できないを返す
        // ——別の理由で止まるので、通したい経路が通っていないことに気付けない。
        let effective = kakiburi_scale::effective::judge(
            &rows(&fixture::samples(&person), Some(&fixture::Chars)),
            &rows(&fixture::samples(&baseline), Some(&fixture::Chars)),
            &|n| remedies::FromDefinitions::load().by_appearance(n),
        );
        c.track_mut("試験").derived.spread = Some(effective_json::write_spread(&effective));
        c.track_mut("試験").derived.effective = Some(effective_json::write_effective(&effective));
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
            "--scene".to_owned(),
            "試験".to_owned(),
            "--source".to_owned(),
            "plain-markdown".to_owned(),
        ];
        // 短い 1 本なので除外に掛かる。<strong>0 ではなく「測れていない」が返る</strong>ので、
        // 1 段目で止まって判定できないになる。
        assert_eq!(run(&args), Exit::Unknown);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 空のカセットを 1 つ作る。
    fn empty_cassette(dir: &std::path::Path) -> String {
        let p = dir.join("c.kbc").to_string_lossy().into_owned();
        assert_eq!(run(&["new".to_owned(), p.clone()]), Exit::Pass);
        assert_eq!(
            run(&["scene".to_owned(), p.clone(), "試験".to_owned()]),
            Exit::Pass
        );
        p
    }

    /// `add` の引数。<strong>場面と取り込み元は毎回書く。</strong>
    fn add_args(cassette: &str, file: &str, role: &str) -> Vec<String> {
        let mut out = vec![
            "add".to_owned(),
            cassette.to_owned(),
            file.to_owned(),
            "--as".to_owned(),
            role.to_owned(),
            "--source".to_owned(),
            "plain-markdown".to_owned(),
        ];
        if role != "other" {
            out.push("--scene".to_owned());
            out.push("試験".to_owned());
        }
        out
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
        let args = |c: &str, f: &str| add_args(c, f, "person");
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
        let args = |role: &str| add_args(&c, &f, role);
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
        assert_eq!(run(&add_args(&c, &f, "person")), Exit::Pass);
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
        assert_eq!(got.corpus.len(), 1, "増えない");
        assert_eq!(
            got.corpus.in_scene("試験", Role::Person)[0].name,
            "もとの名前"
        );
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
                "--scene".to_owned(),
                "試験".to_owned(),
                "boilerplate".to_owned(),
                "お世話になっており".to_owned(),
            ]),
            Exit::Pass
        );
        let (got, _) = open(&c).expect("読める");
        assert_eq!(
            got.track("試験").unwrap().decided.boilerplate,
            vec!["お世話になっており"]
        );
        // <strong>置き換える。</strong> 足していく形にしない。
        assert_eq!(
            run(&[
                "decide".to_owned(),
                c.clone(),
                "--scene".to_owned(),
                "試験".to_owned(),
                "boilerplate".to_owned(),
            ]),
            Exit::Pass
        );
        let (got, _) = open(&c).expect("読める");
        assert!(got.track("試験").unwrap().decided.boilerplate.is_empty());
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
                "--scene".to_owned(),
                "試験".to_owned(),
                "movement".to_owned(),
                "絵文字".to_owned(),
                "stuck".to_owned(),
            ]),
            Exit::Pass
        );
        let (got, _) = open(&c).expect("読める");
        assert!(got.is_stuck("試験", "絵文字"));
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
        assert!(before.track("試験").unwrap().derived.effective.is_some());
        assert_eq!(
            run(&[
                "decide".to_owned(),
                cassette.clone(),
                "--scene".to_owned(),
                "試験".to_owned(),
                "movement".to_owned(),
                "絵文字".to_owned(),
                "stuck".to_owned(),
            ]),
            Exit::Pass
        );
        let (after, _) = open(&cassette).expect("読める");
        assert!(
            after.track("試験").unwrap().derived.effective.is_none(),
            "捨てている"
        );
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
            "--scene".to_owned(),
            "試験".to_owned(),
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
        assert_eq!(got.track("試験").unwrap().decided.baseline.model, "m");
        assert_eq!(got.track("試験").unwrap().decided.baseline.version, "v1");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 既に在るところには作らない() {
        // 取り込みには時間が掛かり、原本は作り直せない。上書きすれば、入れた単位は
        // そこで消える。
        let dir = temp_dir("new-exists");
        let c = empty_cassette(&dir);
        assert_eq!(
            run(&[
                "new".to_owned(),
                c.clone(),
                "--scene".to_owned(),
                "別の場面".to_owned(),
            ]),
            Exit::Usage,
            "既に在れば断る"
        );
        let (got, _) = open(&c).expect("読める");
        assert_eq!(got.scenes(), vec!["試験"], "断ったのだから中身は変わらない");
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
            assert_eq!(run(&["new".to_owned(), p.clone()]), Exit::Pass);
            assert_eq!(
                run(&["scene".to_owned(), p.clone(), scene.to_owned()]),
                Exit::Pass
            );
            made.push(open(&p).expect("読める").0);
        }
        // <strong>片方にしか無い場面として、両方が差に出る。</strong>
        assert_eq!(
            made[0].fingerprint.differences(&made[1].fingerprint),
            vec!["技術記事 の場面ごとの材料", "議事録 の場面ごとの材料"]
        );
        // <strong>道具は同じである。</strong> 場面が違うだけで「辞書が変わった」と言わない。
        assert!(made[0]
            .fingerprint
            .common_differences(&made[1].fingerprint)
            .is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 分かれている場面は外との差が中の散らばりを上回る() {
        // <strong>場面が分かれていないなら、場面ごとに閉じている意味が無い。</strong>
        // 逆に、中が外より散らばっているなら、その「1 つの場面」は 1 つではない。
        let row =
            |v: f64| -> kakiburi_scale::effective::Row { vec![("指標".to_owned(), Some(v))] };
        let tight = vec![row(1.0), row(1.02), row(0.98)];
        let far = vec![row(10.0), row(10.2), row(9.8)];
        assert!(
            mean_gap(&tight, &far) > mean_spread(&tight),
            "離れていれば上回る"
        );
        let overlapping = vec![row(1.0), row(9.0), row(5.0)];
        let also = vec![row(1.2), row(8.8), row(5.1)];
        assert!(
            mean_gap(&overlapping, &also) <= mean_spread(&overlapping),
            "中のほうが散らばっていれば上回らない"
        );
    }

    #[test]
    fn 測れていない指標は分かれ方に数えない() {
        // 測れていないものを 0 として混ぜれば、測れない場面ほど離れて見える。
        let row =
            |v: Option<f64>| -> kakiburi_scale::effective::Row { vec![("指標".to_owned(), v)] };
        assert_eq!(mean_spread(&[row(None), row(None)]), 0.0);
        assert_eq!(mean_gap(&[row(None)], &[row(Some(1.0))]), 0.0);
    }

    #[test]
    fn 各コマンドが自分の節を出す() {
        // 位置引数がファイルなので、そのまま渡すと `--help` がファイル名として
        // 解釈され、<strong>「読めない（65）」で終わる。</strong>
        for name in [
            "measure", "new", "scene", "add", "replace", "decide", "build", "review", "compare",
            "show", "doctor", "metrics",
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
    fn 知らない場面は断る() {
        // 綴りを間違えたまま通れば、誰もいない場面に素材が入り、エラーも出ない。
        // <strong>review では「目盛りが無い」として正常な停止に化ける。</strong>
        let dir = temp_dir("unknown-scene");
        let c = empty_cassette(&dir);
        let f = a_document(&dir, "x");
        let bad = |mut args: Vec<String>| {
            let i = args.iter().position(|a| a == "試験").expect("在る");
            args[i] = "試駼".to_owned();
            args
        };
        assert_eq!(run(&bad(add_args(&c, &f, "person"))), Exit::Usage, "add");
        assert_eq!(
            run(&bad(vec![
                "decide".to_owned(),
                c.clone(),
                "--scene".to_owned(),
                "試験".to_owned(),
                "boilerplate".to_owned(),
            ])),
            Exit::Usage,
            "decide"
        );
        assert_eq!(
            run(&bad(vec![
                "review".to_owned(),
                f.clone(),
                "--cassette".to_owned(),
                c.clone(),
                "--scene".to_owned(),
                "試験".to_owned(),
                "--source".to_owned(),
                "plain-markdown".to_owned(),
            ])),
            Exit::Usage,
            "review"
        );
        assert_eq!(
            run(&bad(vec![
                "build".to_owned(),
                c.clone(),
                "--scene".to_owned(),
                "試験".to_owned(),
            ])),
            Exit::Usage,
            "build"
        );
        // 断ったのだから、何も入っていない。
        let (got, _) = open(&c).expect("読める");
        assert!(got.corpus.is_empty());
        assert_eq!(got.scenes(), vec!["試験"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 他人の文書は場面を取らない() {
        // 仕様の「越境はこの用途に限る」を、引数の形でそのまま言う。
        let dir = temp_dir("other-scene");
        let c = empty_cassette(&dir);
        let f = a_document(&dir, "o");
        let mut with = add_args(&c, &f, "other");
        with.push("--scene".to_owned());
        with.push("試験".to_owned());
        assert_eq!(run(&with), Exit::Usage, "--scene を渡したら断る");
        assert_eq!(run(&add_args(&c, &f, "other")), Exit::Pass);
        let (got, _) = open(&c).expect("読める");
        assert_eq!(got.corpus.for_humanness().len(), 1);
        assert!(
            got.corpus.in_scene("試験", Role::Other).is_empty(),
            "場面で絞る口からは出てこない"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 場面ごとに指紋の欄が分かれる() {
        // <strong>1 つの場面を build しても、ほかの場面の語彙は消えない。</strong>
        // 割らずに 1 つの欄で持てば、2 つ目を作った時点で 1 つ目が上書きされる。
        let dir = temp_dir("scene-fingerprint-split");
        let c = empty_cassette(&dir);
        assert_eq!(
            run(&["scene".to_owned(), c.clone(), "チャット".to_owned()]),
            Exit::Pass
        );
        let (mut cass, _) = open(&c).expect("読める");
        cass.track_mut("試験").decided.boilerplate = vec!["この記事では".into()];
        refresh(&mut cass);
        let before = cass.fingerprint.clone();

        cass.track_mut("チャット").decided.boilerplate = vec!["おつかれさまです".into()];
        refresh(&mut cass);

        assert_eq!(
            before.scene_differences(&cass.fingerprint, "チャット"),
            vec!["人が決めたこと"]
        );
        assert!(
            before
                .scene_differences(&cass.fingerprint, "試験")
                .is_empty(),
            "触っていない場面は動かない"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 作った直後の指紋は環境と合う() {
        // 場面を入れ忘れると、作った直後から「合わない」になる。
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
        let mecab = analyzer::resolve();
        let mecab = mecab.as_ref();
        let before = open(&c).expect("読める").0.fingerprint;

        assert_eq!(
            run(&[
                "decide".to_owned(),
                c.clone(),
                "--scene".to_owned(),
                "試験".to_owned(),
                "boilerplate".to_owned(),
                "お世話になっており".to_owned(),
            ]),
            Exit::Pass
        );
        let after = fingerprint_with(&open(&c).expect("読める").0, mecab);
        assert_eq!(before.differences(&after), vec!["試験 の人が決めたこと"]);

        let metric = measured_names().first().expect("指標が要る").clone();
        assert_eq!(
            run(&[
                "decide".to_owned(),
                c.clone(),
                "--scene".to_owned(),
                "試験".to_owned(),
                "movement".to_owned(),
                metric,
                "stuck".to_owned(),
            ]),
            Exit::Pass
        );
        let stuck = fingerprint_with(&open(&c).expect("読める").0, mecab);
        assert_eq!(after.differences(&stuck), vec!["試験 の人が決めたこと"]);
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
                "--scene".to_owned(),
                "試験".to_owned(),
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
    fn 推論設定が違う基準は混ぜない() {
        // 版が同じでも温度が違えば別の出力になる。<strong>そこまでが作り方である。</strong>
        let dir = temp_dir("baseline-params");
        let c = empty_cassette(&dir);
        let args = |name: &str, temp: &str| {
            vec![
                "add".to_owned(),
                c.clone(),
                a_document(&dir, name),
                "--as".to_owned(),
                "baseline".to_owned(),
                "--scene".to_owned(),
                "試験".to_owned(),
                "--source".to_owned(),
                "plain-markdown".to_owned(),
                "--model".to_owned(),
                "m".to_owned(),
                "--version".to_owned(),
                "v1".to_owned(),
                "--param".to_owned(),
                format!("temperature={temp}"),
            ]
        };
        assert_eq!(run(&args("b1", "1.0")), Exit::Pass);
        assert_eq!(run(&args("b2", "1.0")), Exit::Pass, "同じ設定なら入る");
        assert_eq!(run(&args("b3", "0.2")), Exit::Usage, "違う設定は断る");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 推論設定を省いても消えない() {
        // 空で上書きすれば、記録してあった設定が黙って消える。
        let dir = temp_dir("baseline-params-keep");
        let c = empty_cassette(&dir);
        let mut first = vec![
            "add".to_owned(),
            c.clone(),
            a_document(&dir, "b1"),
            "--as".to_owned(),
            "baseline".to_owned(),
            "--scene".to_owned(),
            "試験".to_owned(),
            "--source".to_owned(),
            "plain-markdown".to_owned(),
            "--model".to_owned(),
            "m".to_owned(),
            "--version".to_owned(),
            "v1".to_owned(),
        ];
        first.push("--param".to_owned());
        first.push("temperature=1.0".to_owned());
        assert_eq!(run(&first), Exit::Pass);

        let mut second = first[..first.len() - 2].to_vec();
        second[2] = a_document(&dir, "b2");
        assert_eq!(run(&second), Exit::Pass, "省いても入る");
        let (got, _) = open(&c).expect("読める");
        assert_eq!(
            got.track("試験")
                .unwrap()
                .decided
                .baseline
                .params
                .get("temperature"),
            Some(&"1.0".to_owned()),
            "省いた設定が消えている"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 差し替えでも取り込み元が指紋に残る() {
        // `add` だけが足すと、別の取り込み元で差し替えたことが指紋から読めない。
        // 取り込み元の取り違えを検出する唯一の手がかりが動かなくなる。
        let dir = temp_dir("replace-source");
        let c = empty_cassette(&dir);
        let f = a_document(&dir, "x");
        assert_eq!(run(&add_args(&c, &f, "person")), Exit::Pass);
        let before = open(&c).expect("読める").0;
        assert_eq!(
            before.fingerprint.inputs.common.normalization.sources,
            vec!["plain-markdown"]
        );
        assert_eq!(
            run(&[
                "replace".to_owned(),
                c.clone(),
                f,
                "--id".to_owned(),
                "x".to_owned(),
                "--source".to_owned(),
                "github-markdown".to_owned(),
            ]),
            Exit::Pass
        );
        let (after, _) = open(&c).expect("読める");
        assert_eq!(
            after.fingerprint.inputs.common.normalization.sources,
            vec!["github-markdown", "plain-markdown"],
            "差し替えた取り込み元が残っていない"
        );
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
                    "--scene".to_owned(),
                    "試験".to_owned(),
                    "--source".to_owned(),
                    "plain-markdown".to_owned(),
                    "--unit".to_owned(),
                    "束".to_owned(),
                ]),
                Exit::Pass
            );
        }
        let (got, _) = open(&c).expect("読める");
        assert_eq!(got.corpus.len(), 2, "取り込んだのは 2 本");
        let bundles = got.bundles("試験", Role::Person);
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
                "--scene".to_owned(),
                "試験".to_owned(),
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
        assert_eq!(save::generation_of(&c), 2, "作って場面を足した時点で 2");
        let f = a_document(&dir, "x");
        assert_eq!(
            run(&[
                "add".to_owned(),
                c.clone(),
                f,
                "--as".to_owned(),
                "person".to_owned(),
                "--scene".to_owned(),
                "試験".to_owned(),
                "--source".to_owned(),
                "plain-markdown".to_owned(),
            ]),
            Exit::Pass
        );
        assert_eq!(save::generation_of(&c), 3);
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
        c.track_mut("試験").derived.effective = None;
        std::fs::write(&cassette, store::write(&c)).expect("書ける");

        let target = dir.join("検める.md");
        std::fs::write(&target, "これは、そうだ、と思う。\n").expect("書ける");
        let args = [
            "review".to_owned(),
            target.to_string_lossy().into_owned(),
            "--cassette".to_owned(),
            cassette,
            "--scene".to_owned(),
            "試験".to_owned(),
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
        let mut units: Vec<Unit> = c
            .corpus
            .in_scene("試験", Role::Person)
            .into_iter()
            .chain(c.corpus.in_scene("試験", Role::BaselineOutput))
            .cloned()
            .collect();
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
            "--scene".to_owned(),
            "試験".to_owned(),
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
