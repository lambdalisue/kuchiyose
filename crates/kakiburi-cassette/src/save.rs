//! カセットをファイルへ置き換える。**壊さないことを優先する。**
//!
//! カセットは[原本](crate::Corpus)を含む。失えば戻らないものを、毎回の書き込みで
//! 上書きしている。**途中で落ちれば、そこで終わる。**
//!
//! # 前提
//!
//! POSIX の `rename` が同じディレクトリの中で原子的に置き換えること、`sync_all` が
//! 返れば書いたものが残ること。**macOS の APFS と Linux の ext4 / btrfs / xfs を
//! 対象とする。** ネットワーク越しのファイルシステムは対象にしない——どれも保証が
//! 違う。
//!
//! # 段によって守れるものが違う
//!
//! | どこで落ちたか | 何が残るか |
//! | --- | --- |
//! | rename の前 | **古いカセット。** 一時ファイルが残ることがある |
//! | rename と親の同期のあいだ | **見えているのは新しいカセット。** 電源が落ちれば古いほうに戻りうる |
//! | 親の同期のあと | **新しいカセット** |
//!
//! **rename が済んだら、もう戻らない。** そこから先で守るのは「壊れていないこと」
//! だけである——新旧どちらが残っても、**読めるカセットが 1 つある。**

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::{store, Cassette};

/// 置き換えに失敗した理由。
#[derive(Debug)]
pub enum SaveError {
    /// ほかの誰かが書いている。
    Locked {
        /// 錠の経路。
        lock: PathBuf,
    },
    /// 読んだあとに、別の誰かが書いた。
    ///
    /// **そのまま置き換えれば、相手の変更が正常終了のまま消える。**
    Raced {
        /// 読んだときの世代。
        expected: u64,
        /// いまの世代。
        found: u64,
    },
    /// 作ろうとしたが、既に在る。
    ///
    /// **世代では守れない唯一の場所である。** 相手が何世代目かではなく、相手が
    /// 居ること自体が断る理由になる。
    Exists {
        /// 在ったカセットの経路。
        path: PathBuf,
    },
    /// 書き出したものを読み直せなかった。**置き換えない。**
    Verify(String),
    /// 入出力に失敗した。
    Io(std::io::Error),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Locked { lock } => {
                write!(f, "ほかの誰かが書いている（{}）", lock.display())
            }
            SaveError::Raced { expected, found } => write!(
                f,
                "読んだあとに別の誰かが書いた（世代 {expected} → {found}）。読み直しが要る"
            ),
            SaveError::Exists { path } => {
                write!(f, "既に在る: {}。入れ直すなら消してから", path.display())
            }
            SaveError::Verify(why) => write!(f, "書き出したものを読み直せない: {why}"),
            SaveError::Io(e) => write!(f, "書けない: {e}"),
        }
    }
}

impl std::error::Error for SaveError {}

impl From<std::io::Error> for SaveError {
    fn from(e: std::io::Error) -> Self {
        SaveError::Io(e)
    }
}

/// `flock` の引数。**排他で、待たない。**
///
/// 値は macOS も Linux も同じである（`LOCK_EX | LOCK_NB`）。**外の crate を
/// 増やさないために自分で名乗る。**
const LOCK_EX_NB: i32 = 2 | 4;

unsafe extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}

/// 錠。**OS に持たせる。**
///
/// 目印ファイルを作って `Drop` で消す形にすると、SIGKILL・abort・電源断で
/// `Drop` が動かず、**以後すべての置き換えが永久に断られる**——直す道が
/// 「手で消す」しかなくなる。`flock` はプロセスが終われば OS が外す。
/// 設計も前提として `flock` を挙げている。
///
/// カセット本体ではなく隣に置く——置き換えでファイルが入れ替わるので、
/// **本体を押さえると、押さえた先が古いほうに取り残される。**
///
/// **sidecar は消さない。** 消す隙に別のプロセスが同じ名前を作れば、2 つの錠が
/// 別の実体を押さえることになる。中身は空のままでよい。
struct Lock {
    /// 押さえている記述子。**開いたまま持つ**——閉じれば錠も外れる。
    _file: fs::File,
}

impl Lock {
    /// 押さえる。**押さえられなければ断る。**
    fn take(path: &Path) -> Result<Self, SaveError> {
        let lock = sidecar(path, "lock");
        let file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock)?;
        // **待たない。** 待てば、落ちた相手をいつまでも待つことになる。
        let taken = {
            use std::os::fd::AsRawFd;
            // SAFETY: 開いたままの記述子を渡すだけで、所有権は `file` にある。
            unsafe { flock(file.as_raw_fd(), LOCK_EX_NB) == 0 }
        };
        if taken {
            Ok(Self { _file: file })
        } else {
            Err(SaveError::Locked { lock })
        }
    }
}

/// まだ無いときだけ作って開く。symlink を追わない。
fn create_new(path: &Path) -> std::io::Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}

/// 書き終えた一時ファイルを本体の名前にする。
///
/// 作るときと置き換えるときで、要る保証が違う。
///
/// | | どうするか | なぜ |
/// | --- | --- | --- |
/// | 置き換える | `rename` | 既存を原子的に差し替える。それが目的である |
/// | 作る | `hard_link` して一時を外す | 既に在れば失敗する。 `rename` は黙って踏む |
///
/// `exists()` を見てから `rename` では足りない。 見てから置き換えるまでのあいだに
/// 誰かが作れば踏むし、錠は助言的なので押さえていない相手は止められない。
/// `hard_link` は「既に在れば失敗する」を OS が保証する——同じディレクトリに置いた
/// 一時ファイルなので、必ず同じファイルシステムである。
fn publish(tmp: &Path, path: &Path, creating: bool) -> Result<(), SaveError> {
    if !creating {
        return fs::rename(tmp, path).map_err(SaveError::Io);
    }
    match fs::hard_link(tmp, path) {
        Ok(()) => {
            // 名前が 2 つになったので、一時のほうを外す。実体は残る。
            fs::remove_file(tmp).ok();
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(SaveError::Exists {
            path: path.to_path_buf(),
        }),
        Err(e) => Err(SaveError::Io(e)),
    }
}

/// 隣を探すディレクトリ。
///
/// 素の相対パスでは親が空になる。`c.kbc` の親は `""` で、そのまま開こうと
/// すると失敗する——いちばん普通の打ち方で片づけが止まる。
fn parent_of(path: &Path) -> Option<&Path> {
    match path.parent() {
        Some(p) if p.as_os_str().is_empty() => Some(Path::new(".")),
        other => other,
    }
}

/// 隣に残った一時ファイルを片づける。錠を持っているあいだにだけ呼ぶ。
///
/// 錠を持っていれば、ほかの書き手は居ない——だから隣の `*.tmp.*` は全部、
/// 誰かが落とした跡である。 pid で名前を分けた以上、次に動かすときには
/// 別の pid になっているので、ここで拾わないと永久に溜まる。
fn sweep_stale(path: &Path) {
    let (Some(dir), Some(name)) = (parent_of(path), path.file_name()) else {
        return;
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let prefix = format!("{}.tmp.", name.to_string_lossy());
    for entry in entries.flatten() {
        let this = entry.file_name();
        // 自分が作る形のものだけを消す。 前置きが合うだけで消すと、
        // `<カセット>.tmp.backup` のような人が置いたファイルを黙って消す——
        // この道具がほかの場所で徹底している「黙って落とさない」からの取りこぼしになる。
        //
        // 名前が UTF-8 でなければ触らない。 潰した文字列で比べると、別のものが
        // 同じ名前に見える。
        let Some(this) = this.to_str() else { continue };
        let Some(rest) = this.strip_prefix(&prefix) else {
            continue;
        };
        if !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()) {
            fs::remove_file(entry.path()).ok();
        }
    }
}

/// 隣に置くファイルの経路。
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(format!(".{suffix}"));
    PathBuf::from(name)
}

/// 置き換える。**世代を 1 つ進めてから書く。**
///
/// `expected` は読んだときの世代。**置き換える直前に、いまの世代と照らす。**
/// 違えば断る——そのまま置き換えれば、相手の変更が正常終了のまま消える。
///
/// **新しく作るときは `expected` に `None` を渡す。** そのときは「まだ無いこと」を
/// 錠の中で確かめる——呼ぶ側で見てから渡すと、見てから書くまでのあいだに割り込まれ、
/// **取り込みずみのカセットを空のカセットで踏める。**
pub fn save(path: impl AsRef<Path>, c: &Cassette, expected: Option<u64>) -> Result<(), SaveError> {
    let path = path.as_ref();
    let _lock = Lock::take(path)?;

    // **置き換える直前に照らす。** 錠を取ったあとに見るので、ここから置き換えまでの
    // あいだに割り込まれることはない。
    if let Some(expected) = expected {
        let found = generation_of(path);
        if found != expected {
            return Err(SaveError::Raced { expected, found });
        }
    }

    // **落ちた跡を片づける。** 錠を持っているあいだは、ほかの書き手が居ないと
    // 言い切れる——だから隣の一時ファイルは全部、誰かが落とした跡である。
    //
    // **ここでしか片づけられない。** 名前に pid を入れたので、次に動かすときには
    // 別の pid になっていて、落ちた跡が自分のものだと分からない。
    sweep_stale(path);

    let mut next = c.clone();
    next.generation = expected.unwrap_or(c.generation).saturating_add(1);
    let bytes = store::write(&next);

    // 1. 同じディレクトリの一時ファイルへ書き切る。既存のファイルを切り詰めない。
    //
    // **名前は書く側ごとに変える。** 固定名だと 2 つが同時に壊れる——先に symlink を
    // 置かれれば `create` がその先を切り詰め、別のプロセスが[残りものの片づけ](sweep)で
    // **書いている最中の一時ファイルを消す。**
    //
    // **`create_new` にする。** 既に在るものを開かないので、symlink を追わない。
    let tmp = sidecar(path, &format!("tmp.{}", std::process::id()));
    {
        let mut f = match create_new(&tmp) {
            Ok(f) => f,
            // 同じ pid の残りものは、自分が前に落ちた跡である。消して作り直す。
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                fs::remove_file(&tmp)?;
                create_new(&tmp)?
            }
            Err(e) => return Err(SaveError::Io(e)),
        };
        f.write_all(&bytes)?;
        // 2. 一時ファイルを同期する。
        f.sync_all()?;
    }

    // 3. 読み直して全部を検める。**開けただけでは足りない。**
    if let Err(e) = verify(&tmp, &next) {
        fs::remove_file(&tmp).ok();
        return Err(e);
    }

    // 4. 置き換える。
    if let Err(e) = publish(&tmp, path, expected.is_none()) {
        fs::remove_file(&tmp).ok();
        return Err(e);
    }

    // 5. 親ディレクトリを同期する。**ここまでで置き換えが残る。**
    if let Some(dir) = path.parent() {
        // 開けなくても、置き換え自体は済んでいる。**戻さない。**
        if let Ok(d) = fs::File::open(dir) {
            d.sync_all().ok();
        }
    }
    Ok(())
}

/// いまファイルにある世代。**読めなければ 0。**
#[must_use]
pub fn generation_of(path: impl AsRef<Path>) -> u64 {
    fs::read(path)
        .ok()
        .and_then(|b| store::read(&b).ok())
        .map_or(0, |c| c.generation)
}

/// 残った一時ファイルを片づける。**錠を取ってからにする。**
///
/// 放っておくと、置き換えの途中まで進んだ zip がディレクトリに溜まる。
///
/// **読むだけの側は片づけない。** 読む側は錠を取らないので、書いている最中の
/// 一時ファイルと落ちた跡を見分けられない——消せば**相手の検めか置き換えを
/// 失敗させられる。** ここは錠を取って、そのうえで隣を片づける。
pub fn sweep(path: impl AsRef<Path>) {
    let path = path.as_ref();
    // 押さえられなければ、ほかの誰かが書いている。**そのときは何もしない。**
    let Ok(_lock) = Lock::take(path) else {
        return;
    };
    sweep_stale(path);
}

/// 書き出したものを読み直して検める。
///
/// **「開けた」では足りない。** 中央の索引が読めても、その先が壊れていることはある。
/// 全部を読み切って、原本がそのまま戻ることを確かめる。
fn verify(tmp: &Path, expected: &Cassette) -> Result<(), SaveError> {
    let raw = fs::read(tmp).map_err(|e| SaveError::Verify(e.to_string()))?;
    let got = store::read(&raw).map_err(|e| SaveError::Verify(e.to_string()))?;
    if got.scene != expected.scene {
        return Err(SaveError::Verify("場面が戻らない".into()));
    }
    if got.decided != expected.decided {
        return Err(SaveError::Verify("人が決めたことが戻らない".into()));
    }
    if got.generation != expected.generation {
        return Err(SaveError::Verify("世代が戻らない".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "kakiburi-save-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(&d).expect("作れる");
        d
    }

    /// 置き換えを試すための、いちばん小さいカセット。
    fn cassette() -> Cassette {
        use crate::{Baseline, Decided, Derived, Fingerprint, Inputs, Normalization, Tool};
        use std::collections::BTreeMap;
        let baseline = Baseline {
            model: "m".into(),
            version: "v".into(),
            params: BTreeMap::new(),
            topics: vec!["t".into()],
        };
        Cassette {
            version: store::VERSION,
            generation: 0,
            fingerprint: Fingerprint::build(Inputs {
                common: crate::Common {
                    metric_definitions: "試験".into(),
                    unit_definitions: "試験".into(),
                    morphology: Tool::unused(),
                    dependency: Tool::unused(),
                    compressor: Tool::unused(),
                    external_tables: BTreeMap::new(),
                    normalization: Normalization {
                        sources: vec!["plain-markdown".into()],
                        implementation: "kakiburi-normalize".into(),
                        version: "0.0.0".into(),
                        mapping: BTreeMap::new(),
                    },
                },
                scene: crate::SceneInputs {
                    baseline: baseline.clone(),
                    ..crate::SceneInputs::default()
                },
            }),
            provisional: vec![],
            scene: "試験".into(),
            decided: Decided {
                boilerplate: vec![],
                baseline,
                movement: BTreeMap::new(),
            },
            derived: Derived::dropped(),
        }
    }

    #[test]
    fn 書けば世代が_1_つ進む() {
        let d = dir("generation");
        let p = d.join("c.kbc");
        let c = cassette();
        save(&p, &c, None).expect("書ける");
        assert_eq!(generation_of(&p), 1);
        save(&p, &c, Some(1)).expect("書ける");
        assert_eq!(generation_of(&p), 2);
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 読んだあとに書かれていたら断る() {
        // そのまま置き換えれば、相手の変更が正常終了のまま消える。
        let d = dir("race");
        let p = d.join("c.kbc");
        let c = cassette();
        save(&p, &c, None).expect("書ける");
        // 別の誰かが書いた。
        save(&p, &c, Some(1)).expect("書ける");
        // こちらは世代 1 のつもりでいる。
        let e = save(&p, &c, Some(1)).unwrap_err();
        assert!(matches!(e, SaveError::Raced { .. }), "{e}");
        fs::remove_dir_all(&d).ok();
    }

    /// この試験の中で使う一時ファイルの名前。書く側ごとに変わる。
    fn tmp_of(p: &Path) -> PathBuf {
        sidecar(p, &format!("tmp.{}", std::process::id()))
    }

    #[test]
    fn 錠が取れなければ断る() {
        // 目印ファイルが在ることではなく、押さえられていることで断る。
        // 在るだけで断ると、落ちた跡のファイルが永久に書き込みを止める。
        let d = dir("lock");
        let p = d.join("c.kbc");
        let held = Lock::take(&p).expect("押さえられる");
        let e = save(&p, &cassette(), None).unwrap_err();
        assert!(matches!(e, SaveError::Locked { .. }), "{e}");
        drop(held);
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 落ちた跡の目印は書き込みを止めない() {
        // ここがこの形にした理由である。 SIGKILL や電源断で `Drop` は
        // 動かない。目印ファイルを在るかどうかで見ていると、以後すべての置き換えが
        // 永久に断られ、直す道が「手で消す」しかなくなる。
        let d = dir("stale-lock");
        let p = d.join("c.kbc");
        fs::write(sidecar(&p, "lock"), b"").expect("書ける");
        save(&p, &cassette(), None).expect("書ける");
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 錠は書き終えたら外れる() {
        let d = dir("unlock");
        let p = d.join("c.kbc");
        save(&p, &cassette(), None).expect("書ける");
        Lock::take(&p).expect("次の書き手が押さえられる");
        save(&p, &cassette(), Some(1)).expect("2 度目も書ける");
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 一時ファイルを残さない() {
        let d = dir("tmp");
        let p = d.join("c.kbc");
        save(&p, &cassette(), None).expect("書ける");
        assert!(!tmp_of(&p).exists(), "一時ファイルが残っている");
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 残った一時ファイルを片づける() {
        // 置き換えの途中まで進んだ zip が溜まらないようにする。
        let d = dir("sweep");
        let p = d.join("c.kbc");
        fs::write(tmp_of(&p), "途中").expect("書ける");
        sweep(&p);
        assert!(!tmp_of(&p).exists());
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 書いている最中は片づけない() {
        // 書いている相手は錠を持っている。 押さえられないなら、隣の一時ファイルは
        // 落ちた跡ではなく書いている最中のものである——消せば相手の検めか
        // 置き換えを失敗させられる。
        let d = dir("sweep-busy");
        let p = d.join("c.kbc");
        let writing = sidecar(&p, "tmp.999999");
        fs::write(&writing, "別の書き手が書いている途中").expect("書ける");
        let held = Lock::take(&p).expect("押さえられる");
        sweep(&p);
        assert!(writing.exists(), "書いている最中のものを消している");
        drop(held);
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 素の相対パスでも隣を探せる() {
        // `c.kbc` の親は空である。 そのまま開こうとすると失敗し、
        // いちばん普通の打ち方で片づけが止まる。
        //
        // 作業ディレクトリを動かして試さない。 プロセス全体のものなので、
        // 並列に走るほかの試験へ漏れる。
        assert_eq!(parent_of(Path::new("c.kbc")), Some(Path::new(".")));
        assert_eq!(parent_of(Path::new("./c.kbc")), Some(Path::new(".")));
        assert_eq!(parent_of(Path::new("a/c.kbc")), Some(Path::new("a")));
        assert_eq!(parent_of(Path::new("/a/c.kbc")), Some(Path::new("/a")));
    }

    #[test]
    fn 人が置いたファイルは片づけない() {
        // 前置きが合うだけで消すと、人が置いたファイルを黙って消す。
        // この道具がほかの場所で徹底している「黙って落とさない」からの取りこぼしになる。
        let d = dir("sweep-keep");
        let p = d.join("c.kbc");
        let mine = sidecar(&p, "tmp.backup");
        fs::write(&mine, "人が置いた控え").expect("書ける");
        save(&p, &cassette(), None).expect("書ける");
        assert!(mine.exists(), "人が置いたファイルを消している");
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 落ちた跡は次に書くときに片づく() {
        // pid で名前を分けた以上、次に動かすときには別の pid になっている。
        // 自分のものだと分からないので、錠を持っているあいだに隣をまとめて拾う
        // ——持っていれば、ほかの書き手が居ないと言い切れる。
        let d = dir("sweep-stale");
        let p = d.join("c.kbc");
        let stale = sidecar(&p, "tmp.999999");
        fs::write(&stale, "落ちた跡").expect("書ける");
        save(&p, &cassette(), None).expect("書ける");
        assert!(!stale.exists(), "落ちた跡が残っている");
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 既に在るところへは作らない() {
        // 世代では守れない唯一の場所である。 呼ぶ側で見てから渡すと、
        // 見てから書くまでのあいだに割り込まれる。
        let d = dir("exists");
        let p = d.join("c.kbc");
        save(&p, &cassette(), None).expect("書ける");
        let before = fs::read(&p).expect("読める");
        let e = save(&p, &cassette(), None).unwrap_err();
        assert!(matches!(e, SaveError::Exists { .. }), "{e}");
        assert_eq!(fs::read(&p).expect("読める"), before, "元のままである");
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 断ったときも元のカセットは無傷である() {
        let d = dir("intact");
        let p = d.join("c.kbc");
        save(&p, &cassette(), None).expect("書ける");
        let before = fs::read(&p).expect("読める");
        // 錠を取られている状態にする。
        let held = Lock::take(&p).expect("押さえられる");
        assert!(save(&p, &cassette(), Some(1)).is_err());
        drop(held);
        assert_eq!(fs::read(&p).expect("読める"), before, "元のままである");
        fs::remove_dir_all(&d).ok();
    }
}
