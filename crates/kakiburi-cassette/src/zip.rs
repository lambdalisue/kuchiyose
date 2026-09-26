//! zip の読み書き。
//!
//! 索引があることが、tar ではなく zip を選んだ理由である。 中の 1 つだけを
//! 読めるので、全部を展開しない。
//!
//! entry は deflate で縮めて書く（[容器は zip](../../../docs/design/100-cassette.md#容器は-zip)）。
//! 同梱の基準は実行ファイルに埋め込むので、無圧縮では実行ファイルがその分だけ膨らむ。
//! 縮めても索引は残るので、中の 1 つだけを読める。読むときは無圧縮の entry も受ける。
//!
//! 容器の圧縮器は指紋に入れない。 指紋が守るのは測った値で、中身のハッシュは
//! 伸ばしたあとの本文から取る——縮め方を変えても値は変わらない。

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

/// 中身。名前の昇順で持つので、書き出しが決定的になる。
pub type Entries = BTreeMap<String, Vec<u8>>;

/// 旗の 11 番目。entry の名前が UTF-8 であると名乗る。
///
/// 立てなければ、読む側は CP437 として解釈してよい。 zip の規格がそう決めて
/// いる。名前が ASCII だけのあいだは差が出ないが、
/// [場面が階層の名前になった](../../../docs/design/100-cassette.md#中身)ので、
/// ふつうの `unzip` で中身の名前が化ける。
///
/// 自分で読み書きするぶんには困らないが、1 ファイルで持ち運べることが容器を
/// zip にした理由である以上、外の道具で開けないのは選んだ理由を損なう。
const UTF8_NAME: u16 = 0x0800;

/// 書き出す。
///
/// 時刻を入れない。 入れると、同じ中身から違うバイトが出て
/// [作り直しても同じものが出る](../../../docs/design/300-test.md#作り直せることを試験する)が
/// 成り立たない。
#[must_use]
pub fn write(entries: &Entries) -> Vec<u8> {
    let mut out = Vec::new();
    let mut index: Vec<Indexed> = Vec::new();
    for (name, body) in entries {
        let offset = u32::try_from(out.len()).unwrap_or(u32::MAX);
        let crc = crc32(body);
        let (method, stored) = encode(body);
        let packed = u32::try_from(stored.len()).unwrap_or(u32::MAX);
        let size = u32::try_from(body.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes()); // 局所札
        out.extend_from_slice(&20u16.to_le_bytes()); // 要る版
        out.extend_from_slice(&UTF8_NAME.to_le_bytes()); // 旗
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // 時刻。0 で固定する
        out.extend_from_slice(&0u16.to_le_bytes()); // 日付。同じ
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&packed.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        let nlen = u16::try_from(name.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // 追加欄なし
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&stored);
        index.push(Indexed {
            name,
            offset,
            crc,
            method,
            packed,
            size,
        });
    }

    let dir_start = u32::try_from(out.len()).unwrap_or(u32::MAX);
    for e in &index {
        out.extend_from_slice(&0x0201_4b50u32.to_le_bytes()); // 索引の札
        out.extend_from_slice(&20u16.to_le_bytes()); // 作った版
        out.extend_from_slice(&20u16.to_le_bytes()); // 要る版
        out.extend_from_slice(&UTF8_NAME.to_le_bytes()); // 旗
        out.extend_from_slice(&e.method.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&e.crc.to_le_bytes());
        out.extend_from_slice(&e.packed.to_le_bytes());
        out.extend_from_slice(&e.size.to_le_bytes());
        let nlen = u16::try_from(e.name.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&e.offset.to_le_bytes());
        out.extend_from_slice(e.name.as_bytes());
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

/// 索引だけを読む。中身を読まない。
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
        // 同じ名前を 2 度許さない。 zip はそれを持てるので、`manifest.json` が
        // 2 つあるカセットを作れてしまう——どちらを読んだかで別のカセットになる。
        if names.contains(&name) {
            return Err(ZipError::Broken {
                detail: format!("entry の名前が重複している: {name}"),
            });
        }
        names.push(name);
        at += 46 + nlen + elen + clen;
    }
    Ok(names)
}

/// 名前で 1 つだけ読む。
///
/// # Errors
///
/// 索引に無いか、zip として読めないか、名乗った大きさが [`ENTRY_LIMIT`] を超えれば断る。
pub fn read_one(bytes: &[u8], name: &str) -> Result<Vec<u8>, ZipError> {
    read_one_within(bytes, name, ENTRY_LIMIT)
}

fn read_one_within(bytes: &[u8], name: &str, limit: usize) -> Result<Vec<u8>, ZipError> {
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
            return read_at(bytes, offset, limit);
        }
        at += 46 + nlen + elen + clen;
    }
    Err(ZipError::NotFound {
        name: name.to_owned(),
    })
}

/// 1 つの entry が伸ばしてよい大きさの上限。256 MiB。
///
/// カセットは配り直すものなので、局所札の名乗る大きさは信じない。 名乗りは
/// 4 GiB まで書けるので、名乗りのまま領域を取れば、数十バイトのカセットが
/// 開くだけで落ちる。 同梱の基準は全部を伸ばしても 17 MB ほどなので、
/// 1 つの entry がこの大きさに届く正しいカセットは無い。
pub const ENTRY_LIMIT: usize = 256 * 1024 * 1024;

/// 1 つのカセットの全部を伸ばした大きさの上限。1 GiB。
///
/// 1 つずつを上限で抑えても、entry を並べれば同じだけ膨らむ。
pub const TOTAL_LIMIT: usize = 1024 * 1024 * 1024;

// 上限で正しいカセットを断らない。 同梱の基準は伸ばして 17 MB ほどなので、
// 桁が 1 つ違うほどの余裕を取る。
const _: () = assert!(ENTRY_LIMIT >= 17 * 1024 * 1024 * 8);
const _: () = assert!(TOTAL_LIMIT >= ENTRY_LIMIT);

/// 伸ばしてよい大きさ。
#[derive(Debug, Clone, Copy)]
struct Limits {
    entry: usize,
    total: usize,
}

/// 全部読む。
///
/// # Errors
///
/// zip として読めないか、名乗った大きさが [`ENTRY_LIMIT`] か [`TOTAL_LIMIT`] を
/// 超えれば断る。
pub fn read(bytes: &[u8]) -> Result<Entries, ZipError> {
    read_within(
        bytes,
        Limits {
            entry: ENTRY_LIMIT,
            total: TOTAL_LIMIT,
        },
    )
}

fn read_within(bytes: &[u8], limits: Limits) -> Result<Entries, ZipError> {
    let mut out = Entries::new();
    let mut used = 0usize;
    for name in index(bytes)? {
        // 残りを 1 つの上限にする。 伸ばしてから合計を数えれば、断る前に取り切っている。
        let body = read_one_within(bytes, &name, limits.entry.min(limits.total - used))?;
        used += body.len();
        out.insert(name, body);
    }
    Ok(out)
}

fn read_at(bytes: &[u8], offset: usize, limit: usize) -> Result<Vec<u8>, ZipError> {
    if read_u32(bytes, offset)? != 0x0403_4b50 {
        return Err(ZipError::Broken {
            detail: "局所札が無い".into(),
        });
    }
    let method = read_u16(bytes, offset + 8)?;
    let crc = read_u32(bytes, offset + 14)?;
    let packed = read_u32(bytes, offset + 18)? as usize;
    let size = read_u32(bytes, offset + 22)? as usize;
    // 伸ばす前に断る。 伸ばしてから数えれば、断る前に領域を取り切っている。
    // 無圧縮は縮めた分をそのまま写すので、縮めた大きさも上限と比べる。
    let claimed = if method == STORED {
        size.max(packed)
    } else {
        size
    };
    if claimed > limit {
        return Err(ZipError::Broken {
            detail: format!("名乗った大きさが上限を超える（{claimed} / {limit} バイト）"),
        });
    }
    if method == STORED && packed != size {
        return Err(ZipError::Broken {
            detail: format!(
                "無圧縮なのに縮めた大きさと伸ばした大きさが合わない（{packed} / {size} バイト）"
            ),
        });
    }
    let nlen = read_u16(bytes, offset + 26)? as usize;
    let elen = read_u16(bytes, offset + 28)? as usize;
    let raw = slice(bytes, offset + 30 + nlen + elen, packed)?;
    let body = match method {
        STORED => raw.to_vec(),
        DEFLATE => inflate(raw, size)?,
        other => {
            return Err(ZipError::Broken {
                detail: format!("知らない圧縮の方式 {other}。無圧縮と deflate だけを読む"),
            })
        }
    };
    if body.len() != size {
        return Err(ZipError::Broken {
            detail: format!(
                "伸ばした大きさが名乗りと合わない（{} / {size} バイト）",
                body.len()
            ),
        });
    }
    if crc32(&body) != crc {
        return Err(ZipError::Broken {
            detail: "検査値が合わない".into(),
        });
    }
    Ok(body)
}

/// 索引に書く 1 つぶん。
struct Indexed<'a> {
    name: &'a str,
    offset: u32,
    crc: u32,
    method: u16,
    packed: u32,
    size: u32,
}

/// 無圧縮の方式。
const STORED: u16 = 0;
/// deflate の方式。
const DEFLATE: u16 = 8;
/// deflate の水準。
///
/// 固定する。 変えれば同じ中身から違うバイトが出て、同梱の基準が作り直したものと
/// 一致しなくなる。
const DEFLATE_LEVEL: u32 = 9;

/// 縮める。縮まなければ無圧縮のまま返す——deflate は短いものを伸ばす。
fn encode(body: &[u8]) -> (u16, Vec<u8>) {
    use std::io::Write;
    let mut enc =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::new(DEFLATE_LEVEL));
    let packed = enc
        .write_all(body)
        .and_then(|()| enc.finish())
        .unwrap_or_default();
    if packed.is_empty() || packed.len() >= body.len() {
        (STORED, body.to_vec())
    } else {
        (DEFLATE, packed)
    }
}

/// 伸ばす。名乗った大きさを 1 バイトでも超えたらそこで止める。
///
/// 名乗りを信じて伸ばし切ると、小さな zip が際限なく膨らむ。 名乗りで領域を
/// 先取りもしない——上限の内でも、中身の無い名乗りだけで領域を取らせない。
fn inflate(raw: &[u8], size: usize) -> Result<Vec<u8>, ZipError> {
    use std::io::Read;
    let mut out = Vec::new();
    flate2::read::DeflateDecoder::new(raw)
        .take(u64::try_from(size).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut out)
        .map_err(|e| ZipError::Broken {
            detail: format!("伸ばせない: {e}"),
        })?;
    Ok(out)
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
        // 立てなければ、読む側は CP437 として解釈してよい。 場面が階層の
        // 名前になったので、ふつうの `unzip` で名前が化ける。
        let mut e = Entries::new();
        e.insert("decided/技術記事/baseline.json".into(), b"{}".to_vec());
        let bytes = write(&e);
        // 局所札の旗は 6 バイト目から。
        assert_eq!(u16::from_le_bytes([bytes[6], bytes[7]]), UTF8_NAME);
        // 索引の旗も同じ。片方だけでは読む側が選ぶ。
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
        // 中身を 1 バイト壊す。 位置を割合で決めると、中身が短くなった
        // ときに検査値の掛かっていない欄を壊すだけになり、試験が黙って
        // 何も確かめなくなる——実際に書いた中身を探して、そこを壊す。
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
    fn 同じ名前の_entry_を_2_つ持つものは断る() {
        // 書き出しは名前の対応表から作るので、重複した zip は外で作られたものである。
        // 長さの同じ別の名前で書いてから、名前のバイトだけを書き換えて作る。
        let mut e = Entries::new();
        e.insert("manifest.json".into(), b"{\"version\":5}".to_vec());
        e.insert("manifest.jsoN".into(), b"{\"version\":4}".to_vec());
        let mut bytes = write(&e);
        let from = b"manifest.jsoN";
        let mut at = 0;
        while let Some(i) = bytes[at..].windows(from.len()).position(|w| w == from) {
            bytes[at + i + from.len() - 1] = b'n';
            at += i + from.len();
        }
        let r = index(&bytes);
        assert!(
            matches!(&r, Err(ZipError::Broken { detail }) if detail.contains("重複")),
            "{r:?}"
        );
        assert!(read(&bytes).is_err());
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

    /// 縮む中身。同じ並びを繰り返す。
    fn repetitive() -> Vec<u8> {
        "の。と思う、".repeat(2000).into_bytes()
    }

    /// 局所札の方式の欄。先頭の entry のものである。
    fn first_method(bytes: &[u8]) -> u16 {
        u16::from_le_bytes([bytes[8], bytes[9]])
    }

    #[test]
    fn 縮む中身は_deflate_で縮めて書く() {
        // 同梱の基準は実行ファイルに埋め込む。 無圧縮では 17 MB を超えた。
        let mut e = Entries::new();
        e.insert("stats/documents.jsonl".into(), repetitive());
        let bytes = write(&e);
        assert_eq!(first_method(&bytes), DEFLATE);
        assert!(bytes.len() < repetitive().len() / 10, "{}", bytes.len());
        assert_eq!(read(&bytes).unwrap(), e);
        assert_eq!(
            read_one(&bytes, "stats/documents.jsonl").unwrap(),
            repetitive(),
            "1 つだけでも読める"
        );
    }

    #[test]
    fn 縮まない中身は無圧縮のまま書いて読める() {
        // deflate は短いものを伸ばす。 伸びるなら縮めない。
        let mut e = Entries::new();
        e.insert("a".into(), b"{}".to_vec());
        let bytes = write(&e);
        assert_eq!(first_method(&bytes), STORED);
        assert_eq!(read(&bytes).unwrap(), e);
    }

    #[test]
    fn 縮めても決定的である() {
        let mut e = Entries::new();
        e.insert("x".into(), repetitive());
        assert_eq!(write(&e), write(&e));
    }

    #[test]
    fn 縮めた中身が壊れていれば断る() {
        let mut e = Entries::new();
        e.insert("x".into(), repetitive());
        let mut bytes = write(&e);
        // 局所札の後ろ（名前 1 バイトの次）から中身が始まる。
        bytes[30 + 1 + 4] ^= 0xFF;
        assert!(read(&bytes).is_err());
    }

    #[test]
    fn 伸ばした大きさが名乗りと違えば断る() {
        // 名乗りを信じて伸ばすと、小さな zip が際限なく膨らむ。
        let mut e = Entries::new();
        e.insert("x".into(), repetitive());
        let mut bytes = write(&e);
        let declared = u32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]);
        bytes[22..26].copy_from_slice(&(declared - 1).to_le_bytes());
        let r = read(&bytes);
        assert!(
            matches!(&r, Err(ZipError::Broken { detail }) if detail.contains("大きさ")),
            "{r:?}"
        );
    }

    #[test]
    fn 名乗った大きさが_1_つの上限を超える_entry_は伸ばす前に断る() {
        // カセットは配り直すものなので、名乗りは信じない。 名乗りのまま
        // 領域を取れば、数十バイトの zip が 4 GiB を取りに行く。
        let mut e = Entries::new();
        e.insert("x".into(), repetitive());
        let mut bytes = write(&e);
        bytes[22..26].copy_from_slice(&u32::MAX.to_le_bytes());
        let r = read_one(&bytes, "x");
        assert!(
            matches!(&r, Err(ZipError::Broken { detail }) if detail.contains("上限")),
            "{r:?}"
        );
        assert!(read(&bytes).is_err());
    }

    #[test]
    fn 名乗った大きさが上限を超えれば無圧縮の_entry_も断る() {
        let mut e = Entries::new();
        e.insert("a".into(), b"{}".to_vec());
        let mut bytes = write(&e);
        assert_eq!(first_method(&bytes), STORED);
        bytes[22..26].copy_from_slice(&u32::MAX.to_le_bytes());
        let r = read_one(&bytes, "a");
        assert!(
            matches!(&r, Err(ZipError::Broken { detail }) if detail.contains("上限")),
            "{r:?}"
        );
    }

    #[test]
    fn 無圧縮の_entry_は_2_つの大きさが食い違えば写す前に断る() {
        // 伸ばした大きさだけを上限と比べれば、小さく名乗って大きな中身を写させられる。
        let mut e = Entries::new();
        e.insert("a".into(), b"123456".to_vec());
        let mut bytes = write(&e);
        assert_eq!(first_method(&bytes), STORED);
        bytes[22..26].copy_from_slice(&2u32.to_le_bytes());
        let r = read_one(&bytes, "a");
        assert!(
            matches!(&r, Err(ZipError::Broken { detail }) if detail.contains("無圧縮")),
            "{r:?}"
        );
    }

    #[test]
    fn 無圧縮の_entry_は縮めた大きさにも上限を掛ける() {
        let mut e = Entries::new();
        e.insert("a".into(), b"123456".to_vec());
        let mut bytes = write(&e);
        assert_eq!(first_method(&bytes), STORED);
        bytes[22..26].copy_from_slice(&2u32.to_le_bytes());
        let r = read_within(
            &bytes,
            Limits {
                entry: 4,
                total: 100,
            },
        );
        assert!(
            matches!(&r, Err(ZipError::Broken { detail }) if detail.contains("上限")),
            "{r:?}"
        );
    }

    #[test]
    fn カセット全体の大きさにも上限がある() {
        // 1 つずつは上限の内でも、数を並べれば同じだけ膨らむ。
        let mut e = Entries::new();
        e.insert("a".into(), b"123456".to_vec());
        e.insert("b".into(), b"789012".to_vec());
        let bytes = write(&e);
        let r = read_within(
            &bytes,
            Limits {
                entry: 100,
                total: 11,
            },
        );
        assert!(
            matches!(&r, Err(ZipError::Broken { detail }) if detail.contains("上限")),
            "{r:?}"
        );
        let fits = Limits {
            entry: 100,
            total: 12,
        };
        assert_eq!(read_within(&bytes, fits).unwrap(), e, "ちょうどなら読む");
    }

    #[test]
    fn 知らない方式は断る() {
        let mut bytes = write(&entries());
        bytes[8..10].copy_from_slice(&12u16.to_le_bytes());
        let r = read(&bytes);
        assert!(
            matches!(&r, Err(ZipError::Broken { detail }) if detail.contains("方式")),
            "{r:?}"
        );
    }

    #[test]
    fn crc32_は既知の値と合う() {
        // 実装を取り違えていないことの確かめ。
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"a"), 0xE8B7_BE43);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }
}
