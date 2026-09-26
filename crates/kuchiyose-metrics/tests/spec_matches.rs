//! 登録簿と定義ファイルが一致することを確かめる。
//!
//! 仕様にあって実装に無い指標を見つける。 逆も見つける。名前を 2 か所に書けば、
//! 片方を直したときにもう片方が古いまま残り、エラーにならない。
//!
//! 読む道は[定義ファイルの読み手](kuchiyose_metrics::definitions)と同じものである。
//! 試験だけが別の読み方をすると、通っているのは試験の読み方でしかない。

use std::path::PathBuf;

use kuchiyose_metrics::definitions::{self, Definition};
use kuchiyose_metrics::{Layer, Registry, Tag};

/// 定義ファイルの置き場。ワークスペースの外から呼ばれても見つかるように遡る。
fn metrics_dir() -> Option<PathBuf> {
    definitions::find_dir(env!("CARGO_MANIFEST_DIR"))
}

/// 定義ファイルから名前と札を取る。
fn read_definitions() -> Vec<Definition> {
    metrics_dir().map(definitions::read).unwrap_or_default()
}

fn registry_from_definitions() -> Registry {
    definitions::registry(&read_definitions()).expect("札が読める")
}

/// 名前を揃える。ASCII と和文のあいだの空白は組み方であって名前の一部ではない。
///
/// `1 文だけの段落の割合` と `1文だけの段落の割合`、`文字 bigram` と `文字bigram` は
/// 同じ名前である。リンクはファイル名を指すので、揃えないと引けなくなる。
fn canonical(name: &str) -> String {
    name.chars()
        .filter(|c| *c != ' ' && *c != '\u{3000}')
        .collect()
}

#[test]
fn 定義ファイルの名前と見出しは一致する() {
    let defs = read_definitions();
    if defs.is_empty() {
        eprintln!("定義ファイルが見つからないので飛ばした");
        return;
    }
    for d in &defs {
        assert_eq!(
            canonical(&d.file),
            canonical(&d.name),
            "ファイル名と見出しがずれている: {} / {}",
            d.file,
            d.name
        );
    }
}

#[test]
fn 札はすべて読める() {
    let defs = read_definitions();
    if defs.is_empty() {
        eprintln!("定義ファイルが見つからないので飛ばした");
        return;
    }
    for d in &defs {
        Tag::parse(&d.tag_line).unwrap_or_else(|e| panic!("{}.md: {e}", d.file));
    }
    eprintln!("{} 本の札を読んだ", defs.len());
}

#[test]
fn 名前は_1_度しか出てこない() {
    let defs = read_definitions();
    if defs.is_empty() {
        eprintln!("定義ファイルが見つからないので飛ばした");
        return;
    }
    // Registry::insert が重なりを断る。
    let _ = registry_from_definitions();
}

#[test]
fn 種別の内訳が仕様と合う() {
    let r = registry_from_definitions();
    if r.is_empty() {
        eprintln!("定義ファイルが見つからないので飛ばした");
        return;
    }
    let directive = r.by_kind(|t| matches!(t, Tag::Directive { .. })).count();
    let matching = r.by_kind(|t| matches!(t, Tag::Matching { .. })).count();
    let humanness = r.by_kind(|t| matches!(t, Tag::Humanness { .. })).count();
    let inspection = r.by_kind(|t| matches!(t, Tag::Inspection { .. })).count();
    eprintln!("指示 {directive} / 照合 {matching} / 人らしさ {humanness} / 検査 {inspection}");
    assert_eq!(directive + matching + humanness + inspection, r.len());
    // 検査は書きぶりの軸ではない。**足す条件が厳しいので、数は増えにくい。**
    // どちらも「道具の指示を機械的に満たすと壊れる」穴を塞ぐために置いている。
    assert_eq!(inspection, 2, "検査は 2 本");
    // 照合の系統は 8 本。判定に使える層 1 はそのうちの一部である。
    assert_eq!(matching, 8, "照合の系統は 8 本");
    // 繰り返しは短いと長いに割れている。**まとめると向きが指標の中で割れる。**
    assert_eq!(humanness, 6, "人らしさは 6 本");
}

#[test]
fn 層_3_の指標は系統を名指しできない() {
    let r = registry_from_definitions();
    if r.is_empty() {
        eprintln!("定義ファイルが見つからないので飛ばした");
        return;
    }
    for e in r.by_kind(|t| t.layer() == Some(Layer::Three)) {
        let Tag::Directive { system, .. } = &e.tag else {
            panic!("{}: 層 3 は指示だけである", e.name);
        };
        assert!(
            system.is_none(),
            "{}: 層 3 なのに系統を名指ししている",
            e.name
        );
    }
}

#[test]
fn 照合の系統名はすべて表にある() {
    // 照合の定義ファイルの名前が System の表に無ければ、層が引けない。
    let r = registry_from_definitions();
    if r.is_empty() {
        eprintln!("定義ファイルが見つからないので飛ばした");
        return;
    }
    for e in r.by_kind(|t| matches!(t, Tag::Matching { .. })) {
        assert!(
            kuchiyose_metrics::System::from_name(&e.name).is_some(),
            "{} が System の表に無い",
            e.name
        );
    }
}

#[test]
fn 両側の指標は直し方を_2_つ書いている() {
    let Some(_dir) = metrics_dir() else {
        eprintln!("定義ファイルが見つからないので飛ばした");
        return;
    };
    for def_ in read_definitions() {
        let Ok(t) = Tag::parse(&def_.tag_line) else {
            continue;
        };
        let Some(d) = t.direction() else { continue };
        if d.remedies_required() < 2 {
            continue;
        }
        assert!(
            !def_.remedy.trim().is_empty(),
            "{}.md: 両側なのに直し方が空である",
            def_.file
        );
        // 上と下の両方に印が要る。 片方だけだと、直す側は指摘を消すために
        // 削る方へ向かう。印は構造で見る——語で当てると「足す」を取りこぼす。
        //
        // 読み手が実際に両方を返せることで確かめる。 印の有無を文字列で見ると、
        // 印はあるが読み手が引けない形（並びが違う、印だけで本文が無い）を見逃す。
        let up = def_.remedy(kuchiyose_metrics::tag::Direction::Upper);
        let down = def_.remedy(kuchiyose_metrics::tag::Direction::Lower);
        assert!(
            up.is_some() && down.is_some(),
            "{}.md: 両側なのに上下が揃っていない（上: {up:?} / 下: {down:?}）",
            def_.file
        );
    }
}

/// 直し方の節から、`上` と `下` の行を取る。
fn directions(remedy: &str) -> (Option<String>, Option<String>) {
    let pick = |label: &str| -> Option<String> {
        remedy.lines().find_map(|l| {
            let l = l.trim();
            // 印は定義ファイルの書式である。 飾りではないので外さない。
            for open in ["<strong>", "**"] {
                let close = if open == "**" { "**" } else { "</strong>" };
                let head = format!("{open}{label}{close}");
                if let Some(rest) = l.strip_prefix(&head) {
                    return Some(rest.trim_start_matches([':', '：']).trim().to_owned());
                }
            }
            None
        })
    };
    (pick("上"), pick("下"))
}

#[test]
fn 両側の指標は上と下の両方を書く() {
    // 片方しか書かなければ、直す側はもう片方を自分で考えることになる。
    // 人らしさは向きを較正が決めるので、両方が要る——決め打つと、較正が
    // 「減らせ」と言った場面でも「増やせ」と指示することになる。
    let defs = read_definitions();
    if defs.is_empty() {
        eprintln!("定義ファイルが見つからないので飛ばした");
        return;
    }
    for d in &defs {
        let Ok(tag) = Tag::parse(&d.tag_line) else {
            continue;
        };
        if !matches!(tag, Tag::Humanness { .. }) {
            continue;
        }
        let (up, down) = directions(&d.remedy);
        assert!(up.is_some(), "{}: 直し方に「上」が無い", d.name);
        assert!(down.is_some(), "{}: 直し方に「下」が無い", d.name);
        assert_ne!(up, down, "{}: 上と下が同じ", d.name);
    }
}

/// この試験は「入れ替わり」を捕まえない。
///
/// 上下を取り違えると、道具は自分の判定を悪くする方向へ指示する。実際に 1 度そう書いた
/// ——較正が「句読点の密度を下げろ」と言うのに、直し方は「文を短く切り、読点を増やす」
/// と出ていた。入れ替えたまま、この試験は通る。 実際に戻して確かめた。
///
/// 捕まえるのは上下が同じ向きを言っているときだけである。入れ替わりを捕まえるには、
/// 定義ごとに「上の直し方を当てた前後の文」を持たせて実際に測るしかない
/// （[句読点の密度でやっている形](kuchiyose_metrics::humanness)）。まだ 1 本しかない。
#[test]
fn 上と下が同じ向きを言っていない() {
    let defs = read_definitions();
    if defs.is_empty() {
        eprintln!("定義ファイルが見つからないので飛ばした");
        return;
    }
    let more = ["増やす", "増やし", "多くする"];
    let less = ["減らす", "減らし", "抑える", "避ける"];
    for d in &defs {
        let Ok(tag) = Tag::parse(&d.tag_line) else {
            continue;
        };
        if !matches!(tag, Tag::Humanness { .. }) {
            continue;
        }
        let (Some(up), Some(down)) = directions(&d.remedy) else {
            continue;
        };
        let has = |s: &str, words: &[&str]| words.iter().any(|w| s.contains(w));
        if has(&up, &more) && has(&down, &more) && !has(&up, &less) && !has(&down, &less) {
            panic!("{}: 上も下も「増やす」と言っている", d.name);
        }
        if has(&up, &less) && has(&down, &less) && !has(&up, &more) && !has(&down, &more) {
            panic!("{}: 上も下も「減らす」と言っている", d.name);
        }
    }
}
