//! 形代を zip に落とし、読み戻す（[中身](../../../docs/design/100-katashiro.md#中身)）。
//!
//! ```text
//! manifest.json              版・世代・場面の名前・素材のフォルダ・指紋・中身のハッシュ
//! tuning.json                調整（作り直せない）
//! persona.md                 ペルソナ（作り直せない。持たなければ entry が無い）
//! stats/documents.jsonl      文書ごとの統計値
//! stats/lexicon.json         語のまとめ方
//! ```
//!
//! 読むときは全部を検める。 容器・版・欄・JSON の重複鍵・中身のハッシュ・指紋・場面。
//! 読める道はここの 1 本だけなので、どの口から読んでも同じ検査を通る
//! （[同じ検査を通す](../../../docs/design/200-command.md#形代を取る口は同じ検査を通す)）。

use std::collections::BTreeMap;

use crate::json::{self, Value};
use crate::tuning::Tuning;
use crate::zip::{self, Entries, ZipError};
use crate::{Inputs, Katashiro, Normalization, Stats, Tool};

/// いま書く版。
///
/// 版は、形か意味が非互換に変わったときに上げる。 版 6 は版 5 に `persona.md` と
/// `material` を足した形である。 版 5 の読み手が版 6 の形代を書き直せば、知らない
/// `persona.md` を黙って落とす——落ちたペルソナは戻らない。
pub const VERSION: u32 = 6;

/// 読める版。
///
/// 版 5 は版 6 から entry と欄を除いただけの形なので、無いものを無いとして読めば
/// 済む。版 4 以前を読む道も、移し替える道も持たない
/// （[古い形代は読み戻さない](../../../docs/design/100-katashiro.md#古い形代は読み戻さない)）。
pub const READABLE: [u32; 2] = [5, VERSION];

/// 版・世代・場面・指紋。
pub const MANIFEST: &str = "manifest.json";
/// 調整。
pub const TUNING: &str = "tuning.json";
/// ペルソナ。
pub const PERSONA: &str = "persona.md";
/// 文書ごとの統計値。
pub const DOCUMENTS: &str = "stats/documents.jsonl";
/// 語のまとめ方。
pub const LEXICON: &str = "stats/lexicon.json";

/// 読み書きできない理由。
///
/// 壊れている・知らない版・欠け・食い違いを別の理由として返す。 まとめると、
/// 壊れた形代と新しすぎる形代が同じ顔になる——前者は作り直しで、後者は
/// 道具の更新である。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// zip として読めない。entry の名前の重複もここである。
    Zip(ZipError),
    /// 知らない版である。
    UnknownVersion {
        /// 形代が名乗った版。
        found: u32,
        /// この道具が読める版。
        known: u32,
    },
    /// JSON として読めない。同じ鍵が 2 度現れたときもここである。
    Json {
        /// どのファイルか。
        file: String,
        /// 何が起きたか。
        detail: String,
    },
    /// 欄が無いか、形が違う。
    Missing {
        /// どのファイルか。
        file: String,
        /// どの欄か。
        field: String,
    },
    /// 名乗った中身のハッシュか指紋が、中身と合わない。
    Mismatch {
        /// どの欄か。
        field: String,
    },
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Zip(e) => write!(f, "{e}"),
            // 古いのか新しいのかで、やることが逆である。 まとめて「道具の更新」と
            // 言うと、古い形代を持っている人が更新を待ち続ける。
            StoreError::UnknownVersion { found, known } if found < known => write!(
                f,
                "古い版の形代である（版 {found}。いまは {known}）。素材のフォルダから katashiro build で作り直す"
            ),
            StoreError::UnknownVersion { found, known } => write!(
                f,
                "新しすぎる版の形代である（版 {found}。読めるのは {known} まで）。道具の更新が要る"
            ),
            StoreError::Json { file, detail } => {
                write!(f, "{file} が JSON として読めない: {detail}")
            }
            StoreError::Missing { file, field } => write!(f, "{file} に {field} が無い"),
            StoreError::Mismatch { field } => {
                write!(f, "manifest.json の {field} が中身と合わない。壊れている")
            }
        }
    }
}

impl std::error::Error for StoreError {}

impl From<ZipError> for StoreError {
    fn from(e: ZipError) -> Self {
        StoreError::Zip(e)
    }
}

/// 形代を zip のバイトにする。
#[must_use]
pub fn write(c: &Katashiro) -> Vec<u8> {
    let mut e = Entries::new();
    e.insert(MANIFEST.into(), manifest(c).write().into_bytes());
    e.insert(TUNING.into(), c.tuning.to_json().write().into_bytes());
    // 持たなければ entry を置かない。 空の entry は「持たない」と区別が付かない。
    if let Some(p) = &c.persona {
        e.insert(PERSONA.into(), p.as_bytes().to_vec());
    }
    for (name, body) in c.stats.entries() {
        e.insert(name.into(), body.as_bytes().to_vec());
    }
    zip::write(&e)
}

/// 形代の原本。作り直せないものだけである。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Originals {
    /// 世代。
    pub generation: u64,
    /// 場面の名前。
    pub scene: String,
    /// 調整。
    pub tuning: Tuning,
    /// ペルソナ。
    pub persona: Option<String>,
}

/// 原本だけを読む。統計値は読まない。
///
/// `katashiro build` が作り直す前に使う。 統計値は素材から作り直せるので、
/// `stats/` を丸ごと失った形代からも調整を引き継げなければならない
/// （[作り直せることを試験する](../../../docs/design/300-test.md#作り直せることを試験する)）。
///
/// # Errors
///
/// 容器が壊れている、知らない版である、原本の欄が欠けているときに断る。
pub fn read_originals(bytes: &[u8]) -> Result<Originals, StoreError> {
    // 索引は全部を検める（重複・数）。 伸ばすのは原本だけで、壊れた統計値に引きずられない。
    let e = zip::read_only(bytes, &[MANIFEST, TUNING, PERSONA])?;
    let (_, originals) = originals(&e)?;
    Ok(originals)
}

/// 版を確かめてから、原本の欄を読む。
fn originals(e: &Entries) -> Result<(Value, Originals), StoreError> {
    let manifest = read_json(e, MANIFEST)?;
    let version = integer(&manifest, "version")?;
    let version = u32::try_from(version).map_err(|_| StoreError::Json {
        file: MANIFEST.into(),
        detail: format!("version が読めない: {version}"),
    })?;
    // 知らない版を「たぶん読める」と読まない。 欠けた項目は空として通り、
    // 空と欠けの区別がそこで崩れる。そして崩れたことはエラーにならない。
    if !READABLE.contains(&version) {
        return Err(StoreError::UnknownVersion {
            found: version,
            known: VERSION,
        });
    }
    let generation = integer(&manifest, "generation")?;

    // 場面は名乗りが正本である。 空に丸めれば、どの場面の形代か分からないまま
    // 検めが通る。
    let scene = manifest
        .get("scene")
        .and_then(Value::as_str)
        .ok_or_else(|| missing(MANIFEST, "scene"))?
        .to_owned();
    if !crate::scene_name_ok(&scene) {
        return Err(missing(MANIFEST, "空でない scene"));
    }

    // 調整を既定で埋めない。 空で通せば、次に書いたときに作り直せない判断が
    // 空として確定する。
    let tuning = Tuning::from_json(&read_json(e, TUNING)?).map_err(|t| StoreError::Json {
        file: TUNING.into(),
        detail: t.0,
    })?;
    let persona = persona(e)?;
    Ok((
        manifest,
        Originals {
            generation,
            scene,
            tuning,
            persona,
        },
    ))
}

/// ペルソナを読む。形は見ず、UTF-8 として読めることだけを確かめる。
///
/// 空の entry は断る。 空のペルソナは持たないことと区別が付かないので、書く側は
/// entry を置かない。 置かれていれば、どこかで書き換えられている。
fn persona(e: &Entries) -> Result<Option<String>, StoreError> {
    if !e.contains_key(PERSONA) {
        return Ok(None);
    }
    let body = text(e, PERSONA)?;
    if body.trim().is_empty() {
        return Err(missing(PERSONA, "本文"));
    }
    Ok(Some(body))
}

/// 素材のフォルダ。版 5 は持たない。版 6 では欄が要り、`null` は「分からない」である。
fn material(manifest: &Value, version: u32) -> Result<Option<String>, StoreError> {
    if version < 6 {
        return Ok(None);
    }
    match manifest.get("material") {
        Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(StoreError::Json {
            file: MANIFEST.into(),
            detail: "material が文字列でも null でもない".into(),
        }),
        None => Err(missing(MANIFEST, "material")),
    }
}

/// zip のバイトから形代を読む。全部を検める。
///
/// # Errors
///
/// 容器が壊れている、知らない版である、欄が欠けている、中身と名乗りが合わない
/// ときに断る。
pub fn read(bytes: &[u8]) -> Result<Katashiro, StoreError> {
    // 版を見る前に容器を検める。 `manifest.json` が 2 つあれば、どちらの版を
    // 見たかで挙動が変わる——重複は索引を読む段で断っている。
    let e = zip::read(bytes)?;
    let (
        manifest,
        Originals {
            generation,
            scene,
            tuning,
            persona,
        },
    ) = originals(&e)?;
    let version = u32::try_from(integer(&manifest, "version")?).unwrap_or(VERSION);
    let material = material(&manifest, version)?;

    let stats = Stats {
        documents: text(&e, DOCUMENTS)?,
        lexicon: text(&e, LEXICON)?,
    };
    // 形は知らないが、JSON として通ることと鍵が重ならないことは確かめる。
    for (i, line) in stats.documents.lines().enumerate() {
        json::parse(line).map_err(|err| StoreError::Json {
            file: format!("{DOCUMENTS} の {} 行目", i + 1),
            detail: err.to_string(),
        })?;
    }
    json::parse(&stats.lexicon).map_err(|err| StoreError::Json {
        file: LEXICON.into(),
        detail: err.to_string(),
    })?;

    // 読んだ版を持つ。 書くときは今の版で書く（[`write`] は `c.version` を見ない）。
    let c = Katashiro {
        version,
        generation,
        scene,
        inputs: read_inputs(&manifest)?,
        tuning,
        stats,
        persona,
        material,
    };
    // 名乗りを中身と照らす。 合わなければ、どこかが書き換えられている。
    let fingerprint = c.fingerprint();
    if manifest.get("content_hash").and_then(Value::as_str)
        != Some(fingerprint.content_hash.as_str())
    {
        return Err(StoreError::Mismatch {
            field: "content_hash".into(),
        });
    }
    if manifest.get("fingerprint").and_then(Value::as_str) != Some(fingerprint.digest()) {
        return Err(StoreError::Mismatch {
            field: "fingerprint".into(),
        });
    }
    Ok(c)
}

/// 整数として厳密に読む。丸めて通さない。
///
/// `5.5` を版 5 として読めば、名乗っていない形を名乗った形として扱うことになる。
fn integer(v: &Value, key: &str) -> Result<u64, StoreError> {
    let raw = v
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| missing(MANIFEST, key))?;
    #[allow(clippy::cast_precision_loss)]
    let ok = raw.fract() == 0.0 && raw >= 0.0 && raw <= u64::MAX as f64;
    if !ok {
        return Err(StoreError::Json {
            file: MANIFEST.into(),
            detail: format!("{key} が整数でない: {raw}"),
        });
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(raw as u64)
}

fn manifest(c: &Katashiro) -> Value {
    let f = c.fingerprint();
    #[allow(clippy::cast_precision_loss)]
    let generation = c.generation as f64;
    Value::obj([
        // 読んだ版ではなく今の版で書く。 版 5 の形のまま版 6 を名乗らせない。
        ("version".into(), Value::Number(f64::from(VERSION))),
        ("generation".into(), Value::Number(generation)),
        ("scene".into(), Value::s(&c.scene)),
        (
            "material".into(),
            c.material.as_ref().map_or(Value::Null, Value::s),
        ),
        ("content_hash".into(), Value::s(&f.content_hash)),
        ("fingerprint".into(), Value::s(f.digest())),
        // 材料を平文で残す。 ハッシュだけでは、何が変わったかが分からない。
        ("fingerprint_inputs".into(), inputs_json(&c.inputs)),
    ])
}

fn strings(m: &BTreeMap<String, String>) -> Value {
    Value::obj(m.iter().map(|(k, v)| (k.clone(), Value::s(v))))
}

fn inputs_json(i: &Inputs) -> Value {
    let tool = |t: &Tool| {
        Value::obj([
            ("name".into(), Value::s(&t.name)),
            ("version".into(), Value::s(&t.version)),
            ("config".into(), strings(&t.config)),
        ])
    };
    let n = &i.normalization;
    Value::obj([
        ("metric_definitions".into(), Value::s(&i.metric_definitions)),
        ("unit_definitions".into(), Value::s(&i.unit_definitions)),
        ("morphology".into(), tool(&i.morphology)),
        ("dependency".into(), tool(&i.dependency)),
        ("compressor".into(), tool(&i.compressor)),
        ("external_tables".into(), strings(&i.external_tables)),
        (
            "normalization".into(),
            Value::obj([
                (
                    "sources".into(),
                    Value::Array(n.sources.iter().map(Value::s).collect()),
                ),
                ("implementation".into(), Value::s(&n.implementation)),
                ("version".into(), Value::s(&n.version)),
                ("mapping".into(), strings(&n.mapping)),
            ]),
        ),
        ("measurement".into(), strings(&i.measurement)),
    ])
}

/// 指紋の材料を読む。欠けていれば断る。
///
/// 空で読めば、照らしたときに「全部違う」と言うことになる——どれが本当に
/// 変わったのかを名指せない。
fn read_inputs(manifest: &Value) -> Result<Inputs, StoreError> {
    const FILE: &str = "manifest.json の fingerprint_inputs";
    let i = manifest
        .get("fingerprint_inputs")
        .ok_or_else(|| missing(MANIFEST, "fingerprint_inputs"))?;
    let text = |v: &Value, key: &str| -> Result<String, StoreError> {
        v.get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| missing(FILE, key))
    };
    let map = |v: &Value, key: &str| -> Result<BTreeMap<String, String>, StoreError> {
        let Some(Value::Object(m)) = v.get(key) else {
            return Err(missing(FILE, key));
        };
        m.iter()
            .map(|(k, x)| {
                x.as_str()
                    .map(|s| (k.clone(), s.to_owned()))
                    .ok_or_else(|| missing(FILE, &format!("{key}.{k} の文字列")))
            })
            .collect()
    };
    let tool = |key: &str| -> Result<Tool, StoreError> {
        let t = i.get(key).ok_or_else(|| missing(FILE, key))?;
        Ok(Tool {
            name: text(t, "name")?,
            version: text(t, "version")?,
            config: map(t, "config")?,
        })
    };
    let n = i
        .get("normalization")
        .ok_or_else(|| missing(FILE, "normalization"))?;
    Ok(Inputs {
        metric_definitions: text(i, "metric_definitions")?,
        unit_definitions: text(i, "unit_definitions")?,
        morphology: tool("morphology")?,
        dependency: tool("dependency")?,
        compressor: tool("compressor")?,
        external_tables: map(i, "external_tables")?,
        normalization: Normalization {
            sources: n
                .get("sources")
                .and_then(Value::as_array)
                .ok_or_else(|| missing(FILE, "normalization.sources"))?
                .iter()
                .map(|x| {
                    x.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| missing(FILE, "normalization.sources の文字列"))
                })
                .collect::<Result<_, _>>()?,
            implementation: text(n, "implementation")?,
            version: text(n, "version")?,
            mapping: map(n, "mapping")?,
        },
        measurement: map(i, "measurement")?,
    })
}

fn text(e: &Entries, name: &str) -> Result<String, StoreError> {
    let body = e.get(name).ok_or_else(|| {
        StoreError::Zip(ZipError::NotFound {
            name: name.to_owned(),
        })
    })?;
    String::from_utf8(body.clone()).map_err(|_| StoreError::Json {
        file: name.to_owned(),
        detail: "UTF-8 でない".into(),
    })
}

fn read_json(e: &Entries, name: &str) -> Result<Value, StoreError> {
    json::parse(&text(e, name)?).map_err(|err| StoreError::Json {
        file: name.to_owned(),
        detail: err.to_string(),
    })
}

fn missing(file: &str, field: &str) -> StoreError {
    StoreError::Missing {
        file: file.to_owned(),
        field: field.to_owned(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::tuning::{MuteKind, Register};

    pub(crate) fn inputs() -> Inputs {
        Inputs {
            metric_definitions: "定義 51 本".into(),
            unit_definitions: "版 1".into(),
            morphology: Tool {
                name: "Lindera".into(),
                version: "6.0".into(),
                config: [("辞書".to_owned(), "UniDic".to_owned())].into(),
            },
            dependency: Tool::unused(),
            compressor: Tool::unused(),
            external_tables: [("Unicode".to_owned(), "15.1".to_owned())].into(),
            normalization: Normalization {
                sources: vec!["markdown".into()],
                implementation: "kuchiyose-normalize".into(),
                version: "0.0.0".into(),
                mapping: [("markdown".to_owned(), "表 1".to_owned())].into(),
            },
            measurement: [("metrics::floor::TOKENS".to_owned(), "729".to_owned())].into(),
        }
    }

    pub(crate) fn katashiro() -> Katashiro {
        let mut tuning = Tuning::default();
        tuning
            .mute
            .get_mut(&MuteKind::Metric)
            .unwrap()
            .insert("強調".into());
        tuning.register = Some(Register::Polite);
        let mut c = Katashiro::new(
            "技術記事",
            inputs(),
            Stats {
                documents: "{\"unit\":\"a\",\"chars\":1200}\n{\"unit\":\"b\",\"chars\":900}\n"
                    .into(),
                lexicon: "{\"pairs\":[[\"かき\",\"ぶり\"]]}".into(),
            },
            tuning,
        );
        c.generation = 3;
        c
    }

    fn entries() -> Entries {
        zip::read(&write(&katashiro())).unwrap()
    }

    fn body(e: &Entries, name: &str) -> String {
        String::from_utf8(e[name].clone()).unwrap()
    }

    #[test]
    fn 書いて読むと同じものが出る() {
        let c = katashiro();
        assert_eq!(read(&write(&c)), Ok(c));
    }

    #[test]
    fn 統計値を失っても原本は読める() {
        // 統計値は素材から作り直せる。 失った形代から調整を引き継げなければ、
        // 作り直すたびに人が決めたことが消える。
        let mut e = entries();
        e.remove(DOCUMENTS);
        e.remove(LEXICON);
        let bytes = zip::write(&e);
        assert!(read(&bytes).is_err(), "形代としては壊れている");
        let o = read_originals(&bytes).expect("原本は読める");
        assert_eq!(o.tuning, katashiro().tuning);
        assert_eq!(o.scene, "技術記事");
        assert_eq!(o.generation, 3);
    }

    #[test]
    fn 統計値の_entry_が壊れていても原本は読める() {
        // 統計値を伸ばせば検査値で断られ、作り直せる統計値のために調整とペルソナを失う。
        let c = with_persona();
        let mut bytes = write(&c);
        let lexicon = c.stats.lexicon.as_bytes();
        let at = bytes
            .windows(lexicon.len())
            .position(|w| w == lexicon)
            .expect("無圧縮で書いた語のまとめ方が在る");
        bytes[at] ^= 0xFF;
        assert!(read(&bytes).is_err(), "形代としては壊れている");
        let o = read_originals(&bytes).expect("原本は読める");
        assert_eq!(o.tuning, c.tuning);
        assert_eq!(o.persona, c.persona);
    }

    #[test]
    fn 原本を読むときも版と調整を検める() {
        let mut e = entries();
        e.remove(TUNING);
        assert!(read_originals(&zip::write(&e)).is_err());
        assert!(read_originals(b"PK").is_err());
    }

    #[test]
    fn 書き出しは決定的である() {
        // 同じフォルダを 2 度 build して同じバイト列が出る。
        assert_eq!(write(&katashiro()), write(&katashiro()));
    }

    #[test]
    fn ペルソナを持たなければ_entry_を置かない() {
        let names = zip::index(&write(&katashiro())).unwrap();
        assert_eq!(names, vec![MANIFEST, DOCUMENTS, LEXICON, TUNING]);
    }

    fn with_persona() -> Katashiro {
        let mut c = katashiro();
        c.persona = Some("## 大事にすること\n\n- 手を動かして確かめる\n".into());
        c.material = Some("/home/alisue/articles".into());
        c
    }

    #[test]
    fn ペルソナと素材のフォルダは書いて読むと同じものが出る() {
        let c = with_persona();
        let names = zip::index(&write(&c)).unwrap();
        assert_eq!(names, vec![MANIFEST, PERSONA, DOCUMENTS, LEXICON, TUNING]);
        assert_eq!(read(&write(&c)), Ok(c));
    }

    #[test]
    fn ペルソナと素材のフォルダは中身のハッシュにも指紋にも入らない() {
        // 測った値を変えない。 入れれば、ペルソナを直しただけで比べられなくなる。
        let a = katashiro();
        let b = with_persona();
        assert_eq!(a.fingerprint(), b.fingerprint());
        assert_eq!(a.stats.content_hash(), b.stats.content_hash());
    }

    #[test]
    fn 統計値を失ってもペルソナは原本として読める() {
        let mut e = zip::read(&write(&with_persona())).unwrap();
        e.remove(DOCUMENTS);
        e.remove(LEXICON);
        let o = read_originals(&zip::write(&e)).expect("原本は読める");
        assert_eq!(o.persona, with_persona().persona);
    }

    #[test]
    fn 空のペルソナの_entry_は壊れているとして断る() {
        // 空のペルソナは持たないことと区別が付かない。 書く側は entry を置かない。
        let mut e = entries();
        e.insert(PERSONA.into(), b" \n".to_vec());
        assert!(matches!(
            read(&zip::write(&e)),
            Err(StoreError::Missing { .. })
        ));
    }

    #[test]
    fn utf8_でないペルソナは断る() {
        let mut e = entries();
        e.insert(PERSONA.into(), vec![0xff, 0xfe]);
        assert!(read(&zip::write(&e)).is_err());
    }

    #[test]
    fn 版_6_は素材のフォルダの欄を要る() {
        let mut e = entries();
        let m = body(&e, MANIFEST).replace("\"material\":null,", "");
        e.insert(MANIFEST.into(), m.into_bytes());
        assert_eq!(
            read(&zip::write(&e)),
            Err(StoreError::Missing {
                file: MANIFEST.into(),
                field: "material".into()
            })
        );
    }

    /// 版 5 の形。版 6 から `persona.md` と `material` を除いたもの。
    fn version_five() -> Vec<u8> {
        let mut e = entries();
        let m = body(&e, MANIFEST)
            .replace("\"material\":null,", "")
            .replace(&format!("\"version\":{VERSION}"), "\"version\":5");
        e.insert(MANIFEST.into(), m.into_bytes());
        zip::write(&e)
    }

    #[test]
    fn 版_5_は無いものを無いとして読む() {
        // 調整を打ち直さずに済む。
        let got = read(&version_five()).expect("読める");
        assert_eq!(got.version, 5);
        assert_eq!(got.tuning, katashiro().tuning);
        assert_eq!(got.persona, None);
        assert_eq!(got.material, None);
    }

    #[test]
    fn 版_5_を読んで書けば版_6_になる() {
        let got = read(&version_five()).expect("読める");
        let again = read(&write(&got)).expect("読める");
        assert_eq!(again.version, VERSION);
        assert!(body(&zip::read(&write(&got)).unwrap(), MANIFEST).contains("\"version\":6"));
    }

    #[test]
    fn 調整だけを変えても中身のハッシュと指紋は変わらない() {
        // 調整は測った値を変えない。 変われば、調整しただけで比べられなくなる。
        let a = katashiro();
        let mut b = katashiro();
        b.tuning.first_person = Some("僕".into());
        assert_eq!(a.fingerprint(), b.fingerprint());
        assert_ne!(write(&a), write(&b), "調整そのものは書かれる");
    }

    #[test]
    fn 知らない版は壊れているとは別の理由で断る() {
        // まとめると、壊れた形代と新しすぎる形代が同じ顔になる。
        let now = format!("\"version\":{VERSION}");
        for found in [4, VERSION + 1] {
            let mut e = entries();
            let m = body(&e, MANIFEST).replace(&now, &format!("\"version\":{found}"));
            e.insert(MANIFEST.into(), m.into_bytes());
            assert_eq!(
                read(&zip::write(&e)),
                Err(StoreError::UnknownVersion {
                    found,
                    known: VERSION
                })
            );
        }
    }

    #[test]
    fn 古い版は作り直しを案内し新しい版は更新を案内する() {
        let old = StoreError::UnknownVersion {
            found: 4,
            known: VERSION,
        };
        assert!(old.to_string().contains("作り直す"), "{old}");
        let new = StoreError::UnknownVersion {
            found: VERSION + 1,
            known: VERSION,
        };
        assert!(new.to_string().contains("更新"), "{new}");
    }

    #[test]
    fn 版が整数でなければ読まない() {
        let mut e = entries();
        let m = body(&e, MANIFEST).replace(
            &format!("\"version\":{VERSION}"),
            &format!("\"version\":{VERSION}.5"),
        );
        e.insert(MANIFEST.into(), m.into_bytes());
        assert!(matches!(
            read(&zip::write(&e)),
            Err(StoreError::Json { .. })
        ));
    }

    #[test]
    fn 必ずある_entry_が欠けていたら断る() {
        for name in [MANIFEST, TUNING, DOCUMENTS, LEXICON] {
            let mut e = entries();
            e.remove(name);
            let err = read(&zip::write(&e)).unwrap_err();
            assert!(
                matches!(err, StoreError::Zip(ZipError::NotFound { .. })),
                "{name}: {err:?}"
            );
        }
    }

    #[test]
    fn 調整の欄が欠けていたら空で埋めずに断る() {
        let mut e = entries();
        let t = body(&e, TUNING).replace("\"register\"", "\"消した\"");
        e.insert(TUNING.into(), t.into_bytes());
        assert!(matches!(
            read(&zip::write(&e)),
            Err(StoreError::Json { .. })
        ));
    }

    #[test]
    fn 文体の申告に知らない値があれば断る() {
        // 捨てれば、申告したはずの文体が数えた結果に戻る。
        let mut e = entries();
        let t = body(&e, TUNING).replace("\"polite\"", "\"desu\"");
        e.insert(TUNING.into(), t.into_bytes());
        assert!(read(&zip::write(&e)).is_err());
    }

    #[test]
    fn 場面を名乗らない形代は読めない() {
        for broken in ["\"scene\"", "\"scene\":\"技術記事\""] {
            let mut e = entries();
            let m = body(&e, MANIFEST);
            let m = if broken == "\"scene\"" {
                m.replace(broken, "\"ばめん\"")
            } else {
                m.replace(broken, "\"scene\":\"\"")
            };
            e.insert(MANIFEST.into(), m.into_bytes());
            assert!(read(&zip::write(&e)).is_err(), "{broken}");
        }
    }

    #[test]
    fn 場面の名前を変えても中身のハッシュと指紋は変わらない() {
        // 場面は名札である。 中身の同一性には入らない。
        let a = katashiro();
        let mut b = katashiro();
        b.scene = "日記".into();
        assert_eq!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn 中身のハッシュが統計値と合わなければ断る() {
        let mut e = entries();
        let d = body(&e, DOCUMENTS).replace("1200", "1201");
        e.insert(DOCUMENTS.into(), d.into_bytes());
        assert_eq!(
            read(&zip::write(&e)),
            Err(StoreError::Mismatch {
                field: "content_hash".into()
            })
        );
    }

    #[test]
    fn 指紋が材料と合わなければ断る() {
        let mut e = entries();
        let m = body(&e, MANIFEST).replace("\"729\"", "\"730\"");
        e.insert(MANIFEST.into(), m.into_bytes());
        assert_eq!(
            read(&zip::write(&e)),
            Err(StoreError::Mismatch {
                field: "fingerprint".into()
            })
        );
    }

    #[test]
    fn 指紋の材料が欠けていたら断る() {
        let mut e = entries();
        let m = body(&e, MANIFEST).replace("\"measurement\"", "\"消した\"");
        e.insert(MANIFEST.into(), m.into_bytes());
        assert!(matches!(
            read(&zip::write(&e)),
            Err(StoreError::Missing { .. })
        ));
    }

    #[test]
    fn 統計値の行に同じ鍵が_2_度あれば断る() {
        // どちらを拾うかは読み手によって違う。
        let mut e = entries();
        let d = body(&e, DOCUMENTS).replace("\"chars\":900", "\"chars\":900,\"chars\":901");
        e.insert(DOCUMENTS.into(), d.into_bytes());
        let err = read(&zip::write(&e)).unwrap_err();
        assert!(
            matches!(&err, StoreError::Json { file, .. } if file.contains("2 行目")),
            "{err:?}"
        );
    }

    #[test]
    fn manifest_に同じ鍵が_2_度あれば断る() {
        let mut e = entries();
        let m = body(&e, MANIFEST).replacen('{', "{\"version\":4,", 1);
        e.insert(MANIFEST.into(), m.into_bytes());
        assert!(matches!(
            read(&zip::write(&e)),
            Err(StoreError::Json { .. })
        ));
    }

    #[test]
    fn 壊れた_json_は読めない() {
        let mut e = entries();
        e.insert(MANIFEST.into(), "{壊れている".as_bytes().to_vec());
        assert!(matches!(
            read(&zip::write(&e)),
            Err(StoreError::Json { .. })
        ));
    }
}
