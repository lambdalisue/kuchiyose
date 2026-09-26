//! 形態素解析器。同梱のものを使う。
//!
//! 利用者に何も用意させない。 外の実行ファイルも、手で落とす辞書も要らない
//! ——解析器は[辞書ごと実行ファイルに畳んである](kuchiyose_metrics::lindera)。
//!
//! 環境から解決する道を持たない。 持っていた頃は、指した先が消えていても
//! 「解析器あり」と名乗り、以後すべての計測が黙って 0 形態素になった
//! ——実測で、nix store から MeCab が消えた環境が「測れた単位が 0 本」と報告し、
//! 素材を疑わせた。環境に置かなければ、消えようがない。

use std::collections::BTreeMap;

use kuchiyose_katashiro::Tool;
use kuchiyose_metrics::lindera::{Lindera, DICT_NAME, DICT_VERSION, ENGINE, ENGINE_VERSION};

/// 解析器を用意する。
///
/// `Option` を返さない。 同梱なので「無い」状態が存在しない——
/// 返せば、呼ぶ側に起こり得ない場合分けを書かせることになる。
#[must_use]
pub fn resolve() -> Lindera {
    Lindera::new()
}

/// 指紋に入れる形。
///
/// 解析器と辞書の両方を名乗る。 同じ辞書でも実装が違えば分かち書きが
/// 変わりうるので、どちらが変わっても指紋が動くようにしておく。
#[must_use]
pub fn tool() -> Tool {
    let mut config = BTreeMap::new();
    config.insert("辞書".to_owned(), DICT_NAME.to_owned());
    config.insert("辞書の版".to_owned(), DICT_VERSION.to_owned());
    config.insert("同梱".to_owned(), "あり".to_owned());
    Tool {
        name: ENGINE.to_owned(),
        version: ENGINE_VERSION.to_owned(),
        config,
    }
}

/// 圧縮器の指紋。実装と版まで名乗る——「zlib」だけでは値が決まらない。
#[must_use]
pub fn compressor() -> Tool {
    let (name, version) = kuchiyose_metrics::humanness::COMPRESSOR;
    let mut config = BTreeMap::new();
    config.insert(
        "水準".to_owned(),
        kuchiyose_metrics::humanness::COMPRESSION_LEVEL.to_string(),
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
    fn 用意しなくても動く() {
        // これが成り立たなければ、利用者はまた辞書を手で落とすことになる。
        let ms = kuchiyose_metrics::morph::Analyzer::analyze(&resolve(), "これは、そうだ。");
        assert!(!ms.is_empty());
    }

    #[test]
    fn 解析器と辞書の両方を名乗る() {
        // 同じ辞書でも実装が違えば分かち書きが変わりうる。
        let t = tool();
        assert_eq!(t.name, ENGINE);
        assert_eq!(t.config.get("辞書").map(String::as_str), Some(DICT_NAME));
        assert_eq!(
            t.config.get("辞書の版").map(String::as_str),
            Some(DICT_VERSION)
        );
    }

    #[test]
    fn 圧縮器も実装と版を名乗る() {
        // 「zlib」だけでは値が決まらない。
        let t = compressor();
        assert!(!t.name.is_empty());
        assert!(!t.version.is_empty());
        assert!(t.config.contains_key("水準"));
    }
}
