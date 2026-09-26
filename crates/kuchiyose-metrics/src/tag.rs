//! 札。指標の定義ファイルの 3 行目。
//!
//! 位置ではなく種別が形を決める。 読む側は種別を見てから残りを解釈する。
//! 持たない位置は詰めるので、`人らしさ` の札は 2 欄になる。

use crate::system::{Layer, System};

/// 分類。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    /// 記号。
    Symbol,
    /// 表記。
    Orthography,
    /// 語。
    Word,
    /// 品詞。
    Pos,
    /// 構造。
    Structure,
    /// 長さ。
    Length,
    /// 埋め込み。
    Embedding,
}

/// 向き。どちら側に外れたら指摘するか。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// 上限。多すぎる側だけを見る。
    Upper,
    /// 下限。少なすぎる側だけを見る。
    Lower,
    /// 両側。直し方を上下の両方について書かなければならない。
    Both,
}

/// 札。種別ごとに形が違う。
///
/// `Eq` は持たない——検査が線を実数で持つためである。
#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    /// 指示できる指標。5 欄。
    Directive {
        /// 名指しした系統。名指しできなければ `None`——層 3 になる。
        system: Option<System>,
        /// 分類。
        class: Class,
        /// 向き。
        direction: Direction,
        /// 単位。
        unit: String,
    },
    /// 照合の系統。2 欄。
    Matching {
        /// 分類。
        class: Class,
    },
    /// 人らしさ。2 欄。層を持たない。
    Humanness {
        /// 向き。
        direction: Direction,
    },
    /// 検査。4 欄。層を持たない。
    ///
    /// 比べる先は書き手ではなく日本語なので、線を定義ファイルが持つ。
    /// 形代の有無で判定が変わらない。
    Inspection {
        /// 向き。
        direction: Direction,
        /// 線。この向きに超えたら、日本語として成立していない。
        limit: f64,
        /// 単位。
        unit: String,
    },
}

/// 札が読めない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagError {
    /// 種別が読めない。
    UnknownKind(String),
    /// 欄の数が種別と合わない。
    WrongArity {
        /// 種別。
        kind: String,
        /// 要る欄の数。
        want: usize,
        /// あった欄の数。
        got: usize,
    },
    /// 表に無い系統。
    UnknownSystem(String),
    /// 表に無い分類。
    UnknownClass(String),
    /// 表に無い向き。
    UnknownDirection(String),
    /// 線が数として読めない。
    UnreadableLimit(String),
}

impl std::fmt::Display for TagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TagError::UnknownKind(s) => write!(f, "種別が読めない: {s}"),
            TagError::WrongArity { kind, want, got } => {
                write!(f, "{kind} は {want} 欄だが {got} 欄あった")
            }
            TagError::UnknownSystem(s) => write!(f, "表に無い系統: {s}"),
            TagError::UnknownClass(s) => write!(f, "表に無い分類: {s}"),
            TagError::UnknownDirection(s) => write!(f, "表に無い向き: {s}"),
            TagError::UnreadableLimit(s) => write!(f, "線が数として読めない: {s}"),
        }
    }
}

impl std::error::Error for TagError {}

impl Class {
    fn from_name(name: &str) -> Result<Self, TagError> {
        match name {
            "記号" => Ok(Class::Symbol),
            "表記" => Ok(Class::Orthography),
            "語" => Ok(Class::Word),
            "品詞" => Ok(Class::Pos),
            "構造" => Ok(Class::Structure),
            "長さ" => Ok(Class::Length),
            "埋め込み" => Ok(Class::Embedding),
            _ => Err(TagError::UnknownClass(name.to_owned())),
        }
    }
}

impl Direction {
    fn from_name(name: &str) -> Result<Self, TagError> {
        match name {
            "上限" => Ok(Direction::Upper),
            "下限" => Ok(Direction::Lower),
            "両側" => Ok(Direction::Both),
            _ => Err(TagError::UnknownDirection(name.to_owned())),
        }
    }

    /// 直し方をいくつ書かなければならないか。
    ///
    /// 片方しか書けないなら、それは `両側` ではない。 片方だけだと、直す側は
    /// 指摘を消すために削る方へ向かう。
    #[must_use]
    pub fn remedies_required(self) -> usize {
        match self {
            Direction::Upper | Direction::Lower => 1,
            Direction::Both => 2,
        }
    }
}

impl Tag {
    /// 札の行を読む。末尾の句点は落とす。
    pub fn parse(line: impl AsRef<str>) -> Result<Self, TagError> {
        let line = line.as_ref().trim().trim_end_matches('。');
        let fields: Vec<&str> = line.split('/').map(str::trim).collect();
        let kind = fields.first().copied().unwrap_or("");
        match kind {
            "指示" => {
                if fields.len() != 5 {
                    return Err(TagError::WrongArity {
                        kind: kind.to_owned(),
                        want: 5,
                        got: fields.len(),
                    });
                }
                // `なし` は「系統を名指しできない」の印である。表に無い名前とは違う。
                let system = if fields[1] == "なし" {
                    None
                } else {
                    Some(
                        System::from_name(fields[1])
                            .ok_or_else(|| TagError::UnknownSystem(fields[1].to_owned()))?,
                    )
                };
                Ok(Tag::Directive {
                    system,
                    class: Class::from_name(fields[2])?,
                    direction: Direction::from_name(fields[3])?,
                    unit: fields[4].to_owned(),
                })
            }
            "照合" => {
                if fields.len() != 2 {
                    return Err(TagError::WrongArity {
                        kind: kind.to_owned(),
                        want: 2,
                        got: fields.len(),
                    });
                }
                Ok(Tag::Matching {
                    class: Class::from_name(fields[1])?,
                })
            }
            "人らしさ" => {
                if fields.len() != 2 {
                    return Err(TagError::WrongArity {
                        kind: kind.to_owned(),
                        want: 2,
                        got: fields.len(),
                    });
                }
                Ok(Tag::Humanness {
                    direction: Direction::from_name(fields[1])?,
                })
            }
            "検査" => {
                if fields.len() != 4 {
                    return Err(TagError::WrongArity {
                        kind: kind.to_owned(),
                        want: 4,
                        got: fields.len(),
                    });
                }
                Ok(Tag::Inspection {
                    direction: Direction::from_name(fields[1])?,
                    limit: fields[2]
                        .parse()
                        .map_err(|_| TagError::UnreadableLimit(fields[2].to_owned()))?,
                    unit: fields[3].to_owned(),
                })
            }
            other => Err(TagError::UnknownKind(other.to_owned())),
        }
    }

    /// 層。系統から引く。札には書かない。
    ///
    /// 人らしさは層を持たない——層が効く 2 か所のどちらにも入らない。
    #[must_use]
    pub fn layer(&self) -> Option<Layer> {
        match self {
            // 系統を名指しできなければ層 3。系統の裏付けを継ぐ足場が無い。
            Tag::Directive { system, .. } => Some(system.map_or(Layer::Three, System::layer)),
            Tag::Matching { .. } | Tag::Humanness { .. } | Tag::Inspection { .. } => None,
        }
    }

    /// 向き。照合は持たない。
    #[must_use]
    pub fn direction(&self) -> Option<Direction> {
        match self {
            Tag::Directive { direction, .. }
            | Tag::Humanness { direction }
            | Tag::Inspection { direction, .. } => Some(*direction),
            Tag::Matching { .. } => None,
        }
    }

    /// 下端を使った割合で見るか。
    ///
    /// 単位で決まる。指標ごとに書かない。
    ///
    /// | 単位 | どう見るか |
    /// | --- | --- |
    /// | 密度（`〜あたり`）・個数・その現象の出現に対する割合（`割合`） | 使った割合 |
    /// | 無次元・常に値を持つ割合（`〜に対する割合`） | 幅の下端 |
    ///
    /// 変動係数は測れるかぎり必ず値を持ち、段落に対する割合も 0 が正当な値になる。
    /// そちらを使った割合で見れば、0 が「使わなかった」と読まれる。
    #[must_use]
    pub fn lower_by_appearance(&self) -> bool {
        let Tag::Directive { unit, .. } = self else {
            return false;
        };
        // 「〜に対する割合」が先である。「割合」で当てると、分母を持つ側まで
        // 使った割合になる。
        if unit.contains("に対する割合") || unit.contains("無次元") {
            return false;
        }
        unit.contains("あたり") || unit.contains("個") || unit.contains("割合")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 下端の見方は単位で決まる() {
        // 密度は使った割合で見る。幅で見れば下限が永久に効かない。
        let density = Tag::parse("指示 / なし / 語 / 上限 / 日本語 1,000 字あたり。").unwrap();
        assert!(density.lower_by_appearance());
        // その現象の出現に対する割合も、現象が無ければ 0 になる。
        let of_event = Tag::parse("指示 / 表記 / 表記 / 両側 / 割合。").unwrap();
        assert!(of_event.lower_by_appearance());
    }

    #[test]
    fn 常に値を持つ割合は幅で見る() {
        // 0 が「使わなかった」を意味しない。
        let cv = Tag::parse("指示 / 長さ / 長さ / 下限 / 無次元。").unwrap();
        assert!(!cv.lower_by_appearance());
        let of_paragraph = Tag::parse("指示 / 長さ / 長さ / 両側 / 段落に対する割合。").unwrap();
        assert!(
            !of_paragraph.lower_by_appearance(),
            "「割合」で当てると分母を持つ側まで拾う"
        );
    }

    #[test]
    fn 照合と人らしさは下端の見方を持たない() {
        assert!(!Tag::parse("照合 / 記号。").unwrap().lower_by_appearance());
        assert!(!Tag::parse("人らしさ / 下限。")
            .unwrap()
            .lower_by_appearance());
    }

    #[test]
    fn 指示の札は_5_欄() {
        let t = Tag::parse("指示 / 読点の打ち方 / 記号 / 両側 / 割合。").unwrap();
        assert_eq!(
            t,
            Tag::Directive {
                system: Some(System::Comma),
                class: Class::Symbol,
                direction: Direction::Both,
                unit: "割合".into(),
            }
        );
    }

    #[test]
    fn 照合の札は_2_欄() {
        assert_eq!(
            Tag::parse("照合 / 表記。").unwrap(),
            Tag::Matching {
                class: Class::Orthography
            }
        );
    }

    #[test]
    fn 人らしさの札は_2_欄() {
        assert_eq!(
            Tag::parse("人らしさ / 上限。").unwrap(),
            Tag::Humanness {
                direction: Direction::Upper
            }
        );
    }

    #[test]
    fn 欄の数が合わなければ読めない() {
        // 種別が形を決める。位置で読むと、詰めた欄がずれる。
        assert!(matches!(
            Tag::parse("指示 / 記号 / 両側 / 割合。"),
            Err(TagError::WrongArity {
                want: 5,
                got: 4,
                ..
            })
        ));
        assert!(matches!(
            Tag::parse("照合 / 表記 / 両側。"),
            Err(TagError::WrongArity {
                want: 2,
                got: 3,
                ..
            })
        ));
    }

    #[test]
    fn なしは層_3_の印である() {
        let t = Tag::parse("指示 / なし / 語 / 上限 / 日本語 1,000 字あたり。").unwrap();
        assert_eq!(t.layer(), Some(Layer::Three));
    }

    #[test]
    fn 系統を名指しすれば層を継ぐ() {
        let t = Tag::parse("指示 / 文字種 / 表記 / 両側 / 割合。").unwrap();
        assert_eq!(t.layer(), Some(Layer::One), "文字種は層 1");
        let t = Tag::parse("指示 / 構造 / 構造 / 上限 / 個数。").unwrap();
        assert_eq!(t.layer(), Some(Layer::Two), "構造は層 2");
    }

    #[test]
    fn 人らしさは層を持たない() {
        let t = Tag::parse("人らしさ / 下限。").unwrap();
        assert_eq!(t.layer(), None);
    }

    #[test]
    fn 照合も層を札から引かない() {
        // 系統そのものなので、System::layer が持つ。
        let t = Tag::parse("照合 / 記号。").unwrap();
        assert_eq!(t.layer(), None);
    }

    #[test]
    fn 表に無い綴りは断る() {
        assert!(matches!(
            Tag::parse("指示 / 読点 / 記号 / 両側 / 割合。"),
            Err(TagError::UnknownSystem(_))
        ));
        assert!(matches!(
            Tag::parse("照合 / きごう。"),
            Err(TagError::UnknownClass(_))
        ));
        assert!(matches!(
            Tag::parse("人らしさ / 多い。"),
            Err(TagError::UnknownDirection(_))
        ));
        assert!(matches!(
            Tag::parse("指標 / 記号。"),
            Err(TagError::UnknownKind(_))
        ));
    }

    #[test]
    fn 両側は直し方を_2_つ要る() {
        assert_eq!(Direction::Both.remedies_required(), 2);
        assert_eq!(Direction::Upper.remedies_required(), 1);
        assert_eq!(Direction::Lower.remedies_required(), 1);
    }

    #[test]
    fn 句点は札の一部ではない() {
        assert_eq!(
            Tag::parse("照合 / 表記").unwrap(),
            Tag::parse("照合 / 表記。").unwrap()
        );
    }
}
