//! 定義ファイルを読む。1 指標 1 ファイルの、その 1 か所である。
//!
//! 名前も札も直し方も、ここから引く。 実装の側に書き写せば、定義を直したときに
//! 書き写しが古いまま残り、エラーにならない。
//!
//! 検めが返す指摘の文も定義ファイルの[直し方](Definition::remedy)である——
//! 指摘の文を実装に持つと、仕様と食い違ったことに誰も気づかない。

use std::path::{Path, PathBuf};

use crate::registry::{Registry, RegistryError};
use crate::tag::Direction;

/// 定義ファイル 1 つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    /// ファイル名（拡張子なし）。
    pub file: String,
    /// 1 行目の見出し。指標の名前である。
    pub name: String,
    /// 札の行。
    pub tag_line: String,
    /// 直し方の節。無ければ空である。
    pub remedy: String,
    /// 切り口そのものを調べた研究があるか。
    ///
    /// `出どころ` の見出しの直後の `直接。` の 1 行で名乗る。 名乗らなければ
    /// 拡張である（[直接だけを名乗らせる](../../../docs/spec/100-metrics.md#直接だけを名乗らせる)）。
    /// 指摘の並びの鍵になるので、説明文から読み取らない。
    pub direct: bool,
    /// 正規化した本文の digest。指紋に入る。
    ///
    /// 本数を指紋にしてはいけない。 同じ本数のまま数え方・除外・直し方を変えれば、
    /// 値の意味が変わったのに指紋が動かず、古い派生値がそのまま使い回される。
    /// この欄が本文ごと変わる。
    pub digest: u64,
}

/// 本文を正規化して digest を取る。
///
/// 取る前に正規化する。 改行や行末の空白の違いで digest が動けば、意味の変わって
/// いない編集で全部の値が捨てられる——捨てられるのが嫌になって、指紋を見なくなる。
fn digest_of(body: &str) -> u64 {
    // FNV-1a。暗号のためではない——同じ本文から同じ値が出ればよい。
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for line in body.lines() {
        for b in line.trim_end().as_bytes() {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h ^= u64::from(b'\n');
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// 直し方の向きを分ける印の開き。定義ファイルの書式である。
const MARK_OPEN: &str = "<strong>";
/// 上へ寄せる直し方の印。
const MARK_UPPER: &str = "上</strong>";
/// 下へ寄せる直し方の印。
const MARK_LOWER: &str = "下</strong>";

impl Definition {
    /// 向きに応じた直し方。書いていなければ `None`。
    ///
    /// 両側の指標は「上」と「下」の印で分けて書く。片側だけの指標は
    /// 節の全体がその向きの直し方である。
    ///
    /// 印は定義ファイルの書式そのものである。 飾りではないので、
    /// 見た目を整えるために外してはいけない——外すと直し方が空になる。
    #[must_use]
    pub fn remedy(&self, want: Direction) -> Option<String> {
        let marked = |mark: &str| -> Option<String> {
            self.remedy
                .split(MARK_OPEN)
                .find_map(|part| part.strip_prefix(mark))
                .map(|s| trim_remedy(s.trim_start_matches([':', '：']).trim()))
        };
        let upper = marked(MARK_UPPER);
        let lower = marked(MARK_LOWER);
        if upper.is_some() || lower.is_some() {
            return match want {
                Direction::Upper => upper,
                Direction::Lower => lower,
                // 両側を訊かれたら 2 つとも返す。
                Direction::Both => match (upper, lower) {
                    (Some(u), Some(l)) => Some(format!("{u} / {l}")),
                    (u, l) => u.or(l),
                },
            };
        }
        let whole = trim_remedy(self.remedy.trim());
        if whole.is_empty() {
            return None;
        }
        Some(whole)
    }
}

/// 印より後ろを 1 文にする。指摘は短くする。
fn trim_remedy(s: &str) -> String {
    let first = s.split("\n\n").next().unwrap_or(s);
    first.replace('\n', "").trim().to_owned()
}

/// 定義ファイルの置き場を探す。呼ばれた場所から遡る。
#[must_use]
pub fn find_dir(from: impl AsRef<Path>) -> Option<PathBuf> {
    let mut at = from.as_ref().to_path_buf();
    loop {
        let p = at.join("docs/spec/metrics");
        if p.is_dir() {
            return Some(p);
        }
        if !at.pop() {
            return None;
        }
    }
}

/// 定義ファイルを全部読む。ファイル名の昇順。
#[must_use]
pub fn read(dir: impl AsRef<Path>) -> Vec<Definition> {
    let Ok(entries) = std::fs::read_dir(dir.as_ref()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in entries.filter_map(Result::ok) {
        let path = e.path();
        if path.extension().is_none_or(|x| x != "md") {
            continue;
        }
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        // README は定義ではない。
        if stem == "README" {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        out.push(parse(&stem, &body));
    }
    out.sort();
    out
}

/// 1 ファイルを読む。
///
/// 1 行目が見出し、空行を挟んだ次の行が札である。
#[must_use]
fn parse(file: &str, body: &str) -> Definition {
    let mut lines = body.lines();
    let name = lines
        .next()
        .unwrap_or("")
        .trim_start_matches('#')
        .trim()
        .to_owned();
    let tag_line = lines
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_owned();
    Definition {
        file: file.to_owned(),
        name,
        tag_line,
        remedy: section(body, "## 直し方"),
        direct: section(body, "## 出どころ")
            .lines()
            .find(|l| !l.trim().is_empty())
            .is_some_and(|l| l.trim() == "直接。"),
        digest: digest_of(body),
    }
}

/// 見出しから次の同じ深さの見出しまで。
fn section(body: &str, heading: &str) -> String {
    let Some(at) = body.find(heading) else {
        return String::new();
    };
    let rest = &body[at + heading.len()..];
    let end = rest.find("\n## ").unwrap_or(rest.len());
    rest[..end].trim().to_owned()
}

/// 定義ファイルから登録簿を組む。
pub fn registry(defs: &[Definition]) -> Result<Registry, RegistryError> {
    let mut r = Registry::new();
    for d in defs {
        r.insert(&d.name, &d.tag_line)?;
    }
    Ok(r)
}

impl PartialOrd for Definition {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Definition {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.file.cmp(&other.file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition(remedy: &str) -> Definition {
        Definition {
            file: "ためし".into(),
            name: "ためし".into(),
            tag_line: "指示 / なし / 記号 / 両側 / 割合。".into(),
            remedy: remedy.to_owned(),
            direct: false,
            digest: digest_of(remedy),
        }
    }

    #[test]
    fn 数え方を変えれば_digest_が変わる() {
        // 本数を指紋にすると、ここが動かない。数え方・除外・直し方を変えても
        // 本数が同じなら、古い派生値がそのまま使い回される。
        let a = parse(
            "x",
            "# 名前\n\n照合 / 記号。\n\n## 数え方\n\n1 つ数える。\n",
        );
        let b = parse(
            "x",
            "# 名前\n\n照合 / 記号。\n\n## 数え方\n\n2 つ数える。\n",
        );
        assert_ne!(a.digest, b.digest);
    }

    #[test]
    fn 行末の空白では_digest_が変わらない() {
        // 意味の変わっていない編集で全部の値が捨てられると、指紋を見なくなる。
        let a = parse("x", "# 名前\n\n照合 / 記号。\n");
        let b = parse("x", "# 名前  \n\n照合 / 記号。   \n");
        assert_eq!(a.digest, b.digest);
    }

    #[test]
    fn 見出しと札を読む() {
        let body = "# 絵文字\n\n指示 / なし / 記号 / 両側 / 割合。\n\n## 意味\n\nある。\n";
        let d = parse("絵文字", body);
        assert_eq!(d.name, "絵文字");
        assert_eq!(d.tag_line, "指示 / なし / 記号 / 両側 / 割合。");
    }

    #[test]
    fn 直し方の節だけを取る() {
        let body = "# x\n\n札。\n\n## 直し方\n\n減らす。\n\n## 覚え書き\n\n別の話。\n";
        let d = parse("x", body);
        assert_eq!(d.remedy, "減らす。");
        assert!(!d.remedy.contains("別の話"), "次の節を含めない");
    }

    #[test]
    fn 両側は印で分ける() {
        let d = definition(
            "<strong>上</strong>: 絵文字を減らす。\n\n<strong>下</strong>: 絵文字を足す。",
        );
        assert_eq!(
            d.remedy(Direction::Upper).as_deref(),
            Some("絵文字を減らす。")
        );
        assert_eq!(
            d.remedy(Direction::Lower).as_deref(),
            Some("絵文字を足す。")
        );
    }

    #[test]
    fn 片側だけの指標は節の全体を返す() {
        let d = definition("下限だけを持つ——段落の長さに緩急をつける。");
        assert!(d.remedy(Direction::Lower).unwrap().contains("緩急をつける"));
    }

    #[test]
    fn 書いていなければ返さない() {
        // 直し方の無い指標は指摘に出ない。だが判定は止まったままである。
        assert_eq!(definition("").remedy(Direction::Upper), None);
    }

    #[test]
    fn 片側しか書いていない両側の指標は片側だけ返す() {
        let d = definition("<strong>上</strong>: 減らす。");
        assert_eq!(d.remedy(Direction::Upper).as_deref(), Some("減らす。"));
        assert_eq!(d.remedy(Direction::Lower), None);
    }

    #[test]
    fn 指摘は_1_文にする() {
        let d = definition("<strong>上</strong>: 減らす。\n\n続きの段落は入れない。");
        assert_eq!(d.remedy(Direction::Upper).as_deref(), Some("減らす。"));
    }

    #[test]
    fn 出どころの直後の行で直接を名乗る() {
        let body = "# x\n\n札。\n\n## 出どころ\n\n直接。\n\n誰それ 2020。\n";
        assert!(parse("x", body).direct);
    }

    #[test]
    fn 名乗らなければ拡張である() {
        // 説明文から読み取らない。 読み取らせると、並び順が読み手ごとに変わる。
        for body in [
            "# x\n\n札。\n\n## 出どころ\n\n誰それ 2020 が直接。\n",
            "# x\n\n札。\n\n## 出どころ\n\n誰それ 2020。\n\n直接。\n",
            "# x\n\n札。\n\n## 意味\n\n直接。\n",
        ] {
            assert!(!parse("x", body).direct, "{body}");
        }
    }

    #[test]
    fn 定義ファイルが実際に読める() {
        // 置き場を遡って見つける。 見つからなければ空を返す——試験は落とさない。
        let Some(dir) = find_dir(env!("CARGO_MANIFEST_DIR")) else {
            return;
        };
        let defs = read(&dir);
        assert!(defs.len() > 40, "{} 本しか読めていない", defs.len());
        let r = registry(&defs).expect("札が読める");
        assert_eq!(r.len(), defs.len());
    }
}
