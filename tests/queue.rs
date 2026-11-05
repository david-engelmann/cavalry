//! Pins for CV-48 to CV-52, the review queue surface.
//!
//! The queue prints the records whose current state still
//! waits for a human, each with the reason it waits. These
//! pins hold the membership on the records' own states, the
//! supersession a confirmation works on its namesake, the
//! reason wording per waiting class, the below-bar flag as a
//! standing reason that never queues a record on its own, the
//! empty form, and the read discipline the surface shares
//! with its sibling listings. Every pin is stated so a
//! rendering that read only a record's own line, or that let
//! the flag settle membership, or that consulted the class
//! list, would fail it.

mod common;

use cavalry::pyjson::Value;
use cavalry::records::{read_records, render_queue};
use common::{run_cli, stored_line, temp_dir, text, write_input};

fn book(dir: &std::path::Path, rows: &[String]) -> std::path::PathBuf {
    let path = dir.join("records.jsonl");
    let content: String = rows.iter().map(|row| format!("{row}\n")).collect();
    std::fs::write(&path, content).expect("book written");
    path
}

fn queue_of(dir: &std::path::Path, rows: &[String]) -> String {
    let path = book(dir, rows);
    render_queue(&read_records(&path).expect("records read"))
}

fn base_record(id: &str) -> Value {
    common::overridden(&common::make_record(), "id", text(id))
}

fn line(id: &str, class: &str, extra: &[(&str, Value)]) -> String {
    let mut record = base_record(id);
    record.set("run_ref", text(&format!("run-{id}")));
    stored_line(&record, class, extra)
}

fn printed(id: &str, class: &str, reason: &str) -> String {
    format!(
        "{id} {class} run-{id} 2026-01-01T09:00:00Z \
         The task finished on the next run {reason}\n"
    )
}

#[test]
fn a_proposed_record_prints_with_its_confirmation_reason() {
    let dir = temp_dir("queue-proposed");
    let rows = vec![line(
        "rec-001",
        "prompt-rephrase",
        &[
            ("review_state", text("proposed")),
            ("proposed_class", text("manual-edit")),
        ],
    )];
    assert_eq!(
        queue_of(&dir, &rows),
        printed(
            "rec-001",
            "prompt-rephrase",
            "awaiting confirmation as manual-edit"
        )
    );
}

#[test]
fn a_proposal_of_a_new_class_waits_with_that_class_named() {
    let dir = temp_dir("queue-new-class");
    let rows = vec![line(
        "rec-001",
        "unclassified",
        &[
            ("review_state", text("proposed")),
            ("proposed_class", text("new-class")),
        ],
    )];
    assert_eq!(
        queue_of(&dir, &rows),
        printed(
            "rec-001",
            "unclassified",
            "awaiting confirmation as new-class"
        )
    );
}

#[test]
fn an_unreviewed_record_prints_with_the_review_reason() {
    let dir = temp_dir("queue-unreviewed");
    let rows = vec![line(
        "rec-001",
        "manual-edit",
        &[("review_state", text("unreviewed"))],
    )];
    assert_eq!(
        queue_of(&dir, &rows),
        printed("rec-001", "manual-edit", "awaiting review")
    );
}

#[test]
fn a_record_carrying_no_review_state_reads_as_unreviewed() {
    let dir = temp_dir("queue-pre-seam");
    let rows = vec![line("rec-001", "manual-edit", &[])];
    assert_eq!(
        queue_of(&dir, &rows),
        printed("rec-001", "manual-edit", "awaiting review")
    );
}

#[test]
fn a_flagged_unreviewed_record_carries_the_flag_in_its_reason() {
    let dir = temp_dir("queue-flagged-unreviewed");
    let rows = vec![line(
        "rec-001",
        "unclassified",
        &[
            ("review_state", text("unreviewed")),
            ("quality_flag", text("below-bar")),
        ],
    )];
    assert_eq!(
        queue_of(&dir, &rows),
        printed(
            "rec-001",
            "unclassified",
            "awaiting review and flagged below-bar"
        )
    );
}

#[test]
fn a_flagged_proposed_record_carries_the_flag_in_its_reason() {
    let dir = temp_dir("queue-flagged-proposed");
    let rows = vec![line(
        "rec-001",
        "prompt-rephrase",
        &[
            ("review_state", text("proposed")),
            ("proposed_class", text("new-class")),
            ("quality_flag", text("below-bar")),
        ],
    )];
    assert_eq!(
        queue_of(&dir, &rows),
        printed(
            "rec-001",
            "prompt-rephrase",
            "awaiting confirmation as new-class and flagged below-bar"
        )
    );
}

#[test]
fn a_confirmed_record_prints_nothing_and_its_flag_does_not_queue_it() {
    let dir = temp_dir("queue-confirmed-flagged");
    let rows = vec![line(
        "rec-001--confirmed",
        "prompt-rephrase",
        &[
            ("corrects", text("rec-001")),
            ("review_state", text("confirmed")),
            ("confirmed_class", text("prompt-rephrase")),
            ("proposed_class", text("prompt-rephrase")),
            ("quality_flag", text("below-bar")),
        ],
    )];
    assert_eq!(queue_of(&dir, &rows), "");
}

#[test]
fn a_record_settled_by_a_confirmation_prints_nothing() {
    let dir = temp_dir("queue-settled");
    let rows = vec![
        line(
            "rec-001",
            "prompt-rephrase",
            &[
                ("review_state", text("proposed")),
                ("proposed_class", text("prompt-rephrase")),
            ],
        ),
        line(
            "rec-001--confirmed",
            "prompt-rephrase",
            &[
                ("corrects", text("rec-001")),
                ("review_state", text("confirmed")),
                ("confirmed_class", text("manual-edit")),
                ("proposed_class", text("prompt-rephrase")),
            ],
        ),
    ];
    assert_eq!(queue_of(&dir, &rows), "");
}

#[test]
fn a_confirmed_record_without_corrects_supersedes_nothing() {
    let dir = temp_dir("queue-forged-confirmed");
    let rows = vec![
        line(
            "rec-001",
            "prompt-rephrase",
            &[
                ("review_state", text("proposed")),
                ("proposed_class", text("prompt-rephrase")),
            ],
        ),
        line(
            "rec-002",
            "manual-edit",
            &[
                ("review_state", text("confirmed")),
                ("confirmed_class", text("manual-edit")),
            ],
        ),
    ];
    assert_eq!(
        queue_of(&dir, &rows),
        printed(
            "rec-001",
            "prompt-rephrase",
            "awaiting confirmation as prompt-rephrase"
        )
    );
}

#[test]
fn a_plain_correction_supersedes_nothing_for_the_queue() {
    let dir = temp_dir("queue-correction");
    let rows = vec![
        line(
            "rec-001",
            "manual-edit",
            &[("review_state", text("unreviewed"))],
        ),
        line("rec-002", "manual-edit", &[("corrects", text("rec-001"))]),
    ];
    assert_eq!(
        queue_of(&dir, &rows),
        format!(
            "{}{}",
            printed("rec-001", "manual-edit", "awaiting review"),
            printed("rec-002", "manual-edit", "awaiting review")
        )
    );
}

#[test]
fn the_queue_prints_in_file_order_with_settled_records_skipped() {
    let dir = temp_dir("queue-order");
    let rows = vec![
        line(
            "rec-001",
            "manual-edit",
            &[("review_state", text("unreviewed"))],
        ),
        line(
            "rec-002",
            "prompt-rephrase",
            &[
                ("review_state", text("proposed")),
                ("proposed_class", text("prompt-rephrase")),
            ],
        ),
        line(
            "rec-002--confirmed",
            "prompt-rephrase",
            &[
                ("corrects", text("rec-002")),
                ("review_state", text("confirmed")),
                ("confirmed_class", text("prompt-rephrase")),
                ("proposed_class", text("prompt-rephrase")),
            ],
        ),
        line(
            "rec-003",
            "run-interruption",
            &[
                ("review_state", text("proposed")),
                ("proposed_class", text("run-interruption")),
            ],
        ),
    ];
    assert_eq!(
        queue_of(&dir, &rows),
        format!(
            "{}{}",
            printed("rec-001", "manual-edit", "awaiting review"),
            printed(
                "rec-003",
                "run-interruption",
                "awaiting confirmation as run-interruption"
            )
        )
    );
}

#[test]
fn an_empty_book_prints_the_empty_text() {
    let dir = temp_dir("queue-empty");
    assert_eq!(queue_of(&dir, &[]), "");
}

#[test]
fn a_record_under_a_class_no_list_knows_prints_as_recorded() {
    let dir = temp_dir("queue-unknown-class");
    let rows = vec![line(
        "rec-001",
        "retired-class",
        &[("review_state", text("unreviewed"))],
    )];
    assert_eq!(
        queue_of(&dir, &rows),
        printed("rec-001", "retired-class", "awaiting review")
    );
}

#[test]
fn queue_on_the_committed_examples_prints_all_four_awaiting_review() {
    let out = run_cli(&["queue", "examples/records.jsonl"], &common::root());
    assert_eq!(out.code, 0);
    assert_eq!(out.stderr, "");
    assert_eq!(
        out.stdout,
        "rec-001 prompt-rephrase run-synthetic-001 2026-01-01T09:00:00Z \
         The task finished on the next run awaiting review\n\
         rec-002 run-interruption run-synthetic-002 2026-01-02T11:30:00Z \
         The bad output was removed and the run was queued again awaiting review\n\
         rec-003 manual-edit run-synthetic-003 2026-01-03T14:15:00Z \
         The file went out with the right heading awaiting review\n\
         rec-004 unclassified run-synthetic-004 2026-01-04T16:45:00Z \
         The number was explained in the note and no run was changed awaiting review\n"
    );
}

#[test]
fn queue_on_a_missing_file_refuses_in_the_lists_own_line() {
    let dir = temp_dir("queue-missing");
    let out = run_cli(&["queue", "no-such-book.jsonl"], &dir);
    assert_eq!(out.code, 2);
    assert_eq!(out.stdout, "");
    assert_eq!(
        out.stderr,
        "error: records file not found: no-such-book.jsonl\n"
    );
}

#[test]
fn queue_on_a_malformed_store_refuses_exactly_as_list_refuses() {
    let dir = temp_dir("queue-malformed");
    let rows = vec![line(
        "rec-001",
        "prompt-rephrase",
        &[("review_state", text("pending"))],
    )];
    let path = book(&dir, &rows);
    let queue = run_cli(&["queue", path.to_str().expect("path text")], &dir);
    let list = run_cli(&["list", path.to_str().expect("path text")], &dir);
    assert_eq!(queue.code, 2);
    assert_eq!(queue.stdout, "");
    assert_eq!(
        queue.stderr,
        "error: records file holds an invalid line 1: \
         invalid field: review_state holds an unknown state\n"
    );
    assert_eq!(queue.stderr, list.stderr);
    assert_eq!(queue.code, list.code);
}

#[test]
fn queue_writes_nothing_and_leaves_the_book_byte_identical() {
    let dir = temp_dir("queue-read-only");
    let rows = vec![line(
        "rec-001",
        "manual-edit",
        &[("review_state", text("unreviewed"))],
    )];
    let path = book(&dir, &rows);
    let before = std::fs::read(&path).expect("book read");
    let out = run_cli(&["queue", path.to_str().expect("path text")], &dir);
    assert_eq!(out.code, 0);
    assert_eq!(std::fs::read(&path).expect("book re-read"), before);
    let entries: Vec<String> = std::fs::read_dir(&dir)
        .expect("dir readable")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(entries, vec!["records.jsonl".to_string()]);
}

#[test]
fn queue_help_prints_its_own_help() {
    let out = run_cli(&["queue", "--help"], &common::root());
    assert_eq!(out.code, 0);
    assert_eq!(
        out.stdout,
        "Print the records still waiting for a human\n\n\
         Usage: cavalry queue <RECORDS_FILE>\n\n\
         Arguments:\n  <RECORDS_FILE>  Path to the records file to read\n\n\
         Options:\n  -h, --help  Print help\n"
    );
}

#[test]
fn queue_takes_the_separator_and_a_space_carrying_name() {
    let dir = temp_dir("queue-surface-forms");
    let rows = [line(
        "rec-001",
        "manual-edit",
        &[("review_state", text("unreviewed"))],
    )];
    let spaced = dir.join("my book.jsonl");
    let content: String = rows.iter().map(|row| format!("{row}\n")).collect();
    std::fs::write(&spaced, &content).expect("book written");
    let plain = run_cli(&["queue", "my book.jsonl"], &dir);
    let separated = run_cli(&["queue", "--", "my book.jsonl"], &dir);
    assert_eq!(plain.code, 0);
    assert_eq!(
        plain.stdout,
        printed("rec-001", "manual-edit", "awaiting review")
    );
    assert_eq!(separated.code, plain.code);
    assert_eq!(separated.stdout, plain.stdout);
    assert_eq!(separated.stderr, plain.stderr);
}

#[test]
fn queue_on_a_dead_stdout_answers_120_as_the_listings_do() {
    let dir = temp_dir("queue-dead-stdout");
    let rows = vec![line(
        "rec-001",
        "manual-edit",
        &[("review_state", text("unreviewed"))],
    )];
    let path = book(&dir, &rows);
    let out = common::run_cli_on_closed_stdout(&["queue", path.to_str().expect("path text")], &dir);
    assert_eq!(out.code, 120);
}

#[test]
fn a_record_waits_after_capture_and_stops_waiting_once_confirmed() {
    let dir = temp_dir("queue-workflow");
    let input = write_input(&dir, &common::agree(), "in.json");
    let records = dir.join("records.jsonl");
    let fixtures = common::fixtures_dir();
    let captured = run_cli(
        &[
            "capture",
            input.to_str().expect("path text"),
            records.to_str().expect("path text"),
            "--fixtures",
            fixtures.to_str().expect("path text"),
        ],
        &dir,
    );
    assert_eq!(captured.code, 0);
    let waiting = run_cli(&["queue", records.to_str().expect("path text")], &dir);
    assert_eq!(waiting.code, 0);
    assert_eq!(
        waiting.stdout,
        "fox-001 prompt-rephrase run-synthetic-001 2026-01-01T09:00:00Z \
         The task finished on the next run \
         awaiting confirmation as prompt-rephrase\n"
    );
    let confirmation = Value::object(vec![
        ("record".to_string(), text("fox-001")),
        ("class".to_string(), text("prompt-rephrase")),
    ]);
    let confirmation_path = dir.join("confirm.json");
    common::write_json(&confirmation_path, &confirmation);
    let confirmed = run_cli(
        &[
            "confirm",
            confirmation_path.to_str().expect("path text"),
            records.to_str().expect("path text"),
        ],
        &dir,
    );
    assert_eq!(confirmed.code, 0);
    let settled = run_cli(&["queue", records.to_str().expect("path text")], &dir);
    assert_eq!(settled.code, 0);
    assert_eq!(settled.stdout, "");
}

#[test]
fn an_abstained_capture_waits_for_review() {
    let dir = temp_dir("queue-workflow-abstain");
    let input = write_input(&dir, &common::abstain(), "in.json");
    let records = dir.join("records.jsonl");
    let fixtures = common::fixtures_dir();
    let captured = run_cli(
        &[
            "capture",
            input.to_str().expect("path text"),
            records.to_str().expect("path text"),
            "--fixtures",
            fixtures.to_str().expect("path text"),
        ],
        &dir,
    );
    assert_eq!(captured.code, 0);
    let waiting = run_cli(&["queue", records.to_str().expect("path text")], &dir);
    assert_eq!(waiting.code, 0);
    assert_eq!(
        waiting.stdout,
        "fox-004 manual-edit run-synthetic-104 2026-01-03T14:15:00Z \
         The file went out with the right heading awaiting review\n"
    );
}
