//! 走らせた環境。環境変数・今のディレクトリ・実行ファイル・端末かどうか。
//!
//! 起動のときに 1 度だけ読み、値として渡す。 コマンドの中で環境を読みに行くと、
//! 試験が環境変数を書き換えることになり、並列に走るほかの試験へ漏れる。

use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

/// 読む環境変数。ほかは読まない。
const VARS: [&str; 8] = [
    "HOME",
    "PATH",
    "EDITOR",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "KUCHIYOSE_KATASHIRO",
    "KUCHIYOSE_AGENT",
    "KUCHIYOSE_AGENT_CMD",
];

/// 標準入力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// 端末である。
    Terminal,
    /// 端末でない。読めば終わりまでを返す。
    Stream,
    /// 端末でない。中身を決めてある。試験のため。
    #[cfg(test)]
    Text(String),
}

/// 走らせた環境。
#[derive(Debug, Clone)]
pub struct Env {
    /// [読む環境変数](VARS)のうち、空でないもの。
    pub vars: BTreeMap<String, String>,
    /// 今のディレクトリ。
    pub cwd: PathBuf,
    /// 今動いている実行ファイルの絶対経路。プロンプトの中で kuchiyose を名指すのに使う。
    ///
    /// 道具の側の `PATH` に kuchiyose があるとは限らない。
    pub exe: PathBuf,
    /// 標準入力。
    pub stdin: Input,
    /// 標準出力と標準エラーがどちらも端末か。
    pub output_terminal: bool,
    /// 端末の経路。対話の画面の入出力をつなぎ直す先である。
    pub tty: PathBuf,
}

impl Env {
    /// 今のプロセスから読む。
    #[must_use]
    pub fn from_process() -> Self {
        let vars = VARS
            .iter()
            .filter_map(|k| {
                std::env::var(k)
                    .ok()
                    .filter(|v| !v.is_empty())
                    .map(|v| ((*k).to_owned(), v))
            })
            .collect();
        Self {
            vars,
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            exe: std::env::current_exe().unwrap_or_else(|_| PathBuf::from("kuchiyose")),
            stdin: if std::io::stdin().is_terminal() {
                Input::Terminal
            } else {
                Input::Stream
            },
            output_terminal: std::io::stdout().is_terminal() && std::io::stderr().is_terminal(),
            tty: PathBuf::from("/dev/tty"),
        }
    }

    /// 空でない環境変数。
    #[must_use]
    pub fn var(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(String::as_str)
    }

    /// 家の経路。
    fn home(&self) -> PathBuf {
        self.var("HOME")
            .map_or_else(|| self.cwd.clone(), PathBuf::from)
    }

    /// 設定ファイルを置くディレクトリ。`$XDG_CONFIG_HOME/kuchiyose`、無ければ `~/.config/kuchiyose`。
    #[must_use]
    pub fn config_dir(&self) -> PathBuf {
        self.var("XDG_CONFIG_HOME")
            .map_or_else(|| self.home().join(".config"), PathBuf::from)
            .join("kuchiyose")
    }

    /// `build` が形代を置くディレクトリ。`$XDG_DATA_HOME/kuchiyose`、無ければ `~/.local/share/kuchiyose`。
    #[must_use]
    pub fn data_dir(&self) -> PathBuf {
        self.var("XDG_DATA_HOME")
            .map_or_else(|| self.home().join(".local/share"), PathBuf::from)
            .join("kuchiyose")
    }

    /// 相対の経路を今のディレクトリから絶対にする。
    #[must_use]
    pub fn absolute(&self, p: impl AsRef<Path>) -> PathBuf {
        let p = p.as_ref();
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.cwd.join(p)
        }
    }

    /// `PATH` から実行できるファイルを探す。返すのは絶対経路である。
    ///
    /// 道具は別のディレクトリで起動するので、`PATH` の相対の項目を相対のまま返すと、
    /// 起動する先で別のものを指す。
    #[must_use]
    pub fn find_program(&self, name: &str) -> Option<PathBuf> {
        use std::os::unix::fs::PermissionsExt;
        self.var("PATH")?
            .split(':')
            .filter(|d| !d.is_empty())
            .map(|d| self.absolute(d).join(name))
            .find(|p| {
                p.metadata()
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            })
    }

    /// 試験のための環境。変数は渡したものだけ、入出力は端末とする。
    #[cfg(test)]
    #[must_use]
    pub fn for_test(cwd: &str, vars: &[(&str, &str)]) -> Self {
        Self {
            vars: vars
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            cwd: PathBuf::from(cwd),
            exe: PathBuf::from("/opt/kuchiyose/bin/kuchiyose"),
            stdin: Input::Terminal,
            output_terminal: true,
            tty: PathBuf::from("/dev/tty"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 設定と形代の置き場は_xdg_に従い無ければ家の下に置く() {
        let e = Env::for_test("/w", &[("HOME", "/home/a")]);
        assert_eq!(e.config_dir(), PathBuf::from("/home/a/.config/kuchiyose"));
        assert_eq!(
            e.data_dir(),
            PathBuf::from("/home/a/.local/share/kuchiyose")
        );
        let e = Env::for_test(
            "/w",
            &[
                ("HOME", "/home/a"),
                ("XDG_CONFIG_HOME", "/c"),
                ("XDG_DATA_HOME", "/d"),
            ],
        );
        assert_eq!(e.config_dir(), PathBuf::from("/c/kuchiyose"));
        assert_eq!(e.data_dir(), PathBuf::from("/d/kuchiyose"));
    }

    #[test]
    fn 相対の経路は今のディレクトリから絶対にする() {
        let e = Env::for_test("/w", &[]);
        assert_eq!(e.absolute("a.md"), PathBuf::from("/w/a.md"));
        assert_eq!(e.absolute("/x/a.md"), PathBuf::from("/x/a.md"));
    }

    #[test]
    fn 実行できるファイルだけを_path_から探す() {
        let dir = crate::testdir::TempDir::new("find-program");
        let exe = dir.write("bin/道具", "#!/bin/sh\n");
        dir.write("bin/読むだけ", "");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let e = Env::for_test("/w", &[("PATH", &format!("/無い:{}", dir.join("bin")))]);
        assert_eq!(e.find_program("道具"), Some(PathBuf::from(&exe)));
        assert_eq!(e.find_program("読むだけ"), None);
        assert_eq!(e.find_program("無い"), None);
    }

    #[test]
    fn path_の相対の項目は今のディレクトリから絶対にして返す() {
        let dir = crate::testdir::TempDir::new("find-program-relative");
        let exe = dir.write("bin/道具", "#!/bin/sh\n");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let e = Env::for_test(dir.path(), &[("PATH", "bin")]);
        assert_eq!(e.find_program("道具"), Some(PathBuf::from(&exe)));
    }
}
