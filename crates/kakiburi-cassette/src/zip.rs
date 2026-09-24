//! zip の読み書き。
//!
//! <strong>索引があることが、tar ではなく zip を選んだ理由である。</strong> 中の 1 つだけを
//! 読めるので、派生物を差し替えるたびに全体を舐めない。
//!
//! <strong>無圧縮で持つ。</strong> 圧縮は[圧縮率](../../../docs/spec/metrics/圧縮率.md)の指標が
//! 使うものであって、容器の役目ではない——容器が圧縮すると、圧縮器と設定が
//! 指紋に 2 度出てくる。

use std::collections::BTreeMap;

/// 読み書きできない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZipError {
    /// zip として読めない。
    Broken {
        /// 何が起きたか。
        detail: String,
    },
    /// 索引に無い。
    NotFound {
        /// 探した名前。
        name: String,
    },
}

impl std::fmt::Display for ZipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZipError::Broken { detail } => write!(f, "zip として読めない: {detail}"),
            ZipError::NotFound { name } => write!(f, "索引に無い: {name}"),
        }
    }
}

impl std::error::Error for ZipError {}

/// 中身。<strong>名前の昇順で持つ</strong>ので、書き出しが決定的になる。
pub type Entries = BTreeMap<String, Vec<u8>>;

/// 旗の 11 番目。<strong>entry の名前が UTF-8 であると名乗る。</strong>
///
/// <strong>立てなければ、読む側は CP437 として解釈してよい。</strong> zip の規格がそう決めて
/// いる。名前が ASCII だけのあいだは差が出ないが、
/// [場面が階層の名前になった](../../../docs/design/100-cassette.md#中身)ので、
/// <strong>ふつうの `unzip` で中身の名前が化ける。</strong>
///
/// 自分で読み書きするぶんには困らないが、<strong>1 ファイルで持ち運べることが容器を
/// zip にした理由</strong>である以上、外の道具で開けないのは選んだ理由を損なう。
const UTF8_NAME: u16 = 0x0800;

/// 書き出す。
///
/// <strong>時刻を入れない。</strong> 入れると、同じ中身から違うバイトが出て
/// [作り直しても同じものが出る](../../../docs/design/300-test.md#作り直せることを試験する)が
/// 成り立たない。
#[must_use]
pub fn write(entries: &Entries) -> Vec<u8> {
    let mut out = Vec::new();
    let mut index: Vec<(String, u32, u32)> = Vec::new();
    for (name, body) in entries {
        let offset = u32::try_from(out.len()).unwrap_or(u32::MAX);
        let crc = crc32(body);
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes()); // 局所札
        out.extend_from_slice(&20u16.to_le_bytes()); // 要る版
        out.extend_from_slice(&UTF8_NAME.to_le_bytes()); // 旗
        out.extend_from_slice(&0u16.to_le_bytes()); // 無圧縮
        out.extend_from_slice(&0u16.to_le_bytes()); // 時刻。<strong>0 で固定する</strong>
        out.extend_from_slice(&0u16.to_le_bytes()); // 日付。同じ
        out.extend_from_slice(&crc.to_le_bytes());
        let size = u32::try_from(body.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        let nlen = u16::try_from(name.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // 追加欄なし
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(body);
        index.push((name.clone(), offset, crc));
    }

    let dir_start = u32::try_from(out.len()).unwrap_or(u32::MAX);
    for (name, offset, crc) in &index {
        let size = u32::try_from(entries[name].len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&0x0201_4b50u32.to_le_bytes()); // 索引の札
        out.extend_from_slice(&20u16.to_le_bytes()); // 作った版
        out.extend_from_slice(&20u16.to_le_bytes()); // 要る版
        out.extend_from_slice(&UTF8_NAME.to_le_bytes()); // 旗
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        let nlen = u16::try_from(name.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
    }
    let dir_size = u32::try_from(out.len()).unwrap_or(u32::MAX) - dir_start;

    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes()); // 終わりの札
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    let n = u16::try_from(index.len()).unwrap_or(u16::MAX);
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&dir_size.to_le_bytes());
    out.extend_from_slice(&dir_start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // 注記なし
    out
}

/// 索引だけを読む。<strong>中身を読まない。</strong>
///
/// これが zip を選んだ理由である——1 つだけ読むために全部を舐めない。
pub fn index(bytes: &[u8]) -> Result<Vec<String>, ZipError> {
    let end = find_end(bytes)?;
    let count = u16::from_le_bytes([bytes[end + 10], bytes[end + 11]]) as usize;
    let dir_start = read_u32(bytes, end + 16)? as usize;
    let mut at = dir_start;
    let mut names = Vec::with_capacity(count);
    for _ in 0..count {
        if read_u32(bytes, at)? != 0x0201_4b50 {
            return Err(ZipError::Broken {
                detail: "索引の札が無い".into(),
            });
        }
        let nlen = read_u16(bytes, at + 28)? as usize;
        let elen = read_u16(bytes, at + 30)? as usize;
        let clen = read_u16(bytes, at + 32)? as usize;
        let name = std::str::from_utf8(slice(bytes, at + 46, nlen)?)
            .map_err(|_| ZipError::Broken {
                detail: "名前が UTF-8 でない".into(),
            })?
            .to_owned();
        names.push(name);
        at += 46 + nlen + elen + clen;
    }
    Ok(names)
}

/// 名前で 1 つだけ読む。
pub fn read_one(bytes: &[u8], name: &str) -> Result<Vec<u8>, ZipError> {
    let end = find_end(bytes)?;
    let count = u16::from_le_bytes([bytes[end + 10], bytes[end + 11]]) as usize;
    let dir_start = read_u32(bytes, end + 16)? as usize;
    let mut at = dir_start;
    for _ in 0..count {
        let nlen = read_u16(bytes, at + 28)? as usize;
        let elen = read_u16(bytes, at + 30)? as usize;
        let clen = read_u16(bytes, at + 32)? as usize;
        let offset = read_u32(bytes, at + 42)? as usize;
        let this = std::str::from_utf8(slice(bytes, at + 46, nlen)?).unwrap_or("");
        if this == name {
            return read_at(bytes, offset);
        }
        at += 46 + nlen + elen + clen;
    }
    Err(ZipError::NotFound {
        name: name.to_owned(),
    })
}

/// 全部読む。
pub fn read(bytes: &[u8]) -> Result<Entries, ZipError> {
    let mut out = Entries::new();
    for name in index(bytes)? {
        let body = read_one(bytes, &name)?;
        out.insert(name, body);
    }
    Ok(out)
}

fn read_at(bytes: &[u8], offset: usize) -> Result<Vec<u8>, ZipError> {
    if read_u32(bytes, offset)? != 0x0403_4b50 {
        return Err(ZipError::Broken {
            detail: "局所札が無い".into(),
        });
    }
    let method = read_u16(bytes, offset + 8)?;
    if method != 0 {
        return Err(ZipError::Broken {
            detail: format!("無圧縮でない（方式 {method}）"),
        });
    }
    let crc = read_u32(bytes, offset + 14)?;
    let size = read_u32(bytes, offset + 18)? as usize;
    let nlen = read_u16(bytes, offset + 26)? as usize;
    let elen = read_u16(bytes, offset + 28)? as usize;
    let body = slice(bytes, offset + 30 + nlen + elen, size)?.to_vec();
    if crc32(&body) != crc {
        return Err(ZipError::Broken {
            detail: "検査値が合わない".into(),
        });
    }
    Ok(body)
}

fn find_end(bytes: &[u8]) -> Result<usize, ZipError> {
    if bytes.len() < 22 {
        return Err(ZipError::Broken {
            detail: "短すぎる".into(),
        });
    }
    // 終わりの札を後ろから探す。注記があれば後ろに付く。
    for i in (0..=bytes.len() - 22).rev() {
        if read_u32(bytes, i)? == 0x0605_4b50 {
            return Ok(i);
        }
    }
    Err(ZipError::Broken {
        detail: "終わりの札が無い".into(),
    })
}

fn read_u16(bytes: &[u8], at: usize) -> Result<u16, ZipError> {
    let s = slice(bytes, at, 2)?;
    Ok(u16::from_le_bytes([s[0], s[1]]))
}

fn read_u32(bytes: &[u8], at: usize) -> Result<u32, ZipError> {
    let s = slice(bytes, at, 4)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn slice(bytes: &[u8], at: usize, len: usize) -> Result<&[u8], ZipError> {
    bytes.get(at..at + len).ok_or(ZipError::Broken {
        detail: "途中で終わっている".into(),
    })
}

/// CRC-32。zip が要求する検査値。
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = if crc & 1 == 1 { 0xEDB8_8320 } else { 0 };
            crc = (crc >> 1) ^ mask;
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Entries {
        let mut e = Entries::new();
        e.insert("manifest.json".into(), b"{\"version\":1}".to_vec());
        e.insert(
            "decided/baseline.json".into(),
            "{\"model\":\"m\"}".as_bytes().to_vec(),
        );
        e.insert("derived/scale.json".into(), b"{}".to_vec());
        e
    }

    #[test]
    fn 名前が_utf8_であると名乗る() {
        // <strong>立てなければ、読む側は CP437 として解釈してよい。</strong> 場面が階層の
        // 名前になったので、ふつうの `unzip` で名前が化ける。
        let mut e = Entries::new();
        e.insert("decided/技術記事/baseline.json".into(), b"{}".to_vec());
        let bytes = write(&e);
        // 局所札の旗は 6 バイト目から。
        assert_eq!(u16::from_le_bytes([bytes[6], bytes[7]]), UTF8_NAME);
        // 索引の旗も同じ。<strong>片方だけでは読む側が選ぶ。</strong>
        let at = bytes
            .windows(4)
            .position(|w| w == 0x0201_4b50u32.to_le_bytes())
            .expect("索引がある");
        assert_eq!(
            u16::from_le_bytes([bytes[at + 8], bytes[at + 9]]),
            UTF8_NAME
        );
        assert_eq!(
            index(&bytes).unwrap(),
            vec!["decided/技術記事/baseline.json"]
        );
    }

    #[test]
    fn 書いて読むと同じものが出る() {
        let e = entries();
        let bytes = write(&e);
        assert_eq!(read(&bytes).unwrap(), e);
    }

    #[test]
    fn 時刻を入れないので決定的である() {
        // 入れると、同じ中身から違うバイトが出る。
        assert_eq!(write(&entries()), write(&entries()));
    }

    #[test]
    fn 索引だけを読める() {
        // これが tar ではなく zip を選んだ理由である。
        let bytes = write(&entries());
        let names = index(&bytes).unwrap();
        assert_eq!(
            names,
            vec![
                "decided/baseline.json".to_owned(),
                "derived/scale.json".to_owned(),
                "manifest.json".to_owned(),
            ],
            "名前の昇順"
        );
    }

    #[test]
    fn 中の_1_つだけを読める() {
        let bytes = write(&entries());
        let one = read_one(&bytes, "manifest.json").unwrap();
        assert_eq!(one, b"{\"version\":1}");
    }

    #[test]
    fn 索引に無いものは読めない() {
        let bytes = write(&entries());
        let e = read_one(&bytes, "derived/vocabulary.json").unwrap_err();
        assert!(matches!(e, ZipError::NotFound { .. }), "{e:?}");
    }

    #[test]
    fn 派生物を差し替えられる() {
        // 全体を書き直さずに済むことが要点だが、差し替えた結果が読めることを見る。
        let mut e = entries();
        e.insert("derived/scale.json".into(), b"{\"new\":true}".to_vec());
        let bytes = write(&e);
        assert_eq!(
            read_one(&bytes, "derived/scale.json").unwrap(),
            b"{\"new\":true}"
        );
        assert_eq!(
            read_one(&bytes, "decided/baseline.json").unwrap(),
            "{\"model\":\"m\"}".as_bytes(),
            "原本は変わらない"
        );
    }

    #[test]
    fn 検査値が合わなければ断る() {
        let mut bytes = write(&entries());
        // <strong>中身を 1 バイト壊す。</strong> 位置を割合で決めると、中身が短くなった
        // ときに検査値の掛かっていない欄を壊すだけになり、<strong>試験が黙って
        // 何も確かめなくなる</strong>——実際に書いた中身を探して、そこを壊す。
        let body = b"{\"model\":\"m\"}";
        let at = bytes
            .windows(body.len())
            .position(|w| w == body)
            .expect("書いた中身が在る");
        bytes[at] ^= 0xFF;
        let r = read(&bytes);
        assert!(r.is_err(), "壊れたものを読んでしまった");
    }

    #[test]
    fn 短すぎるものは断る() {
        assert!(read(b"PK").is_err());
        assert!(read(&[]).is_err());
    }

    #[test]
    fn 空の中身も往復する() {
        let e = Entries::new();
        assert_eq!(read(&write(&e)).unwrap(), e);
    }

    #[test]
    fn 日本語の名前も往復する() {
        let mut e = Entries::new();
        e.insert("corpus/person/日本語.json".into(), b"{}".to_vec());
        assert_eq!(read(&write(&e)).unwrap(), e);
    }

    #[test]
    fn crc32_は既知の値と合う() {
        // 実装を取り違えていないことの確かめ。
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"a"), 0xE8B7_BE43);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }
}
