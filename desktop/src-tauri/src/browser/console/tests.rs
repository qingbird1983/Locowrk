//! ConsoleStore 单测：环形钳制 / 页签让位 / 最近页签缺省读。

use super::*;

#[test]
fn register_and_read_explicit_tab() {
    let s = ConsoleStore::new();
    let id = s.next_tab_id();
    s.report(
        &id,
        vec![ConsoleEntry {
            ts: 1,
            level: "error".into(),
            text: "boom".into(),
        }],
    );
    let (got, entries, truncated) = s.read(Some(&id)).expect("tab present");
    assert_eq!(got, id);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].text, "boom");
    assert!(!truncated);
}

#[test]
fn read_defaults_to_latest_tab() {
    let s = ConsoleStore::new();
    let a = s.next_tab_id();
    let b = s.next_tab_id();
    s.register(&a);
    s.report(
        &a,
        vec![ConsoleEntry {
            ts: 1,
            level: "warn".into(),
            text: "a".into(),
        }],
    );
    s.report(
        &b,
        vec![ConsoleEntry {
            ts: 2,
            level: "error".into(),
            text: "b".into(),
        }],
    );
    let (got, entries, _) = s.read(None).expect("latest tab");
    assert_eq!(got, b);
    assert_eq!(entries[0].text, "b");
}

#[test]
fn ring_cap_drops_oldest() {
    let s = ConsoleStore::new();
    let id = s.next_tab_id();
    let batch: Vec<ConsoleEntry> = (0..PER_TAB_CAP + 10)
        .map(|i| ConsoleEntry {
            ts: i as i64,
            level: "error".into(),
            text: format!("e{i}"),
        })
        .collect();
    s.report(&id, batch);
    let (_, entries, truncated) = s.read(Some(&id)).expect("tab present");
    assert_eq!(entries.len(), PER_TAB_CAP);
    assert!(truncated);
    assert_eq!(entries[0].text, "e10", "最旧 10 条被挤掉");
    assert_eq!(
        entries.last().unwrap().text,
        format!("e{}", PER_TAB_CAP + 9)
    );
}

#[test]
fn tab_cap_evicts_oldest_whole_buffer() {
    let s = ConsoleStore::new();
    let first = s.next_tab_id();
    s.report(
        &first,
        vec![ConsoleEntry {
            ts: 1,
            level: "error".into(),
            text: "first".into(),
        }],
    );
    for _ in 0..TAB_CAP {
        let id = s.next_tab_id();
        s.register(&id);
    }
    assert!(s.read(Some(&first)).is_none(), "最旧页签整体让位");
    // 让位后最近页签缺省读仍可用
    assert!(s.read(None).is_some());
}

#[test]
fn long_text_clipped() {
    let s = ConsoleStore::new();
    let id = s.next_tab_id();
    let long = "x".repeat(TEXT_CLIP + 500);
    s.report(
        &id,
        vec![ConsoleEntry {
            ts: 1,
            level: "error".into(),
            text: long,
        }],
    );
    let (_, entries, _) = s.read(Some(&id)).unwrap();
    assert_eq!(entries[0].text.chars().count(), TEXT_CLIP);
}
