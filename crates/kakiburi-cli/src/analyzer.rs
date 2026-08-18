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
#[must_use]
pub fn resolve() -> Option<Mecab> {
    let dicdir = std::env::var(DICDIR).ok().filter(|s| !s.is_empty())?;
    let program = std::env::var(PROGRAM).unwrap_or_else(|_| "mecab".into());
    let version = std::env::var(VERSION).unwrap_or_else(|_| "版の申告なし".into());
    Some(Mecab::unidic(program, dicdir, version))
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
