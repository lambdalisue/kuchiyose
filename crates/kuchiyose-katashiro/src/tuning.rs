//! 調整。人が決めたことで、作り直せない原本である（[tuning.json](../../../docs/design/100-katashiro.md#tuningjson)）。
//!
//! 調整は測った値を変えない。 無効にするのは判定と指摘から外すことで、申告は
//! 知らせの出し方と基準の文書の絞り方を変えることである。
//!
//! 欠けを既定で埋めない。 欄の無い `tuning.json` は壊れている——空で通せば、
//! 次に書いたときに作り直せない判断が空として確定する。知らない値も捨てない。

use std::collections::{BTreeMap, BTreeSet};

use crate::json::Value;

/// 無効にする対象の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MuteKind {
    /// 指示できる指標。名前で持つ。
    Metric,
    /// 本人の型。言い回しの文字列で持つ。
    Kata,
    /// 基準の型。言い回しの文字列で持つ。
    BaselineKata,
    /// 基準の語。語彙素で持つ。
    BaselineGoi,
}

impl MuteKind {
    /// 全部。`tuning.json` にはこの全部の欄を書く。
    pub const ALL: [MuteKind; 4] = [
        MuteKind::Metric,
        MuteKind::Kata,
        MuteKind::BaselineKata,
        MuteKind::BaselineGoi,
    ];

    /// `tuning.json` と `--kind` で使う名前。
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            MuteKind::Metric => "指標",
            MuteKind::Kata => "型",
            MuteKind::BaselineKata => "基準の型",
            MuteKind::BaselineGoi => "基準の語",
        }
    }

    /// 名前から引く。知らない名前なら `None`。
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.name() == name)
    }
}

/// 申告した文体。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Register {
    /// 敬体。
    Polite,
    /// 常体。
    Plain,
}

impl Register {
    /// `tuning.json` と `katashiro register` で使う綴り。
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Register::Polite => "polite",
            Register::Plain => "plain",
        }
    }

    /// 綴りから引く。ほかの綴りは受けない。
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "polite" => Some(Register::Polite),
            "plain" => Some(Register::Plain),
            _ => None,
        }
    }
}

/// 調整。
///
/// 本人の側として使うときにだけ効く。基準として渡した形代の調整は読むが、使わない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tuning {
    /// 種類ごとに、1 つずつ無効にした対象。
    ///
    /// 型・基準の型・基準の語は、今の組み合わせで出ていなくても捨てない。
    /// 別の基準と組み合わせたときに出てくれば、そこで効く。
    pub mute: BTreeMap<MuteKind, BTreeSet<String>>,
    /// 種類ごと無効にしたもの。
    ///
    /// 1 つずつ無効にしたものとは別に持つ。 種類ごと戻したときに、1 つずつ
    /// 無効にしたものまで戻らないようにするためである。
    pub mute_kinds: BTreeSet<MuteKind>,
    /// 申告した一人称。`None` なら数えた結果を使う。
    pub first_person: Option<String>,
    /// 申告した文体。`None` なら数えた結果を使う。
    pub register: Option<Register>,
}

impl Default for Tuning {
    /// 何も調整していない。全部の種類の欄を空で持つ。
    fn default() -> Self {
        Self {
            mute: MuteKind::ALL
                .into_iter()
                .map(|k| (k, BTreeSet::new()))
                .collect(),
            mute_kinds: BTreeSet::new(),
            first_person: None,
            register: None,
        }
    }
}

/// `tuning.json` が読めない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuningError(pub String);

impl std::fmt::Display for TuningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tuning.json が読めない: {}", self.0)
    }
}

impl std::error::Error for TuningError {}

impl Tuning {
    /// その対象が無効にされているか。種類ごと無効にしたものも含める。
    #[must_use]
    pub fn is_muted(&self, kind: MuteKind, name: &str) -> bool {
        self.mute_kinds.contains(&kind) || self.mute.get(&kind).is_some_and(|s| s.contains(name))
    }

    /// 1 つずつ無効にした対象。
    #[must_use]
    pub fn muted(&self, kind: MuteKind) -> Vec<&str> {
        self.mute
            .get(&kind)
            .map(|s| s.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// 無効にした指標の名前のうち、`known` に無いものを捨てて返す。
    ///
    /// 作り直すときに登録簿と照らす。 捨てずに残すと、綴りの合わない名前が
    /// いつまでも何も無効にしないまま残る。 型と語は捨てない——どれが出るかは
    /// 組み合わせる基準で変わる。
    pub fn drop_unknown_metrics(&mut self, known: &dyn Fn(&str) -> bool) -> Vec<String> {
        let Some(metrics) = self.mute.get_mut(&MuteKind::Metric) else {
            return Vec::new();
        };
        let dropped: Vec<String> = metrics.iter().filter(|n| !known(n)).cloned().collect();
        metrics.retain(|n| known(n));
        dropped
    }

    /// 1 つを無効にするか戻す。変わったら `true`。
    ///
    /// 種類ごと無効にしたものには触らない。
    pub fn set_muted(&mut self, kind: MuteKind, name: &str, muted: bool) -> bool {
        let set = self.mute.entry(kind).or_default();
        if muted {
            set.insert(name.to_owned())
        } else {
            set.remove(name)
        }
    }

    /// 種類ごと無効にするか戻す。変わったら `true`。
    ///
    /// 1 つずつ無効にしたものには触らない。 種類ごと戻したときに、1 つずつ
    /// 無効にしたものまで戻らないようにするためである。
    pub fn set_kind_muted(&mut self, kind: MuteKind, muted: bool) -> bool {
        if muted {
            self.mute_kinds.insert(kind)
        } else {
            self.mute_kinds.remove(&kind)
        }
    }

    /// JSON にする。空の欄も書く。
    #[must_use]
    pub fn to_json(&self) -> Value {
        let strings = |s: &BTreeSet<String>| Value::Array(s.iter().map(Value::s).collect());
        Value::obj([
            (
                "mute".to_owned(),
                Value::obj(MuteKind::ALL.into_iter().map(|k| {
                    (
                        k.name().to_owned(),
                        self.mute.get(&k).map_or(Value::Array(vec![]), strings),
                    )
                })),
            ),
            (
                "mute_kinds".to_owned(),
                Value::Array(self.mute_kinds.iter().map(|k| Value::s(k.name())).collect()),
            ),
            (
                "first_person".to_owned(),
                self.first_person.as_deref().map_or(Value::Null, Value::s),
            ),
            (
                "register".to_owned(),
                self.register.map_or(Value::Null, |r| Value::s(r.name())),
            ),
        ])
    }

    /// JSON から読む。欄が欠けていても、知らない値があっても断る。
    ///
    /// # Errors
    ///
    /// 欄が欠けている、型が違う、知らない種類や綴りがあるときに断る。
    pub fn from_json(v: &Value) -> Result<Self, TuningError> {
        let err = |s: &str| TuningError(s.to_owned());
        let Value::Object(top) = v else {
            return Err(err("対象でない"));
        };
        for key in top.keys() {
            if !["mute", "mute_kinds", "first_person", "register"].contains(&key.as_str()) {
                return Err(TuningError(format!("知らない欄: {key}")));
            }
        }
        let strings = |v: &Value, what: &str| -> Result<BTreeSet<String>, TuningError> {
            v.as_array()
                .ok_or_else(|| TuningError(format!("{what} が配列でない")))?
                .iter()
                .map(|x| {
                    x.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| TuningError(format!("{what} の中身が文字列でない")))
                })
                .collect()
        };

        let Some(Value::Object(m)) = top.get("mute") else {
            return Err(err("mute が無い"));
        };
        let mut mute = BTreeMap::new();
        for (name, list) in m {
            let kind = MuteKind::from_name(name)
                .ok_or_else(|| TuningError(format!("mute に知らない種類: {name}")))?;
            mute.insert(kind, strings(list, &format!("mute.{name}"))?);
        }
        if let Some(k) = MuteKind::ALL.into_iter().find(|k| !mute.contains_key(k)) {
            return Err(TuningError(format!("mute に {} が無い", k.name())));
        }

        let mute_kinds = strings(
            top.get("mute_kinds")
                .ok_or_else(|| err("mute_kinds が無い"))?,
            "mute_kinds",
        )?
        .into_iter()
        .map(|n| {
            MuteKind::from_name(&n)
                .ok_or_else(|| TuningError(format!("mute_kinds に知らない種類: {n}")))
        })
        .collect::<Result<_, _>>()?;

        let first_person = match top.get("first_person") {
            None => return Err(err("first_person が無い")),
            Some(Value::Null) => None,
            Some(Value::String(s)) if !s.trim().is_empty() => Some(s.clone()),
            Some(_) => return Err(err("first_person が空でない文字列か null でない")),
        };
        let register = match top.get("register") {
            None => return Err(err("register が無い")),
            Some(Value::Null) => None,
            Some(Value::String(s)) => Some(Register::from_name(s).ok_or_else(|| {
                TuningError(format!("register が polite か plain か null でない: {s}"))
            })?),
            Some(_) => return Err(err("register が polite か plain か null でない")),
        };
        Ok(Self {
            mute,
            mute_kinds,
            first_person,
            register,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;

    fn tuned() -> Tuning {
        let mut t = Tuning::default();
        t.mute
            .get_mut(&MuteKind::Metric)
            .unwrap()
            .extend(["強調".to_owned(), "コードブロック".to_owned()]);
        t.mute
            .get_mut(&MuteKind::Kata)
            .unwrap()
            .insert("どうも、〜です。".to_owned());
        t.mute
            .get_mut(&MuteKind::BaselineGoi)
            .unwrap()
            .insert("地味".to_owned());
        t.mute_kinds.insert(MuteKind::BaselineKata);
        t.first_person = Some("僕".into());
        t.register = Some(Register::Plain);
        t
    }

    fn reread(text: &str) -> Result<Tuning, TuningError> {
        Tuning::from_json(&json::parse(text).expect("JSON として読める"))
    }

    #[test]
    fn 無効にして戻すと元に戻り変わったかを返す() {
        let mut t = Tuning::default();
        assert!(t.set_muted(MuteKind::Metric, "強調", true));
        assert!(
            !t.set_muted(MuteKind::Metric, "強調", true),
            "2 度目は変わらない"
        );
        assert!(t.is_muted(MuteKind::Metric, "強調"));
        assert!(t.set_muted(MuteKind::Metric, "強調", false));
        assert!(!t.set_muted(MuteKind::Metric, "強調", false));
        assert_eq!(t, Tuning::default());
    }

    #[test]
    fn 種類ごと戻しても_1_つずつ無効にしたものは戻らない() {
        // 別に持つのはこのためである。
        let mut t = Tuning::default();
        t.set_muted(MuteKind::BaselineKata, "のではなく、", true);
        assert!(t.set_kind_muted(MuteKind::BaselineKata, true));
        assert!(t.is_muted(MuteKind::BaselineKata, "ほかの言い回し"));
        assert!(t.set_kind_muted(MuteKind::BaselineKata, false));
        assert!(!t.is_muted(MuteKind::BaselineKata, "ほかの言い回し"));
        assert!(t.is_muted(MuteKind::BaselineKata, "のではなく、"));
        assert!(!t.set_kind_muted(MuteKind::BaselineKata, false));
    }

    #[test]
    fn 書いて読むと同じものが出る() {
        let t = tuned();
        assert_eq!(reread(&t.to_json().write()), Ok(t));
    }

    #[test]
    fn 調整していなければ全部の欄を空で書く() {
        // 空の値は正しい状態である。 欄そのものが無いのとは違う。
        let text = Tuning::default().to_json().write();
        assert_eq!(
            text,
            r#"{"first_person":null,"mute":{"型":[],"基準の型":[],"基準の語":[],"指標":[]},"mute_kinds":[],"register":null}"#
        );
        assert_eq!(reread(&text), Ok(Tuning::default()));
    }

    #[test]
    fn 欄が欠けていたら断る() {
        let text = tuned().to_json().write();
        for key in [
            "\"mute\"",
            "\"mute_kinds\"",
            "\"first_person\"",
            "\"register\"",
        ] {
            let broken = text.replacen(key, "\"消した\"", 1);
            assert!(reread(&broken).is_err(), "{key} が欠けても読めた");
        }
    }

    #[test]
    fn 無効にする種類の欄が欠けていたら断る() {
        let text = Tuning::default()
            .to_json()
            .write()
            .replace("\"基準の語\":[],", "");
        assert!(reread(&text).is_err());
    }

    #[test]
    fn 知らない文体の綴りは捨てずに断る() {
        // 捨てれば、申告したはずの文体が数えた結果に戻る。
        let text = tuned().to_json().write().replace("\"plain\"", "\"dearu\"");
        let e = reread(&text).unwrap_err();
        assert!(e.to_string().contains("dearu"), "{e}");
    }

    #[test]
    fn 知らない種類は断る() {
        let text = tuned().to_json().write().replace(
            "\"mute_kinds\":[\"基準の型\"]",
            "\"mute_kinds\":[\"機械の型\"]",
        );
        assert!(reread(&text).is_err());
    }

    #[test]
    fn 種類ごと無効にしたものは_1_つずつ無効にしたものとは別に効く() {
        let t = tuned();
        assert!(t.is_muted(MuteKind::BaselineKata, "どれでも"));
        assert!(t.is_muted(MuteKind::Metric, "強調"));
        assert!(!t.is_muted(MuteKind::Metric, "三点リーダ"));
        assert!(
            t.muted(MuteKind::BaselineKata).is_empty(),
            "1 つずつのほうは空のまま"
        );
    }

    #[test]
    fn 登録簿に無い指標の名前だけを捨てる() {
        let mut t = tuned();
        let dropped = t.drop_unknown_metrics(&|n| n == "強調");
        assert_eq!(dropped, vec!["コードブロック"]);
        assert_eq!(t.muted(MuteKind::Metric), vec!["強調"]);
        assert_eq!(
            t.muted(MuteKind::Kata),
            vec!["どうも、〜です。"],
            "型は捨てない"
        );
    }
}
