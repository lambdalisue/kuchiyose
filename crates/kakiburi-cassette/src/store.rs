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
    Baseline, Cassette, Corpus, Decided, Derived, Fingerprint, Inputs, Movement, Normalization,
    Role, Tool, Unit,
};

/// 読み書きできない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// zip として読めない。
    Zip(ZipError),
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
        "decided/scene.json".into(),
        Value::s(&c.decided.scene).write().into_bytes(),
    );
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
    for u in &c.corpus.units {
        let path = format!("corpus/{}/{}.json", u.role.dir(), u.name);
        e.insert(path, unit_json(u).write().into_bytes());
    }
    for (name, body) in derived_files(&c.derived) {
        e.insert(format!("derived/{name}"), body.into_bytes());
    }
    zip::write(&e)
}

/// zip のバイトからカセットを読む。
pub fn read(bytes: &[u8]) -> Result<Cassette, StoreError> {
    let e = zip::read(bytes)?;
    let manifest = read_json(&e, "manifest.json")?;
    let version = u32::try_from(
        manifest
            .get("version")
            .and_then(Value::as_f64)
            .ok_or_else(|| missing("manifest.json", "version"))? as i64,
    )
    .unwrap_or(0);
    let scene = manifest
        .get("scene")
        .and_then(Value::as_str)
        .ok_or_else(|| missing("manifest.json", "scene"))?
        .to_owned();
    let provisional = manifest
        .get("provisional")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();

    let baseline = read_baseline(&read_json(&e, "decided/baseline.json")?)?;
    let boilerplate = read_json(&e, "decided/boilerplate.json")?
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let movement = match read_json(&e, "decided/movement.json")? {
        Value::Object(m) => m
            .into_iter()
            .filter_map(|(k, v)| match v.as_str() {
                Some("moves") => Some((k, Movement::Moves)),
                Some("stuck") => Some((k, Movement::Stuck)),
                _ => None,
            })
            .collect(),
        _ => BTreeMap::new(),
    };

    let mut units = Vec::new();
    for name in zip::index(bytes)? {
        let Some(rest) = name.strip_prefix("corpus/") else {
            continue;
        };
        let Some((dir, file)) = rest.split_once('/') else {
            continue;
        };
        let role = match dir {
            "person" => Role::Person,
            "baseline" => Role::BaselineOutput,
            "other" => Role::Other,
            _ => continue,
        };
        let v = read_json(&e, &name)?;
        units.push(Unit {
            name: file.trim_end_matches(".json").to_owned(),
            role,
            document: read_document(&v)?,
        });
    }

    let derived = Derived {
        vocabulary: text_of(&e, "derived/vocabulary.json"),
        values: text_of(&e, "derived/values.jsonl"),
        spread: text_of(&e, "derived/spread.json"),
        calibration: text_of(&e, "derived/calibration.json"),
        scale: text_of(&e, "derived/scale.json"),
        effective: text_of(&e, "derived/effective.json"),
    };

    Ok(Cassette {
        version,
        scene: scene.clone(),
        fingerprint: read_fingerprint(&manifest)?,
        provisional,
        decided: Decided {
            scene,
            boilerplate,
            baseline,
            movement,
        },
        corpus: Corpus::new(units),
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
    ] {
        if let Some(b) = body {
            out.push((name, b.clone()));
        }
    }
    out
}

fn manifest(c: &Cassette) -> Value {
    Value::obj([
        ("version".into(), Value::Number(f64::from(c.version))),
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
    Value::obj([
        ("metric_definitions".into(), Value::s(&i.metric_definitions)),
        ("unit_definitions".into(), Value::s(&i.unit_definitions)),
        (
            "vocabulary".into(),
            Value::obj(
                i.vocabulary
                    .iter()
                    .map(|(k, v)| (k.clone(), Value::Array(v.iter().map(Value::s).collect()))),
            ),
        ),
        (
            "z_scores".into(),
            Value::obj(i.z_scores.iter().map(|(k, v)| {
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
        ("morphology".into(), tool_json(&i.morphology)),
        ("dependency".into(), tool_json(&i.dependency)),
        ("compressor".into(), tool_json(&i.compressor)),
        (
            "external_tables".into(),
            Value::obj(
                i.external_tables
                    .iter()
                    .map(|(k, v)| (k.clone(), Value::s(v))),
            ),
        ),
        ("normalization".into(), normalization_json(&i.normalization)),
        ("baseline".into(), baseline_json(&i.baseline)),
        (
            "decided".into(),
            Value::obj(i.decided.iter().map(|(k, v)| (k.clone(), Value::s(v)))),
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
    Value::obj([
        ("unit".into(), Value::s(&u.name)),
        ("role".into(), Value::s(u.role.dir())),
        (
            "nodes".into(),
            Value::Array(u.document.nodes.iter().map(node_json).collect()),
        ),
    ])
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

fn read_fingerprint(manifest: &Value) -> Result<Fingerprint, StoreError> {
    let i = manifest
        .get("fingerprint_inputs")
        .ok_or_else(|| missing("manifest.json", "fingerprint_inputs"))?;
    Ok(Fingerprint::build(Inputs {
        metric_definitions: str_of(i, "metric_definitions"),
        unit_definitions: str_of(i, "unit_definitions"),
        vocabulary: i
            .get("vocabulary")
            .and_then(|v| match v {
                Value::Object(m) => Some(
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
                        .collect(),
                ),
                _ => None,
            })
            .unwrap_or_default(),
        z_scores: i
            .get("z_scores")
            .and_then(|v| match v {
                Value::Object(m) => Some(
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
                        .collect(),
                ),
                _ => None,
            })
            .unwrap_or_default(),
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
        baseline: i
            .get("baseline")
            .map(read_baseline)
            .transpose()?
            .unwrap_or(Baseline {
                model: String::new(),
                version: String::new(),
                params: BTreeMap::new(),
                topics: vec![],
            }),
        decided: read_map(i.get("decided")),
    }))
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
            version: 1,
            scene: "技術記事".into(),
            fingerprint: Fingerprint::build(Inputs {
                metric_definitions: "51 本".into(),
                unit_definitions: "版 1".into(),
                vocabulary: [("文字bigram".to_owned(), vec!["あい".to_owned()])].into(),
                z_scores: [("文字bigram".to_owned(), vec![(0.1, 0.25)])].into(),
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
                baseline: Baseline {
                    model: "m".into(),
                    version: "v1".into(),
                    params: [("temperature".to_owned(), "1.0".to_owned())].into(),
                    topics: vec!["Vim のファイラー".into(), "GPG 鍵".into()],
                },
                decided: [("場面".to_owned(), "技術記事".to_owned())].into(),
            }),
            provisional: vec!["除外の既定".into()],
            decided: Decided {
                scene: "技術記事".into(),
                boilerplate: vec!["この記事では".into()],
                baseline: Baseline {
                    model: "m".into(),
                    version: "v1".into(),
                    params: [("temperature".to_owned(), "1.0".to_owned())].into(),
                    topics: vec!["Vim のファイラー".into()],
                },
                movement: [("笑い".to_owned(), Movement::Stuck)].into(),
            },
            corpus: Corpus::new(vec![
                Unit {
                    name: "p01".into(),
                    role: Role::Person,
                    document: Document::new(vec![Node::heading(1, "題"), para, table]),
                },
                Unit {
                    name: "b01".into(),
                    role: Role::BaselineOutput,
                    document: Document::new(vec![Node::leaf(Kind::Paragraph, "基準である。")]),
                },
            ]),
            derived: Derived {
                scale: Some("{\"ceiling\":[1,2]}".into()),
                vocabulary: Some("{\"文字bigram\":[\"あい\"]}".into()),
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
        dropped.drop_derived();
        let without = write(&dropped);
        let back = read(&without).unwrap();
        assert!(!back.derived.has_scale(), "捨てられている");
        assert_eq!(back.corpus, c.corpus, "原本は残る");
        assert_eq!(back.decided, c.decided, "決めたことも残る");
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
                role: Role::Person,
                document: Document::new(vec![Node::heading(3, "項")]),
            }]),
            ..cassette()
        };
        let back = read(&write(&c)).unwrap();
        assert_eq!(back.corpus.units[0].document.nodes[0].raw_depth, Some(3));
    }

    #[test]
    fn 題材が往復する() {
        // 外せば古い目盛りが黙って使われる。
        let c = cassette();
        let back = read(&write(&c)).unwrap();
        assert_eq!(back.fingerprint.inputs.baseline.topics.len(), 2);
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
