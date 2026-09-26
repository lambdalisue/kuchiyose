//! 試験のための一時の置き場。落ちても消えるように、手放すときに片づける。

use std::path::PathBuf;

/// 一時のディレクトリ。
pub struct TempDir(PathBuf);

impl TempDir {
    /// 作る。名前は試験ごとに変え、プロセスとスレッドでも分ける。
    pub fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "kakiburi-試験-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("作れる");
        Self(dir)
    }

    /// 経路。
    pub fn path(&self) -> &str {
        self.0.to_str().expect("UTF-8 の経路")
    }

    /// 中の経路。
    pub fn join(&self, rel: &str) -> String {
        self.0.join(rel).to_string_lossy().into_owned()
    }

    /// 中にファイルを置く。途中のディレクトリも作る。
    pub fn write(&self, rel: &str, body: impl AsRef<[u8]>) -> String {
        let path = self.0.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("作れる");
        }
        std::fs::write(&path, body).expect("書ける");
        path.to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}
