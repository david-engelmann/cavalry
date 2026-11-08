//! The joint pin for CV-57, the settled set both renderings
//! read from one helper.
//!
//! The queue pins and the proposals pins each hold their own
//! surface against the supersession a confirmation works, and
//! this pin holds the two surfaces against each other on one
//! book. A confirmation settles its namesake here, and the
//! settled record must vanish from the queue and count in the
//! proposals only through the confirmation, under the
//! confirmed class and never under the class it was stored
//! with. A rendering that built its settled set apart from
//! the other, and read the book under a different
//! supersession, fails one half of this pin or the other.

mod common;

use cavalry::pyjson::Value;
use cavalry::records::{render_proposals, render_queue, shipped_class_list};
use common::text;

fn stored(record_id: &str, rescue_class: &str) -> Value {
    Value::object(vec![
        ("id".to_string(), text(record_id)),
        ("rescue_class".to_string(), text(rescue_class)),
        ("run_ref".to_string(), text(&format!("run-{record_id}"))),
        ("occurred_at".to_string(), text("2026-01-01T09:00:00Z")),
        (
            "outcome".to_string(),
            text("The task finished on the next run"),
        ),
    ])
}

#[test]
fn one_confirmation_settles_its_namesake_for_both_renderings() {
    let mut proposed = stored("rec-001", "prompt-rephrase");
    proposed.set("review_state", text("proposed"));
    proposed.set("proposed_class", text("manual-edit"));
    let mut confirmation = stored("rec-001--confirmed", "manual-edit");
    confirmation.set("corrects", text("rec-001"));
    confirmation.set("review_state", text("confirmed"));
    confirmation.set("confirmed_class", text("manual-edit"));
    let waiting = stored("rec-003", "run-interruption");
    let records = vec![proposed, confirmation, waiting];

    let queue = render_queue(&records);
    assert_eq!(
        queue,
        "rec-003 run-interruption run-rec-003 2026-01-01T09:00:00Z \
         The task finished on the next run awaiting review\n",
        "the settled record and its confirmation print nothing, \
         and the unsettled record still waits"
    );

    let proposals = render_proposals(&records, &shipped_class_list()).expect("proposals render");
    assert!(
        proposals.contains("proposal for class manual-edit"),
        "the settled record counts through its confirmation: {proposals}"
    );
    assert!(
        proposals.contains("proposal for class run-interruption"),
        "the unsettled record counts under its stored class: {proposals}"
    );
    assert!(
        !proposals.contains("proposal for class prompt-rephrase"),
        "the settled record never counts under its stored class: {proposals}"
    );
}
