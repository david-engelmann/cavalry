//! The classification pins of the Python corpus, the class
//! list and the keyword matching.

mod common;

use cavalry::pyjson::Value;
use cavalry::records::{ClassList, classify, shipped_class_list};
use common::text;

fn record(stalled: &str, human_action: &str) -> Value {
    Value::object(vec![
        ("stalled".to_string(), text(stalled)),
        ("human_action".to_string(), text(human_action)),
    ])
}

fn classify_ship(stalled: &str, human_action: &str) -> String {
    classify(&record(stalled, human_action), &shipped_class_list())
}

#[test]
fn class_list_is_versioned_and_held_in_repo() {
    let class_list = shipped_class_list();
    // The version rose to 2 as the mechanical consequence of
    // the CV-17 amendment, which replaced the keyword stems
    // with spelled out forms.
    assert_eq!(class_list.version, "2");
    let ids: Vec<&str> = class_list
        .classes
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec!["prompt-rephrase", "run-interruption", "manual-edit"]
    );
    for entry in &class_list.classes {
        assert!(!entry.keywords.is_empty());
        assert!(entry.proposal.contains("proposal"));
    }
}

#[test]
fn matching_record_lands_in_named_class() {
    let cases = [
        (
            "The task stalled because the prompt wording confused the worker",
            "The operator rephrased the prompt",
            "prompt-rephrase",
        ),
        (
            "The run kept writing the wrong output and would not stop",
            "The reviewer killed the run",
            "run-interruption",
        ),
        (
            "The generated file carried a wrong heading",
            "The maintainer edited the file by hand",
            "manual-edit",
        ),
    ];
    for (stalled, human_action, expected) in cases {
        assert_eq!(classify_ship(stalled, human_action), expected);
    }
}

#[test]
fn record_matching_nothing_lands_in_unclassified() {
    let result = classify_ship(
        "A dashboard number looked odd for a reason nobody could name",
        "The operator checked the source by hand and wrote a note",
    );
    assert_eq!(result, cavalry::records::UNCLASSIFIED);
    assert_eq!(result, "unclassified");
}

#[test]
fn first_matching_class_in_list_order_wins() {
    let result = classify_ship(
        "The prompt wording stalled the task",
        "The operator rephrased the prompt and then killed the run",
    );
    assert_eq!(result, "prompt-rephrase");
}

#[test]
fn keyword_inside_a_longer_word_does_not_classify() {
    let cases = [
        (
            "The task needed a skilled hand to finish it",
            "A skilled operator restarted the service by hand",
        ),
        (
            "The label run had not finished printing",
            "The operator dispatched a courier with the printed label",
        ),
        (
            "The account balance was wrong after the import",
            "The operator credited the account with the missing amount",
        ),
    ];
    for (stalled, human_action) in cases {
        assert_eq!(
            classify_ship(stalled, human_action),
            cavalry::records::UNCLASSIFIED
        );
    }
}

#[test]
fn spelled_out_word_forms_still_classify() {
    let cases = [
        (
            "The wording tangled the worker",
            "The operator is rephrasing the prompt now",
            "prompt-rephrase",
        ),
        (
            "The run would not halt on its own",
            "The operator interrupted the run",
            "run-interruption",
        ),
        (
            "The loop kept running unchecked",
            "An interruption by the operator ended it",
            "run-interruption",
        ),
    ];
    for (stalled, human_action, expected) in cases {
        assert_eq!(classify_ship(stalled, human_action), expected);
    }
}

#[test]
fn keyword_case_is_ignored_on_the_keyword_side() {
    // The dictionary the Python corpus hands classify carries
    // the keyword uncased, as a loaded list never would, and
    // the match still lands, because the pattern is built from
    // the lowered keyword at match time. The list here is built
    // by hand for the same reason, bypassing the load and its
    // own lowering.
    let class_list = ClassList {
        version: "2".to_string(),
        classes: vec![cavalry::records::ClassEntry {
            id: "loud-class".to_string(),
            keywords: vec!["KILLED".to_string()],
            proposal: "Draft a test proposal only".to_string(),
        }],
        unclassified_proposal: "Draft a test proposal only".to_string(),
    };
    let result = classify(
        &record("The run kept going", "The operator killed the run"),
        &class_list,
    );
    assert_eq!(result, "loud-class");
}
