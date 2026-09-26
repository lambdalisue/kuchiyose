//! 形代から素材のフォルダの経路を外す。同梱の基準形代を作るときに使う。
//!
//! 素材のフォルダは手元の事情である。 残せば、作った人の手元の経路が実行ファイルに
//! 入り、どこで作り直したかによってバイト列が変わる——`baselines/` から作り直した
//! ものと一致するかを試験で確かめられなくなる。
//!
//! ```sh
//! cargo run -p kuchiyose-katashiro --example forget_material -- <形代>
//! ```

use kuchiyose_katashiro::store;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("形代の経路を渡す");
        std::process::exit(64);
    };
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        eprintln!("読めない: {path}: {e}");
        std::process::exit(65);
    });
    let mut c = store::read(&bytes).unwrap_or_else(|e| {
        eprintln!("形代が読めない: {path}: {e}");
        std::process::exit(65);
    });
    c.material = None;
    // 世代は進めない。 新しく作った形代の世代 1 のまま置く。
    std::fs::write(&path, store::write(&c)).unwrap_or_else(|e| {
        eprintln!("書けない: {path}: {e}");
        std::process::exit(65);
    });
}
