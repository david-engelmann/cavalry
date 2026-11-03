//! The contracted ceilings and the standard output failure
//! shapes, pinned at the command line.
//!
//! CV-42 states the reader's deepest container path and the
//! integer digit limit, and CV-43 states the two shapes of a
//! standard output that cannot take the bytes, divided at
//! the size of the Python tool's output buffer. The probes
//! and differentials of the port slice measured each value
//! against the Python tool; these pins hold the committed
//! suite able to fail if any of them drifts.

mod common;

use std::path::PathBuf;

use common::{
    make_record, overridden, run_cli, run_cli_on_closed_stdout, stored_line, temp_dir, text,
};

fn write_records(dir: &std::path::Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, content).expect("records file written");
    path
}

#[test]
fn reader_carries_a_records_line_at_the_depth_ceiling_and_refuses_past_it() {
    // The carrier is one records line of nested lists, 9994
    // opening brackets and 9994 closing brackets, so the
    // line's deepest container path is 9994 containers, the
    // ceiling CV-42 states. The line is carried by the
    // reader and refused by the stored shape check, at
    // status 2 with the tool's line, exactly as the Python
    // tool answers it.
    let dir = temp_dir("ceiling-depth");
    let carried = write_records(
        &dir,
        "deep.jsonl",
        &format!("{}{}\n", "[".repeat(9994), "]".repeat(9994)),
    );
    let result = run_cli(&["list", &carried.to_string_lossy()], &common::root());
    assert_eq!(result.code, 2, "{}", result.stderr);
    assert_eq!(
        result.stderr,
        "error: records file holds an invalid line 1: line is not a JSON object\n"
    );
    // One bracket more, 9995 opening and 9995 closing, stands
    // past the ceiling. The Python reader lets the failure
    // escape uncaught, and the port answers it at status 1
    // in its own line.
    let refused = write_records(
        &dir,
        "deeper.jsonl",
        &format!("{}{}\n", "[".repeat(9995), "]".repeat(9995)),
    );
    let result = run_cli(&["list", &refused.to_string_lossy()], &common::root());
    assert_eq!(result.code, 1, "{}", result.stderr);
    assert_eq!(result.stderr, "error: records file is nested too deeply\n");
}

#[test]
fn reader_carries_an_integer_at_the_digit_limit_and_refuses_past_it() {
    // The carrier is one records line holding a bare integer
    // of 4300 digits, the limit CV-42 states. The integer
    // converts, and the line is refused by the stored shape
    // check at status 2, exactly as the Python tool answers.
    let dir = temp_dir("ceiling-digits");
    let carried = write_records(&dir, "digits.jsonl", &format!("{}\n", "9".repeat(4300)));
    let result = run_cli(&["list", &carried.to_string_lossy()], &common::root());
    assert_eq!(result.code, 2, "{}", result.stderr);
    assert_eq!(
        result.stderr,
        "error: records file holds an invalid line 1: line is not a JSON object\n"
    );
    // One digit more, 4301, fails the Python reader's
    // conversion uncaught, and the port answers it at
    // status 1 in its own line.
    let refused = write_records(&dir, "moredigits.jsonl", &format!("{}\n", "9".repeat(4301)));
    let result = run_cli(&["list", &refused.to_string_lossy()], &common::root());
    assert_eq!(result.code, 1, "{}", result.stderr);
    assert_eq!(
        result.stderr,
        "error: records file holds an integer with too many digits\n"
    );
}

/// A records file of one stored record whose outcome is
/// padded so the list document is exactly `target` bytes,
/// proven by an open run before the closed one is made.
fn sized_records_file(dir: &std::path::Path, target: usize) -> PathBuf {
    let path = dir.join("sized.jsonl");
    let write_with = |outcome: String| {
        let record = overridden(&make_record(), "outcome", text(&outcome));
        let line = stored_line(&record, "prompt-rephrase", &[]);
        std::fs::write(&path, format!("{line}\n")).expect("records file written");
    };
    write_with("x".to_string());
    let probe = run_cli(&["list", &path.to_string_lossy()], &common::root());
    assert_eq!(probe.code, 0, "{}", probe.stderr);
    let pad = target - (probe.stdout.len() - 1);
    write_with("x".repeat(pad));
    let sized = run_cli(&["list", &path.to_string_lossy()], &common::root());
    assert_eq!(sized.code, 0, "{}", sized.stderr);
    assert_eq!(sized.stdout.len(), target, "the document is sized");
    path
}

#[test]
fn a_dead_stdout_under_the_buffer_answers_120() {
    // A list document of exactly 8191 bytes stands under
    // the 8192 byte buffer CV-43 states, so the failure
    // surfaces only at the exit flush and answers 120, in
    // the port's own line where the Python tool prints its
    // shutdown report.
    let dir = temp_dir("pipe-under");
    let records = sized_records_file(&dir, 8191);
    let result = run_cli_on_closed_stdout(&["list", &records.to_string_lossy()], &common::root());
    assert_eq!(result.code, 120, "{}", result.stderr);
    assert_eq!(result.stderr, "error: stdout: Broken pipe (os error 32)\n");
}

#[test]
fn a_dead_stdout_at_the_buffer_answers_2_with_the_system_line() {
    // A list document of exactly 8192 bytes reaches the
    // buffer's size, so the failure surfaces during the
    // write and the command line catches it as a system
    // error, answering status 2 with the platform error
    // line, byte for byte the line the Python tool prints.
    let dir = temp_dir("pipe-at");
    let records = sized_records_file(&dir, 8192);
    let result = run_cli_on_closed_stdout(&["list", &records.to_string_lossy()], &common::root());
    assert_eq!(result.code, 2, "{}", result.stderr);
    assert_eq!(result.stderr, "error: [Errno 32] Broken pipe\n");
}
