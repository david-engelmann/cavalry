//! CV-34. The seam at the command line, capture flag and
//! confirm, the pins of the Python judgment CLI corpus.

mod common;

use cavalry::pyjson::{self, Value};
use common::{agree, differ, fixtures_dir, run_cli, temp_dir, write_input};

fn stored_lines(records_path: &std::path::Path) -> Vec<Value> {
    let text = std::fs::read_to_string(records_path).expect("store readable");
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| pyjson::parse(line).expect("line parses"))
        .collect()
}

#[test]
fn capture_with_fixtures_stores_the_proposal() {
    let dir = temp_dir("cli-proposal");
    let records_path = dir.join("records.jsonl");
    let input = write_input(&dir, &agree(), "input.json");
    let result = run_cli(
        &[
            "capture",
            &input.to_string_lossy(),
            &records_path.to_string_lossy(),
            "--fixtures",
            &fixtures_dir().to_string_lossy(),
        ],
        &common::root(),
    );
    assert_eq!(result.code, 0, "{}", result.stderr);
    assert_eq!(result.stdout, "stored fox-001 as prompt-rephrase\n");
    let stored = stored_lines(&records_path);
    assert_eq!(stored.len(), 1);
    assert_eq!(
        stored[0].get("proposed_class").and_then(Value::as_str),
        Some("prompt-rephrase")
    );
    assert_eq!(
        stored[0].get("review_state").and_then(Value::as_str),
        Some("proposed")
    );
}

#[test]
fn capture_without_the_flag_leaves_the_record_unreviewed() {
    let dir = temp_dir("cli-unreviewed");
    let records_path = dir.join("records.jsonl");
    let input = write_input(&dir, &agree(), "input.json");
    let result = run_cli(
        &[
            "capture",
            &input.to_string_lossy(),
            &records_path.to_string_lossy(),
        ],
        &common::root(),
    );
    assert_eq!(result.code, 0, "{}", result.stderr);
    let stored = stored_lines(&records_path);
    assert_eq!(stored.len(), 1);
    assert_eq!(
        stored[0].get("review_state").and_then(Value::as_str),
        Some("unreviewed")
    );
    assert!(stored[0].get("proposed_class").is_none());
}

#[test]
fn capture_with_a_missing_fixtures_directory_writes_nothing() {
    let dir = temp_dir("cli-missing-fixtures");
    let records_path = dir.join("records.jsonl");
    let input = write_input(&dir, &agree(), "input.json");
    let missing = dir.join("nope");
    let result = run_cli(
        &[
            "capture",
            &input.to_string_lossy(),
            &records_path.to_string_lossy(),
            "--fixtures",
            &missing.to_string_lossy(),
        ],
        &common::root(),
    );
    assert_eq!(result.code, 2);
    assert!(result.stderr.starts_with("error:"));
    assert_eq!(result.stderr.matches('\n').count(), 1);
    assert!(!records_path.exists());
}

#[test]
fn capture_with_a_broken_fixture_writes_nothing() {
    let dir = temp_dir("cli-broken-fixture");
    let broken = dir.join("broken-fixtures");
    std::fs::create_dir(&broken).expect("dir created");
    let fixture = Value::object(vec![
        ("judgment_point".to_string(), common::text("quality")),
        (
            "request".to_string(),
            Value::object(vec![
                ("record_id".to_string(), common::text("fox-001")),
                ("stalled".to_string(), common::text("s")),
                ("human_action".to_string(), common::text("h")),
                ("outcome".to_string(), common::text("o")),
            ]),
        ),
        (
            "response".to_string(),
            Value::object(vec![
                ("kind".to_string(), common::text("score")),
                ("score".to_string(), Value::Float(2.0)),
                ("confidence".to_string(), Value::Float(0.9)),
            ]),
        ),
        ("synthetic".to_string(), Value::Bool(true)),
    ]);
    std::fs::write(broken.join("bad.json"), pyjson::dumps_preserving(&fixture))
        .expect("fixture written");
    let records_path = dir.join("records.jsonl");
    let input = write_input(&dir, &agree(), "input.json");
    let result = run_cli(
        &[
            "capture",
            &input.to_string_lossy(),
            &records_path.to_string_lossy(),
            "--fixtures",
            &broken.to_string_lossy(),
        ],
        &common::root(),
    );
    assert_eq!(result.code, 2);
    assert!(result.stderr.contains("bad.json"));
    assert!(!records_path.exists());
}

#[test]
fn confirm_end_to_end_moves_the_class_history() {
    let dir = temp_dir("cli-confirm-history");
    let records_path = dir.join("records.jsonl");
    let input = write_input(&dir, &differ(), "input.json");
    let captured = run_cli(
        &[
            "capture",
            &input.to_string_lossy(),
            &records_path.to_string_lossy(),
            "--fixtures",
            &fixtures_dir().to_string_lossy(),
        ],
        &common::root(),
    );
    assert_eq!(captured.code, 0, "{}", captured.stderr);
    let confirmation = dir.join("confirmation.json");
    std::fs::write(
        &confirmation,
        r#"{"record": "fox-002", "class": "manual-edit"}"#,
    )
    .expect("confirmation written");
    let result = run_cli(
        &[
            "confirm",
            &confirmation.to_string_lossy(),
            &records_path.to_string_lossy(),
        ],
        &common::root(),
    );
    assert_eq!(result.code, 0, "{}", result.stderr);
    assert_eq!(result.stdout, "confirmed fox-002 as manual-edit\n");
    let proposed = run_cli(
        &["propose", &records_path.to_string_lossy()],
        &common::root(),
    );
    assert_eq!(proposed.code, 0, "{}", proposed.stderr);
    assert!(proposed.stdout.contains("proposal for class manual-edit"));
    assert!(!proposed.stdout.contains("run-interruption"));
}

#[test]
fn confirm_refusal_exits_two_with_one_error_line() {
    let dir = temp_dir("cli-confirm-refusal");
    let records_path = dir.join("records.jsonl");
    let input = write_input(&dir, &agree(), "input.json");
    let captured = run_cli(
        &[
            "capture",
            &input.to_string_lossy(),
            &records_path.to_string_lossy(),
        ],
        &common::root(),
    );
    assert_eq!(captured.code, 0, "{}", captured.stderr);
    let confirmation = dir.join("confirmation.json");
    std::fs::write(
        &confirmation,
        r#"{"record": "fox-001", "class": "prompt-rephrase"}"#,
    )
    .expect("confirmation written");
    let result = run_cli(
        &[
            "confirm",
            &confirmation.to_string_lossy(),
            &records_path.to_string_lossy(),
        ],
        &common::root(),
    );
    assert_eq!(result.code, 2);
    assert!(result.stderr.starts_with("error:"));
    assert_eq!(result.stderr.matches('\n').count(), 1);
    assert_eq!(stored_lines(&records_path).len(), 1);
}
