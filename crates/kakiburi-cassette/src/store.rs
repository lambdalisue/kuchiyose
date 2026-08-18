//! カセットを zip に落とし、読み戻す。
//!
//! <strong>3 つのディレクトリが、そのまま 3 つの層である。</strong>`derived/` を丸ごと消しても、
//! `corpus/` と `decided/` があれば同じものが作り直せる。

use std::collections::BTreeMap;

use kakiburi_doc::node::{Kind, Node};
use kakiburi_doc::Document;

use crate::json::{self, Value};
use crate::zip::{self, Entries, ZipError};
use crate::{
    Baseline, Belongs, Cassette, Common, Corpus, Decided, Derived, Fingerprint, Inputs, Movement,
    Normalization, SceneInputs, Tool, Track, Unit,
};

/// いま書く版。
///
/// <strong>版は、形か意味が非互換に変わったときに上げる。</strong>[1 カセット 1 人](../../../docs/design/100-cassette.md#1-カセット-1-人)
/// で `decided/` と `derived/` が場面ごとの階層になり、単位が `scene` を持つように
/// なった——<strong>版 1 のカセットとは形が違う。</strong>
///
/// <strong>古い版を読む道は持たない。</strong> 原本は正規形なので素材から作り直せる
/// ——移し替える道を持つと、作り直せないものが増える。
pub const VERSION: u32 = 2;

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
    // <strong>`decided/` と `derived/` は場面ごとの階層になる。</strong> 共有するのは本文と、
    // 道具・実装・定義の版だけである。
    for (scene, t) in &c.tracks {
        e.insert(
            format!("decided/{scene}/boilerplate.json"),
            Value::Array(t.decided.boilerplate.iter().map(Value::s).collect())
                .write()
                .into_bytes(),
        );
        e.insert(
            format!("decided/{scene}/baseline.json"),
            baseline_json(&t.decided.baseline).write().into_bytes(),
        );
        e.insert(
            format!("decided/{scene}/movement.json"),
            Value::obj(t.decided.movement.iter().map(|(k, v)| {
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
        for (name, body) in derived_files(&t.derived) {
            e.insert(format!("derived/{scene}/{name}"), body.into_bytes());
        }
    }
    // <strong>本文は共有する。</strong> 場面は単位が持つ——役の下に場面の階層を作らないのは、
    // 場面を持たない `other` がその形に収まらないからである。
    for u in &c.corpus.units {
        let path = format!("corpus/{}/{}.json", u.role().dir(), u.name);
        e.insert(path, unit_json(u).write().into_bytes());
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

    let index = zip::index(bytes)?;

    // <strong>場面は entry の名前から拾う。</strong> どのファイルが在ってもトラックは 1 つで、
    // 欄が欠けていれば既定で埋める——`add` した直後は `derived/` がまだ無い。
    // <strong>控えと突き合わせる。</strong> 階層から拾うだけにすると、<strong>場面ごと消えたときに
    // 気付けない</strong>——`decided/<場面>/` の 3 つを全部消せばその場面は一覧に現れず、
    // 検めるループにも入らないまま読めてしまう。
    //
    // どちらが正しいかを決めるためではなく、<strong>食い違いを見つけるために 2 つ持つ。</strong>
    let mut scenes: Vec<String> = manifest
        .get("scenes")
        .and_then(Value::as_array)
        .ok_or_else(|| missing("manifest.json", "scenes"))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| missing("manifest.json", "scenes の中身が文字列でない"))
        })
        .collect::<Result<_, _>>()?;
    scenes.sort_unstable();
    scenes.dedup();

    let mut found: Vec<String> = Vec::new();
    for name in &index {
        for prefix in ["decided/", "derived/"] {
            if let Some(rest) = name.strip_prefix(prefix) {
                if let Some((scene, _)) = rest.split_once('/') {
                    found.push(scene.to_owned());
                }
            }
        }
    }
    found.sort_unstable();
    found.dedup();
    if scenes != found {
        return Err(StoreError::Json {
            file: "manifest.json".into(),
            detail: format!("名乗った場面と中身が食い違う（名乗り {scenes:?} / 中身 {found:?}）"),
        });
    }

    let mut tracks = BTreeMap::new();
    for scene in scenes {
        // <strong>`decided/` の欠損を既定で埋めない。</strong> 書き出しは場面ごとに 3 つとも
        // 必ず出すので、<strong>欠けていること自体が壊れている印である。</strong> 空で通せば、
        // 次に書いたときに<strong>作り直せない判断が空として確定する</strong>——落ちるより悪い。
        //
        // 埋めてよいのは `derived/` だけである。あちらは作り直せる。
        let file = format!("decided/{scene}/baseline.json");
        let baseline = read_decided_baseline(&read_json(&e, &file)?, &file)?;
        let boilerplate = read_json(&e, &format!("decided/{scene}/boilerplate.json"))?
            .as_array()
            .ok_or_else(|| missing(&format!("decided/{scene}/boilerplate.json"), "配列"))?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| missing(&format!("decided/{scene}/boilerplate.json"), "文字列"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let movement = {
            let name = format!("decided/{scene}/movement.json");
            let Value::Object(m) = read_json(&e, &name)? else {
                return Err(missing(&name, "対象"));
            };
            // <strong>知らない値を捨てない。</strong> 捨てれば、`stuck` にしたはずの指標が
            // 「未知」に戻って指摘に出続ける——書き換えたつもりのものが黙って戻る。
            m.into_iter()
                .map(|(k, v)| match v.as_str() {
                    Some("moves") => Ok((k, Movement::Moves)),
                    Some("stuck") => Ok((k, Movement::Stuck)),
                    _ => Err(missing(&name, &format!("{k} が moves か stuck でない"))),
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?
        };
        let derived = Derived {
            vocabulary: text_of(&e, &format!("derived/{scene}/vocabulary.json")),
            values: text_of(&e, &format!("derived/{scene}/values.jsonl")),
            spread: text_of(&e, &format!("derived/{scene}/spread.json")),
            calibration: text_of(&e, &format!("derived/{scene}/calibration.json")),
            scale: text_of(&e, &format!("derived/{scene}/scale.json")),
            effective: text_of(&e, &format!("derived/{scene}/effective.json")),
        };
        tracks.insert(
            scene,
            Track {
                decided: Decided {
                    boilerplate,
                    baseline,
                    movement,
                },
                derived,
            },
        );
    }

    let mut units = Vec::new();
    for name in &index {
        let Some(rest) = name.strip_prefix("corpus/") else {
            continue;
        };
        let Some((dir, file)) = rest.split_once('/') else {
            continue;
        };
        if !matches!(dir, "person" | "baseline" | "other") {
            continue;
        }
        let v = read_json(&e, name)?;
        let id = file.trim_end_matches(".json").to_owned();
        // <strong>`unit` が無ければ `id` と同じ。</strong> 束ねる前のカセットは、1 本が 1 単位である。
        let unit = v
            .get("unit")
            .and_then(Value::as_str)
            .unwrap_or(&id)
            .to_owned();
        // <strong>場面はディレクトリではなく単位が持つ。</strong> 場面を持たない `other` が
        // 役の下の場面の階層に収まらないからである。
        let scene = v.get("scene").and_then(Value::as_str);
        let belongs = match (dir, scene) {
            ("person", Some(s)) => Belongs::Person {
                scene: s.to_owned(),
            },
            ("baseline", Some(s)) => Belongs::Baseline {
                scene: s.to_owned(),
            },
            ("other", _) => Belongs::Other,
            // <strong>場面の無い本人・基準は壊れている。</strong> 既定で埋めれば、どの場面の
            // 材料かが分からないまま目盛りに入る。
            _ => {
                return Err(missing(name, "scene"));
            }
        };
        units.push(Unit {
            name: id,
            unit,
            belongs,
            document: read_document(&v)?,
        });
    }

    // <strong>単位の場面にもトラックが要る。</strong> 無い場面を指す単位は、場面で絞る口から
    // <strong>1 度も出てこない</strong>——`build` も `show` も `doctor` も、その単位を見ないまま
    // 通る。入っているのに効かない状態が、エラーにならずに続く。
    for u in &units {
        if let Some(scene) = u.belongs.scene() {
            if !tracks.contains_key(scene) {
                return Err(StoreError::Json {
                    file: format!("corpus/{}/{}.json", u.role().dir(), u.name),
                    detail: format!("知らない場面を指している: {scene}"),
                });
            }
        }
    }

    Ok(Cassette {
        version,
        generation,
        fingerprint: read_fingerprint(&manifest)?,
        provisional,
        corpus: Corpus::new(units),
        tracks,
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
        // <strong>場面は manifest に置かない。</strong> 1 カセット 1 場面ではなくなったので、
        // ここに 1 つだけ書ける欄があると、どの場面のことかを言えない値になる。
        (
            "scenes".into(),
            Value::Array(c.tracks.keys().map(Value::s).collect()),
        ),
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
    // <strong>共通部分と場面ごとの部分を、構造で分けて残す。</strong> 平文で並べるだけでは、
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
        (
            "scenes".into(),
            Value::obj(i.scenes.iter().map(|(scene, s)| {
                (
                    scene.clone(),
                    Value::obj([
                        (
                            "vocabulary".into(),
                            Value::obj(s.vocabulary.iter().map(|(k, v)| {
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
                                                Value::Array(vec![
                                                    Value::Number(*m),
                                                    Value::Number(*sd),
                                                ])
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
                    ]),
                )
            })),
        ),
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

fn unit_json(u: &Unit) -> Value {
    let mut pairs = vec![
        ("id".to_owned(), Value::s(&u.name)),
        // <strong>測る単位を別に持つ。</strong> 束ねた文書は同じ `unit` を共有する。
        ("unit".to_owned(), Value::s(&u.unit)),
        ("role".to_owned(), Value::s(u.role().dir())),
    ];
    // <strong>場面を持たない単位には欄を書かない。</strong> 空文字を書けば、場面が「空」なのか
    // 「持たない」のかが読み戻しで分からない。
    if let Some(s) = u.belongs.scene() {
        pairs.push(("scene".to_owned(), Value::s(s)));
    }
    pairs.push((
        "nodes".to_owned(),
        Value::Array(u.document.nodes.iter().map(node_json).collect()),
    ));
    Value::obj(pairs)
}

fn node_json(n: &Node) -> Value {
    let mut pairs = vec![
        ("type".into(), Value::s(kind_name(n.kind))),
        ("text".into(), Value::s(&n.text)),
    ];
    if let Some(d) = n.raw_depth {
        pairs.push(("depth".into(), Value::Number(f64::from(d))));
    }
    if !n.children.is_empty() {
        pairs.push((
            "children".into(),
            Value::Array(n.children.iter().map(node_json).collect()),
        ));
    }
    Value::obj(pairs)
}

fn read_document(v: &Value) -> Result<Document, StoreError> {
    let nodes = v
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| missing("corpus", "nodes"))?;
    Ok(Document::new(
        nodes.iter().map(read_node).collect::<Result<Vec<_>, _>>()?,
    ))
}

fn read_node(v: &Value) -> Result<Node, StoreError> {
    let kind = v
        .get("type")
        .and_then(Value::as_str)
        .and_then(kind_from_name)
        .ok_or_else(|| missing("corpus", "type"))?;
    let text = v
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let raw_depth = v.get("depth").and_then(Value::as_f64).map(|d| d as u8);
    let children = v
        .get("children")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(read_node).collect::<Result<Vec<_>, _>>())
        .transpose()?
        .unwrap_or_default();
    Ok(Node {
        kind,
        text,
        raw_depth,
        children,
    })
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
    let mut scenes = BTreeMap::new();
    if let Some(Value::Object(m)) = raw.get("scenes") {
        for (scene, s) in m {
            scenes.insert(
                scene.clone(),
                SceneInputs {
                    vocabulary: read_string_lists(s.get("vocabulary")),
                    z_scores: read_pair_lists(s.get("z_scores")),
                    baseline: s
                        .get("baseline")
                        .map(read_baseline)
                        .transpose()?
                        .unwrap_or_default(),
                    decided: read_map(s.get("decided")),
                },
            );
        }
    }
    Ok(Fingerprint::build(Inputs { common, scenes }))
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

/// node の種類の名前。<strong>仕様の表の名前をそのまま使う。</strong>
fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Paragraph => "段落",
        Kind::Heading => "見出し",
        Kind::Bullet => "箇条書き",
        Kind::Ordered => "番号リスト",
        Kind::Item => "項目",
        Kind::Quote => "引用",
        Kind::Note => "補足",
        Kind::Warning => "警告",
        Kind::Details => "折りたたみ",
        Kind::Footnote => "脚注",
        Kind::Table => "表",
        Kind::Cell => "セル",
        Kind::CodeBlock => "コードブロック",
        Kind::Divider => "区切り線",
        Kind::Image => "画像",
        Kind::Emphasis => "強調",
        Kind::InlineCode => "インラインコード",
        Kind::Link => "リンク",
    }
}

fn kind_from_name(name: &str) -> Option<Kind> {
    Some(match name {
        "段落" => Kind::Paragraph,
        "見出し" => Kind::Heading,
        "箇条書き" => Kind::Bullet,
        "番号リスト" => Kind::Ordered,
        "項目" => Kind::Item,
        "引用" => Kind::Quote,
        "補足" => Kind::Note,
        "警告" => Kind::Warning,
        "折りたたみ" => Kind::Details,
        "脚注" => Kind::Footnote,
        "表" => Kind::Table,
        "セル" => Kind::Cell,
        "コードブロック" => Kind::CodeBlock,
        "区切り線" => Kind::Divider,
        "画像" => Kind::Image,
        "強調" => Kind::Emphasis,
        "インラインコード" => Kind::InlineCode,
        "リンク" => Kind::Link,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Role;

    fn cassette() -> Cassette {
        let mut table = Node::branch(
            Kind::Table,
            vec![
                Node::leaf(Kind::Cell, "機能"),
                Node::leaf(Kind::Cell, "あり"),
            ],
        );
        table.text = String::new();
        let mut para = Node::leaf(Kind::Paragraph, "ここが大事である。");
        para.children.push(Node::leaf(Kind::Emphasis, "大事"));

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
                scenes: [(
                    "技術記事".to_owned(),
                    SceneInputs {
                        vocabulary: [("文字bigram".to_owned(), vec!["あい".to_owned()])].into(),
                        z_scores: [("文字bigram".to_owned(), vec![(0.1, 0.25)])].into(),
                        baseline: Baseline {
                            model: "m".into(),
                            version: "v1".into(),
                            params: [("temperature".to_owned(), "1.0".to_owned())].into(),
                            topics: vec!["Vim のファイラー".into(), "GPG 鍵".into()],
                        },
                        decided: [("落とす定型".to_owned(), "この記事では".to_owned())].into(),
                    },
                )]
                .into(),
            }),
            provisional: vec!["除外の既定".into()],
            corpus: Corpus::new(vec![
                Unit {
                    name: "p01".into(),
                    unit: "p01".into(),
                    belongs: Belongs::Person {
                        scene: "技術記事".into(),
                    },
                    document: Document::new(vec![Node::heading(1, "題"), para, table]),
                },
                Unit {
                    name: "b01".into(),
                    unit: "b01".into(),
                    belongs: Belongs::Baseline {
                        scene: "技術記事".into(),
                    },
                    document: Document::new(vec![Node::leaf(Kind::Paragraph, "基準である。")]),
                },
                // <strong>場面を持たない他人の文書も入れる。</strong> 往復で場面が付いてしまえば、
                // 場面で絞る口から漏れる。
                Unit {
                    name: "o01".into(),
                    unit: "o01".into(),
                    belongs: Belongs::Other,
                    document: Document::new(vec![Node::leaf(Kind::Paragraph, "他人である。")]),
                },
            ]),
            tracks: [(
                "技術記事".to_owned(),
                Track {
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
                        ..Derived::default()
                    },
                },
            )]
            .into(),
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
    fn 三つの層がそのままディレクトリになる() {
        let bytes = write(&cassette());
        let names = zip::index(&bytes).unwrap();
        assert!(names.iter().any(|n| n.starts_with("decided/")));
        assert!(names.iter().any(|n| n.starts_with("corpus/person/")));
        assert!(names.iter().any(|n| n.starts_with("corpus/baseline/")));
        assert!(names.iter().any(|n| n.starts_with("derived/")));
    }

    #[test]
    fn 派生物を捨てて作り直すと同じものが出る() {
        // <strong>これが通らなければ、派生物のどこかに原本が混ざっている。</strong>
        // 気付かないまま運用すると、測り直した瞬間に人が決めたことが消える。
        let c = cassette();
        let full = write(&c);

        // derived/ を丸ごと落とす。
        let mut dropped = c.clone();
        dropped.drop_all_derived();
        let without = write(&dropped);
        let back = read(&without).unwrap();
        assert!(
            !back.track("技術記事").unwrap().derived.has_scale(),
            "捨てられている"
        );
        assert_eq!(back.corpus, c.corpus, "原本は残る");
        assert_eq!(
            back.track("技術記事").unwrap().decided,
            c.track("技術記事").unwrap().decided,
            "決めたことも残る"
        );
        assert!(back.fingerprint.matches(&c.fingerprint), "指紋も残る");

        // 同じ派生物を入れ直すと、元と同じバイトになる。
        let mut rebuilt = back;
        rebuilt.track_mut("技術記事").derived = c.track("技術記事").unwrap().derived.clone();
        assert_eq!(write(&rebuilt), full, "作り直すと同じものが出る");
    }

    #[test]
    fn 派生物が無いカセットも読める() {
        let mut c = cassette();
        c.drop_all_derived();
        let back = read(&write(&c)).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn 場面ごとの階層になる() {
        // <strong>共有するのは本文と、道具・実装・定義の版だけである。</strong>
        let bytes = write(&cassette());
        let names = zip::index(&bytes).unwrap();
        assert!(names.iter().any(|n| n == "decided/技術記事/baseline.json"));
        assert!(names.iter().any(|n| n == "derived/技術記事/scale.json"));
        assert!(
            !names.iter().any(|n| n == "decided/baseline.json"),
            "場面の外に決めたことを置かない"
        );
    }

    #[test]
    fn 場面を持たない単位は場面を持たないまま戻る() {
        // 往復で場面が付いてしまえば、場面で絞る口から他人の文書が漏れる。
        let back = read(&write(&cassette())).unwrap();
        assert_eq!(back.corpus.for_humanness().len(), 1);
        assert_eq!(back.corpus.in_scene("技術記事", Role::Other).len(), 0);
    }

    #[test]
    fn 知らない版は壊れているとは別の理由で断る() {
        // <strong>まとめると、壊れたカセットと新しすぎるカセットが同じ顔になる。</strong>
        // 前者は作り直しで、後者は道具の更新である。
        let mut e = zip::read(&write(&cassette())).unwrap();
        let m = String::from_utf8(e.get("manifest.json").unwrap().clone()).unwrap();
        for (found, body) in [
            (VERSION + 1, m.replace("\"version\":2", "\"version\":3")),
            (1, m.replace("\"version\":2", "\"version\":1")),
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
            m.replace("\"version\":2", "\"version\":2.5").into_bytes(),
        );
        assert!(matches!(
            read(&zip::write(&e)),
            Err(StoreError::Json { .. })
        ));
    }

    #[test]
    fn 決めたことが欠けていたら断る() {
        // <strong>書き出しは場面ごとに 3 つとも必ず出す。</strong> 欠けていること自体が
        // 壊れている印である——空で通せば、次に書いたときに作り直せない判断が
        // 空として確定する。
        for name in [
            "decided/技術記事/baseline.json",
            "decided/技術記事/boilerplate.json",
            "decided/技術記事/movement.json",
        ] {
            let mut e = zip::read(&write(&cassette())).unwrap();
            e.remove(name);
            assert!(read(&zip::write(&e)).is_err(), "{name}");
        }
    }

    #[test]
    fn 場面ごと消えたら断る() {
        // <strong>`decided/<場面>/` を 3 つとも消すと、その場面は階層から現れない。</strong>
        // 階層だけを見ていると、検めるループにも入らないまま読めてしまう
        // ——場面ごと消えたことが、いちばん見えにくい形で通る。
        let mut e = zip::read(&write(&cassette())).unwrap();
        for name in [
            "decided/技術記事/baseline.json",
            "decided/技術記事/boilerplate.json",
            "decided/技術記事/movement.json",
            "derived/技術記事/scale.json",
            "derived/技術記事/vocabulary.json",
        ] {
            e.remove(name);
        }
        assert!(read(&zip::write(&e)).is_err(), "場面ごと消えたのに読めた");
    }

    #[test]
    fn 名乗った場面と中身が食い違えば断る() {
        // 控えと階層を 2 つ持つのは、どちらが正しいかを決めるためではなく、
        // <strong>食い違いを見つけるため</strong>である。
        let mut e = zip::read(&write(&cassette())).unwrap();
        let m = String::from_utf8(e.get("manifest.json").unwrap().clone()).unwrap();
        e.insert(
            "manifest.json".into(),
            m.replace("[\"技術記事\"]", "[\"技術記事\",\"チャット\"]")
                .into_bytes(),
        );
        assert!(read(&zip::write(&e)).is_err());
    }

    #[test]
    fn 決めた基準の欄が欠けていたら断る() {
        // <strong>空の値は正しい状態である</strong>——場面を作っただけなら 4 つとも空で書かれる。
        // <strong>断るのは欄そのものが無いときである</strong>：空に丸めれば、次に書いたときに
        // 作り直せない設定がそこで確定する。
        for key in ["model", "version", "params", "topics"] {
            let mut e = zip::read(&write(&cassette())).unwrap();
            let body = String::from_utf8(e.get("decided/技術記事/baseline.json").unwrap().clone())
                .unwrap()
                .replace(&format!("\"{key}\""), &format!("\"{key}を消した\""));
            e.insert("decided/技術記事/baseline.json".into(), body.into_bytes());
            assert!(read(&zip::write(&e)).is_err(), "{key}");
        }
    }

    #[test]
    fn 知らない場面を指す単位は断る() {
        // <strong>場面で絞る口から 1 度も出てこない。</strong> `build` も `show` も `doctor` も
        // その単位を見ないまま通る——入っているのに効かない状態が、エラーに
        // ならずに続く。
        let mut e = zip::read(&write(&cassette())).unwrap();
        let body = String::from_utf8(e.get("corpus/person/p01.json").unwrap().clone())
            .unwrap()
            .replace("\"技術記事\"", "\"チャット\"");
        e.insert("corpus/person/p01.json".into(), body.into_bytes());
        assert!(read(&zip::write(&e)).is_err());
    }

    #[test]
    fn 空の基準は正しい状態である() {
        // 場面を作っただけで基準をまだ入れていなければ、4 つとも空で書かれる。
        let mut c = cassette();
        c.track_mut("技術記事").decided.baseline = Baseline::default();
        let back = read(&write(&c)).expect("読める");
        assert_eq!(
            back.track("技術記事").unwrap().decided.baseline,
            Baseline::default()
        );
    }

    #[test]
    fn 知らない_movement_の値は捨てない() {
        // 捨てれば、`stuck` にしたはずの指標が「未知」に戻って指摘に出続ける。
        let mut e = zip::read(&write(&cassette())).unwrap();
        let body = String::from_utf8(e.get("decided/技術記事/movement.json").unwrap().clone())
            .unwrap()
            .replace("\"stuck\"", "\"うごかない\"");
        e.insert("decided/技術記事/movement.json".into(), body.into_bytes());
        assert!(read(&zip::write(&e)).is_err());
    }

    #[test]
    fn 場面の無い本人は壊れている() {
        // 既定で埋めれば、どの場面の材料かが分からないまま目盛りに入る。
        let mut e = zip::read(&write(&cassette())).unwrap();
        let body = e.get("corpus/person/p01.json").unwrap().clone();
        let text = String::from_utf8(body).unwrap();
        let broken = text.replace("\"scene\"", "\"ばめん\"");
        e.insert("corpus/person/p01.json".into(), broken.into_bytes());
        assert!(read(&zip::write(&e)).is_err());
    }

    #[test]
    fn 複数の場面が_1_本に入る() {
        // 1 カセットが 1 人である。
        let mut c = cassette();
        c.corpus.push(Unit {
            name: "c01".into(),
            unit: "c01".into(),
            belongs: Belongs::Person {
                scene: "チャット".into(),
            },
            document: Document::new(vec![Node::leaf(Kind::Paragraph, "短いやつ。")]),
        });
        c.track_mut("チャット").decided.boilerplate = vec!["おつかれさまです".into()];
        let back = read(&write(&c)).unwrap();
        assert_eq!(back.scenes(), vec!["チャット", "技術記事"]);
        assert_eq!(back.corpus.in_scene("チャット", Role::Person).len(), 1);
        assert_eq!(back.corpus.in_scene("技術記事", Role::Person).len(), 1);
        assert_eq!(
            back.track("チャット").unwrap().decided.boilerplate,
            vec!["おつかれさまです"]
        );
        assert!(
            back.track("チャット").unwrap().decided.movement.is_empty(),
            "場面ごとに別である"
        );
    }

    #[test]
    fn node_の名前は仕様の表と同じである() {
        assert_eq!(kind_name(Kind::Cell), "セル");
        assert_eq!(kind_from_name("セル"), Some(Kind::Cell));
        assert_eq!(kind_from_name("行"), None, "表に無い名前は受けない");
    }

    #[test]
    fn すべての種類が往復する() {
        for k in [
            Kind::Paragraph,
            Kind::Heading,
            Kind::Bullet,
            Kind::Ordered,
            Kind::Item,
            Kind::Quote,
            Kind::Note,
            Kind::Warning,
            Kind::Details,
            Kind::Footnote,
            Kind::Table,
            Kind::Cell,
            Kind::CodeBlock,
            Kind::Divider,
            Kind::Image,
            Kind::Emphasis,
            Kind::InlineCode,
            Kind::Link,
        ] {
            assert_eq!(kind_from_name(kind_name(k)), Some(k), "{k:?}");
        }
    }

    #[test]
    fn 見出しの深さが往復する() {
        let c = Cassette {
            corpus: Corpus::new(vec![Unit {
                name: "p01".into(),
                unit: "p01".into(),
                belongs: Belongs::Person {
                    scene: "技術記事".into(),
                },
                document: Document::new(vec![Node::heading(3, "項")]),
            }]),
            ..cassette()
        };
        let back = read(&write(&c)).unwrap();
        assert_eq!(
            back.corpus.in_scene("技術記事", Role::Person)[0]
                .document
                .nodes[0]
                .raw_depth,
            Some(3)
        );
    }

    #[test]
    fn 題材が往復する() {
        // 外せば古い目盛りが黙って使われる。
        let c = cassette();
        let back = read(&write(&c)).unwrap();
        assert_eq!(
            back.fingerprint.inputs.scenes["技術記事"]
                .baseline
                .topics
                .len(),
            2
        );
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
