//! 登録簿と定義ファイルが一致することを確かめる。
//!
//! <strong>仕様にあって実装に無い指標を見つける。</strong> 逆も見つける。名前を 2 か所に書けば、
//! 片方を直したときにもう片方が古いまま残り、エラーにならない。
//!
//! <strong>読む道は[定義ファイルの読み手](kakiburi_metrics::definitions)と同じものである。</strong>
//! 試験だけが別の読み方をすると、通っているのは試験の読み方でしかない。

use std::path::PathBuf;

use kakiburi_metrics::definitions::{self, Definition};
use kakiburi_metrics::{Layer, Registry, Tag};

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

/// 名前を揃える。<strong>ASCII と和文のあいだの空白は組み方であって名前の一部ではない。</strong>
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
    assert_eq!(humanness, 5, "人らしさは 5 本");
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
            kakiburi_metrics::System::from_name(&e.name).is_some(),
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
        // <strong>上と下の両方に印が要る。</strong> 片方だけだと、直す側は指摘を消すために
        // 削る方へ向かう。印は構造で見る——語で当てると「足す」を取りこぼす。
        //
        // <strong>読み手が実際に両方を返せることで確かめる。</strong> 印の有無を文字列で見ると、
        // 印はあるが読み手が引けない形（並びが違う、印だけで本文が無い）を見逃す。
        let up = def_.remedy(kakiburi_metrics::tag::Direction::Upper);
        let down = def_.remedy(kakiburi_metrics::tag::Direction::Lower);
        assert!(
            up.is_some() && down.is_some(),
            "{}.md: 両側なのに上下が揃っていない（上: {up:?} / 下: {down:?}）",
            def_.file
        );
    }
}
