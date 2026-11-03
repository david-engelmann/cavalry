//! The validation pins of the Python corpus, capture against
//! the required fields and the refusal shapes.

mod common;

use cavalry::pyjson::Value;
use cavalry::records::{REQUIRED_FIELDS, RecordError, capture, read_records};
use common::{dropped, make_record, overridden, temp_dir, text, write_input};

/// The message of a refusal the capture raises, the text the
/// Python corpus reads off its error.
fn refusal_of(result: Result<Value, RecordError>) -> String {
    match result {
        Ok(_) => panic!("capture must refuse"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn complete_record_validates_and_round_trips() {
    let dir = temp_dir("round-trip");
    let records_file = dir.join("records.jsonl");
    let input = write_input(&dir, &make_record(), "input.json");
    let stored = capture(&input, &records_file, None).expect("capture stores");
    assert_eq!(
        stored.get("rescue_class").and_then(Value::as_str),
        Some("prompt-rephrase")
    );
    let loaded = read_records(&records_file).expect("records read");
    assert_eq!(loaded.len(), 1);
    common::assert_py_eq(&loaded[0], &stored);
    for field in REQUIRED_FIELDS {
        common::assert_py_eq(
            loaded[0].get(field).expect("field stands"),
            make_record().get(field).expect("field stands"),
        );
    }
}

#[test]
fn record_missing_each_required_field_is_refused_with_field_named() {
    for field in REQUIRED_FIELDS {
        let dir = temp_dir("missing-field");
        let records_file = dir.join("records.jsonl");
        let record = dropped(&make_record(), field);
        let input = write_input(&dir, &record, "input.json");
        let message = refusal_of(capture(&input, &records_file, None));
        assert!(
            message.contains(field),
            "message names the field: {message}"
        );
        assert!(!records_file.exists());
    }
}

#[test]
fn record_with_empty_required_field_is_refused_with_field_named() {
    for field in REQUIRED_FIELDS {
        let dir = temp_dir("empty-field");
        let records_file = dir.join("records.jsonl");
        let record = overridden(&make_record(), field, text("  "));
        let input = write_input(&dir, &record, "input.json");
        let message = refusal_of(capture(&input, &records_file, None));
        assert!(
            message.contains(field),
            "message names the field: {message}"
        );
        assert!(!records_file.exists());
    }
}

#[test]
fn unknown_field_is_refused_with_field_named() {
    let dir = temp_dir("unknown-field");
    let records_file = dir.join("records.jsonl");
    let record = overridden(&make_record(), "surprise", text("nope"));
    let input = write_input(&dir, &record, "input.json");
    let message = refusal_of(capture(&input, &records_file, None));
    assert!(message.contains("surprise"));
    assert!(!records_file.exists());
}

#[test]
fn real_name_shaped_role_is_refused_with_field_named() {
    let dir = temp_dir("name-role");
    let records_file = dir.join("records.jsonl");
    let record = overridden(&make_record(), "rescuer_role", text("Jane Smith"));
    let input = write_input(&dir, &record, "input.json");
    let message = refusal_of(capture(&input, &records_file, None));
    assert!(message.contains("rescuer_role"));
    assert!(!records_file.exists());
}

#[test]
fn duplicate_id_is_refused_and_file_keeps_one_record() {
    let dir = temp_dir("duplicate");
    let records_file = dir.join("records.jsonl");
    let input = write_input(&dir, &make_record(), "input.json");
    capture(&input, &records_file, None).expect("first capture stores");
    let message = refusal_of(capture(&input, &records_file, None));
    assert!(message.contains("id"));
    assert_eq!(read_records(&records_file).expect("records read").len(), 1);
}

#[test]
fn automatic_rescue_is_refused_as_not_human() {
    let dir = temp_dir("not-human");
    let records_file = dir.join("records.jsonl");
    let record = overridden(&make_record(), "rescuer_kind", text("automatic"));
    let input = write_input(&dir, &record, "input.json");
    let message = refusal_of(capture(&input, &records_file, None));
    assert!(message.contains("rescuer_kind"));
    assert!(!records_file.exists());
}
