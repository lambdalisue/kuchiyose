//! 自分だけが読み書きできるファイルとディレクトリを、推し量れない名前で新しく作る。
//!
//! 共有の一時ディレクトリに決まった名前で書けば、先に同じ名前の symlink を置かれて
//! 別のファイルを書き換えさせられる。 名前を推し量れなくし、既にあれば開かずに作り直す。

use std::fs::{DirBuilder, OpenOptions};
use std::hash::{BuildHasher, Hasher};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// 名前がぶつかったときに作り直す回数。
const ATTEMPTS: usize = 64;

/// 推し量れない名前の部分。
///
/// `RandomState` の鍵は OS の乱数から取る。 外の crate を増やさないためにこれを使う。
fn random_part() -> String {
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    h.write_u32(std::process::id());
    format!("{:016x}", h.finish())
}

/// 名前を変えながら、作れるまで試す。
fn create<T>(
    parent: &Path,
    prefix: &str,
    suffix: &str,
    make: impl Fn(&Path) -> std::io::Result<T>,
) -> std::io::Result<(PathBuf, T)> {
    for _ in 0..ATTEMPTS {
        let path = parent.join(format!("{prefix}{}{suffix}", random_part()));
        match make(&path) {
            Ok(v) => return Ok((path, v)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "名前がぶつかり続けて作れない",
    ))
}

/// `dir` の下に、自分だけが読み書きできるファイル（0600）を新しく作って `body` を書く。
///
/// # Errors
///
/// 作れない、書けないときに返す。
pub fn create_file(
    dir: &Path,
    prefix: &str,
    suffix: &str,
    body: &[u8],
) -> std::io::Result<PathBuf> {
    let (path, mut file) = create(dir, prefix, suffix, |p| {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(p)
    })?;
    if let Err(e) = file.write_all(body) {
        std::fs::remove_file(&path).ok();
        return Err(e);
    }
    Ok(path)
}

/// `parent` の下に、自分だけが入れるディレクトリ（0700）を新しく作る。
///
/// # Errors
///
/// 作れないときに返す。
pub fn create_dir(parent: &Path, prefix: &str) -> std::io::Result<PathBuf> {
    create(parent, prefix, "", |p| {
        DirBuilder::new().mode(0o700).create(p)
    })
    .map(|(p, ())| p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdir::TempDir;
    use std::os::unix::fs::PermissionsExt;

    fn mode(p: &Path) -> u32 {
        std::fs::symlink_metadata(p).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn ファイルは自分だけが読み書きでき毎回違う名前で作る() {
        let dir = TempDir::new("private-file");
        let a = create_file(Path::new(dir.path()), "要約-", ".md", "中身".as_bytes()).unwrap();
        let b = create_file(Path::new(dir.path()), "要約-", ".md", b"").unwrap();
        assert_ne!(a, b);
        assert_eq!(mode(&a), 0o600);
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "中身");
        let name = a.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("要約-") && name.ends_with(".md"), "{name}");
    }

    #[test]
    fn ディレクトリは自分だけが入れ毎回違う名前で作る() {
        let dir = TempDir::new("private-dir");
        let a = create_dir(Path::new(dir.path()), "persona-").unwrap();
        let b = create_dir(Path::new(dir.path()), "persona-").unwrap();
        assert_ne!(a, b);
        assert!(a.is_dir());
        assert_eq!(mode(&a), 0o700);
    }
}
