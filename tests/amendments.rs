//! The amendment pins of the Python corpus, CV-14 through
//! CV-22 in their own groups. The class list seam the Python
//! corpus reaches by monkeypatching the module path is the
//! load from a path the library exposes for exactly this.

mod common;

use cavalry::pyjson::{self, Value};
use cavalry::records::{
    RecordError, capture, load_class_list_at, read_records, render_list, render_proposals,
    shipped_class_list,
};
use common::{make_record, overridden, run_cli, temp_dir, text, write_input};

fn refusal_of<T>(result: Result<T, RecordError>) -> String {
    match result {
        Ok(_) => panic!("the call must refuse"),
        Err(error) => error.to_string(),
    }
}

fn write_store(dir: &std::path::Path, content: &str) -> std::path::PathBuf {
    let path = dir.join("records.jsonl");
    std::fs::write(&path, content).expect("store written");
    path
}

fn stored_record(overrides: &[(&str, Value)]) -> Value {
    let mut record = make_record();
    record.set("rescue_class", text("prompt-rephrase"));
    for (key, replacement) in overrides {
        record.set(key, replacement.clone());
    }
    record
}

// CV-14. Unterminated store.

#[test]
fn capture_repairs_an_unterminated_store() {
    let dir = temp_dir("unterminated");
    let records_file = dir.join("records.jsonl");
    let one = write_input(&dir, &make_record(), "one.json");
    capture(&one, &records_file, None).expect("first capture");
    let original = std::fs::read(&records_file).expect("store read");
    assert!(original.ends_with(b"\n"));
    std::fs::write(&records_file, &original[..original.len() - 1]).expect("truncated");
    let two_record = common::with_fields(
        &make_record(),
        &[
            ("id", text("rec-002")),
            ("occurred_at", text("2026-01-01T10:00:00Z")),
        ],
    );
    let two = write_input(&dir, &two_record, "two.json");
    capture(&two, &records_file, None).expect("second capture");
    let final_bytes = std::fs::read(&records_file).expect("store read");
    assert_eq!(
        &final_bytes[..original.len() - 1],
        &original[..original.len() - 1]
    );
    let loaded = read_records(&records_file).expect("records read");
    let ids: Vec<&str> = loaded
        .iter()
        .map(|record| record.get("id").and_then(Value::as_str).unwrap_or(""))
        .collect();
    assert_eq!(ids, vec!["rec-001", "rec-002"]);
    let listed = render_list(&loaded);
    assert!(listed.contains("rec-001"));
    assert!(listed.contains("rec-002"));
}

// CV-15. Stored line shape validation.

#[test]
fn non_object_store_line_is_refused() {
    for line in ["[1, 2, 3]", "\"just a string\"", "42"] {
        let dir = temp_dir("non-object-line");
        let store = write_store(&dir, &format!("{line}\n"));
        assert!(read_records(&store).is_err(), "line {line} refuses");
    }
}

#[test]
fn store_line_missing_a_field_is_refused_with_field_named() {
    let dir = temp_dir("line-missing-field");
    let record = common::dropped(&stored_record(&[]), "outcome");
    let store = write_store(&dir, &format!("{}\n", pyjson::dumps_preserving(&record)));
    let message = refusal_of(read_records(&store));
    assert!(message.contains("outcome"));
}

#[test]
fn store_line_missing_rescue_class_is_refused_with_field_named() {
    let dir = temp_dir("line-missing-class");
    let record = common::dropped(&stored_record(&[]), "rescue_class");
    let store = write_store(&dir, &format!("{}\n", pyjson::dumps_preserving(&record)));
    let message = refusal_of(read_records(&store));
    assert!(message.contains("rescue_class"));
}

#[test]
fn store_line_with_an_unknown_field_is_refused_with_field_named() {
    let dir = temp_dir("line-unknown-field");
    let record = stored_record(&[("surprise", text("nope"))]);
    let store = write_store(&dir, &format!("{}\n", pyjson::dumps_preserving(&record)));
    let message = refusal_of(read_records(&store));
    assert!(message.contains("surprise"));
}

#[test]
fn capture_onto_a_malformed_store_writes_nothing() {
    let dir = temp_dir("malformed-store");
    let store = write_store(&dir, "[1, 2, 3]\n");
    let before = std::fs::read(&store).expect("store read");
    let input = write_input(&dir, &make_record(), "input.json");
    assert!(capture(&input, &store, None).is_err());
    assert_eq!(std::fs::read(&store).expect("store read"), before);
}

#[test]
fn cli_list_on_a_malformed_store_exits_2_without_a_traceback() {
    let dir = temp_dir("malformed-cli");
    let store = write_store(&dir, "{\"id\": \"rec-x\"}\n");
    let result = run_cli(&["list", &store.to_string_lossy()], &common::root());
    assert_eq!(result.code, 2);
    assert!(!result.stderr.contains("Traceback"));
}

// CV-16. Unknown stored class in propose.

#[test]
fn propose_refuses_an_unknown_stored_class_and_names_it() {
    let record = stored_record(&[("rescue_class", text("bogus-class"))]);
    let loaded = vec![record];
    assert!(render_list(&loaded).contains("bogus-class"));
    let message = refusal_of(render_proposals(&loaded, &shipped_class_list()));
    assert!(message.contains("bogus-class"));
}

// CV-18. Role label over the whole value.

#[test]
fn role_label_with_a_trailing_newline_is_refused() {
    let dir = temp_dir("role-newline");
    let records_file = dir.join("records.jsonl");
    let record = overridden(&make_record(), "rescuer_role", text("operator\n"));
    let input = write_input(&dir, &record, "input.json");
    let message = refusal_of(capture(&input, &records_file, None));
    assert!(message.contains("rescuer_role"));
    assert!(!records_file.exists());
}

// CV-19. Class list validation at load.

fn valid_class_list_text() -> String {
    r#"{"version": 2, "unclassified_proposal": "Draft a review. This is a proposal only.", "classes": [{"id": "some-class", "keywords": ["something"], "proposal": "Draft a rule. This is a proposal only."}]}"#
        .to_string()
}

fn class_list_file(dir: &std::path::Path, content: &str) -> std::path::PathBuf {
    let path = dir.join("classes.json");
    std::fs::write(&path, content).expect("list written");
    path
}

fn mutated_list(dir: &std::path::Path, mutate: impl Fn(&mut Value)) -> std::path::PathBuf {
    let mut data = common::value(&valid_class_list_text());
    mutate(&mut data);
    class_list_file(dir, &pyjson::dumps_preserving(&data))
}

#[test]
fn class_list_entry_missing_keywords_is_refused() {
    let dir = temp_dir("list-no-keywords");
    let path = mutated_list(&dir, |data| {
        let classes = common::list_mut(common::field_mut(data, "classes"));
        common::remove_key(&mut classes[0], "keywords");
    });
    let message = refusal_of(load_class_list_at(&path));
    assert!(message.contains("some-class"));
    assert!(message.contains("keywords"));
}

#[test]
fn class_list_entry_missing_proposal_is_refused() {
    let dir = temp_dir("list-no-proposal");
    let path = mutated_list(&dir, |data| {
        let classes = common::list_mut(common::field_mut(data, "classes"));
        common::remove_key(&mut classes[0], "proposal");
    });
    let message = refusal_of(load_class_list_at(&path));
    assert!(message.contains("some-class"));
    assert!(message.contains("proposal"));
}

#[test]
fn class_list_missing_unclassified_proposal_is_refused() {
    let dir = temp_dir("list-no-unclassified");
    let path = mutated_list(&dir, |data| {
        common::remove_key(data, "unclassified_proposal");
    });
    let message = refusal_of(load_class_list_at(&path));
    assert!(message.contains("unclassified_proposal"));
}

#[test]
fn class_list_keywords_are_lowercased_at_load() {
    let dir = temp_dir("list-lowercase");
    let path = mutated_list(&dir, |data| {
        let classes = common::list_mut(common::field_mut(data, "classes"));
        classes[0].set("keywords", Value::List(vec![text("KILLED")]));
    });
    let loaded = load_class_list_at(&path).expect("list loads");
    assert_eq!(loaded.classes[0].keywords, vec!["killed".to_string()]);
}

// CV-20. Unclassified proposal text lives in the class list.

#[test]
fn unclassified_proposal_text_comes_from_the_class_list() {
    let dir = temp_dir("list-unclassified-text");
    let path = mutated_list(&dir, |data| {
        data.set(
            "unclassified_proposal",
            text("Draft a bespoke unclassified review. This is a proposal only."),
        );
    });
    let list = load_class_list_at(&path).expect("list loads");
    let record = stored_record(&[("rescue_class", text("unclassified"))]);
    let output = render_proposals(&[record], &list).expect("proposals render");
    assert!(output.contains("bespoke unclassified review"));
}

#[test]
fn shipped_unclassified_proposal_matches_the_class_list_file() {
    let class_list = shipped_class_list();
    let record = stored_record(&[("rescue_class", text("unclassified"))]);
    let output = render_proposals(&[record], &class_list).expect("proposals render");
    assert!(output.contains(&class_list.unclassified_proposal));
}

// CV-21. occurred_at shape.

#[test]
fn occurred_at_in_another_shape_is_refused() {
    for bad in [
        "January 1 2026",
        "2026-01-01 09:00:00",
        "2026-01-01",
        "banana",
    ] {
        let dir = temp_dir("bad-timestamp");
        let records_file = dir.join("records.jsonl");
        let record = overridden(&make_record(), "occurred_at", text(bad));
        let input = write_input(&dir, &record, "input.json");
        let message = refusal_of(capture(&input, &records_file, None));
        assert!(message.contains("occurred_at"));
        assert!(!records_file.exists());
    }
}

#[test]
fn occurred_at_without_a_trailing_z_is_accepted() {
    let dir = temp_dir("timestamp-no-z");
    let records_file = dir.join("records.jsonl");
    let record = overridden(&make_record(), "occurred_at", text("2026-01-01T09:00:00"));
    let input = write_input(&dir, &record, "input.json");
    let stored = capture(&input, &records_file, None).expect("capture stores");
    assert_eq!(
        stored.get("occurred_at").and_then(Value::as_str),
        Some("2026-01-01T09:00:00")
    );
}

// CV-22. Newline safety.

#[test]
fn field_value_carrying_a_newline_is_refused() {
    let dir = temp_dir("newline-value");
    let records_file = dir.join("records.jsonl");
    let record = overridden(
        &make_record(),
        "outcome",
        text("The task finished\nand a second line appeared"),
    );
    let input = write_input(&dir, &record, "input.json");
    let message = refusal_of(capture(&input, &records_file, None));
    assert!(message.contains("outcome"));
    assert!(!records_file.exists());
}
