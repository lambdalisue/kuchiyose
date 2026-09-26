//! kakiburi。人が触る面（[コマンドの体系](../../../docs/design/200-command.md)）。
//!
//! トップレベルのコマンドは `review` 1 つである。 `review` だけが周回に出てくる。
//! カセットを作る・見る操作は `cassette` の下に並べる。

mod analyzer;
mod cassette_cmd;
mod cassettes;
mod diff_cmd;
mod edit_ui;
mod environment;
mod exit;
#[cfg(test)]
mod fixture;
mod folder;
mod machine;
mod pair;
mod remedies;
mod review;
mod stats_json;
#[cfg(test)]
mod testdir;
mod tuning_cmd;

use exit::Exit;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = run(&args);
    std::process::exit(code.code());
}

fn run(args: &[String]) -> Exit {
    // `--help` は引数解析の前に見る。 位置引数がファイルなので、そのまま渡すと
    // `--help` がファイル名として解釈され、「読めない（65）」で終わる。
    if args.iter().any(|a| a == "--help" || a == "-h") && args.len() > 1 {
        return match section(args) {
            Some(text) => {
                println!("{text}");
                Exit::Pass
            }
            None => {
                eprintln!("知らないコマンド: {}", args[0]);
                Exit::Usage
            }
        };
    }
    match args.first().map(String::as_str) {
        Some("review") => review::run(&args[1..]),
        Some("cassette") => cassette_cmd::run(&args[1..]),
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

/// 1 つのコマンドの help。知らない名前なら `None`。
///
/// 全体の help と同じ文を使う——2 か所に書けば、片方だけが古くなる。
fn section(args: &[String]) -> Option<String> {
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        (Some("review"), _) => Some(review_help()),
        (Some("cassette"), Some("build")) => Some(CASSETTE_BUILD.to_owned()),
        (Some("cassette"), Some("show")) => Some(CASSETTE_SHOW.to_owned()),
        (Some("cassette"), Some("diff")) => Some(CASSETTE_DIFF.to_owned()),
        (
            Some("cassette"),
            Some("list" | "mute" | "unmute" | "first-person" | "register" | "edit"),
        ) => Some(tuning_help()),
        (Some("cassette"), _) => Some(CASSETTE.to_owned()),
        _ => None,
    }
}

/// `cassette` の下のコマンドの一覧。
pub const CASSETTE: &str = "\
kakiburi cassette build        <フォルダ> [-o <カセット>] [--scene <場面>] [--json]
kakiburi cassette show         <カセット> [--json]
kakiburi cassette diff         <本人のカセット> <基準のカセット> [--json]
kakiburi cassette list         <カセット> [--kind <種類>] [--state on|off] [--baseline <カセット>]
kakiburi cassette mute         <カセット> <名前または ID>... | --kind <種類>
kakiburi cassette unmute       <カセット> <名前または ID>... | --kind <種類>
kakiburi cassette first-person <カセット> <一人称>|auto
kakiburi cassette register     <カセット> polite|plain|auto
kakiburi cassette edit         <カセット> [--baseline <カセット>]
    カセットを作る・見る・比べる・調整する。素材が増えたときと、指摘の出し方を
    変えたいときにしか動かさない。";

/// `cassette diff` の help。
pub const CASSETTE_DIFF: &str = "\
kakiburi cassette diff <本人のカセット> <基準のカセット> [--json]
    2 つのカセットから review と同じ手順で目盛りを組み立て、どこで違うかを出す。
    1 つ目を本人の側、2 つ目を基準の側として組み立てる。
    見分けられるか（照合値の帯が分かれ、本人がいちばん高く出るか）、見分けられない
    なら理由、効く指標と本人の癖、片方だけが使う言い回し（型・基準の型・基準の語）、
    使った基準の文書と束ね方、割りを出す。
    本人のカセットの調整は効かせない。無効にしているものにも印を付けて出す。
    終了コード: 0 見分けられる / 2 見分けられない / 64 以上 使う前の問題。";

/// 調整のサブコマンドの help。`--kind` に渡せる名前は実装から出す。
#[must_use]
pub fn tuning_help() -> String {
    format!(
        "\
kakiburi cassette list         <カセット> [--kind <種類>] [--state on|off] [--baseline <カセット>]
kakiburi cassette mute         <カセット> <名前または ID>... | --kind <種類> [--baseline <カセット>]
kakiburi cassette unmute       <カセット> <名前または ID>... | --kind <種類> [--baseline <カセット>]
kakiburi cassette first-person <カセット> <一人称>|auto
kakiburi cassette register     <カセット> polite|plain|auto
kakiburi cassette edit         <カセット> [--baseline <カセット>]
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

/// `cassette build` の help。
pub const CASSETTE_BUILD: &str = "\
kakiburi cassette build <フォルダ> [-o <カセット>] [--scene <場面>] [--json]
    フォルダの文書を測り、文書ごとの統計値と語のまとめ方をカセットに書く。
    比べる相手は見ない。目盛りは作らない——目盛りは review が組み立てる。
    本人の文書も基準の文書も、同じコマンドでカセットにする。どちらとして
    使うかは review に渡す位置で決まる。

    フォルダの中の読める文書を、下の階層まで経路の昇順で全部読む。
    単位の名前はファイル名の拡張子を除いた部分で、重なれば断る。
    1 本でも読めなければ、そのフォルダは 1 本も使わない。

    -o       書き出し先。既定は <フォルダ>.kb。
             無ければ作り、在れば作り直す。作り直すときは調整を引き継ぎ、
             無効にした指標の名前が今の登録簿に無ければ知らせて捨てる。
             カセットとして読めないものや、版の違うカセットは消さずに断る。
    --scene  場面の名前。省けば、作り直すときは前の名前、新しく作るときは
             フォルダの名前。

    言うのは統計値が測れたかどうかだけである——下限に届かない文書、
    測れなかった系統と指標（理由ごと）、語のまとめ方の語の数、中身のハッシュ。
    環境の側の理由で測れなかった文書があれば、カセットを書かずに断る（69）。";

/// `cassette show` の help。
pub const CASSETTE_SHOW: &str = "\
kakiburi cassette show <カセット> [--json]
    中身を出す。場面・世代・中身のハッシュ・文書の本数・長さの範囲・文体の内訳・
    測れなかった文書と理由・語のまとめ方・調整。
    指紋が今の道具と合っているかも見る。合わなければ中身を出したうえで 66 で終わる。
    素材は要らない。受け取っただけのカセットでも走る。";

/// `review` の help。調整の種類は実装から出す。
fn review_help() -> String {
    let kinds: Vec<&str> = kakiburi_cassette::MuteKind::ALL
        .iter()
        .map(|k| k.name())
        .collect();
    format!(
        "\
kakiburi review <草稿>... --cassette <カセット> [--baseline <カセット>]
                          [--values] [--json]
    2 つのカセットから目盛りを組み立て、草稿を検めて 3 値と指摘を返す。
    組み立ては草稿を読む前に終える。草稿は本人のカセットの語のまとめ方で測る。
    場面は訊かない——1 カセットが 1 場面なので、どのカセットを渡すかが場面の指定である。
    出力には本人と基準の場面の名前と、基準の中身のハッシュを出す。

    --cassette     本人のカセット。cassette build で作る。
    --baseline     基準のカセット。省けば同梱の基準を使う。cassette build で
                   作った誰のカセットでも渡せる——他人のカセットを渡せば、その人を
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

    本人のカセットの調整（{}、一人称と文体の申告）を当てる。
    無効にしているものは出力に並べて見せる。

    目盛りが組み立てられないとき、本人を基準と見分けられないとき、草稿が
    短すぎるときは、判定できない（2）を返す。どれも正常な状態である。",
        kinds.join("、")
    )
}

/// 用意するもの。help に出さなければ、仕様を読むまで進めない。
const ENVIRONMENT: &str = "\
用意するもの

  無い。 形態素解析器も辞書も基準のカセットも実行ファイルに同梱してある
  ——解析器は Lindera と UniDic 2.1.2、基準は baselines/ の文書から作ったカセットである。
  環境に置けば消える：指した先が消えていても道具は「解析器あり」と
  名乗り、以後すべての計測が黙って 0 形態素になった。";

fn print_help() {
    println!("kakiburi — どこがその人と違うかを、言えるようにする");
    println!();
    println!("作る・見る——素材が増えたとき");
    println!();
    println!("  1 カセットが 1 つのフォルダの 1 場面である。 カセットは本文を持たない。");
    println!("  素材のフォルダが正本で、カセットは文書ごとの統計値と調整だけを持つ。");
    println!();
    println!("{CASSETTE_BUILD}");
    println!();
    println!("{CASSETTE_SHOW}");
    println!();
    println!("{CASSETTE_DIFF}");
    println!();
    println!("調整する——指摘の出し方を変えたいとき");
    println!();
    println!("{}", tuning_help());
    println!();
    println!("回す——毎周");
    println!();
    println!("{}", review_help());
    println!();
    println!("{}", folder::EXTENSIONS);
    println!("フォルダからは読める拡張子のファイルだけを拾い、ほかは見ない。");
    println!();
    println!("{ENVIRONMENT}");
    println!();
    println!("--json は道具向けである。 人向けの表示は変えない。但し書きは stderr に出る。");
    println!();
    println!("終了コード: 0 通る / 1 通らない / 2 判定できない / 64 以上 使う前の問題");
    println!();
    println!("<コマンド> --help でその節だけを出せる。");
}

/// 桁を区切る。下限は文書でも区切って書いてある。
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

/// 指示できる指標の名前。登録簿を回して得る——使う側は一覧を持たない。
fn directive_names() -> Vec<String> {
    let empty = kakiburi_doc::Document::new(vec![]);
    kakiburi_metrics::directive::measure(&empty, None)
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
    fn 知らないコマンドは使い方の誤りである() {
        assert_eq!(run(&args(&["なにか"])), Exit::Usage);
        assert_eq!(run(&args(&["cassette", "なにか"])), Exit::Usage);
    }

    #[test]
    fn 移ったコマンドは知らないコマンドである() {
        // 黙って別の意味で受けない。 移った先は設計の表にある。
        for name in ["build", "doctor", "decide", "measure", "compare", "metrics"] {
            assert_eq!(run(&args(&[name])), Exit::Usage, "{name}");
            assert!(section(&args(&[name, "--help"])).is_none(), "{name}");
        }
    }

    #[test]
    fn 各コマンドが自分の節を出す() {
        // 位置引数がファイルなので、そのまま渡すと `--help` がファイル名として
        // 解釈され、「読めない（65）」で終わる。
        for v in [
            &["review", "--help"][..],
            &["cassette", "--help"],
            &["cassette", "build", "--help"],
            &["cassette", "show", "--help"],
            &["cassette", "build", "/存在しない経路", "--help"],
            &["cassette", "diff", "--help"],
            &["cassette", "list", "--help"],
            &["cassette", "mute", "a.kb", "--help"],
            &["cassette", "unmute", "--help"],
            &["cassette", "first-person", "--help"],
            &["cassette", "register", "--help"],
            &["cassette", "edit", "--help"],
        ] {
            assert_eq!(run(&args(v)), Exit::Pass, "{v:?}");
        }
        assert_eq!(
            section(&args(&["cassette", "build", "--help"])).as_deref(),
            Some(CASSETTE_BUILD)
        );
    }

    #[test]
    fn 検めの_help_は同梱の基準と渡せる基準と調整の種類を言う() {
        let h = review_help();
        assert!(h.contains("同梱の基準"), "{h}");
        assert!(h.contains("誰のカセットでも"), "{h}");
        for k in kakiburi_cassette::MuteKind::ALL {
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
        for gone in ["KAKIBURI_BASELINES", "KAKIBURI_UNIDIC", "KAKIBURI_MECAB"] {
            assert!(!ENVIRONMENT.contains(gone), "{gone} が残っている");
        }
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
