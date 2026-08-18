//! 単位を 2 つに割る。
//!
//! <strong>交差検証ではなく固定の 2 分割にする。</strong> 交差検証は束ごとに違う重みを作るので、
//! 新しい文書をどの重みで採点するかが決まらない。<strong>1 組しか作らない。</strong>

/// 単位。名前と、判定に使うものが測れたかどうか。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    /// 単位の名前。<strong>ファイルの名前ではない</strong>——束ねた単位では食い違う。
    pub name: String,
    /// 判定に使う系統が全部測れたか。
    pub systems_measured: bool,
    /// 人らしさの 4 指標が全部測れたか。
    ///
    /// <strong>忘れると人らしさ側だけが痩せる。</strong> 4 指標の除外は 5 系統より厳しいので、
    /// 系統は測れるのに人らしさは測れない単位が普通に出る。
    pub humanness_measured: bool,
}

impl Unit {
    /// 帯に使える単位か。<strong>両方測れていなければ入れない。</strong>
    #[must_use]
    pub fn usable(&self) -> bool {
        self.systems_measured && self.humanness_measured
    }
}

/// 片側に要る本数。
pub const PER_SIDE: usize = 5;

/// 1 つの側に要る単位の下限。相手集合 5 ＋ 測る分 5。
pub const UNITS_FLOOR: usize = PER_SIDE * 2;

/// 割った結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    /// 相手集合。<strong>あらゆる照合の相手。</strong> 天井も床も検めも、必ずこれと対にする。
    pub partners: Vec<Unit>,
    /// 測る分。天井の点になる。
    pub points: Vec<Unit>,
}

/// 割れない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitError {
    /// 条件を満たした単位が下限に届かない。
    NotEnough {
        /// 使える単位の数。
        usable: usize,
        /// 要る数。
        need: usize,
    },
}

impl std::fmt::Display for SplitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SplitError::NotEnough { usable, need } => {
                write!(f, "測れた単位が {usable} 本で、{need} 本に届かない")
            }
        }
    }
}

impl std::error::Error for SplitError {}

/// 単位を割る。
///
/// <strong>取れるのは、判定に使うものをすべて測れた単位だけである。</strong> 除外に掛かった単位を
/// 入れれば、それを含む対から照合値が出ず、相手の本数が黙って 5 を割る。
///
/// <strong>どちらも 5 ちょうど取る。</strong> 残った単位は値と幅には使うが、帯には使わない——
/// 端は最小値と最大値なので、点が多いほうが外へ広がる。<strong>素材を足すほど帯が広がって
/// 判定が鈍る</strong>という逆向きの挙動になる。
pub fn split(units: &[Unit]) -> Result<Split, SplitError> {
    let mut usable: Vec<Unit> = units.iter().filter(|u| u.usable()).cloned().collect();
    // 単位名の昇順。ファイル名ではない。
    usable.sort_by(|a, b| a.name.cmp(&b.name));
    if usable.len() < UNITS_FLOOR {
        return Err(SplitError::NotEnough {
            usable: usable.len(),
            need: UNITS_FLOOR,
        });
    }
    let partners = usable[..PER_SIDE].to_vec();
    let points = usable[PER_SIDE..PER_SIDE * 2].to_vec();
    Ok(Split { partners, points })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(name: &str) -> Unit {
        Unit {
            name: name.into(),
            systems_measured: true,
            humanness_measured: true,
        }
    }

    fn units(n: usize) -> Vec<Unit> {
        (0..n).map(|i| unit(&format!("u{i:02}"))).collect()
    }

    #[test]
    fn 名前の昇順で先頭から取る() {
        let s = split(&units(10)).unwrap();
        let names: Vec<&str> = s.partners.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, vec!["u00", "u01", "u02", "u03", "u04"]);
        let names: Vec<&str> = s.points.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, vec!["u05", "u06", "u07", "u08", "u09"]);
    }

    #[test]
    fn 渡した順に依らない() {
        let mut shuffled = units(10);
        shuffled.reverse();
        assert_eq!(split(&shuffled).unwrap(), split(&units(10)).unwrap());
    }

    #[test]
    fn 下限を割れば割らない() {
        let e = split(&units(9)).unwrap_err();
        assert!(
            matches!(
                e,
                SplitError::NotEnough {
                    usable: 9,
                    need: 10
                }
            ),
            "{e:?}"
        );
    }

    #[test]
    fn どちらも_5_ちょうど取る() {
        // 素材を足すほど帯が広がって判定が鈍る、という逆向きの挙動を防ぐ。
        let s = split(&units(20)).unwrap();
        assert_eq!(s.partners.len(), 5);
        assert_eq!(s.points.len(), 5);
    }

    #[test]
    fn 系統が測れない単位は入れない() {
        let mut us = units(10);
        us[0].systems_measured = false;
        let e = split(&us).unwrap_err();
        assert!(
            matches!(e, SplitError::NotEnough { usable: 9, .. }),
            "{e:?}"
        );
    }

    #[test]
    fn 人らしさが測れない単位も入れない() {
        // 忘れると人らしさ側だけが痩せる。
        let mut us = units(10);
        us[3].humanness_measured = false;
        let e = split(&us).unwrap_err();
        assert!(
            matches!(e, SplitError::NotEnough { usable: 9, .. }),
            "{e:?}"
        );
    }

    #[test]
    fn 測れない単位を飛ばして詰める() {
        let mut us = units(12);
        us[0].systems_measured = false;
        us[1].humanness_measured = false;
        let s = split(&us).unwrap();
        let names: Vec<&str> = s.partners.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, vec!["u02", "u03", "u04", "u05", "u06"]);
    }
}
