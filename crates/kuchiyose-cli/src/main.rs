//! kuchiyose。人が触る面（[コマンドの体系](../../../docs/design/200-command.md)）。
//!
//! ふだん使うのは `build`、`write`、`polish` である。 判定と指摘だけを見るのが `review`、
//! 形代を作る・見る・比べる・調整する操作は `katashiro` の下に並べる。

mod agent;
mod analyzer;
mod build_cmd;
mod config;
mod diff_cmd;
mod edit_ui;
mod env;
mod environment;
mod exit;
mod facts;
#[cfg(test)]
mod fixture;
mod folder;
mod katashiro_cmd;
mod katashiros;
mod machine;
mod pair;
mod persona_cmd;
mod polish_cmd;
mod private;
mod remedies;
mod review;
mod stats_json;
#[cfg(test)]
mod testdir;
mod tuning_cmd;
mod write_cmd;

use env::Env;
use exit::Exit;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = run_in(&args, &Env::from_process());
    std::process::exit(code.code());
}

/// 今のプロセスの環境で走らせる。
#[cfg(test)]
fn run(args: &[String]) -> Exit {
    run_in(args, &Env::from_process())
}

/// 渡した環境で走らせる。
fn run_in(args: &[String], env: &Env) -> Exit {
    // `--help` は引数解析の前に見る。 位置引数がファイルなので、そのまま渡すと
    // `--help` がファイル名として解釈され、「読めない（65）」で終わる。
    if args.iter().any(|a| a == "--help" || a == "-h") && args.len() > 1 {
        return print_section(args, env);
    }
    match args.first().map(String::as_str) {
        Some("build") => build_cmd::run(&args[1..], env),
        Some("write") => write_cmd::run(&args[1..], env),
        Some("polish") => polish_cmd::run(&args[1..], env),
        Some("review") => review::run(&args[1..], env),
        Some("katashiro") => katashiro_cmd::run(&args[1..]),
        Some("help") if args.len() > 1 => print_section(&args[1..], env),
        Some("--version" | "-V") => {
            println!("{}", version_line());
            Exit::Pass
        }
        Some("help" | "--help" | "-h") | None => {
            println!("{OVERVIEW}");
            Exit::Pass
        }
        Some(other) => {
            eprintln!("知らないコマンド: {other}");
            eprintln!("{OVERVIEW}");
            Exit::Usage
        }
    }
}

/// `--version` の 1 行。配る実行ファイルは、リリースのタグから版を入れて組む。
fn version_line() -> String {
    format!("kuchiyose {}", env!("CARGO_PKG_VERSION"))
}

fn print_section(args: &[String], env: &Env) -> Exit {
    match section(args, env) {
        Some(text) => {
            println!("{text}");
            Exit::Pass
        }
        None => {
            eprintln!("知らないコマンド: {}", args[0]);
            Exit::Usage
        }
    }
}

/// 全体の help。コマンドを 1 行ずつ並べるだけにし、詳しい説明は各コマンドの節に置く。
const OVERVIEW: &str = "\
kuchiyose — その人に寄せて書かせ、どこがその人と違うかを言えるようにする

使い方: kuchiyose <コマンド> [<引数>...]

ふだん使う
  build                   記事のフォルダから形代を作り、ペルソナを下書きさせる
  write                   要約から代筆させ、検めて寄せる
  polish                  手元の文章の表現だけを寄せる

形代を作る・見る・調整する
  review                  草稿を検めて、判定と指摘だけを返す
  katashiro build         フォルダを測って形代を作る。ペルソナは作らない
  katashiro show          形代の中身を出し、壊れていないかを確かめる
  katashiro diff          2 つの形代がどこで違うかを出す
  katashiro persona       ペルソナを取り込む・確かめる・出す・外す
  katashiro list          調整できるものと状態を並べる
  katashiro mute          指標や言い回しを判定と指摘から外す
  katashiro unmute        外した指標や言い回しを戻す
  katashiro first-person  一人称を申告する
  katashiro register      文体（敬体か常体か）を申告する
  katashiro edit          調整を対話画面で行う

よく使うオプション
  --katashiro <形代>      本人の形代。省けば既定の形代
  --baseline <形代>       基準の形代。省けば同梱の基準
  --agent <道具>          LLM の道具。claude、codex、custom
  --print                 道具を起動せず、プロンプトを出す
  --json                  道具向けの出口。人向けの表示は変えず、但し書きは stderr に出る
  --version, -V           版を出す

終了コード: 0 通る / 1 通らない / 2 判定できない / 64 以上 使う前の問題
            69 LLM の道具が見つからないか失敗した

`kuchiyose <コマンド> --help` で詳しい説明";

/// 1 つのコマンドの help。知らない名前なら `None`。
///
/// 複数のコマンドに関わる節は、同じ定数をそれぞれに並べる——2 か所に書けば、
/// 片方だけが古くなる。
fn section(args: &[String], env: &Env) -> Option<String> {
    let launches = |help: &str| [help, AGENTS, &defaults(env), ENVIRONMENT].join("\n\n");
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        (Some("build"), _) => Some(format!("{}\n\n{}", launches(build_cmd::HELP), folders())),
        (Some("write"), _) => Some(launches(write_cmd::HELP)),
        (Some("polish"), _) => Some(launches(polish_cmd::HELP)),
        (Some("review"), _) => Some([review_help().as_str(), ENVIRONMENT].join("\n\n")),
        (Some("katashiro"), Some("build")) => {
            Some([KATASHIRO_BUILD, &folders(), ENVIRONMENT].join("\n\n"))
        }
        (Some("katashiro"), Some("show")) => Some(KATASHIRO_SHOW.to_owned()),
        (Some("katashiro"), Some("diff")) => Some(KATASHIRO_DIFF.to_owned()),
        (Some("katashiro"), Some("persona")) => Some(persona_cmd::HELP.to_owned()),
        (
            Some("katashiro"),
            Some("list" | "mute" | "unmute" | "first-person" | "register" | "edit"),
        ) => Some(tuning_help()),
        (Some("katashiro"), _) => Some([KATASHIRO, KATASHIRO_INTRO].join("\n\n")),
        _ => None,
    }
}

/// 形代とは何か。`katashiro` の下のコマンドに共通する前提。
const KATASHIRO_INTRO: &str = "\
1 形代が 1 つのフォルダの 1 場面である。 形代は本文を持たない。
素材のフォルダが正本で、形代は文書ごとの統計値と調整とペルソナを持つ。
`kuchiyose katashiro <コマンド> --help` で詳しい説明";

/// フォルダから何を読むか。拡張子の一覧は断りの文と同じ文を使う。
fn folders() -> String {
    format!(
        "{}\nフォルダからは読める拡張子のファイルだけを拾い、ほかは見ない。",
        folder::EXTENSIONS
    )
}

/// `katashiro` の下のコマンドの一覧。
pub const KATASHIRO: &str = "\
kuchiyose katashiro build        <フォルダ> [-o <形代>] [--scene <場面>] [--json]
kuchiyose katashiro show         <形代> [--json]
kuchiyose katashiro diff         <本人の形代> <基準の形代> [--json]
kuchiyose katashiro list         <形代> [--kind <種類>] [--state on|off] [--baseline <形代>]
kuchiyose katashiro mute         <形代> <名前または ID>... | --kind <種類>
kuchiyose katashiro unmute       <形代> <名前または ID>... | --kind <種類>
kuchiyose katashiro first-person <形代> <一人称>|auto
kuchiyose katashiro register     <形代> polite|plain|auto
kuchiyose katashiro edit         <形代> [--baseline <形代>]
kuchiyose katashiro persona      <形代> [<ファイル> [--material <フォルダ>] [--json] | --remove]
kuchiyose katashiro persona      --check <ファイル> --material <フォルダ> [--json]
    形代を作る・見る・比べる・調整する・ペルソナを入れる。素材が増えたときと、
    指摘の出し方を変えたいときにしか動かさない。";

/// `katashiro diff` の help。
pub const KATASHIRO_DIFF: &str = "\
kuchiyose katashiro diff <本人の形代> <基準の形代> [--json]
    2 つの形代から review と同じ手順で目盛りを組み立て、どこで違うかを出す。
    1 つ目を本人の側、2 つ目を基準の側として組み立てる。
    見分けられるか（照合値の帯が分かれ、本人がいちばん高く出るか）、見分けられない
    なら理由、効く指標と本人の癖、片方だけが使う言い回し（型・基準の型・基準の語）、
    使った基準の文書と束ね方、割りを出す。
    本人の形代の調整は効かせない。無効にしているものにも印を付けて出す。
    終了コード: 0 見分けられる / 2 見分けられない / 64 以上 使う前の問題。";

/// 調整のサブコマンドの help。`--kind` に渡せる名前は実装から出す。
#[must_use]
pub fn tuning_help() -> String {
    format!(
        "\
kuchiyose katashiro list         <形代> [--kind <種類>] [--state on|off] [--baseline <形代>]
kuchiyose katashiro mute         <形代> <名前または ID>... | --kind <種類> [--baseline <形代>]
kuchiyose katashiro unmute       <形代> <名前または ID>... | --kind <種類> [--baseline <形代>]
kuchiyose katashiro first-person <形代> <一人称>|auto
kuchiyose katashiro register     <形代> polite|plain|auto
kuchiyose katashiro edit         <形代> [--baseline <形代>]
    調整する。測った値に触らないので作り直しは要らない。書いたら次の review から効く。
    種類: {}。
    list は 1 行 1 項目で、種類・名前または ID・状態・説明をタブで区切って出す。
    型・基準の型・基準の語は基準と組み合わせて組み立てるまで決まらないので、
    --baseline の基準（省けば同梱の基準）と組み立てて並べる。長い言い回しには
    種類と文字列のハッシュから作る ID が付き、mute にも unmute にも ID で渡せる。
    知らない名前は受けない（64）。--kind で種類ごと無効にしたものは、1 つずつ
    無効にしたものとは別に持つので、種類ごと戻しても 1 つずつのものは残る。
    first-person と register は数えた結果の代わりに申告を書く。auto で消す。
    一人称は {} のどれか。
    edit は対話画面で同じことをする。端末でなければ断る（64）。",
        tuning_cmd::Subject::names(),
        tuning_cmd::first_person_choices().join("、")
    )
}

/// `katashiro build` の help。
pub const KATASHIRO_BUILD: &str = "\
kuchiyose katashiro build <フォルダ> [-o <形代>] [--scene <場面>] [--json]
    フォルダの文書を測り、文書ごとの統計値と語のまとめ方を形代に書く。
    比べる相手は見ない。目盛りは作らない——目盛りは review が組み立てる。
    本人の文書も基準の文書も、同じコマンドで形代にする。どちらとして
    使うかは review に渡す位置で決まる。

    フォルダの中の読める文書を、下の階層まで経路の昇順で全部読む。
    単位の名前はファイル名の拡張子を除いた部分で、重なれば断る。
    1 本でも読めなければ、そのフォルダは 1 本も使わない。

    -o       書き出し先。既定は <フォルダ>.katashiro。
             無ければ作り、在れば作り直す。作り直すときは調整とペルソナを引き継ぎ、
             無効にした指標の名前が今の登録簿に無ければ知らせて捨てる。
             形代として読めないものや、版の違う形代は消さずに断る。
    --scene  場面の名前。省けば、作り直すときは前の名前、新しく作るときは
             フォルダの名前。

    素材のフォルダの絶対経路を覚える。ペルソナの引用を照らす先になる。
    言うのは統計値が測れたかどうかだけである——下限に届かない文書、
    測れなかった系統と指標（理由ごと）、語のまとめ方の語の数、中身のハッシュ。
    環境の側の理由で測れなかった文書があれば、形代を書かずに断る（69）。";

/// `katashiro show` の help。
pub const KATASHIRO_SHOW: &str = "\
kuchiyose katashiro show <形代> [--json]
    中身を出す。場面・世代・中身のハッシュ・文書の本数・長さの範囲・文体の内訳・
    測れなかった文書と理由・語のまとめ方・調整・素材のフォルダ・ペルソナを持っているか。
    指紋が今の道具と合っているかも見る。合わなければ中身を出したうえで 66 で終わる。
    素材は要らない。受け取っただけの形代でも走る。";

/// `review` の help。調整の種類は実装から出す。
fn review_help() -> String {
    let kinds: Vec<&str> = kuchiyose_katashiro::MuteKind::ALL
        .iter()
        .map(|k| k.name())
        .collect();
    format!(
        "\
kuchiyose review <草稿>... [--katashiro <形代>] [--baseline <形代>]
                           [--values] [--json]
    2 つの形代から目盛りを組み立て、草稿を検めて 3 値と指摘を返す。
    組み立ては草稿を読む前に終える。草稿は本人の形代の語のまとめ方で測る。
    場面は訊かない——1 形代が 1 場面なので、どの形代を渡すかが場面の指定である。
    出力には本人と基準の場面の名前と、基準の中身のハッシュを出す。

    --katashiro    本人の形代。省けば既定の形代（KUCHIYOSE_KATASHIRO、設定ファイル）。
                   既定を使ったときも、どの形代かを名乗りに出す。
    --baseline     基準の形代。省けば同梱の基準を使う。katashiro build で
                   作った誰の形代でも渡せる——他人の形代を渡せば、その人を
                   基準にしたときに際立つところが出る。
                   同じ判定どうしを比べてよいのは、基準の中身のハッシュが同じときだけ。
    --values       測った値を全部出す。指示できる指標の値、系統ごとの距離、
                   基準との距離の指標ごと・次元ごとの値を、幅や帯と並べる。
                   草稿が 2 本以上のときは --json の中に草稿ごとに出す。
    --json         道具向けの出口。較正の設定も名前と値の組で出す。

    草稿を 2 本以上渡すと、目盛りを 1 度だけ組み立てて全部に当て、1 本 1 行で
    判定・止まった段・照合値・基準との距離を並べる。指摘は出さない。
    --json では草稿ごとの結果と、天井と比べた散らばりを出す。終了コードは、
    1 本でも通らないがあれば 1、それ以外で 1 本でも判定できないがあれば 2。

    本人の形代の調整（{}、一人称と文体の申告）を当てる。
    無効にしているものは出力に並べて見せる。

    目盛りが組み立てられないとき、本人を基準と見分けられないとき、草稿が
    短すぎるときは、判定できない（2）を返す。どれも正常な状態である。",
        kinds.join("、")
    )
}

/// 用意するもの。help に出さなければ、仕様を読むまで進めない。
const ENVIRONMENT: &str = "\
用意するもの

  測るものは無い。 形態素解析器も辞書も基準の形代も実行ファイルに同梱してある
  ——解析器は Lindera と UniDic 2.1.2、基準は baselines/ の文書から作った形代である。
  環境に置けば消える：指した先が消えていても道具は「解析器あり」と
  名乗り、以後すべての計測が黙って 0 形態素になった。
  書かせるには、手元の LLM の道具が要る。kuchiyose は LLM の API を呼ばない。";

/// 同梱している道具の定義と、自前の道具のひな形の書き方。
///
/// どの道具が起動するかを、動かす前に知れるようにする。
const AGENTS: &str = "\
LLM の道具

  --agent、KUCHIYOSE_AGENT、設定ファイルの agent、ひな形の有無（あれば custom）、
  PATH にある claude か codex の順に決める。
  claude  対話の画面は `claude <プロンプト>`。対話なしは `claude -p` に
          --permission-mode dontAsk と --tools / --allowedTools で許す操作を絞る。
  codex   対話の画面は `codex <プロンプト>`。対話なしは
          `codex exec --sandbox workspace-write --skip-git-repo-check --cd <作業の場所>`。
  custom  KUCHIYOSE_AGENT_CMD（または設定ファイルの agent_cmd）のひな形を /bin/sh -c で
          走らせる。{prompt_file} はプロンプトを書いたファイル（必須）、{mode} は
          interactive か batch に置き換わる。例: my-llm --prompt-file {prompt_file}
  build、write、polish は --print を付ければ、道具を起動せずにプロンプトを出す。";

/// 既定の形代と設定ファイルの場所、今の既定。どの形代で代筆するかを、動かす前に知れる。
fn defaults(env: &Env) -> String {
    let current = katashiros::resolve_quiet(env).unwrap_or_else(|| "無い".to_owned());
    format!(
        "\
既定の形代

  --katashiro、KUCHIYOSE_KATASHIRO、設定ファイルの katashiro の順に決める。
  build が作った形代を設定ファイルに覚える。
  設定ファイル: {}
  今の既定: {current}",
        config::path(env).display()
    )
}

/// 桁を区切る。下限は文書でも区切って書いてある。
fn with_commas(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// 指示できる指標の名前。登録簿を回して得る——使う側は一覧を持たない。
fn directive_names() -> Vec<String> {
    let empty = kuchiyose_doc::Document::new(vec![]);
    kuchiyose_metrics::directive::measure(&empty, None)
        .into_iter()
        .map(|(n, _)| n)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn 引数が無ければ助けを出す() {
        assert_eq!(run(&[]), Exit::Pass);
    }

    #[test]
    fn 版を聞かれたら版を名乗る() {
        for flag in ["--version", "-V"] {
            assert_eq!(run(&args(&[flag])), Exit::Pass, "{flag}");
        }
        assert_eq!(
            version_line(),
            format!("kuchiyose {}", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn 知らないコマンドは使い方の誤りである() {
        assert_eq!(run(&args(&["なにか"])), Exit::Usage);
        assert_eq!(run(&args(&["katashiro", "なにか"])), Exit::Usage);
    }

    #[test]
    fn 移ったコマンドは知らないコマンドである() {
        // 黙って別の意味で受けない。 移った先は設計の表にある。
        for name in ["doctor", "decide", "measure", "compare", "metrics"] {
            assert_eq!(run(&args(&[name])), Exit::Usage, "{name}");
            let env = Env::for_test("/w", &[]);
            assert!(section(&args(&[name, "--help"]), &env).is_none(), "{name}");
        }
    }

    #[test]
    fn 全体の_help_は一覧だけを短く出す() {
        let lines = OVERVIEW.lines().count();
        assert!(lines <= 45, "{lines} 行ある");
        for name in [
            "build",
            "write",
            "polish",
            "review",
            "katashiro build",
            "katashiro show",
            "katashiro diff",
            "katashiro persona",
            "katashiro list",
            "katashiro mute",
            "katashiro unmute",
            "katashiro first-person",
            "katashiro register",
            "katashiro edit",
            "--katashiro",
            "--baseline",
            "--agent",
            "--print",
            "--json",
        ] {
            assert!(OVERVIEW.contains(name), "{name} が無い");
        }
        assert!(
            !OVERVIEW.contains(KATASHIRO_BUILD),
            "詳しい説明が混ざっている"
        );
        assert!(!OVERVIEW.contains(AGENTS), "詳しい説明が混ざっている");
        assert!(
            OVERVIEW.ends_with("`kuchiyose <コマンド> --help` で詳しい説明"),
            "{OVERVIEW}"
        );
    }

    #[test]
    fn help_コマンドにコマンドを続ければその節を出す() {
        assert_eq!(run(&args(&["help", "polish"])), Exit::Pass);
        assert_eq!(run(&args(&["help", "katashiro", "diff"])), Exit::Pass);
        assert_eq!(run(&args(&["help", "なにか"])), Exit::Usage);
    }

    #[test]
    fn 道具を起動するコマンドの節は道具の決め方と既定の形代を言う() {
        let env = Env::for_test("/w", &[("XDG_CONFIG_HOME", "/設定")]);
        for name in ["build", "write", "polish"] {
            let h = section(&args(&[name, "--help"]), &env).unwrap();
            assert!(h.contains(AGENTS), "{name} に道具の決め方が無い");
            assert!(h.contains("/設定/kuchiyose/config.json"), "{name}: {h}");
            assert!(h.contains(ENVIRONMENT), "{name} に用意するものが無い");
        }
    }

    #[test]
    fn 全体の_help_から外した説明はどれかのコマンドの節にある() {
        let env = Env::for_test("/w", &[]);
        let all: String = [
            &["build", "--help"][..],
            &["write", "--help"],
            &["polish", "--help"],
            &["review", "--help"],
            &["katashiro", "--help"],
            &["katashiro", "build", "--help"],
            &["katashiro", "show", "--help"],
            &["katashiro", "diff", "--help"],
            &["katashiro", "persona", "--help"],
            &["katashiro", "list", "--help"],
        ]
        .iter()
        .map(|v| section(&args(v), &env).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
        for part in [
            build_cmd::HELP,
            write_cmd::HELP,
            polish_cmd::HELP,
            persona_cmd::HELP,
            KATASHIRO,
            KATASHIRO_INTRO,
            KATASHIRO_BUILD,
            KATASHIRO_SHOW,
            KATASHIRO_DIFF,
            &folders(),
            AGENTS,
            ENVIRONMENT,
            &tuning_help(),
            &review_help(),
            &defaults(&env),
        ] {
            assert!(all.contains(part), "どの節にも無い: {part}");
        }
    }

    #[test]
    fn 各コマンドが自分の節を出す() {
        // 位置引数がファイルなので、そのまま渡すと `--help` がファイル名として
        // 解釈され、「読めない（65）」で終わる。
        for v in [
            &["review", "--help"][..],
            &["katashiro", "--help"],
            &["katashiro", "build", "--help"],
            &["katashiro", "show", "--help"],
            &["katashiro", "build", "/存在しない経路", "--help"],
            &["katashiro", "diff", "--help"],
            &["katashiro", "list", "--help"],
            &["katashiro", "mute", "a.katashiro", "--help"],
            &["katashiro", "unmute", "--help"],
            &["katashiro", "first-person", "--help"],
            &["katashiro", "register", "--help"],
            &["katashiro", "edit", "--help"],
            &["katashiro", "persona", "a.katashiro", "--help"],
            &["build", "--help"],
            &["write", "要約", "--help"],
            &["polish", "a.md", "--help"],
        ] {
            assert_eq!(run(&args(v)), Exit::Pass, "{v:?}");
        }
        let env = Env::for_test("/w", &[]);
        let h = section(&args(&["katashiro", "build", "--help"]), &env).unwrap();
        assert!(h.starts_with(KATASHIRO_BUILD), "{h}");
        assert!(h.contains(&folders()), "{h}");
    }

    #[test]
    fn 検めの_help_は同梱の基準と渡せる基準と調整の種類を言う() {
        let h = review_help();
        assert!(h.contains("同梱の基準"), "{h}");
        assert!(h.contains("誰の形代でも"), "{h}");
        for k in kuchiyose_katashiro::MuteKind::ALL {
            assert!(h.contains(k.name()), "{} が無い", k.name());
        }
    }

    #[test]
    fn 調整の_help_は_kind_に渡せる名前を実装から出す() {
        let h = tuning_help();
        for s in tuning_cmd::Subject::ALL {
            assert!(h.contains(s.name()), "{} が無い", s.name());
        }
        for w in tuning_cmd::first_person_choices() {
            assert!(h.contains(w), "{w} が無い");
        }
    }

    #[test]
    fn 用意するものは無いと言い古い環境変数を案内しない() {
        assert!(ENVIRONMENT.contains("同梱"), "{ENVIRONMENT}");
        for gone in ["KUCHIYOSE_BASELINES", "KUCHIYOSE_UNIDIC", "KUCHIYOSE_MECAB"] {
            assert!(!ENVIRONMENT.contains(gone), "{gone} が残っている");
        }
    }

    #[test]
    fn help_は同梱の道具の定義と自前の道具のひな形の書き方を言う() {
        for word in [
            "claude",
            "codex",
            "custom",
            "{prompt_file}",
            "{mode}",
            "--print",
        ] {
            assert!(AGENTS.contains(word), "{word} が無い");
        }
    }

    #[test]
    fn help_は設定ファイルの場所と今の既定の形代を言う() {
        let env = Env::for_test(
            "/w",
            &[
                ("XDG_CONFIG_HOME", "/設定"),
                ("KUCHIYOSE_KATASHIRO", "/k/本人.katashiro"),
            ],
        );
        let d = defaults(&env);
        assert!(d.contains("/設定/kuchiyose/config.json"), "{d}");
        assert!(d.contains("今の既定: /k/本人.katashiro"), "{d}");
    }

    #[test]
    fn 桁を区切る() {
        assert_eq!(with_commas(1000), "1,000");
        assert_eq!(with_commas(999), "999");
        assert_eq!(with_commas(1_234_567), "1,234,567");
    }

    #[test]
    fn 指示できる指標の名前は登録簿から回して得る() {
        let names = directive_names();
        assert!(!names.is_empty());
        let unique: std::collections::BTreeSet<&String> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "重なりが無い");
    }
}
