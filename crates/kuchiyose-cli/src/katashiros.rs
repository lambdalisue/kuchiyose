//! 形代を取る口。どの口も同じ検査を通す（[決定](../../../docs/design/200-command.md#形代を取る口は同じ検査を通す)）。
//!
//! 口ごとに検査を書かない。 書き分ければ、同じ形代が口によって通ったり
//! 断られたりする。
//!
//! | 確かめること | 通らなければ |
//! | --- | --- |
//! | 容器と版が読め、中身のハッシュが合う | 壊れている、または知らない版として断る（65） |
//! | 指紋が今の道具と合う | 使う前の問題として断る（66） |
//! | 統計値が読み戻せる | 壊れているとして断る（65） |

use kuchiyose_katashiro::{save, store, Katashiro};
use kuchiyose_scale::stats::KatashiroStats;

use crate::config;
use crate::env::Env;
use crate::environment;
use crate::exit::Exit;
use crate::remedies::FromDefinitions;
use crate::stats_json;

/// 同梱の基準形代。`--baseline` を省いたときに使う。
///
/// リポジトリの `baselines/` から `katashiro build` で作ったものを埋め込む
/// （[同梱の基準形代](../../../docs/design/100-katashiro.md#同梱の基準形代)）。
/// 実行ファイルだけで `review` が動き、リポジトリが手元に無くてもよい。
/// 作り直し方は `tools/build-baseline-katashiro.sh` にある。
pub const BUNDLED: &[u8] = include_bytes!("../assets/baseline.katashiro");

/// 同梱の基準形代を名乗るときの名前。
pub const BUNDLED_NAME: &str = "同梱の基準";

/// 形代の在りか。どれを渡されたかを出力で名乗るのに使う。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// ファイル。
    File(String),
    /// 実行ファイルに埋め込んだ同梱の基準。
    Bundled,
}

impl Origin {
    /// 人向けの名前。
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Origin::File(p) => p,
            Origin::Bundled => BUNDLED_NAME,
        }
    }
}

/// 読んだ形代。統計値を読み戻したものも持つ。
pub struct Opened {
    /// どこから読んだか。
    pub origin: Origin,
    /// 形代。
    pub katashiro: Katashiro,
    /// 読み戻した統計値。
    pub stats: KatashiroStats,
}

impl Opened {
    /// 場面の名前に、どこから読んだかを添える。
    ///
    /// 同梱の基準は場面の名前と出どころが同じ語なので、添えれば同じ語が 2 度並ぶ。
    #[must_use]
    pub fn named(&self) -> String {
        match self.distinct_origin() {
            Some(origin) => format!("{}（{origin}）", self.katashiro.scene),
            None => self.katashiro.scene.clone(),
        }
    }

    /// 名乗りと中身のハッシュを 1 行で出す。
    #[must_use]
    pub fn name_line(&self, role: &str) -> String {
        let hash = self.katashiro.stats.content_hash();
        match self.distinct_origin() {
            Some(origin) => format!(
                "{role}: {}（{origin}）中身のハッシュ {hash}",
                self.katashiro.scene
            ),
            None => format!("{role}: {} 中身のハッシュ {hash}", self.katashiro.scene),
        }
    }

    fn distinct_origin(&self) -> Option<&str> {
        let origin = self.origin.label();
        (origin != self.katashiro.scene).then_some(origin)
    }
}

/// バイト列を読む。容器・版・欄・中身のハッシュを検める。
///
/// # Errors
///
/// 読めなければ、理由を出して 65 で断る。
pub fn parse(bytes: &[u8], origin: &Origin) -> Result<Katashiro, Exit> {
    store::read(bytes).map_err(|e| {
        eprintln!("形代が読めない: {}: {e}", origin.label());
        Exit::Unreadable
    })
}

/// ファイルを読む。残った一時ファイルを片づけてから開く。
///
/// # Errors
///
/// 読めなければ 65 で断る。
pub fn read_file(path: &str) -> Result<Katashiro, Exit> {
    save::sweep(path);
    let Ok(raw) = std::fs::read(path) else {
        eprintln!("形代が読めない: {path}");
        return Err(Exit::Unreadable);
    };
    parse(&raw, &Origin::File(path.to_owned()))
}

/// 指紋を今の道具と照らす。合わなければ違う材料を並べて 66 で断る。
///
/// # Errors
///
/// 合わなければ 66 で断る。
pub fn check(c: &Katashiro, origin: &Origin, defs: &FromDefinitions) -> Result<(), Exit> {
    environment::check(c, defs).map_err(|diff| {
        eprintln!(
            "指紋が今の道具と合わない: {}: {}",
            origin.label(),
            diff.join("、")
        );
        match origin {
            Origin::File(_) => {
                eprintln!("素材のフォルダから katashiro build で作り直すか、道具の版を合わせる");
            }
            Origin::Bundled => {
                eprintln!(
                    "同梱の基準が作り直されていない。tools/build-baseline-katashiro.sh で作り直す"
                );
            }
        }
        Exit::FingerprintMismatch
    })
}

/// 統計値を読み戻す。
///
/// # Errors
///
/// 形が合わなければ 65 で断る。
pub fn decode(c: &Katashiro, origin: &Origin) -> Result<KatashiroStats, Exit> {
    stats_json::read(&c.stats).map_err(|e| {
        eprintln!("形代の統計値が読めない: {}: {e}", origin.label());
        Exit::Unreadable
    })
}

/// 読んで、照らして、統計値まで読み戻す。
///
/// # Errors
///
/// 読めなければ 65、指紋が合わなければ 66 で断る。
pub fn open(origin: Origin, defs: &FromDefinitions) -> Result<Opened, Exit> {
    let katashiro = match &origin {
        Origin::File(path) => read_file(path)?,
        Origin::Bundled => parse(BUNDLED, &origin)?,
    };
    check(&katashiro, &origin, defs)?;
    let stats = decode(&katashiro, &origin)?;
    Ok(Opened {
        origin,
        katashiro,
        stats,
    })
}

/// 本人の形代を決める（[既定の形代](../../../docs/design/200-command.md#既定の形代)）。
///
/// `--katashiro`、環境変数 `KUCHIYOSE_KATASHIRO`、設定ファイルの `katashiro` の順に見る。
/// 設定ファイルは、前の 2 つで決まらないときだけ読む。
///
/// # Errors
///
/// どれでも決まらなければ 64 で断る。設定ファイルが読めなくても 64 である。
pub fn resolve(flag: Option<&str>, env: &Env) -> Result<String, Exit> {
    if let Some(k) = flag.or_else(|| env.var("KUCHIYOSE_KATASHIRO")) {
        return Ok(k.to_owned());
    }
    config::load(env)?.katashiro.ok_or_else(|| {
        eprintln!("形代が決まらない。kuchiyose build で作るか、--katashiro で渡す");
        eprintln!(
            "既定の形代は KUCHIYOSE_KATASHIRO か設定ファイル（{}）の katashiro で決まる",
            config::path(env).display()
        );
        Exit::Usage
    })
}

/// 今の既定の形代。help で見せる。決まらなければ `None`。
#[must_use]
pub fn resolve_quiet(env: &Env) -> Option<String> {
    env.var("KUCHIYOSE_KATASHIRO")
        .map(str::to_owned)
        .or_else(|| config::load(env).ok()?.katashiro)
}

/// 同梱の基準形代の場面の名前。`tools/build-baseline-katashiro.sh` と同じ名前を使う。
#[cfg(test)]
const BUNDLED_SCENE: &str = "同梱の基準";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::katashiro_cmd;

    #[test]
    fn 同梱の基準は_baselines_から作り直したものと同じバイト列である() {
        // 通らなければ、同梱の基準がどの文書から作られたかを言えない。
        // baselines/ や指標の定義・解析器・測り方を変えたのに作り直し忘れると、ここで落ちる。
        // 作り直すには tools/build-baseline-katashiro.sh を走らせる。
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../baselines");
        let defs = FromDefinitions::load();
        let measured =
            katashiro_cmd::measure_folder(dir, &crate::analyzer::resolve()).expect("測れる");
        let mut c = katashiro_cmd::assemble_katashiro(
            BUNDLED_SCENE,
            &measured,
            kuchiyose_katashiro::Tuning::default(),
            &defs,
        );
        // 新しく書いた形代は世代 1 である。
        c.generation = 1;
        let rebuilt = store::write(&c);
        let embedded = parse(BUNDLED, &Origin::Bundled).expect("同梱の基準が読める");
        assert_eq!(embedded.scene, BUNDLED_SCENE);
        assert!(
            rebuilt == BUNDLED,
            "同梱の基準が baselines/ から作り直したものと違う（埋め込み {} / 作り直し {}）。\
             tools/build-baseline-katashiro.sh で作り直す",
            embedded.stats.content_hash(),
            c.stats.content_hash()
        );
    }

    #[test]
    fn 同梱の基準は今の道具と合う() {
        let c = parse(BUNDLED, &Origin::Bundled).expect("読める");
        assert_eq!(
            environment::check(&c, &FromDefinitions::load()),
            Ok(()),
            "作り直し忘れ。tools/build-baseline-katashiro.sh で作り直す"
        );
    }

    #[test]
    fn 出どころが場面の名前と同じなら名乗りに添えない() {
        let o = open(Origin::Bundled, &FromDefinitions::load()).expect("開ける");
        let hash = o.katashiro.stats.content_hash();

        assert_eq!(o.named(), BUNDLED_SCENE);
        assert_eq!(
            o.name_line("基準"),
            format!("基準: {BUNDLED_SCENE} 中身のハッシュ {hash}")
        );
    }

    #[test]
    fn 出どころが場面の名前と違えば括弧で添える() {
        let mut o = open(Origin::Bundled, &FromDefinitions::load()).expect("開ける");
        o.origin = Origin::File("基準.katashiro".to_owned());
        let hash = o.katashiro.stats.content_hash();

        assert_eq!(o.named(), format!("{BUNDLED_SCENE}（基準.katashiro）"));
        assert_eq!(
            o.name_line("基準"),
            format!("基準: {BUNDLED_SCENE}（基準.katashiro）中身のハッシュ {hash}")
        );
    }

    #[test]
    fn 形代は引数_環境変数_設定ファイルの順に決める() {
        let dir = crate::testdir::TempDir::new("resolve-katashiro");
        dir.write(
            "kuchiyose/config.json",
            r#"{"katashiro":"/設定.katashiro"}"#,
        );
        let with_env = Env::for_test(
            "/w",
            &[
                ("XDG_CONFIG_HOME", dir.path()),
                ("KUCHIYOSE_KATASHIRO", "/環境.katashiro"),
            ],
        );
        assert_eq!(
            resolve(Some("/引数.katashiro"), &with_env).unwrap(),
            "/引数.katashiro"
        );
        assert_eq!(resolve(None, &with_env).unwrap(), "/環境.katashiro");
        let config_only = Env::for_test("/w", &[("XDG_CONFIG_HOME", dir.path())]);
        assert_eq!(resolve(None, &config_only).unwrap(), "/設定.katashiro");
        let none = Env::for_test("/w", &[("XDG_CONFIG_HOME", "/無い")]);
        assert_eq!(resolve(None, &none), Err(Exit::Usage));
    }
}
