//! `cassette edit` の対話画面（[対話は edit だけにする](../../../docs/design/200-command.md#対話は-edit-だけにする)）。
//!
//! 種類を選び、無効にする種類はチェックリストで、申告する種類は選択肢で決める。
//! できることは引数で受けるサブコマンドと同じで、それより多くはない。
//!
//! 状態を変える規則は[`tuning_cmd`](crate::tuning_cmd)にあり、ここは画面だけを持つ。
//! 画面は端末が無ければ試せないので、規則を画面に書かない。

use dialoguer::{Confirm, MultiSelect, Select};
use kakiburi_cassette::{MuteKind, Register, Tuning};

use crate::tuning_cmd::{apply_checklist, first_person_choices, Item, Subject};

/// 画面を回す。保存して終えるなら `true`、書かずに終えるなら `false`。
///
/// # Errors
///
/// 端末から読めなくなったときに返す。
pub fn run(t: &mut Tuning, items: &dyn Fn(&Tuning) -> Vec<Item>) -> Result<bool, dialoguer::Error> {
    let original = t.clone();
    loop {
        let all = items(t);
        let mut menu: Vec<String> = Subject::ALL
            .iter()
            .map(|s| section_label(*s, &all, t))
            .collect();
        menu.push("保存して終える".to_owned());
        menu.push("保存せずに終える".to_owned());
        let Some(choice) = Select::new()
            .with_prompt("調整する種類を選ぶ")
            .items(&menu)
            .default(0)
            .interact_opt()?
        else {
            // Esc で抜けたら、書かずに終える側に倒す。
            if *t == original || confirm_discard()? {
                return Ok(false);
            }
            continue;
        };
        match Subject::ALL.get(choice) {
            Some(Subject::Mute(kind)) => checklist(t, *kind, &all)?,
            Some(Subject::FirstPerson) => pick_first_person(t, &all)?,
            Some(Subject::Register) => pick_register(t, &all)?,
            None if choice == Subject::ALL.len() => return Ok(true),
            None => {
                if *t == original || confirm_discard()? {
                    return Ok(false);
                }
            }
        }
    }
}

/// メニューの 1 行。無効にした数か、今の値を添える。
fn section_label(s: Subject, all: &[Item], t: &Tuning) -> String {
    let mine: Vec<&Item> = all.iter().filter(|i| i.subject == s).collect();
    match s {
        Subject::Mute(kind) => {
            let off = mine.iter().filter(|i| i.muted == Some(true)).count();
            let whole = if t.mute_kinds.contains(&kind) {
                "。種類ごと無効"
            } else {
                ""
            };
            format!(
                "{}（{} 件のうち {off} 件を無効{whole}）",
                s.name(),
                mine.len()
            )
        }
        Subject::FirstPerson | Subject::Register => {
            let state = mine.first().map_or("", |i| i.state.as_str());
            format!("{}: {state}", s.name())
        }
    }
}

fn confirm_discard() -> Result<bool, dialoguer::Error> {
    Confirm::new()
        .with_prompt("変えたものを捨てて終える？")
        .default(false)
        .interact()
}

/// 無効にする種類。先頭の 1 行で種類ごと無効にし、残りで 1 つずつ選ぶ。
fn checklist(t: &mut Tuning, kind: MuteKind, all: &[Item]) -> Result<(), dialoguer::Error> {
    let shown: Vec<&Item> = all
        .iter()
        .filter(|i| i.subject == Subject::Mute(kind))
        .collect();
    let mut labels = vec![format!("（{}を種類ごと無効にする）", kind.name())];
    let mut defaults = vec![t.mute_kinds.contains(&kind)];
    for i in &shown {
        labels.push(format!("{}  {}", i.name, i.description));
        // 種類ごと無効にしたことは先頭の行が持つ。 ここでは 1 つずつ無効にしたものだけに印を付ける。
        defaults.push(t.muted(kind).contains(&i.text.as_str()));
    }
    let Some(picked) = MultiSelect::new()
        .with_prompt("無効にするものに印を付ける（空白で切り替え、Enter で決める、Esc で戻る）")
        .items(&labels)
        .defaults(&defaults)
        .interact_opt()?
    else {
        return Ok(());
    };
    let checked: Vec<bool> = (1..labels.len()).map(|i| picked.contains(&i)).collect();
    apply_checklist(t, kind, &shown, picked.contains(&0), &checked);
    Ok(())
}

fn pick_first_person(t: &mut Tuning, all: &[Item]) -> Result<(), dialoguer::Error> {
    let counted = described(all, Subject::FirstPerson);
    let words = first_person_choices();
    let mut labels = vec![format!("auto（{counted}）")];
    labels.extend(words.iter().map(|w| (*w).to_owned()));
    let current = t
        .first_person
        .as_deref()
        .and_then(|f| words.iter().position(|w| *w == f))
        .map_or(0, |i| i + 1);
    if let Some(i) = Select::new()
        .with_prompt("一人称を申告する")
        .items(&labels)
        .default(current)
        .interact_opt()?
    {
        t.first_person = i.checked_sub(1).map(|i| words[i].to_owned());
    }
    Ok(())
}

fn pick_register(t: &mut Tuning, all: &[Item]) -> Result<(), dialoguer::Error> {
    let counted = described(all, Subject::Register);
    let choices = [None, Some(Register::Polite), Some(Register::Plain)];
    let labels = [
        format!("auto（{counted}）"),
        "polite（敬体）".to_owned(),
        "plain（常体）".to_owned(),
    ];
    let current = choices.iter().position(|c| *c == t.register).unwrap_or(0);
    if let Some(i) = Select::new()
        .with_prompt("文体を申告する")
        .items(&labels)
        .default(current)
        .interact_opt()?
    {
        t.register = choices[i];
    }
    Ok(())
}

/// 申告する種類の説明。数えた結果を言う。
fn described(all: &[Item], s: Subject) -> String {
    all.iter()
        .find(|i| i.subject == s)
        .map(|i| i.description.clone())
        .unwrap_or_default()
}
