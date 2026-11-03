//! The command line pins of the Python corpus, the three end
//! to end runs of test_cli.

mod common;

use common::{dropped, make_record, run_cli, temp_dir, write_input};

#[test]
fn capture_list_and_propose_end_to_end() {
    let dir = temp_dir("e2e");
    let input = write_input(&dir, &make_record(), "input.json");
    let records_file = dir.join("records.jsonl");
    let input_arg = input.to_string_lossy().into_owned();
    let records_arg = records_file.to_string_lossy().into_owned();

    let captured = run_cli(&["capture", &input_arg, &records_arg], &common::root());
    assert_eq!(captured.code, 0, "{}", captured.stderr);
    assert!(captured.stdout.contains("rec-001"));
    assert!(captured.stdout.contains("prompt-rephrase"));

    let listed = run_cli(&["list", &records_arg], &common::root());
    assert_eq!(listed.code, 0, "{}", listed.stderr);
    assert!(listed.stdout.contains("rec-001"));
    assert!(listed.stdout.contains("prompt-rephrase"));
    assert!(listed.stdout.contains("2026-01-01T09:00:00Z"));

    let proposed = run_cli(&["propose", &records_arg], &common::root());
    assert_eq!(proposed.code, 0, "{}", proposed.stderr);
    assert!(
        proposed
            .stdout
            .contains("proposal for class prompt-rephrase")
    );
    assert!(proposed.stdout.contains("proposal"));
}

#[test]
fn cli_capture_refusal_names_field_and_writes_nothing() {
    let dir = temp_dir("e2e-refusal");
    let record = dropped(&make_record(), "outcome");
    let input = write_input(&dir, &record, "input.json");
    let records_file = dir.join("records.jsonl");
    let result = run_cli(
        &[
            "capture",
            &input.to_string_lossy(),
            &records_file.to_string_lossy(),
        ],
        &common::root(),
    );
    assert_ne!(result.code, 0);
    assert!(result.stderr.contains("outcome"));
    assert!(!records_file.exists());
}

#[test]
fn cli_output_is_deterministic_across_runs() {
    let dir = temp_dir("e2e-determinism");
    let input = write_input(&dir, &make_record(), "input.json");
    let records_file = dir.join("records.jsonl");
    let input_arg = input.to_string_lossy().into_owned();
    let records_arg = records_file.to_string_lossy().into_owned();
    assert_eq!(
        run_cli(&["capture", &input_arg, &records_arg], &common::root()).code,
        0
    );
    let first_list = run_cli(&["list", &records_arg], &common::root());
    let second_list = run_cli(&["list", &records_arg], &common::root());
    assert_eq!(first_list.stdout, second_list.stdout);
    let first_propose = run_cli(&["propose", &records_arg], &common::root());
    let second_propose = run_cli(&["propose", &records_arg], &common::root());
    assert_eq!(first_propose.stdout, second_propose.stdout);
}
