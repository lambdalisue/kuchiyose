//! 指摘の文の出どころ。<strong>定義ファイルである。</strong>
//!
//! <strong>実装に持たない。</strong> 指摘の文を書き写せば、定義を直したときに書き写しが古いまま
//! 残り、<strong>仕様と食い違ったことに誰も気づかない。</strong>
//!
//! <strong>向きは札から引く。</strong> 上限だけの指標に「足す」と言わせない——言えば、直す側は
//! 指摘を消すために逆へ動く。

use std::path::Path;

use kakiburi_metrics::definitions::{self, Definition};
use kakiburi_metrics::tag::Direction;
use kakiburi_metrics::Registry;
use kakiburi_review::Remedies;

/// 定義ファイルから引く直し方。
pub struct FromDefinitions {
    defs: Vec<Definition>,
    registry: Registry,
}

impl FromDefinitions {
    /// 置き場を探して読む。<strong>見つからなければ空である</strong>——指摘が出ないだけで、
    /// 判定は止まったままになる。
    #[must_use]
    pub fn load() -> Self {
        let defs = definitions::find_dir(std::env::current_dir().unwrap_or_else(|_| ".".into()))
            .or_else(|| definitions::find_dir(Path::new(env!("CARGO_MANIFEST_DIR"))))
            .map(definitions::read)
            .unwrap_or_default();
        let registry = definitions::registry(&defs).unwrap_or_default();
        Self { defs, registry }
    }

    /// 読めた定義の数。
    #[must_use]
    pub fn len(&self) -> usize {
        self.defs.len()
    }

    /// 空か。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    /// 札に書かれた向き。
    fn direction(&self, name: &str) -> Option<Direction> {
        self.registry.get(name)?.tag.direction()
    }

    /// 下端の見方。<strong>札の単位から引く。</strong>
    ///
    /// <strong>札が読めなければ幅で見る。</strong> 使った割合で見るほうが強い判定なので、
    /// 分からないときに強い側へ倒さない。
    #[must_use]
    pub fn lower_rule(&self, name: &str, rate: f64) -> kakiburi_review::Lower {
        let by_appearance = self
            .registry
            .get(name)
            .is_some_and(|e| e.tag.lower_by_appearance());
        if by_appearance {
            kakiburi_review::Lower::Appearance { rate }
        } else {
            kakiburi_review::Lower::Spread
        }
    }

    /// その向きの直し方。<strong>札が持たない向きは返さない。</strong>
    fn pick(&self, name: &str, want: Direction) -> Option<String> {
        let declared = self.direction(name)?;
        if declared != Direction::Both && declared != want {
            return None;
        }
        self.defs.iter().find(|d| d.name == name)?.remedy(want)
    }
}

impl Remedies for FromDefinitions {
    fn upper(&self, name: &str) -> Option<String> {
        self.pick(name, Direction::Upper)
    }

    fn lower(&self, name: &str) -> Option<String> {
        self.pick(name, Direction::Lower)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 定義ファイルから引ける() {
        let r = FromDefinitions::load();
        if r.is_empty() {
            eprintln!("定義ファイルが見つからないので飛ばした");
            return;
        }
        // 両側の指標は上下の両方が引ける。
        assert!(r.upper("絵文字").is_some());
        assert!(r.lower("絵文字").is_some());
    }

    #[test]
    fn 札が持たない向きは返さない() {
        // 上限だけの指標に「足す」と言わせない。
        let r = FromDefinitions::load();
        if r.is_empty() {
            return;
        }
        assert_eq!(r.upper("段落長の変動係数"), None, "下限だけの指標である");
        assert!(r.lower("段落長の変動係数").is_some());
    }

    #[test]
    fn 知らない指標には返さない() {
        let r = FromDefinitions::load();
        assert_eq!(r.upper("存在しない指標"), None);
        assert_eq!(r.lower("存在しない指標"), None);
    }
}
