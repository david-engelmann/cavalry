//! CV-27 to CV-33. Proposals, review states, quality flags,
//! confirmation, the pins of the Python review corpus.

mod common;

use std::collections::HashMap;
use std::path::PathBuf;

use cavalry::judgment::{FixturePlayer, JudgmentSource};
use cavalry::pyjson::{self, Value};
use cavalry::records::{
    RecordError, build_stored_record, capture, confirm, read_records, render_list,
    render_proposals, shipped_class_list,
};
use common::{
    abstain, agree, differ, faint, fixtures_dir, make_record, new_record, temp_dir, text,
    write_input,
};

const REVIEW_FIELDS: [&str; 4] = [
    "review_state",
    "proposed_class",
    "quality_flag",
    "confirmed_class",
];

fn player() -> FixturePlayer {
    FixturePlayer::load(&fixtures_dir()).expect("player loads")
}

/// A judgment source that is not the fixture player.
struct StubSource {
    answers: HashMap<String, Value>,
}

impl JudgmentSource for StubSource {
    fn judge(&self, point: &str, _request: &Value) -> Option<Value> {
        self.answers.get(point).cloned()
    }
}

fn stub(pairs: &[(&str, Value)]) -> StubSource {
    StubSource {
        answers: pairs
            .iter()
            .map(|(point, answer)| (point.to_string(), answer.clone()))
            .collect(),
    }
}

fn capture_one(
    dir: &std::path::Path,
    record: &Value,
    source: Option<&dyn JudgmentSource>,
    name: &str,
) -> (Value, PathBuf) {
    let records_path = dir.join(name);
    let input = write_input(dir, record, "input.json");
    let stored = capture(&input, &records_path, source).expect("capture stores");
    (stored, records_path)
}

fn write_confirmation(dir: &std::path::Path, payload: &Value, name: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, pyjson::dumps_preserving(payload)).expect("written");
    path
}

fn confirmation_payload(record: &str, class: &str) -> Value {
    Value::object(vec![
        ("record".to_string(), text(record)),
        ("class".to_string(), text(class)),
    ])
}

fn proposals_of(records: &[Value]) -> String {
    render_proposals(records, &shipped_class_list()).expect("proposals render")
}

fn field_is(record: &Value, name: &str, expected: &str) {
    assert_eq!(
        record.get(name).and_then(Value::as_str),
        Some(expected),
        "field {name}"
    );
}

#[test]
fn a_proposal_attaches_and_the_deterministic_class_is_unchanged() {
    let dir = temp_dir("flow-differ");
    let (stored, _) = capture_one(&dir, &differ(), Some(&player()), "records.jsonl");
    field_is(&stored, "rescue_class", "run-interruption");
    field_is(&stored, "proposed_class", "manual-edit");
    field_is(&stored, "review_state", "proposed");
}

#[test]
fn an_agreeing_proposal_still_attaches_as_a_second_opinion() {
    let dir = temp_dir("flow-agree");
    let (stored, _) = capture_one(&dir, &agree(), Some(&player()), "records.jsonl");
    field_is(&stored, "rescue_class", "prompt-rephrase");
    field_is(&stored, "proposed_class", "prompt-rephrase");
    field_is(&stored, "review_state", "proposed");
}

#[test]
fn a_new_class_proposal_attaches_to_an_unclassified_record() {
    let dir = temp_dir("flow-new");
    let (stored, _) = capture_one(&dir, &new_record(), Some(&player()), "records.jsonl");
    field_is(&stored, "rescue_class", "unclassified");
    field_is(&stored, "proposed_class", "new-class");
    field_is(&stored, "review_state", "proposed");
}

#[test]
fn no_source_leaves_the_record_in_the_unreviewed_state() {
    let dir = temp_dir("flow-no-source");
    let (stored, _) = capture_one(&dir, &agree(), None, "records.jsonl");
    field_is(&stored, "review_state", "unreviewed");
    assert!(stored.get("proposed_class").is_none());
    assert!(stored.get("quality_flag").is_none());
}

#[test]
fn an_abstention_leaves_the_record_in_the_unreviewed_state() {
    let dir = temp_dir("flow-abstain");
    let (stored, _) = capture_one(&dir, &abstain(), Some(&player()), "records.jsonl");
    field_is(&stored, "rescue_class", "manual-edit");
    field_is(&stored, "review_state", "unreviewed");
    assert!(stored.get("proposed_class").is_none());
}

#[test]
fn a_below_threshold_answer_leaves_the_record_in_the_unreviewed_state() {
    let dir = temp_dir("flow-faint");
    let (stored, _) = capture_one(&dir, &faint(), Some(&player()), "records.jsonl");
    field_is(&stored, "rescue_class", "prompt-rephrase");
    field_is(&stored, "review_state", "unreviewed");
    assert!(stored.get("proposed_class").is_none());
}

#[test]
fn a_malformed_source_answer_counts_as_no_judgment_and_never_fails_capture() {
    let dir = temp_dir("flow-malformed");
    let source = stub(&[
        (
            "classification",
            Value::object(vec![
                ("kind".to_string(), text("score")),
                ("score".to_string(), Value::Float(0.9)),
                ("confidence".to_string(), Value::Float(0.99)),
            ]),
        ),
        ("quality", text("not even an object")),
    ]);
    let (stored, _) = capture_one(&dir, &agree(), Some(&source), "records.jsonl");
    field_is(&stored, "review_state", "unreviewed");
    assert!(stored.get("proposed_class").is_none());
    assert!(stored.get("quality_flag").is_none());
}

#[test]
fn the_human_fields_are_byte_identical_with_and_without_a_source() {
    let dir = temp_dir("flow-identical");
    let (plain, plain_path) = capture_one(&dir, &abstain(), None, "plain.jsonl");
    let (judged, judged_path) = capture_one(&dir, &abstain(), Some(&player()), "judged.jsonl");
    assert_eq!(
        std::fs::read(&judged_path).expect("read"),
        std::fs::read(&plain_path).expect("read")
    );
    common::assert_py_eq(&judged, &plain);

    let (proposed, _) = capture_one(&dir, &agree(), Some(&player()), "proposed.jsonl");
    let (baseline, _) = capture_one(&dir, &agree(), None, "baseline.jsonl");
    let stripped = common::without_many(&proposed, &REVIEW_FIELDS);
    let stripped_baseline = common::without_many(&baseline, &REVIEW_FIELDS);
    common::assert_py_eq(&stripped, &stripped_baseline);
}

#[test]
fn a_below_bar_quality_score_flags_the_record() {
    let dir = temp_dir("flow-below-bar");
    let (stored, _) = capture_one(&dir, &new_record(), Some(&player()), "records.jsonl");
    field_is(&stored, "quality_flag", "below-bar");
}

#[test]
fn an_above_bar_quality_score_records_no_flag() {
    let dir = temp_dir("flow-above-bar");
    let (stored, _) = capture_one(&dir, &agree(), Some(&player()), "records.jsonl");
    assert!(stored.get("quality_flag").is_none());
}

#[test]
fn a_quality_flag_never_changes_the_review_state() {
    let dir = temp_dir("flow-flag-state");
    let source = stub(&[
        (
            "classification",
            Value::object(vec![("kind".to_string(), text("abstention"))]),
        ),
        (
            "quality",
            Value::object(vec![
                ("kind".to_string(), text("score")),
                ("score".to_string(), Value::Float(0.2)),
                ("confidence".to_string(), Value::Float(0.95)),
            ]),
        ),
    ]);
    let (stored, _) = capture_one(&dir, &agree(), Some(&source), "records.jsonl");
    field_is(&stored, "review_state", "unreviewed");
    field_is(&stored, "quality_flag", "below-bar");
}

#[test]
fn the_round_1_examples_still_read_list_and_propose() {
    let records =
        read_records(&common::root().join("examples/records.jsonl")).expect("examples read");
    assert_eq!(records.len(), 4);
    assert!(
        records
            .iter()
            .all(|record| record.get("review_state").is_none())
    );
    assert!(!render_list(&records).is_empty());
    let expected = std::fs::read_to_string(common::root().join("examples/proposals.txt"))
        .expect("proposals read");
    assert_eq!(proposals_of(&records), expected);
}

fn write_lines(dir: &std::path::Path, records: &[Value]) -> PathBuf {
    let path = dir.join("records.jsonl");
    let text: String = records
        .iter()
        .map(|record| pyjson::dumps_compact(record) + "\n")
        .collect();
    std::fs::write(&path, text).expect("lines written");
    path
}

fn stored_default() -> Value {
    build_stored_record(&make_record(), &[], None, None).expect("record builds")
}

#[test]
fn a_round_1_stored_line_still_reads() {
    let dir = temp_dir("flow-round-1-line");
    let mut stored = stored_default();
    for field in REVIEW_FIELDS {
        common::remove_key(&mut stored, field);
    }
    let path = write_lines(&dir, &[stored]);
    let loaded = read_records(&path).expect("records read");
    assert_eq!(loaded[0].get("id").and_then(Value::as_str), Some("rec-001"));
}

#[test]
fn a_stored_record_with_an_unknown_review_state_is_refused() {
    let dir = temp_dir("flow-bad-state");
    let mut stored = stored_default();
    stored.set("review_state", text("pending"));
    let path = write_lines(&dir, &[stored]);
    assert!(read_records(&path).is_err());
}

#[test]
fn a_stored_proposal_without_a_proposed_class_is_refused() {
    let dir = temp_dir("flow-proposal-no-class");
    let mut stored = stored_default();
    stored.set("review_state", text("proposed"));
    let path = write_lines(&dir, &[stored]);
    assert!(read_records(&path).is_err());
}

#[test]
fn a_stored_confirmation_without_a_confirmed_class_is_refused() {
    let dir = temp_dir("flow-confirmation-no-class");
    let mut stored = stored_default();
    stored.set("review_state", text("confirmed"));
    let path = write_lines(&dir, &[stored]);
    assert!(read_records(&path).is_err());
}

#[test]
fn a_stored_review_field_that_is_not_text_is_refused() {
    let dir = temp_dir("flow-review-field-type");
    let mut stored = stored_default();
    stored.set("proposed_class", Value::int(7));
    stored.set("review_state", text("proposed"));
    let path = write_lines(&dir, &[stored]);
    assert!(read_records(&path).is_err());
}

fn capture_differ_with_proposal(dir: &std::path::Path) -> PathBuf {
    let (_, records_path) = capture_one(dir, &differ(), Some(&player()), "records.jsonl");
    records_path
}

#[test]
fn confirmation_appends_one_record_and_edits_no_existing_line() {
    let dir = temp_dir("flow-confirm-append");
    let records_path = capture_differ_with_proposal(&dir);
    let before = std::fs::read(&records_path).expect("store read");
    let entry = confirm(
        &write_confirmation(
            &dir,
            &confirmation_payload("fox-002", "manual-edit"),
            "confirmation.json",
        ),
        &records_path,
    )
    .expect("confirm stores");
    let after = std::fs::read(&records_path).expect("store read");
    assert!(after.starts_with(&before));
    assert_eq!(read_records(&records_path).expect("records read").len(), 2);
    field_is(&entry, "id", "fox-002--confirmed");
}

#[test]
fn the_confirmation_entry_carries_the_confirmed_state() {
    let dir = temp_dir("flow-confirm-entry");
    let records_path = capture_differ_with_proposal(&dir);
    let entry = confirm(
        &write_confirmation(
            &dir,
            &confirmation_payload("fox-002", "manual-edit"),
            "confirmation.json",
        ),
        &records_path,
    )
    .expect("confirm stores");
    field_is(&entry, "corrects", "fox-002");
    field_is(&entry, "review_state", "confirmed");
    field_is(&entry, "confirmed_class", "manual-edit");
    field_is(&entry, "proposed_class", "manual-edit");
    field_is(&entry, "rescue_class", "run-interruption");
    common::assert_py_eq(
        entry.get("stalled").expect("field stands"),
        differ().get("stalled").expect("field stands"),
    );
    common::assert_py_eq(
        entry.get("human_action").expect("field stands"),
        differ().get("human_action").expect("field stands"),
    );
}

#[test]
fn the_class_history_counts_the_confirmed_class_in_place_of_the_record() {
    let dir = temp_dir("flow-confirm-history");
    let records_path = capture_differ_with_proposal(&dir);
    let before = proposals_of(&read_records(&records_path).expect("read"));
    assert!(before.contains("proposal for class run-interruption"));
    assert!(!before.contains("manual-edit"));
    confirm(
        &write_confirmation(
            &dir,
            &confirmation_payload("fox-002", "manual-edit"),
            "confirmation.json",
        ),
        &records_path,
    )
    .expect("confirm stores");
    let after = proposals_of(&read_records(&records_path).expect("read"));
    assert!(after.contains("proposal for class manual-edit"));
    assert!(!after.contains("run-interruption"));
}

#[test]
fn an_unconfirmed_proposal_counts_nowhere_in_the_class_history() {
    let dir = temp_dir("flow-unconfirmed-counts");
    let (_, records_path) = capture_one(&dir, &new_record(), Some(&player()), "records.jsonl");
    let output = proposals_of(&read_records(&records_path).expect("read"));
    assert!(output.contains("proposal for class unclassified"));
    assert!(!output.contains("new-class"));
}

#[test]
fn a_confirmation_can_settle_a_record_as_unclassified() {
    let dir = temp_dir("flow-confirm-unclassified");
    let records_path = capture_differ_with_proposal(&dir);
    confirm(
        &write_confirmation(
            &dir,
            &confirmation_payload("fox-002", "unclassified"),
            "confirmation.json",
        ),
        &records_path,
    )
    .expect("confirm stores");
    let output = proposals_of(&read_records(&records_path).expect("read"));
    assert!(output.contains("proposal for class unclassified"));
    assert!(!output.contains("run-interruption"));
}

#[test]
fn confirmation_refuses_an_object_with_an_unknown_field() {
    let dir = temp_dir("flow-confirm-unknown-field");
    let records_path = capture_differ_with_proposal(&dir);
    let mut payload = confirmation_payload("fox-002", "manual-edit");
    payload.set("note", text("ok"));
    let result = confirm(
        &write_confirmation(&dir, &payload, "confirmation.json"),
        &records_path,
    );
    assert!(matches!(result, Err(RecordError::Refused(_))));
}

#[test]
fn confirmation_refuses_a_missing_class() {
    let dir = temp_dir("flow-confirm-missing-class");
    let records_path = capture_differ_with_proposal(&dir);
    let payload = Value::object(vec![("record".to_string(), text("fox-002"))]);
    let result = confirm(
        &write_confirmation(&dir, &payload, "confirmation.json"),
        &records_path,
    );
    assert!(result.is_err());
}

#[test]
fn confirmation_refuses_a_record_that_does_not_exist() {
    let dir = temp_dir("flow-confirm-missing-record");
    let records_path = capture_differ_with_proposal(&dir);
    let result = confirm(
        &write_confirmation(
            &dir,
            &confirmation_payload("fox-999", "manual-edit"),
            "confirmation.json",
        ),
        &records_path,
    );
    assert!(result.is_err());
}

#[test]
fn confirmation_refuses_a_record_with_no_proposal_under_review() {
    let dir = temp_dir("flow-confirm-no-proposal");
    let (_, records_path) = capture_one(&dir, &agree(), None, "records.jsonl");
    let result = confirm(
        &write_confirmation(
            &dir,
            &confirmation_payload("fox-001", "prompt-rephrase"),
            "confirmation.json",
        ),
        &records_path,
    );
    assert!(result.is_err());
}

#[test]
fn confirmation_refuses_the_new_class_label() {
    let dir = temp_dir("flow-confirm-new-class");
    let records_path = capture_differ_with_proposal(&dir);
    let result = confirm(
        &write_confirmation(
            &dir,
            &confirmation_payload("fox-002", "new-class"),
            "confirmation.json",
        ),
        &records_path,
    );
    assert!(result.is_err());
}

#[test]
fn confirmation_refuses_an_unknown_class() {
    let dir = temp_dir("flow-confirm-unknown-class");
    let records_path = capture_differ_with_proposal(&dir);
    let result = confirm(
        &write_confirmation(
            &dir,
            &confirmation_payload("fox-002", "hovercraft"),
            "confirmation.json",
        ),
        &records_path,
    );
    assert!(result.is_err());
}

#[test]
fn confirmation_refuses_a_record_that_already_stands_confirmed() {
    let dir = temp_dir("flow-confirm-twice");
    let records_path = capture_differ_with_proposal(&dir);
    let payload = confirmation_payload("fox-002", "manual-edit");
    confirm(
        &write_confirmation(&dir, &payload, "confirmation.json"),
        &records_path,
    )
    .expect("first confirm stores");
    let result = confirm(
        &write_confirmation(&dir, &payload, "again.json"),
        &records_path,
    );
    assert!(result.is_err());
}

#[test]
fn a_refused_confirmation_writes_nothing() {
    let dir = temp_dir("flow-confirm-refused-writes");
    let records_path = capture_differ_with_proposal(&dir);
    let before = std::fs::read(&records_path).expect("store read");
    let result = confirm(
        &write_confirmation(
            &dir,
            &confirmation_payload("fox-002", "new-class"),
            "confirmation.json",
        ),
        &records_path,
    );
    assert!(result.is_err());
    assert_eq!(std::fs::read(&records_path).expect("store read"), before);
}
