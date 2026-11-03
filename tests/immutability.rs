//! The immutability pins of the Python corpus, stored bytes
//! standing untouched across appends.

mod common;

use cavalry::pyjson::Value;
use cavalry::records::{RecordError, capture, read_records};
use common::{make_record, temp_dir, text, with_fields, write_input};

#[test]
fn written_record_bytes_unchanged_after_correction_appended() {
    let dir = temp_dir("immutable-correction");
    let records_file = dir.join("records.jsonl");
    let one = write_input(&dir, &make_record(), "one.json");
    capture(&one, &records_file, None).expect("first capture");
    let before = std::fs::read(&records_file).expect("store read");
    let correction = with_fields(
        &make_record(),
        &[
            ("id", text("rec-002")),
            ("occurred_at", text("2026-01-01T10:00:00Z")),
            ("outcome", text("The outcome was corrected on review")),
            ("corrects", text("rec-001")),
        ],
    );
    let two = write_input(&dir, &correction, "two.json");
    capture(&two, &records_file, None).expect("correction capture");
    let after = std::fs::read(&records_file).expect("store read");
    assert_eq!(&after[..before.len()], &before[..]);
    assert!(after.len() > before.len());
    let loaded = read_records(&records_file).expect("records read");
    assert_eq!(loaded[0].get("id").and_then(Value::as_str), Some("rec-001"));
    assert_eq!(
        loaded[0].get("outcome").and_then(Value::as_str),
        Some("The task finished on the next run")
    );
    assert_eq!(
        loaded[1].get("corrects").and_then(Value::as_str),
        Some("rec-001")
    );
}

#[test]
fn plain_append_also_preserves_existing_bytes() {
    let dir = temp_dir("immutable-append");
    let records_file = dir.join("records.jsonl");
    let one = write_input(&dir, &make_record(), "one.json");
    capture(&one, &records_file, None).expect("first capture");
    let before = std::fs::read(&records_file).expect("store read");
    let second = common::overridden(&make_record(), "id", text("rec-002"));
    let two = write_input(&dir, &second, "two.json");
    capture(&two, &records_file, None).expect("second capture");
    let after = std::fs::read(&records_file).expect("store read");
    assert_eq!(&after[..before.len()], &before[..]);
}

#[test]
fn correction_naming_no_existing_record_is_refused() {
    let dir = temp_dir("immutable-bad-correction");
    let records_file = dir.join("records.jsonl");
    let one = write_input(&dir, &make_record(), "one.json");
    capture(&one, &records_file, None).expect("first capture");
    let before = std::fs::read(&records_file).expect("store read");
    let bad = with_fields(
        &make_record(),
        &[("id", text("rec-003")), ("corrects", text("rec-999"))],
    );
    let bad_input = write_input(&dir, &bad, "bad.json");
    let error = match capture(&bad_input, &records_file, None) {
        Ok(_) => panic!("capture must refuse"),
        Err(error) => error,
    };
    assert!(matches!(error, RecordError::Refused(_)));
    assert!(error.to_string().contains("corrects"));
    assert_eq!(std::fs::read(&records_file).expect("store read"), before);
}
