//! Shared helpers for the ported test corpus.

#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use cavalry::pyjson::{self, Value, py_eq};

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn value(text: &str) -> Value {
    pyjson::parse(text).expect("test fixture parses")
}

pub fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    value.get(name).unwrap_or(&Value::Null)
}

pub fn list<'a>(value: &'a Value, name: &str) -> &'a Vec<Value> {
    field(value, name).as_list().expect("field is a list")
}

pub fn text_of<'a>(value: &'a Value, name: &str) -> &'a str {
    match field(value, name) {
        Value::Str(text) => text,
        _ => panic!("field {} is a string", name),
    }
}

/// Assert two values are equal the way Python dict and list
/// equality would judge them.
pub fn assert_py_eq(left: &Value, right: &Value) {
    assert!(
        py_eq(left, right),
        "values differ\nleft:  {}\nright: {}",
        left,
        right
    );
}

/// The value with one top level field removed, the shape of the
/// Python dict comprehension the record comparisons use.
pub fn without(value: &Value, key: &str) -> Value {
    match value {
        Value::Object(entries) => Value::Object(
            entries
                .iter()
                .filter(|(name, _)| name != key)
                .cloned()
                .collect(),
        ),
        _ => panic!("value is an object"),
    }
}

/// The value with several top level fields removed.
pub fn without_many(value: &Value, keys: &[&str]) -> Value {
    match value {
        Value::Object(entries) => Value::Object(
            entries
                .iter()
                .filter(|(name, _)| !keys.contains(&name.as_str()))
                .cloned()
                .collect(),
        ),
        _ => panic!("value is an object"),
    }
}

/// One synthetic rescue record, the record_factory builder,
/// with the fields in the factory's own order.
pub fn make_record() -> Value {
    Value::object(vec![
        ("id".to_string(), Value::Str("rec-001".to_string())),
        (
            "run_ref".to_string(),
            Value::Str("run-synthetic-001".to_string()),
        ),
        (
            "rescued_role".to_string(),
            Value::Str("support-operator".to_string()),
        ),
        (
            "rescuer_role".to_string(),
            Value::Str("on-call-operator".to_string()),
        ),
        ("rescuer_kind".to_string(), Value::Str("human".to_string())),
        (
            "stalled".to_string(),
            Value::Str(
                "The task stalled because the prompt wording sent the worker in a circle"
                    .to_string(),
            ),
        ),
        (
            "human_action".to_string(),
            Value::Str("The operator rephrased the prompt and restarted the task".to_string()),
        ),
        (
            "occurred_at".to_string(),
            Value::Str("2026-01-01T09:00:00Z".to_string()),
        ),
        (
            "outcome".to_string(),
            Value::Str("The task finished on the next run".to_string()),
        ),
    ])
}

/// The entries of an object value, mutably, for the tests that
/// mutate a document the way the Python corpus mutates a dict.
pub fn object_entries_mut(value: &mut Value) -> &mut Vec<(String, Value)> {
    match value {
        Value::Object(entries) => entries,
        _ => panic!("value is an object"),
    }
}

/// One field of an object value, mutably.
pub fn field_mut<'a>(value: &'a mut Value, name: &str) -> &'a mut Value {
    let entries = object_entries_mut(value);
    let at = entries
        .iter()
        .position(|(key, _)| key == name)
        .expect("field stands");
    &mut entries[at].1
}

/// The items of a list value, mutably.
pub fn list_mut(value: &mut Value) -> &mut Vec<Value> {
    match value {
        Value::List(items) => items,
        _ => panic!("value is a list"),
    }
}

/// Remove one field from an object value.
pub fn remove_key(value: &mut Value, key: &str) {
    object_entries_mut(value).retain(|(name, _)| name != key);
}

/// A record with one field set to the value given, the shape of
/// a make_record keyword override, including a field the
/// factory does not carry.
pub fn overridden(record: &Value, key: &str, replacement: Value) -> Value {
    let mut out = record.clone();
    out.set(key, replacement);
    out
}

/// A record with one field removed.
pub fn dropped(record: &Value, key: &str) -> Value {
    without(record, key)
}

/// A record with several overrides applied in order.
pub fn with_fields(record: &Value, pairs: &[(&str, Value)]) -> Value {
    let mut out = record.clone();
    for (key, replacement) in pairs {
        out.set(key, replacement.clone());
    }
    out
}

pub fn text(value: &str) -> Value {
    Value::Str(value.to_string())
}

/// The review_factory record whose classification judgment the
/// committed fixtures answer with agreement.
pub fn agree() -> Value {
    overridden(&make_record(), "id", text("fox-001"))
}

/// The review_factory record the fixtures' classification
/// answers against its deterministic class.
pub fn differ() -> Value {
    with_fields(
        &make_record(),
        &[
            ("id", text("fox-002")),
            ("run_ref", text("run-synthetic-102")),
            (
                "stalled",
                text("The run kept writing to the wrong output and would not stop"),
            ),
            (
                "human_action",
                text("The reviewer killed the run and removed the bad output"),
            ),
            ("occurred_at", text("2026-01-02T11:30:00Z")),
            (
                "outcome",
                text("The bad output was removed and the run was queued again"),
            ),
        ],
    )
}

/// The review_factory record the fixtures read as a new class.
pub fn new_record() -> Value {
    with_fields(
        &make_record(),
        &[
            ("id", text("fox-003")),
            ("run_ref", text("run-synthetic-103")),
            (
                "stalled",
                text("A dashboard number looked odd for a reason nobody could name"),
            ),
            (
                "human_action",
                text("The maintainer checked the source by hand and wrote a note for follow up"),
            ),
            ("occurred_at", text("2026-01-04T16:45:00Z")),
            (
                "outcome",
                text("The number was explained in the note and no run was changed"),
            ),
        ],
    )
}

/// The review_factory record the fixtures abstain on.
pub fn abstain() -> Value {
    with_fields(
        &make_record(),
        &[
            ("id", text("fox-004")),
            ("run_ref", text("run-synthetic-104")),
            (
                "stalled",
                text("The generated file carried a wrong heading that the checker passed"),
            ),
            (
                "human_action",
                text("The maintainer edited the file by hand and sent it on"),
            ),
            ("occurred_at", text("2026-01-03T14:15:00Z")),
            ("outcome", text("The file went out with the right heading")),
        ],
    )
}

/// The review_factory record whose classification answer stands
/// below the confidence threshold.
pub fn faint() -> Value {
    with_fields(
        &make_record(),
        &[
            ("id", text("fox-005")),
            ("run_ref", text("run-synthetic-105")),
            (
                "stalled",
                text("The worker looped on the same step because the prompt wording was vague"),
            ),
            (
                "human_action",
                text("The operator rephrased the prompt with plainer steps"),
            ),
            ("occurred_at", text("2026-01-05T08:20:00Z")),
            (
                "outcome",
                text("The task finished once the wording was plain"),
            ),
        ],
    )
}

/// All five review_factory records.
pub fn all_review_records() -> Vec<Value> {
    vec![agree(), differ(), new_record(), abstain(), faint()]
}

/// Write a record as a capture input file, in the dumps form
/// the record_factory writer writes, and return its path.
pub fn write_input(dir: &std::path::Path, record: &Value, name: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, pyjson::dumps_preserving(record)).expect("input written");
    path
}

/// The committed fixtures directory.
pub fn fixtures_dir() -> PathBuf {
    root().join("fixtures")
}

/// A stored record line for one record at one class, in the
/// stored dumps form, with any review fields given standing
/// after the class.
pub fn stored_line(record: &Value, rescue_class: &str, extra: &[(&str, Value)]) -> String {
    let mut stored = record.clone();
    stored.set("rescue_class", text(rescue_class));
    for (key, replacement) in extra {
        stored.set(key, replacement.clone());
    }
    pyjson::dumps_compact(&stored)
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn temp_dir(tag: &str) -> PathBuf {
    let number = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "cavalry-test-{}-{}-{}",
        std::process::id(),
        tag,
        number
    ));
    std::fs::create_dir_all(&dir).expect("temp dir created");
    dir
}

pub struct CliOutput {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn run_cli(args: &[&str], cwd: &std::path::Path) -> CliOutput {
    let output = Command::new(env!("CARGO_BIN_EXE_cavalry"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("binary runs");
    CliOutput {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr is UTF-8"),
    }
}

/// Run the binary at the repo root, the working directory the
/// Python end to end tests use.
pub fn run_cli_at_root(args: &[&str]) -> CliOutput {
    run_cli(args, &root())
}

pub fn write_json(path: &std::path::Path, data: &Value) {
    let text = pyjson::dumps_compact(data);
    std::fs::write(path, text).expect("file written");
}
