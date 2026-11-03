//! Pins for CV-36, the history and current list amendment,
//! carried over from the Python history corpus.
//!
//! The records store is append-only history and the class list
//! is versioned current data. These pins hold the two apart. A
//! stored record reads as recorded under any later list. The
//! proposals document counts each record under its confirmed
//! class when it stands confirmed and otherwise under its
//! recorded rescue class, and it refuses when a counted class
//! is not on the current books. A proposed class is never
//! counted, so a stale one refuses nothing. A confirmation is
//! judged against the current list, so a record proposed under
//! a retired class still confirms into a current one. The list
//! is accepted on its shape alone, whatever whole number its
//! version field carries. The review fix pins at the end refuse
//! the corrupt shapes CV-36 names, two records under one id, a
//! class list with a repeated id or a class named unclassified,
//! and a version that is not a whole number.
//!
//! The Python corpus reaches a supplied list through a
//! monkeypatched module path. The port reaches it through the
//! load from a path the library exposes, and through lists
//! built by hand from the shipped one.

mod common;

use cavalry::pyjson::{self, Value};
use cavalry::records::{
    ClassEntry, ClassList, RecordError, build_confirmation, classify, load_class_list_at,
    read_records, render_list, render_proposals, shipped_class_list,
};
use common::{make_record, temp_dir, text};
use sha2::Digest;

const RETIRED: &str = "retired-class";

fn stored(record_id: &str, rescue_class: &str, extra: &[(&str, Value)]) -> Value {
    let mut record = make_record();
    record.set("id", text(record_id));
    record.set("run_ref", text(&format!("run-{record_id}")));
    record.set("rescue_class", text(rescue_class));
    for (key, replacement) in extra {
        record.set(key, replacement.clone());
    }
    record
}

fn write_store(dir: &std::path::Path, rows: &[Value]) -> std::path::PathBuf {
    let path = dir.join("records.jsonl");
    let content: String = rows
        .iter()
        .map(|row| pyjson::dumps_compact(row) + "\n")
        .collect();
    std::fs::write(&path, content).expect("store written");
    path
}

fn proposals(records: &[Value]) -> Result<String, RecordError> {
    render_proposals(records, &shipped_class_list())
}

fn proposals_with(records: &[Value], list: &ClassList) -> Result<String, RecordError> {
    render_proposals(records, list)
}

#[test]
fn a_record_under_a_retired_class_reads_as_recorded() {
    let dir = temp_dir("history-retired-reads");
    let path = write_store(&dir, &[stored("rec-001", RETIRED, &[])]);
    let loaded = read_records(&path).expect("records read");
    assert_eq!(
        loaded[0].get("rescue_class").and_then(Value::as_str),
        Some(RETIRED)
    );
}

#[test]
fn a_record_under_a_retired_class_lists_byte_as_recorded() {
    let dir = temp_dir("history-retired-lists");
    let path = write_store(&dir, &[stored("rec-001", RETIRED, &[])]);
    let listing = render_list(&read_records(&path).expect("records read"));
    assert_eq!(
        listing,
        "rec-001 retired-class run-rec-001 2026-01-01T09:00:00Z \
         The task finished on the next run\n"
    );
}

#[test]
fn propose_refuses_an_unknown_rescue_class_and_names_it() {
    let dir = temp_dir("history-retired-propose");
    let path = write_store(&dir, &[stored("rec-001", RETIRED, &[])]);
    let message = match proposals(&read_records(&path).expect("records read")) {
        Ok(_) => panic!("propose must refuse"),
        Err(error) => error.to_string(),
    };
    assert_eq!(
        message,
        format!("records hold an unknown rescue_class: {RETIRED}")
    );
}

#[test]
fn propose_refuses_an_unknown_confirmed_class_and_names_it() {
    let dir = temp_dir("history-retired-confirmed");
    let rows = vec![
        stored(
            "rec-002",
            "prompt-rephrase",
            &[
                ("review_state", text("proposed")),
                ("proposed_class", text("prompt-rephrase")),
            ],
        ),
        stored(
            "rec-002--confirmed",
            "prompt-rephrase",
            &[
                ("corrects", text("rec-002")),
                ("review_state", text("confirmed")),
                ("confirmed_class", text(RETIRED)),
                ("proposed_class", text("prompt-rephrase")),
            ],
        ),
    ];
    let path = write_store(&dir, &rows);
    let message = match proposals(&read_records(&path).expect("records read")) {
        Ok(_) => panic!("propose must refuse"),
        Err(error) => error.to_string(),
    };
    assert_eq!(
        message,
        format!("records hold an unknown rescue_class: {RETIRED}")
    );
}

#[test]
fn propose_names_the_first_unknown_counted_class_in_sorted_order() {
    let dir = temp_dir("history-sorted-unknown");
    let rows = vec![
        stored("rec-003", "zzz-retired", &[]),
        stored("rec-004", "aaa-retired", &[]),
    ];
    let path = write_store(&dir, &rows);
    let message = match proposals(&read_records(&path).expect("records read")) {
        Ok(_) => panic!("propose must refuse"),
        Err(error) => error.to_string(),
    };
    assert_eq!(message, "records hold an unknown rescue_class: aaa-retired");
}

#[test]
fn a_counted_class_the_later_list_keeps_still_proposes() {
    let dir = temp_dir("history-later-list");
    let default = shipped_class_list();
    let mut later_classes: Vec<ClassEntry> = Vec::new();
    for entry in &default.classes {
        if entry.id == "manual-edit" {
            continue;
        }
        let mut entry = entry.clone();
        if entry.id == "prompt-rephrase" {
            entry.proposal = "A distinct proposal only the later list carries.".to_string();
        }
        later_classes.push(entry);
    }
    let later = ClassList {
        version: "3".to_string(),
        classes: later_classes,
        unclassified_proposal: default.unclassified_proposal.clone(),
    };
    let path = write_store(&dir, &[stored("rec-005", "prompt-rephrase", &[])]);
    let output = proposals_with(&read_records(&path).expect("records read"), &later)
        .expect("proposals render");
    assert!(output.contains("proposal for class prompt-rephrase"));
    assert!(output.contains("A distinct proposal only the later list carries."));
    let default_text = default
        .classes
        .iter()
        .find(|entry| entry.id == "prompt-rephrase")
        .map(|entry| entry.proposal.as_str())
        .unwrap_or("");
    assert!(!output.contains(default_text));
    assert!(!output.contains("manual-edit"));
}

#[test]
fn a_stale_proposed_class_alone_never_refuses_propose() {
    let dir = temp_dir("history-stale-proposal");
    let rows = vec![stored(
        "rec-006",
        "prompt-rephrase",
        &[
            ("review_state", text("proposed")),
            ("proposed_class", text(RETIRED)),
        ],
    )];
    let path = write_store(&dir, &rows);
    let output = proposals(&read_records(&path).expect("records read")).expect("proposals render");
    assert!(output.contains("proposal for class prompt-rephrase"));
    let plain = proposals(&[stored("rec-006", "prompt-rephrase", &[])]).expect("proposals render");
    assert_eq!(output, plain);
}

#[test]
fn a_record_proposed_under_a_retired_class_confirms_into_a_current_class() {
    let target = stored(
        "rec-007",
        RETIRED,
        &[
            ("review_state", text("proposed")),
            ("proposed_class", text(RETIRED)),
        ],
    );
    let payload = Value::object(vec![
        ("record".to_string(), text("rec-007")),
        ("class".to_string(), text("manual-edit")),
    ]);
    let entry = build_confirmation(&payload, std::slice::from_ref(&target), None)
        .expect("confirmation builds");
    assert_eq!(
        entry.get("id").and_then(Value::as_str),
        Some("rec-007--confirmed")
    );
    assert_eq!(
        entry.get("corrects").and_then(Value::as_str),
        Some("rec-007")
    );
    assert_eq!(
        entry.get("review_state").and_then(Value::as_str),
        Some("confirmed")
    );
    assert_eq!(
        entry.get("confirmed_class").and_then(Value::as_str),
        Some("manual-edit")
    );
    assert_eq!(
        entry.get("proposed_class").and_then(Value::as_str),
        Some(RETIRED)
    );
    let entry_class = entry
        .get("rescue_class")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    assert_eq!(entry_class, classify(&entry, &shipped_class_list()));
    assert_eq!(entry_class, "prompt-rephrase");
    assert_ne!(
        Some(entry_class.as_str()),
        target.get("rescue_class").and_then(Value::as_str)
    );
    let output = proposals(&[target, entry]).expect("proposals render");
    assert!(output.contains("proposal for class manual-edit"));
    assert!(!output.contains(RETIRED));
}

#[test]
fn a_well_formed_list_with_another_version_number_behaves_identically() {
    let dir = temp_dir("history-version-7");
    let shipped_text = std::fs::read_to_string(common::root().join("cavalry/classes.json"))
        .expect("list readable");
    let mut raw = pyjson::parse(&shipped_text).expect("list parses");
    raw.set("version", Value::int(7));
    let changed = dir.join("classes.json");
    std::fs::write(&changed, pyjson::dumps_preserving(&raw)).expect("list written");
    let loaded = load_class_list_at(&changed).expect("list loads");
    assert_eq!(loaded.version, "7");
    let sample = Value::object(vec![
        (
            "stalled".to_string(),
            text("The task stalled because the prompt wording sent the worker in a circle"),
        ),
        (
            "human_action".to_string(),
            text("The operator rephrased the prompt and restarted the task"),
        ),
    ]);
    assert_eq!(classify(&sample, &loaded), "prompt-rephrase");
    let records =
        read_records(&common::root().join("examples/records.jsonl")).expect("examples read");
    let expected = std::fs::read_to_string(common::root().join("examples/proposals.txt"))
        .expect("proposals read");
    assert_eq!(
        proposals_with(&records, &loaded).expect("proposals render"),
        expected
    );
}

#[test]
fn a_superseded_record_with_a_stale_class_counts_only_through_its_confirmation() {
    let dir = temp_dir("history-superseded");
    let rows = vec![
        stored(
            "rec-008",
            RETIRED,
            &[
                ("review_state", text("proposed")),
                ("proposed_class", text(RETIRED)),
            ],
        ),
        stored(
            "rec-008--confirmed",
            "prompt-rephrase",
            &[
                ("corrects", text("rec-008")),
                ("review_state", text("confirmed")),
                ("confirmed_class", text("manual-edit")),
                ("proposed_class", text(RETIRED)),
            ],
        ),
    ];
    let path = write_store(&dir, &rows);
    let output = proposals(&read_records(&path).expect("records read")).expect("proposals render");
    assert!(output.contains("proposal for class manual-edit"));
}

#[test]
fn two_records_under_one_id_refuse_at_read() {
    let dir = temp_dir("history-duplicate-id");
    let rows = vec![
        stored("rec-009", "prompt-rephrase", &[]),
        stored("rec-009", RETIRED, &[]),
    ];
    let path = write_store(&dir, &rows);
    let message = match read_records(&path) {
        Ok(_) => panic!("read must refuse"),
        Err(error) => error.to_string(),
    };
    assert_eq!(
        message,
        "records file holds an invalid line 2: duplicate record id: rec-009"
    );
}

#[test]
fn a_forged_third_line_under_a_confirmed_records_id_refuses_at_read() {
    let dir = temp_dir("history-forged-line");
    let rows = vec![
        stored(
            "rec-010",
            RETIRED,
            &[
                ("review_state", text("proposed")),
                ("proposed_class", text(RETIRED)),
            ],
        ),
        stored(
            "rec-010--confirmed",
            "prompt-rephrase",
            &[
                ("corrects", text("rec-010")),
                ("review_state", text("confirmed")),
                ("confirmed_class", text("manual-edit")),
                ("proposed_class", text(RETIRED)),
            ],
        ),
        stored("rec-010", "zzz-retired", &[]),
    ];
    let path = write_store(&dir, &rows);
    let message = match read_records(&path) {
        Ok(_) => panic!("read must refuse"),
        Err(error) => error.to_string(),
    };
    assert_eq!(
        message,
        "records file holds an invalid line 3: duplicate record id: rec-010"
    );
}

fn list_with(dir: &std::path::Path, mutate: impl Fn(&mut Value)) -> std::path::PathBuf {
    let shipped_text = std::fs::read_to_string(common::root().join("cavalry/classes.json"))
        .expect("list readable");
    let mut raw = pyjson::parse(&shipped_text).expect("list parses");
    mutate(&mut raw);
    let changed = dir.join("classes.json");
    std::fs::write(&changed, pyjson::dumps_preserving(&raw)).expect("list written");
    changed
}

#[test]
fn a_class_list_with_a_repeated_class_id_refuses_at_load() {
    let dir = temp_dir("history-list-duplicate");
    let path = list_with(&dir, |raw| {
        let classes = common::field_mut(raw, "classes");
        let first = match classes {
            Value::List(items) => items[0].clone(),
            _ => panic!("classes stand"),
        };
        match classes {
            Value::List(items) => items.push(first),
            _ => panic!("classes stand"),
        }
    });
    let message = match load_class_list_at(&path) {
        Ok(_) => panic!("load must refuse"),
        Err(error) => error.to_string(),
    };
    assert_eq!(
        message,
        "class list holds two classes with the id: prompt-rephrase"
    );
}

#[test]
fn a_class_list_naming_a_class_unclassified_refuses_at_load() {
    let dir = temp_dir("history-list-unclassified");
    let path = list_with(&dir, |raw| {
        let classes = common::list_mut(common::field_mut(raw, "classes"));
        classes[0].set("id", text("unclassified"));
    });
    let message = match load_class_list_at(&path) {
        Ok(_) => panic!("load must refuse"),
        Err(error) => error.to_string(),
    };
    assert_eq!(
        message,
        "class list names a class with the unclassified value: unclassified"
    );
}

#[test]
fn a_version_that_is_not_a_whole_number_refuses_at_load() {
    for bad in [text("7"), Value::Float(2.5), Value::Null, Value::Bool(true)] {
        let dir = temp_dir("history-list-version");
        let path = list_with(&dir, |raw| {
            raw.set("version", bad.clone());
        });
        let message = match load_class_list_at(&path) {
            Ok(_) => panic!("load must refuse"),
            Err(error) => error.to_string(),
        };
        assert_eq!(message, "class list version must be a whole number");
    }
}

#[test]
fn the_committed_examples_are_byte_identical() {
    let records_bytes =
        std::fs::read(common::root().join("examples/records.jsonl")).expect("examples readable");
    let proposals_bytes =
        std::fs::read(common::root().join("examples/proposals.txt")).expect("examples readable");
    let hex = |bytes: &[u8]| {
        sha2::Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect::<String>()
    };
    assert_eq!(
        hex(&records_bytes),
        "e4d2cd027681cd23463683c0fbdfbbbcf4365dbf7ae0608aa0430d25a8d64d59"
    );
    assert_eq!(
        hex(&proposals_bytes),
        "d69e0049f0d512e305a6adc2d52374eef666913337a12cb9d657ce3f9e2a39c6"
    );
}
