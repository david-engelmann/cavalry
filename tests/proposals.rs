//! The proposals pins of the Python corpus, the blocks of the
//! proposals document.

mod common;

use cavalry::pyjson::Value;
use cavalry::records::{render_proposals, shipped_class_list};
use common::text;

fn stored(record_id: &str, rescue_class: &str) -> Value {
    Value::object(vec![
        ("id".to_string(), text(record_id)),
        ("rescue_class".to_string(), text(rescue_class)),
        ("run_ref".to_string(), text(&format!("run-{record_id}"))),
        ("occurred_at".to_string(), text("2026-01-01T09:00:00Z")),
        ("outcome".to_string(), text("Done")),
    ])
}

fn proposals(records: &[Value]) -> String {
    render_proposals(records, &shipped_class_list()).expect("proposals render")
}

fn blocks(output: &str) -> Vec<&str> {
    output
        .trim()
        .split("\n\n")
        .filter(|block| !block.is_empty())
        .collect()
}

#[test]
fn proposals_produced_per_class_present() {
    let records = vec![
        stored("rec-001", "prompt-rephrase"),
        stored("rec-002", "prompt-rephrase"),
        stored("rec-003", "run-interruption"),
    ];
    assert_eq!(blocks(&proposals(&records)).len(), 2);
}

#[test]
fn each_proposal_names_its_class_and_carries_the_word_proposal() {
    let records = vec![
        stored("rec-001", "manual-edit"),
        stored("rec-002", "prompt-rephrase"),
        stored("rec-003", "unclassified"),
    ];
    let output = proposals(&records);
    let found = blocks(&output);
    assert_eq!(found.len(), 3);
    for block in &found {
        assert!(block.contains("proposal"));
    }
    assert!(found[0].contains("proposal for class prompt-rephrase"));
    assert!(found[1].contains("proposal for class manual-edit"));
    assert!(found[2].contains("proposal for class unclassified"));
    assert!(output.contains("never applied"));
}

#[test]
fn class_with_no_records_gets_no_proposal() {
    let output = proposals(&[stored("rec-001", "run-interruption")]);
    assert!(output.contains("run-interruption"));
    assert!(!output.contains("manual-edit"));
    assert!(!output.contains("prompt-rephrase"));
}

#[test]
fn no_records_produces_no_proposals() {
    assert_eq!(proposals(&[]), "");
}
