//! 目盛りを派生物として書き出し、読み戻す。
//!
//! ここが繋ぎ目である。 入れ物はカセット、組み立ては目盛り、繋ぐのはこちら——
//! 検めが目盛りを作り直せる経路を作らないために、この 3 つを分けている。
//!
//! 読み戻したものが元と一致することを試験が確かめる。 一致しなければ、
//! 保存したカセットで測った値は、作ったときの値と違う。

use kakiburi_cassette::json::Value;
use kakiburi_scale::band::{Band, Ends};
use kakiburi_scale::calibrate::{Calibration, Weights};
use kakiburi_scale::humanness::HumannessScale;
use kakiburi_scale::vocabulary::{Frozen, FrozenSet};
use kakiburi_scale::Scale;

/// 数の並び。
fn numbers(v: &[f64]) -> Value {
    Value::Array(v.iter().copied().map(Value::Number).collect())
}

/// 文字列の並び。
fn strings(v: &[String]) -> Value {
    Value::Array(v.iter().map(Value::s).collect())
}

fn read_numbers(v: Option<&Value>) -> Option<Vec<f64>> {
    v?.as_array()?.iter().map(Value::as_f64).collect()
}

fn read_strings(v: Option<&Value>) -> Option<Vec<String>> {
    Some(
        v?.as_array()?
            .iter()
            .map(|x| x.as_str().unwrap_or_default().to_owned())
            .collect(),
    )
}

/// 重み。平均と標準偏差も書く。
///
/// 落とせば、検めが標準化なしの値に標準化ずみの重みを当てることになり、
/// 値だけが静かに変わる。エラーにはならない。
fn weights(w: &Weights) -> Value {
    Value::obj([
        ("切片".to_owned(), Value::Number(w.intercept())),
        ("傾き".to_owned(), numbers(w.slopes())),
        ("平均".to_owned(), numbers(w.centers())),
        ("標準偏差".to_owned(), numbers(w.scales())),
    ])
}

fn read_weights(v: &Value) -> Option<Weights> {
    // 欠けていたら読まない。 平均 0・標準偏差 1 で補うと、標準化して当てはめた
    // 重みを標準化なしの値に当てることになる。
    //
    // 欄があることだけでは足りない。 長さが揃わなければ足りない次元だけが
    // 埋められるので、`restore` が中身まで検める。
    Weights::restore(
        v.get("切片")?.as_f64()?,
        read_numbers(v.get("傾き"))?,
        read_numbers(v.get("平均"))?,
        read_numbers(v.get("標準偏差"))?,
    )
}

fn frozen(f: &Frozen) -> Value {
    Value::obj([
        ("次元".to_owned(), strings(f.dims())),
        ("平均".to_owned(), numbers(f.mean())),
        ("標準偏差".to_owned(), numbers(f.sd())),
    ])
}

fn read_frozen(v: &Value) -> Option<Frozen> {
    Frozen::restore(
        read_strings(v.get("次元"))?,
        read_numbers(v.get("平均"))?,
        read_numbers(v.get("標準偏差"))?,
    )
}

fn ends(e: Ends) -> Value {
    Value::obj([
        ("下端".to_owned(), Value::Number(e.low)),
        ("上端".to_owned(), Value::Number(e.high)),
    ])
}

fn read_ends(v: &Value) -> Option<Ends> {
    Some(Ends {
        low: v.get("下端")?.as_f64()?,
        high: v.get("上端")?.as_f64()?,
    })
}

fn band(b: Band) -> Value {
    Value::obj([
        ("天井".to_owned(), ends(b.ceiling)),
        ("床".to_owned(), ends(b.floor)),
    ])
}

fn read_band(v: &Value) -> Option<Band> {
    Some(Band {
        ceiling: read_ends(v.get("天井")?)?,
        floor: read_ends(v.get("床")?)?,
    })
}

/// 書き出す。
#[must_use]
pub fn write(s: &Scale) -> String {
    let frozen_v = Value::Array(
        s.frozen
            .iter()
            .map(|(name, set)| {
                Value::obj([
                    ("系統".to_owned(), Value::s(name)),
                    (
                        "部分".to_owned(),
                        Value::Array(set.parts().iter().map(frozen).collect()),
                    ),
                ])
            })
            .collect(),
    );
    let calibration = Value::obj([
        ("系統".to_owned(), strings(s.calibration.systems())),
        (
            "系統ごと".to_owned(),
            Value::Array(s.calibration.per_system().iter().map(weights).collect()),
        ),
        ("合算".to_owned(), weights(s.calibration.fusion())),
    ]);
    let humanness = Value::obj([
        (
            "次元ごと".to_owned(),
            Value::Array(s.humanness.per_dim().iter().map(weights).collect()),
        ),
        ("合算".to_owned(), weights(s.humanness.fusion())),
    ]);
    Value::obj([
        ("語彙".to_owned(), frozen_v),
        ("較正".to_owned(), calibration),
        ("帯".to_owned(), band(s.band)),
        // 割りは 4 つとも書く。 相手集合だけでは、どこで割れたかが読めない。
        (
            "割り".to_owned(),
            Value::obj([
                (
                    "本人の相手集合".to_owned(),
                    strings(&s.selection.person_partners),
                ),
                (
                    "本人の測る分".to_owned(),
                    strings(&s.selection.person_points),
                ),
                (
                    "基準の較正分".to_owned(),
                    strings(&s.selection.baseline_partners),
                ),
                (
                    "基準の床の点".to_owned(),
                    strings(&s.selection.baseline_points),
                ),
            ]),
        ),
        // 実例を書く。 数値と向きだけを渡された側は、その人の文章を
        // 自分で読みに行くことになる——読みに行く先がもう無い。
        (
            "実例".to_owned(),
            Value::Array(
                s.examples
                    .iter()
                    .map(|(system, dim, found)| {
                        Value::obj([
                            ("系統".to_owned(), Value::s(system)),
                            ("次元".to_owned(), Value::s(dim)),
                            ("例".to_owned(), strings(found)),
                        ])
                    })
                    .collect(),
            ),
        ),
        // 相手集合のベクトルを書く。 カセットは本文を持たないので、
        // ここに無ければ検めるときに照合値を出せない。
        // 並びを保つ。 対象にすると鍵の順で並び替わり、系統の並びが
        // 語彙の並びと食い違う。
        (
            "相手集合のベクトル".to_owned(),
            Value::Array(
                s.partner_vectors
                    .iter()
                    .map(|(unit, parts)| {
                        Value::obj([
                            ("単位".to_owned(), Value::s(unit)),
                            (
                                "系統".to_owned(),
                                Value::Array(
                                    parts
                                        .iter()
                                        .map(|(system, v)| {
                                            Value::obj([
                                                ("名前".to_owned(), Value::s(system)),
                                                (
                                                    "値".to_owned(),
                                                    Value::Array(
                                                        v.iter()
                                                            .copied()
                                                            .map(Value::Number)
                                                            .collect(),
                                                    ),
                                                ),
                                            ])
                                        })
                                        .collect(),
                                ),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("人らしさ".to_owned(), humanness),
        ("人らしさの帯".to_owned(), band(s.humanness_band)),
        (
            // 効く量を数える相手。 正反対を指す直し方が同時に出たとき、
            // どちらが勝つかは動く量でしか言えない。
            "人らしさの代表値".to_owned(),
            Value::obj(
                s.humanness_target
                    .iter()
                    .map(|(n, v)| (n.clone(), Value::Number(*v))),
            ),
        ),
        (
            // 直し方に載せる言い回し。 数値と向きだけでは、受け取った側は
            // 自分ででっち上げた定型句を挿し込むことになる。
            "言い回し".to_owned(),
            Value::Array(s.phrases.iter().map(Value::s).collect()),
        ),
        (
            // 一人称は閉じた集合なので、選ばれなかったことがそのまま癖になる。
            "一人称".to_owned(),
            Value::Array(
                s.first_person
                    .iter()
                    .map(|(name, rate)| {
                        Value::Array(vec![Value::s(name), Value::Number(*rate)])
                    })
                    .collect(),
            ),
        ),
        (
            // 書き出しに何を置くかは、密度でも語の位置でも言えない。
            "書き出し".to_owned(),
            Value::Array(
                s.opening
                    .iter()
                    .map(|(kind, rate)| {
                        Value::Array(vec![Value::s(kind), Value::Number(*rate)])
                    })
                    .collect(),
            ),
        ),
        (
            // 繰り返せと言うなら、上限も渡す。
            "言い回しの上限".to_owned(),
            Value::Array(
                s.phrase_ceilings
                    .iter()
                    .map(|(p, c)| Value::Array(vec![Value::s(p), Value::Number(*c)]))
                    .collect(),
            ),
        ),
        (
            // その人の型。 コーパスから見つけたものなので、カセットに残さないと
            // 検めのたびに素材を読み直すことになる。
            // コーパスから見つけた語。 検めるときも同じ辞書で割らなければ、
            // 比べたものに意味が無い。
            "語".to_owned(),
            Value::Array(
                s.lexicon
                    .pairs()
                    .iter()
                    .map(|(a, b)| Value::Array(vec![Value::s(a), Value::s(b)]))
                    .collect(),
            ),
        ),
        (
            "型".to_owned(),
            Value::Array(
                s.katas
                    .iter()
                    .map(|k| {
                        Value::obj([
                            ("並び".to_owned(), Value::s(&k.text)),
                            ("出現割合".to_owned(), Value::Number(k.rate)),
                            ("位置".to_owned(), Value::Number(k.at)),
                            ("ばらつき".to_owned(), Value::Number(k.spread)),
                            ("相手側".to_owned(), Value::Number(k.base)),
                            ("上限".to_owned(), Value::Number(k.ceiling)),
                            (
                                // 穴あきなら、後ろの固定部を持つ。 間は書き手が埋める。
                                "後ろ".to_owned(),
                                k.tail.as_ref().map_or(Value::Null, Value::s),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        // 役を入れ替えた側も持つ。 機械の言い回しが残っていることは、
        // 本人の型が入っていないことからは言えない。
        (
            "機械の型".to_owned(),
            Value::Array(
                s.machine_katas
                    .iter()
                    .map(|k| {
                        Value::obj([
                            ("並び".to_owned(), Value::s(&k.text)),
                            ("出現割合".to_owned(), Value::Number(k.rate)),
                            ("位置".to_owned(), Value::Number(k.at)),
                            ("ばらつき".to_owned(), Value::Number(k.spread)),
                            ("相手側".to_owned(), Value::Number(k.base)),
                            ("上限".to_owned(), Value::Number(k.ceiling)),
                            (
                                "後ろ".to_owned(),
                                k.tail.as_ref().map_or(Value::Null, Value::s),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        // 並びで割れた癖を、語彙素でも持つ。 同じ癖が語形ごとに割れると、
        // どの綴りも床を割って機械の型に出てこない。
        (
            "機械の語".to_owned(),
            Value::Array(
                s.machine_gois
                    .iter()
                    .map(|g| {
                        Value::obj([
                            ("語彙素".to_owned(), Value::s(&g.text)),
                            ("出現割合".to_owned(), Value::Number(g.rate)),
                            ("相手側".to_owned(), Value::Number(g.base)),
                            // 置き換える先も持つ。 「別の言い方にする」だけでは、
                            // 受け取った側が道具の外で語を探すことになる。
                            (
                                "本人の語".to_owned(),
                                Value::Array(g.theirs.iter().map(Value::s).collect()),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
    .write()
}

/// 語の配列を読み戻す。無くてもよい——持たない版のカセットは指摘が 1 本減る。
fn gois_at(v: &Value, key: &str) -> Vec<kakiburi_scale::assemble::Goi> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    Some(kakiburi_scale::assemble::Goi {
                        text: x.get("語彙素")?.as_str()?.to_owned(),
                        rate: x.get("出現割合")?.as_f64()?,
                        base: x.get("相手側").and_then(Value::as_f64).unwrap_or(0.0),
                        theirs: x
                            .get("本人の語")
                            .and_then(Value::as_array)
                            .map(|a| {
                                a.iter()
                                    .filter_map(|w| w.as_str().map(str::to_owned))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 型の配列を読み戻す。無くてもよい——持たない版のカセットは指摘が 1 本減る。
fn katas_at(v: &Value, key: &str) -> Vec<kakiburi_scale::assemble::Kata> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    Some(kakiburi_scale::assemble::Kata {
                        text: x.get("並び")?.as_str()?.to_owned(),
                        rate: x.get("出現割合")?.as_f64()?,
                        at: x.get("位置")?.as_f64()?,
                        spread: x.get("ばらつき").and_then(Value::as_f64).unwrap_or(1.0),
                        base: x.get("相手側").and_then(Value::as_f64).unwrap_or(0.0),
                        ceiling: x.get("上限").and_then(Value::as_f64).unwrap_or(0.0),
                        tail: x.get("後ろ").and_then(Value::as_str).map(str::to_owned),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 読み戻す。1 つでも欠けたら組み立てない。
///
/// 半端に組み立てれば、次元の意味がずれたまま距離を取ることになる。
#[must_use]
pub fn read(text: &str) -> Option<Scale> {
    let v = kakiburi_cassette::json::parse(text).ok()?;
    let mut frozen = Vec::new();
    for e in v.get("語彙")?.as_array()? {
        let name = e.get("系統")?.as_str()?.to_owned();
        let parts: Option<Vec<Frozen>> =
            e.get("部分")?.as_array()?.iter().map(read_frozen).collect();
        frozen.push((name, FrozenSet::from_parts(parts?)));
    }
    let c = v.get("較正")?;
    let per_system: Option<Vec<Weights>> = c
        .get("系統ごと")?
        .as_array()?
        .iter()
        .map(read_weights)
        .collect();
    let calibration = Calibration::restore(
        read_strings(c.get("系統"))?,
        per_system?,
        read_weights(c.get("合算")?)?,
    )?;
    let h = v.get("人らしさ")?;
    let per_dim: Option<Vec<Weights>> = h
        .get("次元ごと")?
        .as_array()?
        .iter()
        .map(read_weights)
        .collect();
    let humanness = HumannessScale::restore(per_dim?, read_weights(h.get("合算")?)?)?;
    Some(Scale {
        frozen,
        calibration,
        band: read_band(v.get("帯")?)?,
        selection: {
            let s = v.get("割り")?;
            kakiburi_scale::Selection {
                person_partners: read_strings(s.get("本人の相手集合"))?,
                person_points: read_strings(s.get("本人の測る分"))?,
                baseline_partners: read_strings(s.get("基準の較正分"))?,
                baseline_points: read_strings(s.get("基準の床の点"))?,
            }
        },
        // 無くてもよい。 実例を持たない目盛りは、直し方に例が付かない
        // だけで判定は変わらない。
        examples: v
            .get("実例")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| {
                        Some((
                            x.get("系統")?.as_str()?.to_owned(),
                            x.get("次元")?.as_str()?.to_owned(),
                            read_strings(x.get("例"))?,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        // 欠けていたら読まない。 空で通せば照合値が出なくなり、
        // 目盛りがあるのに判定できないが返る——壊れていることが正常に見える。
        partner_vectors: v
            .get("相手集合のベクトル")?
            .as_array()?
            .iter()
            .map(|u| {
                Some((
                    u.get("単位")?.as_str()?.to_owned(),
                    u.get("系統")?
                        .as_array()?
                        .iter()
                        .map(|s| {
                            Some((
                                s.get("名前")?.as_str()?.to_owned(),
                                s.get("値")?
                                    .as_array()?
                                    .iter()
                                    .map(Value::as_f64)
                                    .collect::<Option<Vec<f64>>>()?,
                            ))
                        })
                        .collect::<Option<Vec<_>>>()?,
                ))
            })
            .collect::<Option<Vec<_>>>()?,
        humanness,
        humanness_band: read_band(v.get("人らしさの帯")?)?,
        // 無くてもよい。 持たない版のカセットは、直し方の並びが粗くなる
        // だけで判定は変わらない。
        humanness_target: {
            // 指標の並びから引く。 欄の並びに頼らない。
            let o = v.get("人らしさの代表値");
            kakiburi_metrics::humanness::Metric::ALL
                .into_iter()
                .filter_map(|m| {
                    let n = m.name();
                    Some((n.to_owned(), o.and_then(|x| x.get(n))?.as_f64()?))
                })
                .collect()
        },
        // 無くてもよい。 言い回しを持たない版のカセットは、直し方が短くなる
        // だけで判定は変わらない。
        // 無くてもよい。 語を持たない版のカセットは、辞書どおりに割る。
        lexicon: kakiburi_metrics::lexicon::Lexicon::from_pairs(
            v.get("語")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|x| {
                            let p = x.as_array()?;
                            Some((
                                p.first()?.as_str()?.to_owned(),
                                p.get(1)?.as_str()?.to_owned(),
                            ))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        ),
        katas: katas_at(&v, "型"),
        machine_katas: katas_at(&v, "機械の型"),
        machine_gois: gois_at(&v, "機械の語"),
        phrases: v
            .get("言い回し")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        // 無くてもよい。 書き出しを持たない版のカセットは、指摘が 1 本減るだけである。
        opening: v
            .get("書き出し")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| {
                        let p = x.as_array()?;
                        Some((p.first()?.as_str()?.to_owned(), p.get(1)?.as_f64()?))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        // 無くてもよい。 一人称を持たない版のカセットは、指摘が 1 本減るだけである。
        first_person: v
            .get("一人称")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| {
                        let p = x.as_array()?;
                        Some((p.first()?.as_str()?.to_owned(), p.get(1)?.as_f64()?))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        // 無くてもよい。 上限を持たない版のカセットは、指摘が 1 本減るだけである。
        phrase_ceilings: v
            .get("言い回しの上限")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| {
                        let p = x.as_array()?;
                        Some((p.first()?.as_str()?.to_owned(), p.get(1)?.as_f64()?))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;

    #[test]
    fn 書き出して読み戻すと同じものになる() {
        // 一致しなければ、保存したカセットで測った値は作ったときの値と違う。
        let s = fixture::scale();
        let text = write(&s);
        let back = read(&text).expect("読み戻せる");
        assert_eq!(back, s);
    }

    #[test]
    fn 割りは_4_つとも残る() {
        // 相手集合だけでは、どこで割れたかが読めない。単位名の昇順で取るので、
        // 名前に年や媒体が入っていれば境目で分かれる——出さなければ気付けない。
        let s = fixture::scale();
        let back = read(&write(&s)).expect("読み戻せる");
        for names in [
            &back.selection.person_partners,
            &back.selection.person_points,
            &back.selection.baseline_partners,
            &back.selection.baseline_points,
        ] {
            assert!(!names.is_empty(), "割りが空になっている");
        }
        assert_eq!(back.selection, s.selection);
    }

    #[test]
    fn 欠けていれば組み立てない() {
        // 半端に組み立てれば、次元の意味がずれたまま距離を取る。
        let text = write(&fixture::scale());
        let broken = text.replace("\"帯\"", "\"おび\"");
        assert!(read(&broken).is_none());
    }

    #[test]
    fn 標準化が半端な重みは組み立てない() {
        // 足りない次元は「平均 0・標準偏差 1」で埋まる。 つまりその次元だけ
        // 標準化が外れた値が、エラーにならずに出る——仕様が名指しで禁じている経路。
        let s = fixture::scale();
        let text = write(&s);
        // 平均の配列を 1 つ短くする。
        let broken = text.replacen("\"平均\":[", "\"平均\":[0,", 1);
        assert!(read(&broken).is_none(), "長さが揃わなければ読まない");
    }

    #[test]
    fn 標準偏差_0_の重みは組み立てない() {
        // 割れば無限大か NaN になり、そこから先の比較がすべて壊れる。
        // 広がり 0 の次元は 1 として持つのが仕様なので、0 は壊れている印である。
        let bad = kakiburi_scale::calibrate::Weights::restore(
            0.0,
            vec![1.0, 1.0],
            vec![0.0, 0.0],
            vec![1.0, 0.0],
        );
        assert!(bad.is_none());
        let nan =
            kakiburi_scale::calibrate::Weights::restore(f64::NAN, vec![1.0], vec![0.0], vec![1.0]);
        assert!(nan.is_none(), "有限でない切片も断る");
    }

    #[test]
    fn 読めない文字列は組み立てない() {
        assert!(read("{").is_none());
        assert!(read("{}").is_none());
    }
}
