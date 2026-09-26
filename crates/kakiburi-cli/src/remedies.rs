//! 指摘の文の出どころ。定義ファイルである。
//!
//! 実装に持たない。 指摘の文を書き写せば、定義を直したときに書き写しが古いまま
//! 残り、仕様と食い違ったことに誰も気づかない。
//!
//! 向きは札から引く。 上限だけの指標に「足す」と言わせない——言えば、直す側は
//! 指摘を消すために逆へ動く。

use kakiburi_metrics::definitions::{self, Definition};
use kakiburi_metrics::tag::{Direction, Tag};
use kakiburi_metrics::Registry;
use kakiburi_review::Remedies;

/// 実行ファイルに埋め込んだ定義ファイル。ファイル名（拡張子なし）と本文の組。
///
/// `build.rs` が `docs/spec/metrics` から一覧を作る。 実行時に置き場を探さない
/// ——リポジトリの外では見つからず、指摘の文が引けないうえに、定義を読めなかった
/// ことが指紋に入って同梱の基準と合わなくなる。
const EMBEDDED: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/definitions.rs"));

/// 定義ファイルから引く直し方。
pub struct FromDefinitions {
    defs: Vec<Definition>,
    registry: Registry,
}

impl FromDefinitions {
    /// 埋め込んだ定義を読む。どのディレクトリから走らせても同じものを読む。
    #[must_use]
    pub fn load() -> Self {
        let defs = definitions::from_texts(EMBEDDED.iter().copied());
        let registry = definitions::registry(&defs).unwrap_or_default();
        Self { defs, registry }
    }

    /// 指紋に入れる、定義の集合そのもの。
    ///
    /// 本数ではない。 同じ本数のまま数え方・除外・直し方を変えれば、値の意味が
    /// 変わったのに指紋が動かず、古い派生値が使い回される。
    ///
    /// 1 本も読めなければ、そう書く。 定義が無い環境では[指摘の文](Self::load)が
    /// 出ないので、同じ条件で測ったとは言えない。
    #[must_use]
    pub fn digest(&self) -> String {
        if self.defs.is_empty() {
            return "定義を読めない".to_owned();
        }
        // 本数も添える。 合わないときに、何が変わったかを人が見当を付けられる。
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for d in &self.defs {
            for b in d.file.as_bytes().iter().chain(&d.digest.to_le_bytes()) {
                h ^= u64::from(*b);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        format!("定義 {} 本 fnv1a:{h:016x}", self.defs.len())
    }

    /// 意味の節の最初の 1 文。一覧で名前に添える。
    ///
    /// `敬体率・段落` のように node の種類ごとに割った指標は、割る前の定義の意味を使う。
    /// 定義ファイルは割る前の 1 本しか無い。
    #[must_use]
    pub fn meaning(&self, name: &str) -> Option<&str> {
        let family = name.split('・').next().unwrap_or(name);
        [name, family].into_iter().find_map(|n| {
            self.defs
                .iter()
                .find(|d| d.name == n)
                .map(|d| d.meaning.as_str())
                .filter(|m| !m.is_empty())
        })
    }

    /// 札に書かれた向き。
    fn direction(&self, name: &str) -> Option<Direction> {
        self.registry.get(name)?.tag.direction()
    }

    /// 下端の見方。札の単位から引く。
    ///
    /// 札が読めなければ幅で見る。 使った割合で見るほうが強い判定なので、
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

    /// 下端を使った割合で見る指標か。効くかの判定も同じ分け方に従う。
    ///
    /// 密度や個数では 0 が「使わなかった」を意味するので、素の幅は 0 から
    /// 最大までに広がる。下端のためにこの規則を置いておきながら、効くかの判定を
    /// 素の幅で行えば、いちばん指示しやすい指標が門前払いされる。
    ///
    /// 札が読めなければ幅で見る。[下端](Self::lower_rule)と同じく、分からない
    /// ときに強い側へ倒さない。
    #[must_use]
    pub fn by_appearance(&self, name: &str) -> bool {
        self.registry
            .get(name)
            .is_some_and(|e| e.tag.lower_by_appearance())
    }

    /// [層 3](kakiburi_metrics::Layer::Three) か。指摘にも判定にも使わない。
    ///
    /// どの系統から切り出したのかを言えないものである。止めた理由を言えないものは
    /// 止めてはいけないので、判定の 3 段目から外す。
    ///
    /// 札が読めなければ層 3 として扱う。 分からないものを前に出す側へ倒さない。
    #[must_use]
    pub fn is_layer_three(&self, name: &str) -> bool {
        self.registry
            .get(name)
            .is_none_or(|e| e.tag.layer() == Some(kakiburi_metrics::Layer::Three))
    }

    /// 切り口そのものを調べた研究があるか。定義ファイルの `直接。` の行で名乗る。
    ///
    /// 読めなければ拡張として扱う。 名乗らないものの既定が拡張である
    /// （[直接だけを名乗らせる](../../../docs/spec/100-metrics.md#直接だけを名乗らせる)）。
    #[must_use]
    pub fn is_direct(&self, name: &str) -> bool {
        self.defs.iter().any(|d| d.name == name && d.direct)
    }

    /// 検査の指標か。幅ではなく線で見る。
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
    /// 直し方は定義ファイルが持つ。 超えたと言うだけでは直せない。
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

    /// その向きの直し方。札が持たない向きは返さない。
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
    fn 埋め込んだ定義は置き場の定義と同じである() {
        // 食い違えば、定義を直したのに実行ファイルが古い定義で指摘し、指紋を作る。
        let dir = definitions::find_dir(env!("CARGO_MANIFEST_DIR")).expect("置き場がある");
        let on_disk = definitions::read(dir);
        assert!(!on_disk.is_empty());
        assert_eq!(FromDefinitions::load().defs, on_disk);
    }

    #[test]
    fn 埋め込んだ定義はすべて札まで読める() {
        // 定義は実行ファイルに埋め込むので、読めないのは実行時の事情ではなく作り方の誤りである。
        // review は読めなかったときの断りを出さないので、ここで止める。
        let r = FromDefinitions::load();
        assert!(!r.defs.is_empty());
        assert!(definitions::registry(&r.defs).is_ok());
    }

    #[test]
    fn 割った指標の意味は割る前の定義から引く() {
        let r = FromDefinitions::load();
        assert_eq!(r.meaning("敬体率・段落"), r.meaning("敬体率"));
        assert!(r.meaning("敬体率").is_some());
        assert_eq!(r.meaning("存在しない指標"), None);
    }

    #[test]
    fn 定義ファイルから引ける() {
        let r = FromDefinitions::load();
        // 両側の指標は上下の両方が引ける。
        assert!(r.upper("絵文字").is_some());
        assert!(r.lower("絵文字").is_some());
    }

    #[test]
    fn 札が持たない向きは返さない() {
        // 上限だけの指標に「足す」と言わせない。
        let r = FromDefinitions::load();
        assert_eq!(r.upper("段落長の変動係数"), None, "下限だけの指標である");
        assert!(r.lower("段落長の変動係数").is_some());
    }

    #[test]
    fn 名乗らない指標と知らない指標は直接でない() {
        let r = FromDefinitions::load();
        // 接続詞直後の読点は、個人差を見た報告が無いので名乗らない。
        assert!(!r.is_direct("接続詞直後の読点"));
        assert!(!r.is_direct("存在しない指標"));
    }

    #[test]
    fn 知らない指標には返さない() {
        let r = FromDefinitions::load();
        assert_eq!(r.upper("存在しない指標"), None);
        assert_eq!(r.lower("存在しない指標"), None);
    }
}
