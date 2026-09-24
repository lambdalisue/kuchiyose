//! 形態素解析器を環境から解決する。
//!
//! <strong>名乗りは人の責任である。</strong> 環境変数の名前が `KAKIBURI_UNIDIC` であることが、
//! そこに UniDic を指すという申告になる——中身から当てにいかない。
//!
//! <strong>指していなければ `None` である。</strong> 既定の辞書で埋めない——別の体系で黙って
//! 測れば、語彙素で引く指標が 0 件として静かに落ちる。

use std::collections::BTreeMap;

use kakiburi_cassette::Tool;
use kakiburi_metrics::mecab::Mecab;

/// 辞書の経路を渡す環境変数。
pub const DICDIR: &str = "KAKIBURI_UNIDIC";
/// 辞書の版を渡す環境変数。
pub const VERSION: &str = "KAKIBURI_UNIDIC_VERSION";
/// MeCab の実行ファイルを渡す環境変数。
pub const PROGRAM: &str = "KAKIBURI_MECAB";

/// 環境から解決する。<strong>辞書の経路が無ければ `None`。</strong>
///
/// <strong>名乗るだけでは足りない。</strong> 経路が設定されていても、実行ファイルが消えて
/// いれば[解析は黙って 0 形態素を返す](kakiburi_metrics::mecab)——<strong>道具の側の
/// 壊れが、素材が足りないという顔で出る。</strong> 実測で、nix store から MeCab が
/// 消えた環境が「測れた単位が 0 本」と報告し、素材を疑わせた。
///
/// <strong>だから 1 度動かして確かめる。</strong> 動かなければ[壊れている](Broken)として返す
/// ——未設定とは別の状態である。
pub fn resolve() -> Result<Option<Mecab>, Broken> {
    let Some(dicdir) = std::env::var(DICDIR).ok().filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let program = std::env::var(PROGRAM).unwrap_or_else(|_| "mecab".into());
    let version = std::env::var(VERSION).unwrap_or_else(|_| "版の申告なし".into());
    let m = Mecab::unidic(program, dicdir, version);
    // <strong>日本語を 1 つ通す。</strong> 形態素が返らなければ、呼べていない。
    if kakiburi_metrics::morph::Analyzer::analyze(&m, "これは。").is_empty() {
        return Err(Broken {
            program: m.program,
            dicdir: m.dicdir.unwrap_or_default(),
        });
    }
    Ok(Some(m))
}

/// 名乗っているのに動かない。
///
/// <strong>未設定と分ける。</strong> 未設定なら辞書を入れる話だが、こちらは<strong>指した先が
/// 消えている</strong>——同じ文言で案内すると、設定済みの人が設定をやり直す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Broken {
    /// 呼ぼうとした実行ファイル。
    pub program: String,
    /// 指していた辞書。
    pub dicdir: String,
}

impl std::fmt::Display for Broken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "形態素解析器が動かない（{} を {DICDIR}={} で呼べない）",
            self.program, self.dicdir
        )
    }
}

/// 解決する。<strong>壊れていたら「無い」として扱う。</strong>
///
/// <strong>値を出さない口だけが使う。</strong> 指紋を組み直すような内部の処理は、
/// 壊れていることを利用者へ言う立場に無い——言うのは
/// [入口](resolve_or_report)の仕事である。
#[must_use]
pub fn resolved() -> Option<Mecab> {
    resolve().unwrap_or(None)
}

/// 解決するか、理由を言って止める。
///
/// <strong>壊れているときは進まない。</strong> 進めば、道具の壊れが素材の不足として出る。
pub fn resolve_or_report() -> Result<Option<Mecab>, ()> {
    match resolve() {
        Ok(v) => Ok(v),
        Err(e) => {
            eprintln!("断る: {e}");
            eprintln!("<strong>素材の問題ではない。</strong> 足しても直らない");
            eprintln!("{PROGRAM} と {DICDIR} が指す先を確かめる");
            Err(())
        }
    }
}

/// 指紋に入れる形。<strong>使わないことも書いて渡す。</strong>
#[must_use]
pub fn tool(m: Option<&Mecab>) -> Tool {
    let Some(m) = m else {
        return Tool::unused();
    };
    let mut config = BTreeMap::new();
    config.insert("辞書".to_owned(), m.dict_name.clone());
    if let Some(d) = &m.dicdir {
        // <strong>経路は入れない。</strong> 機械ごとに違うので、同じ辞書でも指紋が変わる。
        // 入れるのは「経路を指した」という事実だけである。
        config.insert("辞書の指定".to_owned(), format!("あり（{} 文字）", d.len()));
    }
    Tool {
        name: "MeCab".to_owned(),
        version: m.dict_version.clone(),
        config,
    }
}

/// 圧縮器の指紋。<strong>実装と版まで名乗る</strong>——「zlib」だけでは値が決まらない。
#[must_use]
pub fn compressor() -> Tool {
    let (name, version) = kakiburi_metrics::humanness::COMPRESSOR;
    let mut config = BTreeMap::new();
    config.insert(
        "水準".to_owned(),
        kakiburi_metrics::humanness::COMPRESSION_LEVEL.to_string(),
    );
    Tool {
        name: name.to_owned(),
        version: version.to_owned(),
        config,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 壊れていることと未設定を分ける() {
        // <strong>同じ文言で案内すると、設定済みの人が設定をやり直す。</strong>
        // 未設定なら辞書を入れる話だが、こちらは指した先が消えている。
        let broken = Broken {
            program: "/消えた/mecab".to_owned(),
            dicdir: "/dic".to_owned(),
        };
        let said = broken.to_string();
        assert!(said.contains("/消えた/mecab"), "{said}");
        assert!(said.contains(DICDIR), "どの環境変数かを言う: {said}");
        assert!(said.contains("動かない"), "未設定とは別の言い方: {said}");
    }

    #[test]
    fn 壊れた解析器は内部では無いものとして扱う() {
        // <strong>言うのは入口の仕事である。</strong> 指紋を組み直すような内部の処理は、
        // 利用者へ言う立場に無い——二重に言えば、同じ壊れが何度も出る。
        //
        // <strong>解決そのものは環境が決めるので、ここでは畳み方だけを確かめる。</strong>
        fn fold(r: Result<Option<Mecab>, Broken>) -> Option<Mecab> {
            r.unwrap_or(None)
        }
        assert!(fold(Err(Broken {
            program: "x".to_owned(),
            dicdir: "y".to_owned(),
        }))
        .is_none());
        assert!(fold(Ok(None)).is_none());
    }

    #[test]
    fn 使わないときも書いて渡す() {
        // 「未設定」と「使わない」を分ける。
        assert_eq!(tool(None), Tool::unused());
    }

    #[test]
    fn 辞書の経路は指紋に入れない() {
        // 機械ごとに違うので、同じ辞書でも指紋が変わる。
        let m = Mecab::unidic("mecab", "/どこか/unidic", "2.1.2");
        let t = tool(Some(&m));
        assert!(
            !t.config.values().any(|v| v.contains("どこか")),
            "{:?}",
            t.config
        );
        assert_eq!(t.version, "2.1.2");
    }

    #[test]
    fn 圧縮器は実装と版を名乗る() {
        // 「zlib、水準 6」では値が決まらない。
        let c = compressor();
        assert!(!c.version.is_empty(), "版を名乗る");
        assert_eq!(c.config.get("水準").map(String::as_str), Some("6"));
    }
}
