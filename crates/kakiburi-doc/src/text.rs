//! 日本語の文字の判定。範囲はブロックで固定する。
//!
//! スクリプト属性で書くと `。` `、` まで日本語の文字になる。Unicode の
//! Script_Extensions は句点や読点に Hiragana / Katakana を含むためである。
//! そうなると率の分母が読点の多さと連動し、読点の率が静かに下がる。

/// ひらがな。
///
/// Hiragana ブロックから、かなでないものを外す——未割り当て（U+3040、U+3097、
/// U+3098）と、濁点・半濁点（U+3099〜U+309C）。後者は前の字に付く記号である。
/// 繰り返し記号 `ゝ` `ゞ` `ゟ` は残す（`々` と同じ扱い）。
#[must_use]
pub fn is_hiragana(c: char) -> bool {
    matches!(c, '\u{3041}'..='\u{3096}' | '\u{309D}'..='\u{309F}')
}

/// カタカナ。
///
/// Katakana ブロックから <strong>約物を外す</strong>——`゠`（U+30A0）と `・`（U+30FB）。
/// どちらもブロックの中にあるが、[文字種](../../../docs/spec/metrics/文字種.md)は
/// `・` を約物に数えている。外さなければ <strong>中黒が分母に入り</strong>、中黒の多い書き手ほど
/// 分母が膨らんで中黒の率が下がる——分母が測る対象と連動してはいけない。
///
/// `ー`（U+30FC）はブロックの中にあり、残す。`々`（U+3005）は外にあるので明示する。
/// 半角カタカナは U+FF66〜U+FF9F——U+FF61〜U+FF65 は半角の約物なので入れない。
#[must_use]
pub fn is_katakana(c: char) -> bool {
    matches!(c,
        '\u{30A1}'..='\u{30FA}'   // ァ〜ヺ
        | '\u{30FC}'..='\u{30FF}' // ー ヽ ヾ ヿ
        | '\u{FF66}'..='\u{FF9F}' // 半角カタカナと半角の濁点・半濁点
        | '\u{3005}'              // 々
    )
}

/// 漢字。CJK 統合漢字とその拡張、および康熙部首。
#[must_use]
pub fn is_kanji(c: char) -> bool {
    matches!(c,
        '\u{2F00}'..='\u{2FDF}'   // 康熙部首
        | '\u{3400}'..='\u{4DBF}' // 拡張 A
        | '\u{4E00}'..='\u{9FFF}' // 統合漢字
        | '\u{F900}'..='\u{FAFF}' // 互換漢字
        | '\u{20000}'..='\u{3FFFF}' // 拡張 B 以降
    )
}

/// 日本語の文字。率の分母はこれである。
///
/// 英数字・約物・空白・記号は数えない。
#[must_use]
pub fn is_japanese(c: char) -> bool {
    is_hiragana(c) || is_katakana(c) || is_kanji(c)
}

/// 日本語の散文と呼べる文字。繰り返し記号と長音符を除いた[`is_japanese`]。
///
/// どれも前の字に付く記号であって単独で語にならない。そして `ー` は比較表の
/// 「該当なし」に使われる。数に入れると、`-` を使う書き手のセルは地の文から
/// 落ち `ー` を使う書き手のセルは残り、その差が文字種のカタカナの割合に出る。
#[must_use]
pub fn is_japanese_prose(c: char) -> bool {
    match c {
        '\u{30FC}' | '\u{FF70}'              // ー ｰ
        | '\u{3005}'                          // 々
        | '\u{309D}' | '\u{309E}'             // ゝ ゞ
        | '\u{30FD}' | '\u{30FE}' => false,   // ヽ ヾ
        _ => is_japanese(c),
    }
}

/// 日本語の文字を数える。
#[must_use]
pub fn count_japanese(s: &str) -> usize {
    s.chars().filter(|&c| is_japanese(c)).count()
}

/// 日本語の散文と呼べる文字を 1 つでも含むか。
#[must_use]
pub fn has_japanese_prose(s: &str) -> bool {
    s.chars().any(is_japanese_prose)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 約物は日本語の文字ではない() {
        // Script_Extensions で書くと、ここが全部 true になって分母が狂う。
        for c in ['。', '、', '！', '？', '「', '」', '（', '）', '…'] {
            assert!(!is_japanese(c), "{c} を日本語の文字にしてはいけない");
        }
    }

    #[test]
    fn かなのブロックの中の約物も外す() {
        // ここが仕様の穴だった。ブロックで書くと約物が入り、中黒が分母に入る。
        assert!(!is_japanese('・'), "U+30FB は約物である");
        assert!(!is_japanese('\u{30A0}'), "U+30A0 ゠ は約物である");
        assert!(!is_japanese('\u{FF65}'), "U+FF65 ･ は半角の約物である");
        assert!(!is_japanese('\u{FF61}'), "U+FF61 ｡ は半角の約物である");
        assert!(!is_japanese('\u{FF64}'), "U+FF64 ､ は半角の約物である");
    }

    #[test]
    fn 中黒は分母に入らない() {
        // 入れば、中黒の多い書き手ほど分母が膨らんで中黒の率が下がる。
        // 分母が測る対象と連動してはいけない。
        assert_eq!(count_japanese("アプリケーション・サーバ"), 11);
    }

    #[test]
    fn 濁点は日本語の文字ではない() {
        // 前の字に付く記号である。合成済みの「が」は 1 字。
        assert!(!is_japanese('\u{3099}'));
        assert!(!is_japanese('\u{309B}'));
        assert_eq!(count_japanese("が"), 1);
    }

    #[test]
    fn 英数字と空白は日本語の文字ではない() {
        for c in ['a', 'Z', '0', '9', ' ', '\u{3000}', '\t', '-', '/'] {
            assert!(!is_japanese(c), "{c} を日本語の文字にしてはいけない");
        }
    }

    #[test]
    fn かな漢字は日本語の文字である() {
        for c in ['あ', 'ん', 'ア', 'ン', '漢', '字', 'ー', '々'] {
            assert!(is_japanese(c), "{c} は日本語の文字である");
        }
    }

    #[test]
    fn 半角カタカナも数える() {
        // 表記を潰さないので、半角カタカナは来る。
        assert!(is_katakana('ｱ'));
        assert!(is_japanese('ﾝ'));
        assert_eq!(count_japanese("ｱｲｳ"), 3);
    }

    #[test]
    fn 康熙部首と拡張漢字も漢字である() {
        assert!(is_kanji('\u{2F00}')); // 康熙部首 一
        assert!(is_kanji('\u{3400}')); // 拡張 A
        assert!(is_kanji('\u{20000}')); // 拡張 B
    }

    #[test]
    fn 長音符と繰り返し記号は散文の判定から外す() {
        assert!(is_japanese('ー') && !is_japanese_prose('ー'));
        assert!(is_japanese('々') && !is_japanese_prose('々'));
        assert!(!is_japanese_prose('\u{FF70}')); // 半角長音
        assert!(is_japanese_prose('あ'));
    }

    #[test]
    fn 記号だけのセルは散文を含まない() {
        assert!(!has_japanese_prose("o"));
        assert!(!has_japanese_prose("-"));
        assert!(!has_japanese_prose("〇"));
        assert!(!has_japanese_prose("×"));
        assert!(!has_japanese_prose("ー")); // 「該当なし」の印
        assert!(has_japanese_prose("あり"));
        assert!(has_japanese_prose("デフォルト")); // カタカナ語は残る
    }

    #[test]
    fn 丸印は漢字ではない() {
        // U+3007 は統合漢字の範囲外。表の印として使われるので落ちる側に来る。
        assert!(!is_kanji('\u{3007}'));
        assert!(!is_japanese('\u{3007}'));
    }
}
