//! JSON の読み書き。
//!
//! 外の crate を持たない。 使うのはこちらが書いた形だけなので、必要な分だけを
//! 自分で持つ——依存を足すより、書ける量が小さい。
//!
//! 並び順を固定する。[作り直しても同じものが出る](../../../docs/design/300-test.md#作り直せることを試験する)
//! ためには、バイトまで同じでなければならない。

use std::collections::BTreeMap;

/// JSON の値。
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// 空。
    Null,
    /// 真偽。
    Bool(bool),
    /// 数。
    Number(f64),
    /// 文字列。
    String(String),
    /// 配列。
    Array(Vec<Value>),
    /// 対象。鍵の昇順で持つので、書き出しが決定的になる。
    Object(BTreeMap<String, Value>),
}

impl Value {
    /// 文字列を作る。
    #[must_use]
    pub fn s(v: impl Into<String>) -> Self {
        Value::String(v.into())
    }

    /// 対象を作る。
    #[must_use]
    pub fn obj(pairs: impl IntoIterator<Item = (String, Value)>) -> Self {
        Value::Object(pairs.into_iter().collect())
    }

    /// 文字列として読む。
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// 数として読む。
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// 真偽として読む。
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// 配列として読む。
    #[must_use]
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(v) => Some(v),
            _ => None,
        }
    }

    /// 鍵で引く。
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(m) => m.get(key),
            _ => None,
        }
    }

    /// 書き出す。鍵の昇順、余計な空白なし。
    #[must_use]
    pub fn write(&self) -> String {
        let mut s = String::new();
        self.write_into(&mut s);
        s
    }

    fn write_into(&self, out: &mut String) {
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Number(n) => {
                // 整数は整数として書く。 `1` と `1.0` でバイトが変わる。
                //
                // ほかは読み戻して同じ値になる最短の桁で書く。 桁を固定すると
                // `0.42` が `4.20000000000000018e-1` になり、統計値が倍の大きさになる。
                // 桁の多い端だけ指数で書く——`1e-20` を 0 を並べて書かない。
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    out.push_str(&format!("{}", *n as i64));
                } else if (1e-6..1e15).contains(&n.abs()) {
                    out.push_str(&format!("{n}"));
                } else {
                    out.push_str(&format!("{n:e}"));
                }
            }
            Value::String(s) => escape(s, out),
            Value::Array(v) => {
                out.push('[');
                for (i, x) in v.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    x.write_into(out);
                }
                out.push(']');
            }
            Value::Object(m) => {
                out.push('{');
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    escape(k, out);
                    out.push(':');
                    v.write_into(out);
                }
                out.push('}');
            }
        }
    }
}

/// 文字列を書き出す。
///
/// 非 ASCII をそのまま出す。 日本語を `\uXXXX` にすると、人が読めなくなる。
fn escape(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// 読めない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// 何が起きたか。
    pub detail: String,
    /// 何文字目か。
    pub at: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} 文字目: {}", self.at, self.detail)
    }
}

impl std::error::Error for ParseError {}

/// 読む。
pub fn parse(s: &str) -> Result<Value, ParseError> {
    let chars: Vec<char> = s.chars().collect();
    let mut p = Parser { chars, at: 0 };
    p.skip_ws();
    let v = p.value()?;
    p.skip_ws();
    if p.at < p.chars.len() {
        return Err(p.err("末尾に余りがある"));
    }
    Ok(v)
}

struct Parser {
    chars: Vec<char>,
    at: usize,
}

impl Parser {
    fn err(&self, detail: &str) -> ParseError {
        ParseError {
            detail: detail.to_owned(),
            at: self.at,
        }
    }

    fn skip_ws(&mut self) {
        while self
            .chars
            .get(self.at)
            .is_some_and(char::is_ascii_whitespace)
        {
            self.at += 1;
        }
    }

    fn eat(&mut self, c: char) -> Result<(), ParseError> {
        if self.chars.get(self.at) == Some(&c) {
            self.at += 1;
            Ok(())
        } else {
            Err(self.err(&format!("{c} が要る")))
        }
    }

    fn value(&mut self) -> Result<Value, ParseError> {
        match self.chars.get(self.at) {
            None => Err(self.err("値が無い")),
            Some('n') => self.literal("null", Value::Null),
            Some('t') => self.literal("true", Value::Bool(true)),
            Some('f') => self.literal("false", Value::Bool(false)),
            Some('"') => Ok(Value::String(self.string()?)),
            Some('[') => self.array(),
            Some('{') => self.object(),
            Some(_) => self.number(),
        }
    }

    fn literal(&mut self, word: &str, v: Value) -> Result<Value, ParseError> {
        let n = word.chars().count();
        if self.chars[self.at..].starts_with(&word.chars().collect::<Vec<_>>()[..]) {
            self.at += n;
            Ok(v)
        } else {
            Err(self.err(&format!("{word} が要る")))
        }
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.eat('"')?;
        let mut out = String::new();
        loop {
            let Some(&c) = self.chars.get(self.at) else {
                return Err(self.err("文字列が閉じていない"));
            };
            self.at += 1;
            match c {
                '"' => return Ok(out),
                '\\' => {
                    let Some(&e) = self.chars.get(self.at) else {
                        return Err(self.err("逃がしが閉じていない"));
                    };
                    self.at += 1;
                    match e {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'b' => out.push('\u{0008}'),
                        'f' => out.push('\u{000C}'),
                        'u' => {
                            let hex: String = self.chars[self.at..].iter().take(4).collect();
                            if hex.chars().count() < 4 {
                                return Err(self.err("\\u の桁が足りない"));
                            }
                            self.at += 4;
                            let code = u32::from_str_radix(&hex, 16)
                                .map_err(|_| self.err("\\u が 16 進でない"))?;
                            out.push(
                                char::from_u32(code).ok_or_else(|| self.err("符号位置でない"))?,
                            );
                        }
                        _ => return Err(self.err("知らない逃がし")),
                    }
                }
                c => out.push(c),
            }
        }
    }

    fn array(&mut self) -> Result<Value, ParseError> {
        self.eat('[')?;
        let mut out = Vec::new();
        self.skip_ws();
        if self.chars.get(self.at) == Some(&']') {
            self.at += 1;
            return Ok(Value::Array(out));
        }
        loop {
            self.skip_ws();
            out.push(self.value()?);
            self.skip_ws();
            match self.chars.get(self.at) {
                Some(',') => self.at += 1,
                Some(']') => {
                    self.at += 1;
                    return Ok(Value::Array(out));
                }
                _ => return Err(self.err("配列が閉じていない")),
            }
        }
    }

    fn object(&mut self) -> Result<Value, ParseError> {
        self.eat('{')?;
        let mut out = BTreeMap::new();
        self.skip_ws();
        if self.chars.get(self.at) == Some(&'}') {
            self.at += 1;
            return Ok(Value::Object(out));
        }
        loop {
            self.skip_ws();
            let at = self.at;
            let k = self.string()?;
            self.skip_ws();
            self.eat(':')?;
            self.skip_ws();
            let v = self.value()?;
            // 同じ鍵を 2 度許さない。 どちらを拾うかは読み手によって違い、
            // `version` が 2 つある manifest は読み手ごとに別のカセットになる。
            if out.insert(k.clone(), v).is_some() {
                return Err(ParseError {
                    detail: format!("同じ鍵が 2 度現れる: {k}"),
                    at,
                });
            }
            self.skip_ws();
            match self.chars.get(self.at) {
                Some(',') => self.at += 1,
                Some('}') => {
                    self.at += 1;
                    return Ok(Value::Object(out));
                }
                _ => return Err(self.err("対象が閉じていない")),
            }
        }
    }

    fn number(&mut self) -> Result<Value, ParseError> {
        let start = self.at;
        while self
            .chars
            .get(self.at)
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E'))
        {
            self.at += 1;
        }
        let raw: String = self.chars[start..self.at].iter().collect();
        raw.parse::<f64>()
            .map(Value::Number)
            .map_err(|_| self.err("数として読めない"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 鍵の昇順で書き出す() {
        // 作り直しても同じバイトが出るために要る。
        let v = Value::obj([("z".into(), Value::s("後")), ("a".into(), Value::s("先"))]);
        assert_eq!(v.write(), r#"{"a":"先","z":"後"}"#);
    }

    #[test]
    fn 日本語をそのまま出す() {
        // `\uXXXX` にすると人が読めなくなる。
        assert_eq!(Value::s("日本語").write(), "\"日本語\"");
    }

    #[test]
    fn 整数は整数として書く() {
        // `1` と `1.0` でバイトが変わる。
        assert_eq!(Value::Number(1.0).write(), "1");
        assert_eq!(Value::Number(-3.0).write(), "-3");
    }

    #[test]
    fn 小数は桁を落とさない() {
        for x in [
            0.1,
            0.42,
            1.0 / 3.0,
            -2.5e-9,
            1.234_567_890_123_456_7e-300,
            6.02e23,
        ] {
            let back = parse(&Value::Number(x).write()).unwrap();
            assert_eq!(back.as_f64(), Some(x), "{x}");
        }
    }

    #[test]
    fn 小数は最短の桁で書く() {
        assert_eq!(Value::Number(0.42).write(), "0.42");
        assert_eq!(Value::Number(1e-20).write(), "1e-20");
    }

    #[test]
    fn 読んで書いて読むと同じになる() {
        let src = r#"{"あ":[1,2.5,true,null,"文\n字"],"い":{"う":-1}}"#;
        let a = parse(src).unwrap();
        let b = parse(&a.write()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn 書き出しは決定的である() {
        let src = r#"{"z":1,"a":2}"#;
        let a = parse(src).unwrap();
        assert_eq!(a.write(), parse(&a.write()).unwrap().write());
    }

    #[test]
    fn 逃がしを読む() {
        let v = parse(r#""a\nb\tc\\d\"eあ""#).unwrap();
        assert_eq!(v.as_str(), Some("a\nb\tc\\d\"eあ"));
    }

    #[test]
    fn 空の配列と対象を読む() {
        assert_eq!(parse("[]").unwrap(), Value::Array(vec![]));
        assert_eq!(parse("{}").unwrap(), Value::Object(BTreeMap::new()));
    }

    #[test]
    fn 空白を飛ばす() {
        let v = parse("  { \"a\" : [ 1 , 2 ] }  ").unwrap();
        assert_eq!(
            v.get("a").and_then(Value::as_array).map(<[Value]>::len),
            Some(2)
        );
    }

    #[test]
    fn 壊れたものは読めない() {
        for bad in ["{", "[1,", r#"{"a"}"#, r#""閉じない"#, "{}extra", "nul"] {
            assert!(parse(bad).is_err(), "{bad} が読めてしまった");
        }
    }

    #[test]
    fn 同じ鍵が_2_度現れたら読まない() {
        // どちらを拾うかは読み手によって違う。
        let e = parse(r#"{"version":5,"a":{"b":1},"version":4}"#).unwrap_err();
        assert!(e.to_string().contains("version"), "{e}");
        assert!(
            parse(r#"{"a":{"b":1,"b":2}}"#).is_err(),
            "入れ子の中でも断る"
        );
    }

    #[test]
    fn 読めない理由に位置が出る() {
        let e = parse("[1,]").unwrap_err();
        assert!(e.to_string().contains("文字目"), "{e}");
    }
}
