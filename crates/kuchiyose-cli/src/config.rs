//! 設定ファイル（[設定ファイル](../../../docs/design/200-command.md#設定ファイル)）。
//!
//! `$XDG_CONFIG_HOME/kuchiyose/config.json` に置く。 持つのは決めた鍵だけである。
//! 読めない設定ファイルは断る——黙って既定に戻れば、別の形代や別の道具で動いたことに
//! 気付けない。 知らない鍵も同じ理由で断る。綴りを間違えた鍵は何も設定しない。

use std::path::PathBuf;

use kuchiyose_katashiro::json::{self, Value};

use crate::env::Env;
use crate::exit::Exit;

/// 設定ファイルの名前。
pub const FILE: &str = "config.json";

/// 鍵。
const KEYS: [&str; 4] = ["katashiro", "agent", "agent_cmd", "rounds"];

/// 設定。無い鍵は `None`。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    /// 既定の形代。
    pub katashiro: Option<String>,
    /// 起動する道具。
    pub agent: Option<String>,
    /// 自前の道具のひな形。
    pub agent_cmd: Option<String>,
    /// 上限の周回数。
    pub rounds: Option<usize>,
}

/// 設定ファイルの経路。
#[must_use]
pub fn path(env: &Env) -> PathBuf {
    env.config_dir().join(FILE)
}

fn refuse(env: &Env, why: &str) -> Exit {
    eprintln!(
        "断る: 設定ファイルが読めない: {}: {why}",
        path(env).display()
    );
    eprintln!("鍵は {} だけを持てる", KEYS.join("、"));
    Exit::Usage
}

/// 読む。無ければ空の設定である。
///
/// # Errors
///
/// JSON として読めない、知らない鍵がある、値の形が違うときに、64 で断る。
pub fn load(env: &Env) -> Result<Config, Exit> {
    let Some(v) = raw(env)? else {
        return Ok(Config::default());
    };
    let Value::Object(m) = &v else {
        return Err(refuse(env, "JSON の対象でない"));
    };
    if let Some(k) = m.keys().find(|k| !KEYS.contains(&k.as_str())) {
        return Err(refuse(env, &format!("知らない鍵 {k}")));
    }
    let text = |key: &str| -> Result<Option<String>, Exit> {
        match m.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) if !s.is_empty() => Ok(Some(s.clone())),
            Some(_) => Err(refuse(env, &format!("{key} が空でない文字列でない"))),
        }
    };
    let rounds = match m.get("rounds") {
        None | Some(Value::Null) => None,
        Some(Value::Number(n))
            if n.fract() == 0.0 && *n >= 1.0 && *n <= crate::polish_cmd::MAX_ROUNDS as f64 =>
        {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            Some(*n as usize)
        }
        Some(_) => {
            return Err(refuse(
                env,
                &format!(
                    "rounds が 1 以上 {} 以下の整数でない",
                    crate::polish_cmd::MAX_ROUNDS
                ),
            ))
        }
    };
    Ok(Config {
        katashiro: text("katashiro")?,
        agent: text("agent")?,
        agent_cmd: text("agent_cmd")?,
        rounds,
    })
}

/// 設定ファイルを JSON として読む。無ければ `None`。
fn raw(env: &Env) -> Result<Option<Value>, Exit> {
    let p = path(env);
    let body = match std::fs::read_to_string(&p) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(refuse(env, &e.to_string())),
    };
    json::parse(&body)
        .map(Some)
        .map_err(|e| refuse(env, &e.to_string()))
}

/// 既定の形代を書く。ほかの鍵は触らずに残す。
///
/// # Errors
///
/// 今の設定ファイルが読めない、書けないときに断る。
pub fn remember_katashiro(env: &Env, katashiro: &str) -> Result<(), Exit> {
    // 読めない設定ファイルを上書きしない。 人が書いたほかの鍵が消える。
    load(env)?;
    let mut m = match raw(env)? {
        Some(Value::Object(m)) => m,
        _ => std::collections::BTreeMap::new(),
    };
    m.insert("katashiro".to_owned(), Value::s(katashiro));
    let p = path(env);
    let write = || -> std::io::Result<()> {
        std::fs::create_dir_all(env.config_dir())?;
        let tmp = p.with_extension(format!("json.tmp.{}", std::process::id()));
        std::fs::write(&tmp, format!("{}\n", Value::Object(m.clone()).write()))?;
        std::fs::rename(&tmp, &p)
    };
    write().map_err(|e| {
        eprintln!("断る: 設定ファイルに書けない: {}: {e}", p.display());
        Exit::Unreadable
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdir::TempDir;

    fn env(dir: &TempDir) -> Env {
        Env::for_test(dir.path(), &[("XDG_CONFIG_HOME", dir.path())])
    }

    #[test]
    fn 無ければ空の設定である() {
        let dir = TempDir::new("config-none");
        assert_eq!(load(&env(&dir)), Ok(Config::default()));
    }

    #[test]
    fn 決めた鍵を読む() {
        let dir = TempDir::new("config-read");
        dir.write(
            "kuchiyose/config.json",
            r#"{"katashiro":"/k/a.katashiro","agent":"codex","agent_cmd":"x {prompt_file}","rounds":6}"#,
        );
        assert_eq!(
            load(&env(&dir)),
            Ok(Config {
                katashiro: Some("/k/a.katashiro".into()),
                agent: Some("codex".into()),
                agent_cmd: Some("x {prompt_file}".into()),
                rounds: Some(6),
            })
        );
    }

    #[test]
    fn 読めない設定ファイルと知らない鍵は断る() {
        // 黙って既定に戻れば、別の形代や別の道具で動いたことに気付けない。
        for body in [
            "{壊れている",
            r#"{"katashro":"/k"}"#,
            r#"{"rounds":0}"#,
            r#"{"rounds":1.5}"#,
            r#"{"agent":3}"#,
            "[]",
        ] {
            let dir = TempDir::new("config-broken");
            dir.write("kuchiyose/config.json", body);
            assert_eq!(load(&env(&dir)), Err(Exit::Usage), "{body}");
        }
    }

    #[test]
    fn 既定の形代だけを書き換えてほかの鍵は残す() {
        let dir = TempDir::new("config-remember");
        dir.write(
            "kuchiyose/config.json",
            r#"{"katashiro":"/old","agent":"codex","rounds":2}"#,
        );
        remember_katashiro(&env(&dir), "/new/a.katashiro").expect("書ける");
        let got = load(&env(&dir)).unwrap();
        assert_eq!(got.katashiro.as_deref(), Some("/new/a.katashiro"));
        assert_eq!(got.agent.as_deref(), Some("codex"));
        assert_eq!(got.rounds, Some(2));
    }

    #[test]
    fn 無ければ作って書く() {
        let dir = TempDir::new("config-create");
        remember_katashiro(&env(&dir), "/new/a.katashiro").expect("書ける");
        assert_eq!(
            load(&env(&dir)).unwrap().katashiro.as_deref(),
            Some("/new/a.katashiro")
        );
    }

    #[test]
    fn 読めない設定ファイルは上書きしない() {
        let dir = TempDir::new("config-keep-broken");
        let p = dir.write("kuchiyose/config.json", "{壊れている");
        assert_eq!(remember_katashiro(&env(&dir), "/new"), Err(Exit::Usage));
        assert_eq!(std::fs::read_to_string(p).unwrap(), "{壊れている");
    }
}
