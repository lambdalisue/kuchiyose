//! カセットを zip に落とし、読み戻す。
//!
//! <strong>2 つのディレクトリが、そのまま 2 つの層である。</strong>`derived/` を丸ごと消しても、
//! `decided/` と素材のフォルダがあれば同じものが作り直せる。

use std::collections::BTreeMap;

use crate::json::{self, Value};
use crate::zip::{self, Entries, ZipError};
use crate::{
    Baseline, Cassette, Common, Decided, Derived, Fingerprint, Inputs, Movement, Normalization,
    SceneInputs, Tool,
};

/// いま書く版。
///
/// <strong>版は、形か意味が非互換に変わったときに上げる。</strong>
/// 版 4 では[1 カセット 1 場面](../../../docs/design/100-cassette.md#1-カセット-1-場面)に
/// なって場面ごとの階層が消え、[本文を持たなくなった](../../../docs/spec/200-extract.md#素材を正本にする)
/// ——<strong>版 3 のカセットとは形が違う。</strong>
///
/// <strong>古い版を読む道は持たない。</strong> 原本は素材のフォルダなので作り直せる
/// ——移し替える道を持つと、作り直せないものが増える。
pub const VERSION: u32 = 4;

/// 読み書きできない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// zip として読めない。
    Zip(ZipError),
    /// 知らない版である。
    ///
    /// <strong>壊れているとは別の理由で返す。</strong> まとめると、壊れたカセットと新しすぎる
    /// カセットが同じ顔になる——前者は作り直しで、後者は道具の更新である。
    UnknownVersion {
        /// カセットが名乗った版。
        found: u32,
        /// この道具が読める版。
        known: u32,
    },
    /// JSON として読めない。
    Json {
        /// どのファイルか。
        file: String,
        /// 何が起きたか。
        detail: String,
    },
    /// 欄が無い。
    Missing {
        /// どのファイルか。
        file: String,
        /// どの欄か。
        field: String,
    },
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Zip(e) => write!(f, "{e}"),
            // <strong>古いのか新しいのかで、やることが逆である。</strong> まとめて「道具の更新」と
            // 言うと、古いカセットを持っている人が更新を待ち続ける。
            StoreError::UnknownVersion { found, known } if found < known => write!(
                f,
                "古い版のカセットである（版 {found}。いまは {known}）。素材から作り直す"
            ),
            StoreError::UnknownVersion { found, known } => write!(
                f,
                "新しすぎる版のカセットである（版 {found}。読めるのは {known} まで）。道具の更新が要る"
            ),
            StoreError::Json { file, detail } => {
                write!(f, "{file} が JSON として読めない: {detail}")
            }
            StoreError::Missing { file, field } => write!(f, "{file} に {field} が無い"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<ZipError> for StoreError {
    fn from(e: ZipError) -> Self {
        StoreError::Zip(e)
    }
}

/// カセットを zip のバイトにする。
#[must_use]
pub fn write(c: &Cassette) -> Vec<u8> {
    let mut e = Entries::new();
    e.insert("manifest.json".into(), manifest(c).write().into_bytes());
    e.insert(
        "decided/boilerplate.json".into(),
        Value::Array(c.decided.boilerplate.iter().map(Value::s).collect())
            .write()
            .into_bytes(),
    );
    e.insert(
        "decided/baseline.json".into(),
        baseline_json(&c.decided.baseline).write().into_bytes(),
    );
    e.insert(
        "decided/movement.json".into(),
        Value::obj(c.decided.movement.iter().map(|(k, v)| {
            (
                k.clone(),
                Value::s(match v {
                    Movement::Moves => "moves",
                    Movement::Stuck => "stuck",
                }),
            )
        }))
        .write()
        .into_bytes(),
    );
    for (name, body) in derived_files(&c.derived) {
        e.insert(format!("derived/{name}"), body.into_bytes());
    }
    zip::write(&e)
}

/// zip のバイトからカセットを読む。
pub fn read(bytes: &[u8]) -> Result<Cassette, StoreError> {
    let e = zip::read(bytes)?;
    let manifest = read_json(&e, "manifest.json")?;
    // <strong>版を確かめる。</strong> 知らない版を「たぶん読める」と読んではいけない——
    // 欠けた項目は空として通り、空と欠けの区別がそこで崩れる。
    // <strong>そして崩れたことはエラーにならない。</strong>
    let raw = manifest
        .get("version")
        .and_then(Value::as_f64)
        .ok_or_else(|| missing("manifest.json", "version"))?;
    // 整数として厳密に読む。<strong>丸めて通さない</strong>——`1.5` を版 1 として読めば、
    // 名乗っていない形を名乗った形として扱うことになる。
    if raw.fract() != 0.0 || raw < 0.0 || raw > f64::from(u32::MAX) {
        return Err(StoreError::Json {
            file: "manifest.json".into(),
            detail: format!("version が整数でない: {raw}"),
        });
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let version = raw as u32;
    if version != VERSION {
        return Err(StoreError::UnknownVersion {
            found: version,
            known: VERSION,
        });
    }
    // <strong>世代は欠けていてもよい。</strong> 世代を持たない頃のカセットは 0 から数え直す
    // ——止めるほどのことではない。次に書いた時点で 1 になる。
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let generation = manifest
        .get("generation")
        .and_then(Value::as_f64)
        .unwrap_or(0.0) as u64;
    let provisional = manifest
        .get("provisional")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();

    // <strong>場面は名乗りが正本である。</strong> 保存の中の階層ではなくなったので、
    // 突き合わせる相手が無い——欠けていたら断る。空に丸めれば、どの場面の目盛りか
    // <strong>分からないまま検めが通る。</strong>
    let scene = manifest
        .get("scene")
        .and_then(Value::as_str)
        .ok_or_else(|| missing("manifest.json", "scene"))?
        .to_owned();
    if !crate::scene_name_ok(&scene) {
        return Err(StoreError::Json {
            file: "manifest.json".into(),
            detail: "scene が空である".into(),
        });
    }

    // <strong>`decided/` の欠損を既定で埋めない。</strong> 書き出しは 3 つとも必ず出すので、
    // <strong>欠けていること自体が壊れている印である。</strong> 空で通せば、次に書いたときに
    // <strong>作り直せない判断が空として確定する</strong>——落ちるより悪い。
    //
    // 埋めてよいのは `derived/` だけである。あちらは作り直せる。
    let baseline = read_decided_baseline(&read_json(&e, "decided/baseline.json")?, "decided/baseline.json")?;
    let boilerplate = read_json(&e, "decided/boilerplate.json")?
        .as_array()
        .ok_or_else(|| missing("decided/boilerplate.json", "配列"))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| missing("decided/boilerplate.json", "文字列"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let movement = {
        let name = "decided/movement.json";
        let Value::Object(m) = read_json(&e, name)? else {
            return Err(missing(name, "対象"));
        };
        // <strong>知らない値を捨てない。</strong> 捨てれば、`stuck` にしたはずの指標が
        // 「未知」に戻って指摘に出続ける——書き換えたつもりのものが黙って戻る。
        m.into_iter()
            .map(|(k, v)| match v.as_str() {
                Some("moves") => Ok((k, Movement::Moves)),
                Some("stuck") => Ok((k, Movement::Stuck)),
                _ => Err(missing(name, &format!("{k} が moves か stuck でない"))),
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?
    };
    let derived = Derived {
        vocabulary: text_of(&e, "derived/vocabulary.json"),
        values: text_of(&e, "derived/values.jsonl"),
        spread: text_of(&e, "derived/spread.json"),
        calibration: text_of(&e, "derived/calibration.json"),
        scale: text_of(&e, "derived/scale.json"),
        effective: text_of(&e, "derived/effective.json"),
        phrases: text_of(&e, "derived/phrases.jsonl"),
    };

    Ok(Cassette {
        version,
        generation,
        fingerprint: read_fingerprint(&manifest)?,
        provisional,
        scene,
        decided: Decided {
            boilerplate,
            baseline,
            movement,
        },
        derived,
    })
}

fn derived_files(d: &Derived) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    for (name, body) in [
        ("vocabulary.json", &d.vocabulary),
        ("values.jsonl", &d.values),
        ("spread.json", &d.spread),
        ("calibration.json", &d.calibration),
        ("scale.json", &d.scale),
        ("effective.json", &d.effective),
        ("phrases.jsonl", &d.phrases),
    ] {
        if let Some(b) = body {
            out.push((name, b.clone()));
        }
    }
    out
}

fn manifest(c: &Cassette) -> Value {
    #[allow(clippy::cast_precision_loss)]
    let generation = c.generation as f64;
    Value::obj([
        ("version".into(), Value::Number(f64::from(c.version))),
        ("generation".into(), Value::Number(generation)),
        // <strong>場面はここにしか無い。</strong> 保存の中の階層ではなくなったので、
        // 突き合わせる相手も無い——ここが原本である。
        ("scene".into(), Value::s(&c.scene)),
        (
            "provisional".into(),
            Value::Array(c.provisional.iter().map(Value::s).collect()),
        ),
        // <strong>材料を平文で残す。</strong> ハッシュだけでは、何が変わったかが分からない。
        (
            "fingerprint_inputs".into(),
            inputs_json(&c.fingerprint.inputs),
        ),
    ])
}

fn inputs_json(i: &Inputs) -> Value {
    let c = &i.common;
    // <strong>共通部分と場面の部分を、構造で分けて残す。</strong> 平文で並べるだけでは、
    // 道具が変わったのか語彙が変わったのかを読み手が数えることになる。
    Value::obj([
        (
            "common".into(),
            Value::obj([
                ("metric_definitions".into(), Value::s(&c.metric_definitions)),
                ("unit_definitions".into(), Value::s(&c.unit_definitions)),
                ("morphology".into(), tool_json(&c.morphology)),
                ("dependency".into(), tool_json(&c.dependency)),
                ("compressor".into(), tool_json(&c.compressor)),
                (
                    "external_tables".into(),
                    Value::obj(
                        c.external_tables
                            .iter()
                            .map(|(k, v)| (k.clone(), Value::s(v))),
                    ),
                ),
                ("normalization".into(), normalization_json(&c.normalization)),
            ]),
        ),
        ("scene".into(), {
            let s = &i.scene;
            Value::obj([
                (
                    "vocabulary".into(),
                    Value::obj(s.vocabulary.iter().map(|(k, v)| {
                        (k.clone(), Value::Array(v.iter().map(Value::s).collect()))
                    })),
                ),
                (
                    "selection".into(),
                    Value::obj(s.selection.iter().map(|(k, v)| {
                        (k.clone(), Value::Array(v.iter().map(Value::s).collect()))
                    })),
                ),
                (
                    "z_scores".into(),
                    Value::obj(s.z_scores.iter().map(|(k, v)| {
                        (
                            k.clone(),
                            Value::Array(
                                v.iter()
                                    .map(|(m, sd)| {
                                        Value::Array(vec![Value::Number(*m), Value::Number(*sd)])
                                    })
                                    .collect(),
                            ),
                        )
                    })),
                ),
                ("baseline".into(), baseline_json(&s.baseline)),
                (
                    "decided".into(),
                    Value::obj(s.decided.iter().map(|(k, v)| (k.clone(), Value::s(v)))),
                ),
            ])
        }),
    ])
}

fn tool_json(t: &Tool) -> Value {
    Value::obj([
        ("name".into(), Value::s(&t.name)),
        ("version".into(), Value::s(&t.version)),
        (
            "config".into(),
            Value::obj(t.config.iter().map(|(k, v)| (k.clone(), Value::s(v)))),
        ),
    ])
}

fn normalization_json(n: &Normalization) -> Value {
    Value::obj([
        (
            "sources".into(),
            Value::Array(n.sources.iter().map(Value::s).collect()),
        ),
        ("implementation".into(), Value::s(&n.implementation)),
        ("version".into(), Value::s(&n.version)),
        (
            "mapping".into(),
            Value::obj(n.mapping.iter().map(|(k, v)| (k.clone(), Value::s(v)))),
        ),
    ])
}

fn baseline_json(b: &Baseline) -> Value {
    Value::obj([
        ("model".into(), Value::s(&b.model)),
        ("version".into(), Value::s(&b.version)),
        (
            "params".into(),
            Value::obj(b.params.iter().map(|(k, v)| (k.clone(), Value::s(v)))),
        ),
        // <strong>題材は指紋に入る。</strong> 言葉づかいだけで帯が動く。
        (
            "topics".into(),
            Value::Array(b.topics.iter().map(Value::s).collect()),
        ),
    ])
}

fn read_baseline(v: &Value) -> Result<Baseline, StoreError> {
    Ok(Baseline {
        model: v
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        version: v
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        params: read_map(v.get("params")),
        topics: v
            .get("topics")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// 人が決めた基準の作り方を読む。<strong>欄が欠けていたら断る。</strong>
///
/// <strong>[指紋に写したほう](read_baseline)とは扱いが違う。</strong> あちらは診断のための控えで、
/// 欠けても作り直せる。こちらは<strong>作り直せない原本</strong>なので、空に丸めれば
/// 次に書いたときにそこで確定する。
///
/// <strong>空の値は正しい状態である。</strong> 場面を作っただけで基準をまだ入れていなければ、
/// 4 つとも空で書かれる。<strong>断るのは欄そのものが無いときと、型が違うときである。</strong>
fn read_decided_baseline(v: &Value, file: &str) -> Result<Baseline, StoreError> {
    let text = |key: &str| -> Result<String, StoreError> {
        v.get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| missing(file, key))
    };
    let Some(Value::Object(params)) = v.get("params") else {
        return Err(missing(file, "params"));
    };
    let params = params
        .iter()
        .map(|(k, v)| {
            v.as_str()
                .map(|s| (k.clone(), s.to_owned()))
                .ok_or_else(|| missing(file, &format!("params.{k} が文字列でない")))
        })
        .collect::<Result<_, _>>()?;
    let topics = v
        .get("topics")
        .and_then(Value::as_array)
        .ok_or_else(|| missing(file, "topics"))?
        .iter()
        .map(|x| {
            x.as_str()
                .map(str::to_owned)
                .ok_or_else(|| missing(file, "topics の中身が文字列でない"))
        })
        .collect::<Result<_, _>>()?;
    Ok(Baseline {
        model: text("model")?,
        version: text("version")?,
        params,
        topics,
    })
}

fn read_fingerprint(manifest: &Value) -> Result<Fingerprint, StoreError> {
    let raw = manifest
        .get("fingerprint_inputs")
        .ok_or_else(|| missing("manifest.json", "fingerprint_inputs"))?;
    let i = raw
        .get("common")
        .ok_or_else(|| missing("manifest.json", "fingerprint_inputs.common"))?;
    let common = Common {
        metric_definitions: str_of(i, "metric_definitions"),
        unit_definitions: str_of(i, "unit_definitions"),
        morphology: read_tool(i.get("morphology")),
        dependency: read_tool(i.get("dependency")),
        compressor: read_tool(i.get("compressor")),
        external_tables: read_map(i.get("external_tables")),
        normalization: {
            let n = i.get("normalization");
            Normalization {
                sources: n
                    .and_then(|v| v.get("sources"))
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default(),
                implementation: n.map(|v| str_of(v, "implementation")).unwrap_or_default(),
                version: n.map(|v| str_of(v, "version")).unwrap_or_default(),
                mapping: read_map(n.and_then(|v| v.get("mapping"))),
            }
        },
    };
    let scene = match raw.get("scene") {
        Some(s) => SceneInputs {
            vocabulary: read_string_lists(s.get("vocabulary")),
            z_scores: read_pair_lists(s.get("z_scores")),
            selection: read_string_lists(s.get("selection")),
            baseline: s
                .get("baseline")
                .map(read_baseline)
                .transpose()?
                .unwrap_or_default(),
            decided: read_map(s.get("decided")),
        },
        None => SceneInputs::default(),
    };
    Ok(Fingerprint::build(Inputs { common, scene }))
}

fn read_string_lists(v: Option<&Value>) -> BTreeMap<String, Vec<String>> {
    let Some(Value::Object(m)) = v else {
        return BTreeMap::new();
    };
    m.iter()
        .map(|(k, v)| {
            (
                k.clone(),
                v.as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default(),
            )
        })
        .collect()
}

fn read_pair_lists(v: Option<&Value>) -> BTreeMap<String, Vec<(f64, f64)>> {
    let Some(Value::Object(m)) = v else {
        return BTreeMap::new();
    };
    m.iter()
        .map(|(k, v)| {
            (
                k.clone(),
                v.as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|p| {
                                let p = p.as_array()?;
                                Some((p.first()?.as_f64()?, p.get(1)?.as_f64()?))
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            )
        })
        .collect()
}

fn read_tool(v: Option<&Value>) -> Tool {
    let Some(v) = v else { return Tool::unused() };
    Tool {
        name: str_of(v, "name"),
        version: str_of(v, "version"),
        config: read_map(v.get("config")),
    }
}

fn read_map(v: Option<&Value>) -> BTreeMap<String, String> {
    match v {
        Some(Value::Object(m)) => m
            .iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
            .collect(),
        _ => BTreeMap::new(),
    }
}

fn str_of(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_owned()
}

fn text_of(e: &Entries, name: &str) -> Option<String> {
    e.get(name).and_then(|b| String::from_utf8(b.clone()).ok())
}

fn read_json(e: &Entries, name: &str) -> Result<Value, StoreError> {
    let body = e.get(name).ok_or_else(|| {
        StoreError::Zip(ZipError::NotFound {
            name: name.to_owned(),
        })
    })?;
    let s = std::str::from_utf8(body).map_err(|_| StoreError::Json {
        file: name.to_owned(),
        detail: "UTF-8 でない".into(),
    })?;
    json::parse(s).map_err(|e| StoreError::Json {
        file: name.to_owned(),
        detail: e.to_string(),
    })
}

fn missing(file: &str, field: &str) -> StoreError {
    StoreError::Missing {
        file: file.to_owned(),
        field: field.to_owned(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn cassette() -> Cassette {
        Cassette {
            version: VERSION,
            generation: 3,
            fingerprint: Fingerprint::build(Inputs {
                common: Common {
                    metric_definitions: "51 本".into(),
                    unit_definitions: "版 1".into(),
                    morphology: Tool {
                        name: "UniDic".into(),
                        version: "3.1.0".into(),
                        config: [("dicdir".to_owned(), "/dic".to_owned())].into(),
                    },
                    dependency: Tool::unused(),
                    compressor: Tool::unused(),
                    external_tables: [("Unicode".to_owned(), "15.1".to_owned())].into(),
                    normalization: Normalization {
                        sources: vec!["directive-markdown".into()],
                        implementation: "kakiburi-normalize".into(),
                        version: "0.0.0".into(),
                        mapping: [("message".to_owned(), "補足".to_owned())].into(),
                    },
                },
                scene: SceneInputs {
                    vocabulary: [("文字bigram".to_owned(), vec!["あい".to_owned()])].into(),
                    z_scores: [("文字bigram".to_owned(), vec![(0.1, 0.25)])].into(),
                    selection: [("本人の相手集合".to_owned(), vec!["p00".to_owned()])].into(),
                    baseline: Baseline {
                        model: "m".into(),
                        version: "v1".into(),
                        params: [("temperature".to_owned(), "1.0".to_owned())].into(),
                        topics: vec!["Vim のファイラー".into(), "GPG 鍵".into()],
                    },
                    decided: [("落とす定型".to_owned(), "この記事では".to_owned())].into(),
                },
            }),
            provisional: vec!["除外の既定".into()],
            scene: "技術記事".into(),
            decided: Decided {
                boilerplate: vec!["この記事では".into()],
                baseline: Baseline {
                    model: "m".into(),
                    version: "v1".into(),
                    params: [("temperature".to_owned(), "1.0".to_owned())].into(),
                    topics: vec!["Vim のファイラー".into()],
                },
                movement: [("笑い".to_owned(), Movement::Stuck)].into(),
            },
            derived: Derived {
                scale: Some("{\"ceiling\":[1,2]}".into()),
                vocabulary: Some("{\"文字bigram\":[\"あい\"]}".into()),
                phrases: Some("{\"text\":\"と思います。\",\"ceiling\":3.4}\n".into()),
                ..Derived::default()
            },
        }
    }

    #[test]
    fn 書いて読むと同じものが出る() {
        let c = cassette();
        let back = read(&write(&c)).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn 書き出しは決定的である() {
        // 作り直しても同じバイトが出る。
        assert_eq!(write(&cassette()), write(&cassette()));
    }

    #[test]
    fn 二つの層がそのままディレクトリになる() {
        let bytes = write(&cassette());
        let names = zip::index(&bytes).unwrap();
        assert!(names.iter().any(|n| n.starts_with("decided/")));
        assert!(names.iter().any(|n| n.starts_with("derived/")));
        assert!(
            !names.iter().any(|n| n.starts_with("corpus/")),
            "本文は持たない"
        );
    }

    #[test]
    fn 場面で割る階層を持たない() {
        // <strong>1 カセットが 1 場面である。</strong> 割る相手が無い。
        let bytes = write(&cassette());
        let names = zip::index(&bytes).unwrap();
        assert!(names.iter().any(|n| n == "decided/baseline.json"));
        assert!(names.iter().any(|n| n == "derived/scale.json"));
        assert!(
            !names.iter().any(|n| n.contains("技術記事")),
            "場面は名前ではなく manifest の欄である"
        );
    }

    #[test]
    fn 派生物を捨てて作り直すと同じものが出る() {
        // <strong>これが通らなければ、派生物のどこかに原本が混ざっている。</strong>
        // 気付かないまま運用すると、測り直した瞬間に人が決めたことが消える。
        let c = cassette();
        let full = write(&c);

        let mut dropped = c.clone();
        dropped.drop_derived();
        let back = read(&write(&dropped)).unwrap();
        assert!(!back.derived.has_scale(), "捨てられている");
        assert_eq!(back.decided, c.decided, "決めたことは残る");
        assert_eq!(back.scene, c.scene, "場面も残る");
        assert!(back.fingerprint.matches(&c.fingerprint), "指紋も残る");

        // 同じ派生物を入れ直すと、元と同じバイトになる。
        let mut rebuilt = back;
        rebuilt.derived = c.derived.clone();
        assert_eq!(write(&rebuilt), full, "作り直すと同じものが出る");
    }

    #[test]
    fn 派生物が無いカセットも読める() {
        let mut c = cassette();
        c.drop_derived();
        let back = read(&write(&c)).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn 言い回しの表が往復する() {
        // <strong>本文の代わりである。</strong> 落ちれば、繰り返しの上限を言えなくなる。
        let back = read(&write(&cassette())).unwrap();
        assert_eq!(back.derived.phrases, cassette().derived.phrases);
    }

    #[test]
    fn 知らない版は壊れているとは別の理由で断る() {
        // <strong>まとめると、壊れたカセットと新しすぎるカセットが同じ顔になる。</strong>
        // 前者は作り直しで、後者は道具の更新である。
        let mut e = zip::read(&write(&cassette())).unwrap();
        let m = String::from_utf8(e.get("manifest.json").unwrap().clone()).unwrap();
        // **いま書く版から作る。** 版を上げるたびに書き換える定数を残さない。
        let now = format!("\"version\":{VERSION}");
        for (found, body) in [
            (
                VERSION + 1,
                m.replace(&now, &format!("\"version\":{}", VERSION + 1)),
            ),
            (
                VERSION - 1,
                m.replace(&now, &format!("\"version\":{}", VERSION - 1)),
            ),
        ] {
            e.insert("manifest.json".into(), body.into_bytes());
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
    fn 版が整数でなければ読まない() {
        // 丸めて通せば、名乗っていない形を名乗った形として扱うことになる。
        let mut e = zip::read(&write(&cassette())).unwrap();
        let m = String::from_utf8(e.get("manifest.json").unwrap().clone()).unwrap();
        e.insert(
            "manifest.json".into(),
            m.replace(
                &format!("\"version\":{VERSION}"),
                &format!("\"version\":{VERSION}.5"),
            )
            .into_bytes(),
        );
        assert!(matches!(
            read(&zip::write(&e)),
            Err(StoreError::Json { .. })
        ));
    }

    #[test]
    fn 決めたことが欠けていたら断る() {
        // <strong>書き出しは 3 つとも必ず出す。</strong> 欠けていること自体が壊れている印
        // である——空で通せば、次に書いたときに作り直せない判断が空として確定する。
        for name in [
            "decided/baseline.json",
            "decided/boilerplate.json",
            "decided/movement.json",
        ] {
            let mut e = zip::read(&write(&cassette())).unwrap();
            e.remove(name);
            assert!(read(&zip::write(&e)).is_err(), "{name}");
        }
    }

    #[test]
    fn 場面を名乗らないカセットは読めない() {
        // <strong>保存の中の階層ではなくなったので、突き合わせる相手が無い。</strong>
        // 空に丸めれば、どの場面の目盛りか分からないまま検めが通る。
        for broken in ["\"scene\"", "\"scene\":\"技術記事\""] {
            let mut e = zip::read(&write(&cassette())).unwrap();
            let m = String::from_utf8(e.get("manifest.json").unwrap().clone()).unwrap();
            let body = if broken == "\"scene\"" {
                m.replace("\"scene\"", "\"ばめん\"")
            } else {
                m.replace(broken, "\"scene\":\"\"")
            };
            e.insert("manifest.json".into(), body.into_bytes());
            assert!(read(&zip::write(&e)).is_err(), "{broken}");
        }
    }

    #[test]
    fn 決めた基準の欄が欠けていたら断る() {
        // <strong>空の値は正しい状態である</strong>——作っただけなら 4 つとも空で書かれる。
        // <strong>断るのは欄そのものが無いときである</strong>：空に丸めれば、次に書いたときに
        // 作り直せない設定がそこで確定する。
        for key in ["model", "version", "params", "topics"] {
            let mut e = zip::read(&write(&cassette())).unwrap();
            let body = String::from_utf8(e.get("decided/baseline.json").unwrap().clone())
                .unwrap()
                .replace(&format!("\"{key}\""), &format!("\"{key}を消した\""));
            e.insert("decided/baseline.json".into(), body.into_bytes());
            assert!(read(&zip::write(&e)).is_err(), "{key}");
        }
    }

    #[test]
    fn 空の基準は正しい状態である() {
        // 作っただけで基準をまだ入れていなければ、4 つとも空で書かれる。
        let mut c = cassette();
        c.decided.baseline = Baseline::default();
        let back = read(&write(&c)).expect("読める");
        assert_eq!(back.decided.baseline, Baseline::default());
    }

    #[test]
    fn 知らない_movement_の値は捨てない() {
        // 捨てれば、`stuck` にしたはずの指標が「未知」に戻って指摘に出続ける。
        let mut e = zip::read(&write(&cassette())).unwrap();
        let body = String::from_utf8(e.get("decided/movement.json").unwrap().clone())
            .unwrap()
            .replace("\"stuck\"", "\"うごかない\"");
        e.insert("decided/movement.json".into(), body.into_bytes());
        assert!(read(&zip::write(&e)).is_err());
    }

    #[test]
    fn 題材が往復する() {
        // 外せば古い目盛りが黙って使われる。
        let back = read(&write(&cassette())).unwrap();
        assert_eq!(back.fingerprint.inputs.scene.baseline.topics.len(), 2);
    }

    #[test]
    fn manifest_が無ければ読めない() {
        let mut e = zip::read(&write(&cassette())).unwrap();
        e.remove("manifest.json");
        let err = read(&zip::write(&e)).unwrap_err();
        assert!(
            matches!(err, StoreError::Zip(ZipError::NotFound { .. })),
            "{err:?}"
        );
    }

    #[test]
    fn 壊れた_json_は読めない() {
        let mut e = zip::read(&write(&cassette())).unwrap();
        e.insert("manifest.json".into(), "{壊れている".as_bytes().to_vec());
        let err = read(&zip::write(&e)).unwrap_err();
        assert!(matches!(err, StoreError::Json { .. }), "{err:?}");
    }
}
