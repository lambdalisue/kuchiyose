//! 指摘の文の出どころ。<strong>定義ファイルである。</strong>
//!
//! <strong>実装に持たない。</strong> 指摘の文を書き写せば、定義を直したときに書き写しが古いまま
//! 残り、<strong>仕様と食い違ったことに誰も気づかない。</strong>
//!
//! <strong>向きは札から引く。</strong> 上限だけの指標に「足す」と言わせない——言えば、直す側は
//! 指摘を消すために逆へ動く。

use std::path::Path;

use kakiburi_metrics::definitions::{self, Definition};
use kakiburi_metrics::tag::{Direction, Tag};
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

    /// 指紋に入れる、定義の集合そのもの。
    ///
    /// <strong>本数ではない。</strong> 同じ本数のまま数え方・除外・直し方を変えれば、値の意味が
    /// 変わったのに指紋が動かず、古い派生値が使い回される。
    ///
    /// <strong>1 本も読めなければ、そう書く。</strong> 定義が無い環境では[指摘の文](Self::load)が
    /// 出ないので、同じ条件で測ったとは言えない。
    #[must_use]
    pub fn digest(&self) -> String {
        if self.defs.is_empty() {
            return "定義を読めない".to_owned();
        }
        // <strong>本数も添える。</strong> 合わないときに、何が変わったかを人が見当を付けられる。
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for d in &self.defs {
            for b in d.file.as_bytes().iter().chain(&d.digest.to_le_bytes()) {
                h ^= u64::from(*b);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        format!("定義 {} 本 fnv1a:{h:016x}", self.defs.len())
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

    /// 下端を使った割合で見る指標か。<strong>効くかの判定も同じ分け方に従う。</strong>
    ///
    /// 密度や個数では <strong>0 が「使わなかった」を意味する</strong>ので、素の幅は 0 から
    /// 最大までに広がる。<strong>下端のためにこの規則を置いておきながら、効くかの判定を
    /// 素の幅で行えば、いちばん指示しやすい指標が門前払いされる。</strong>
    ///
    /// <strong>札が読めなければ幅で見る。</strong>[下端](Self::lower_rule)と同じく、分からない
    /// ときに強い側へ倒さない。
    #[must_use]
    pub fn by_appearance(&self, name: &str) -> bool {
        self.registry
            .get(name)
            .is_some_and(|e| e.tag.lower_by_appearance())
    }

    /// [層 3](kakiburi_metrics::Layer::Three) か。<strong>指摘にも判定にも使わない。</strong>
    ///
    /// どの系統から切り出したのかを言えないものである。<strong>止めた理由を言えないものは
    /// 止めてはいけない</strong>ので、判定の 3 段目から外す。
    ///
    /// <strong>札が読めなければ層 3 として扱う。</strong> 分からないものを前に出す側へ倒さない。
    #[must_use]
    pub fn is_layer_three(&self, name: &str) -> bool {
        self.registry
            .get(name)
            .is_none_or(|e| e.tag.layer() == Some(kakiburi_metrics::Layer::Three))
    }

    /// 検査の指標か。<strong>幅ではなく線で見る。</strong>
    ///
    /// 効くかの判定にも指摘にも入れない。比べる先が本人ではないので、
    /// [効くかの 3 条件](kakiburi_scale::effective)が意味を持たない。
    #[must_use]
    pub fn is_inspection(&self, name: &str) -> bool {
        self.registry
            .get(name)
            .is_some_and(|e| matches!(e.tag, Tag::Inspection { .. }))
    }

    /// 検査の一覧。名前・向き・線・超えたときに言うこと。
    ///
    /// <strong>直し方は定義ファイルが持つ。</strong> 超えたと言うだけでは直せない。
    #[must_use]
    pub fn inspections(&self) -> Vec<(String, bool, f64, String)> {
        self.registry
            .entries()
            .iter()
            .filter_map(|e| {
                let Tag::Inspection {
                    direction, limit, ..
                } = &e.tag
                else {
                    return None;
                };
                let upper = *direction != Direction::Lower;
                let want = if upper {
                    Direction::Upper
                } else {
                    Direction::Lower
                };
                Some((e.name.clone(), upper, *limit, self.pick(&e.name, want)?))
            })
            .collect()
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
