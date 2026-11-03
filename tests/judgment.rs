//! CV-24 to CV-27. Requests, the recorded shape, the player,
//! and trust, the pins of the Python judgment corpus.

mod common;

use cavalry::judgment::{
    CONFIDENCE_THRESHOLD, FixturePlayer, JudgmentSource, check_answer, classification_labels,
    classification_request, quality_request, trusted, validate_fixture,
};
use cavalry::pyjson::{self, Value};
use cavalry::records::shipped_class_list;
use common::{agree, differ, fixtures_dir, temp_dir, text};

fn labels() -> Vec<String> {
    classification_labels(&shipped_class_list())
}

fn agree_pair() -> Value {
    let request = classification_request(&agree(), &shipped_class_list());
    let probabilities = Value::object(vec![
        ("prompt-rephrase".to_string(), Value::Float(0.9)),
        ("run-interruption".to_string(), Value::Float(0.04)),
        ("manual-edit".to_string(), Value::Float(0.03)),
        ("new-class".to_string(), Value::Float(0.03)),
    ]);
    let response = Value::object(vec![
        ("kind".to_string(), text("choice")),
        ("choice".to_string(), text("prompt-rephrase")),
        ("confidence".to_string(), Value::Float(0.9)),
        ("probabilities".to_string(), probabilities),
    ]);
    Value::object(vec![
        ("judgment_point".to_string(), text("classification")),
        ("request".to_string(), request),
        ("response".to_string(), response),
        ("synthetic".to_string(), Value::Bool(true)),
    ])
}

fn write_fixture(dir: &std::path::Path, name: &str, pair: &Value) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, pyjson::dumps_preserving(pair)).expect("fixture written");
    path
}

#[test]
fn classification_request_is_built_from_the_record_and_the_class_list() {
    let request = classification_request(&agree(), &shipped_class_list());
    let expected = Value::object(vec![
        ("record_id".to_string(), text("fox-001")),
        (
            "stalled".to_string(),
            agree().get("stalled").cloned().unwrap_or(Value::Null),
        ),
        (
            "human_action".to_string(),
            agree().get("human_action").cloned().unwrap_or(Value::Null),
        ),
        (
            "classes".to_string(),
            Value::List(vec![
                text("prompt-rephrase"),
                text("run-interruption"),
                text("manual-edit"),
            ]),
        ),
    ]);
    common::assert_py_eq(&request, &expected);
}

#[test]
fn quality_request_is_built_from_the_record() {
    let request = quality_request(&differ());
    let expected = Value::object(vec![
        ("record_id".to_string(), text("fox-002")),
        (
            "stalled".to_string(),
            differ().get("stalled").cloned().unwrap_or(Value::Null),
        ),
        (
            "human_action".to_string(),
            differ().get("human_action").cloned().unwrap_or(Value::Null),
        ),
        (
            "outcome".to_string(),
            differ().get("outcome").cloned().unwrap_or(Value::Null),
        ),
    ]);
    common::assert_py_eq(&request, &expected);
}

#[test]
fn labels_offer_every_class_plus_the_new_class_label() {
    assert_eq!(
        labels(),
        vec![
            "prompt-rephrase".to_string(),
            "run-interruption".to_string(),
            "manual-edit".to_string(),
            "new-class".to_string()
        ]
    );
}

#[test]
fn player_answers_a_recorded_request_exactly() {
    let player = FixturePlayer::load(&fixtures_dir()).expect("player loads");
    let answer = player
        .judge(
            "classification",
            &classification_request(&agree(), &shipped_class_list()),
        )
        .expect("an answer stands");
    assert_eq!(
        answer.get("choice").and_then(Value::as_str),
        Some("prompt-rephrase")
    );
    assert_eq!(answer.get("confidence"), Some(&Value::Float(0.9)));
}

#[test]
fn player_returns_no_answer_where_nothing_matches() {
    let dir = temp_dir("player-miss");
    write_fixture(&dir, "one.json", &agree_pair());
    let player = FixturePlayer::load(&dir).expect("player loads");
    let request = classification_request(&agree(), &shipped_class_list());
    assert!(player.judge("classification", &request).is_some());
    let mut shifted = request.clone();
    let stalled = format!(
        "{} again",
        request.get("stalled").and_then(Value::as_str).unwrap_or("")
    );
    shifted.set("stalled", text(&stalled));
    assert!(player.judge("classification", &shifted).is_none());
    assert!(
        player
            .judge("quality", &quality_request(&agree()))
            .is_none()
    );
}

#[test]
fn player_refuses_two_fixtures_for_the_same_request() {
    let dir = temp_dir("player-duplicate");
    write_fixture(&dir, "one.json", &agree_pair());
    write_fixture(&dir, "two.json", &agree_pair());
    let message = match FixturePlayer::load(&dir) {
        Ok(_) => panic!("the player must refuse"),
        Err(error) => error.to_string(),
    };
    assert!(message.contains("two.json"), "message: {message}");
}

#[test]
fn player_refuses_a_missing_directory() {
    let dir = temp_dir("player-missing").join("nope");
    assert!(FixturePlayer::load(&dir).is_err());
}

#[test]
fn validator_refuses_a_response_of_the_wrong_kind_for_the_point() {
    let mut pair = agree_pair();
    pair.set(
        "response",
        Value::object(vec![
            ("kind".to_string(), text("score")),
            ("score".to_string(), Value::Float(0.5)),
            ("confidence".to_string(), Value::Float(0.9)),
        ]),
    );
    assert!(validate_fixture("mutant", &pair).is_err());
}

#[test]
fn validator_refuses_a_choice_response_carrying_an_extra_field() {
    let mut pair = agree_pair();
    let mut response = pair.get("response").cloned().unwrap_or(Value::Null);
    response.set("score", Value::Float(0.5));
    pair.set("response", response);
    assert!(validate_fixture("mutant", &pair).is_err());
}

#[test]
fn validator_refuses_an_abstention_carrying_a_confidence() {
    let mut pair = agree_pair();
    pair.set(
        "response",
        Value::object(vec![
            ("kind".to_string(), text("abstention")),
            ("confidence".to_string(), Value::Float(0.9)),
        ]),
    );
    assert!(validate_fixture("mutant", &pair).is_err());
}

#[test]
fn validator_refuses_probabilities_missing_a_label() {
    let mut pair = agree_pair();
    let mut response = pair.get("response").cloned().unwrap_or(Value::Null);
    let mut probabilities = response
        .get("probabilities")
        .cloned()
        .unwrap_or(Value::Null);
    common::remove_key(&mut probabilities, "new-class");
    response.set("probabilities", probabilities);
    pair.set("response", response);
    assert!(validate_fixture("mutant", &pair).is_err());
}

#[test]
fn validator_refuses_probabilities_that_do_not_add_up_to_one() {
    let mut pair = agree_pair();
    let mut response = pair.get("response").cloned().unwrap_or(Value::Null);
    let mut probabilities = response
        .get("probabilities")
        .cloned()
        .unwrap_or(Value::Null);
    probabilities.set("new-class", Value::Float(0.5));
    response.set("probabilities", probabilities);
    pair.set("response", response);
    assert!(validate_fixture("mutant", &pair).is_err());
}

#[test]
fn validator_refuses_a_choice_outside_the_labels_offered() {
    let mut pair = agree_pair();
    let mut response = pair.get("response").cloned().unwrap_or(Value::Null);
    response.set("choice", text("some-other-class"));
    pair.set("response", response);
    assert!(validate_fixture("mutant", &pair).is_err());
}

#[test]
fn validator_refuses_a_fixture_not_marked_synthetic() {
    let mut pair = agree_pair();
    pair.set("synthetic", Value::Bool(false));
    assert!(validate_fixture("mutant", &pair).is_err());
}

#[test]
fn validator_refuses_a_fixture_carrying_an_extra_field() {
    let mut pair = agree_pair();
    pair.set("note", text("recorded by hand"));
    assert!(validate_fixture("mutant", &pair).is_err());
}

#[test]
fn validator_refuses_a_score_outside_the_unit_interval() {
    let pair = Value::object(vec![
        ("judgment_point".to_string(), text("quality")),
        ("request".to_string(), quality_request(&agree())),
        (
            "response".to_string(),
            Value::object(vec![
                ("kind".to_string(), text("score")),
                ("score".to_string(), Value::Float(1.4)),
                ("confidence".to_string(), Value::Float(0.9)),
            ]),
        ),
        ("synthetic".to_string(), Value::Bool(true)),
    ]);
    assert!(validate_fixture("mutant", &pair).is_err());
}

#[test]
fn a_malformed_fixture_file_refuses_the_directory_naming_the_file() {
    let dir = temp_dir("player-broken");
    let mut pair = agree_pair();
    let mut response = pair.get("response").cloned().unwrap_or(Value::Null);
    common::remove_key(&mut response, "probabilities");
    pair.set("response", response);
    write_fixture(&dir, "broken.json", &pair);
    let message = match FixturePlayer::load(&dir) {
        Ok(_) => panic!("the player must refuse"),
        Err(error) => error.to_string(),
    };
    assert!(message.contains("broken.json"), "message: {message}");
}

#[test]
fn trusted_edges_at_the_contract_threshold() {
    let base = |confidence: Value| {
        Value::object(vec![
            ("kind".to_string(), text("score")),
            ("score".to_string(), Value::Float(0.5)),
            ("confidence".to_string(), confidence),
        ])
    };
    assert!(trusted(&base(Value::Float(CONFIDENCE_THRESHOLD))));
    assert!(trusted(&base(Value::Float(1.0))));
    assert!(!trusted(&base(Value::Float(0.59))));
    assert!(!trusted(&Value::object(vec![(
        "kind".to_string(),
        text("abstention")
    )])));
    assert!(!trusted(&Value::Null));
    assert!(!trusted(&base(text("high"))));
}

#[test]
fn the_trust_threshold_is_the_contract_value() {
    // The edge test above pins trust relative to the
    // threshold constant, so it cannot see the constant's
    // own value drift. The contract value is asserted here
    // as a literal, beside a judgment trusted at the literal
    // value itself, so an edit to the constant fails this
    // test for its own reason.
    assert_eq!(CONFIDENCE_THRESHOLD, 0.6);
    let at_the_value = Value::object(vec![
        ("kind".to_string(), text("score")),
        ("score".to_string(), Value::Float(0.5)),
        ("confidence".to_string(), Value::Float(0.6)),
    ]);
    assert!(trusted(&at_the_value));
}

#[test]
fn check_answer_passes_a_well_formed_answer_through() {
    let answer = Value::object(vec![
        ("kind".to_string(), text("choice")),
        ("choice".to_string(), text("manual-edit")),
        ("confidence".to_string(), Value::Float(0.8)),
        (
            "probabilities".to_string(),
            Value::object(vec![
                ("prompt-rephrase".to_string(), Value::Float(0.1)),
                ("run-interruption".to_string(), Value::Float(0.05)),
                ("manual-edit".to_string(), Value::Float(0.8)),
                ("new-class".to_string(), Value::Float(0.05)),
            ]),
        ),
    ]);
    let labels = labels();
    let checked = check_answer("classification", Some(answer.clone()), Some(&labels));
    common::assert_py_eq(&checked.expect("answer passes"), &answer);
    let quality = Value::object(vec![
        ("kind".to_string(), text("score")),
        ("score".to_string(), Value::Float(0.4)),
        ("confidence".to_string(), Value::Float(0.7)),
    ]);
    assert!(check_answer("quality", Some(quality), None).is_some());
}

#[test]
fn check_answer_counts_a_malformed_answer_as_no_judgment() {
    let labels = labels();
    let wrong_kind = Value::object(vec![
        ("kind".to_string(), text("score")),
        ("score".to_string(), Value::Float(0.5)),
        ("confidence".to_string(), Value::Float(0.9)),
    ]);
    assert!(check_answer("classification", Some(wrong_kind), Some(&labels)).is_none());
    let missing_score = Value::object(vec![
        ("kind".to_string(), text("score")),
        ("confidence".to_string(), Value::Float(0.9)),
    ]);
    assert!(check_answer("quality", Some(missing_score), None).is_none());
    let text_score = Value::object(vec![
        ("kind".to_string(), text("score")),
        ("score".to_string(), text("deep")),
        ("confidence".to_string(), Value::Float(0.9)),
    ]);
    assert!(check_answer("quality", Some(text_score), None).is_none());
    assert!(check_answer("classification", Some(text("not an object")), Some(&labels)).is_none());
    assert!(check_answer("classification", None, Some(&labels)).is_none());
}

#[test]
fn check_answer_counts_an_answer_with_an_extra_field_as_no_judgment() {
    let answer = Value::object(vec![
        ("kind".to_string(), text("score")),
        ("score".to_string(), Value::Float(0.5)),
        ("confidence".to_string(), Value::Float(0.9)),
        ("choice".to_string(), text("recovering")),
    ]);
    assert!(check_answer("quality", Some(answer), None).is_none());
}

#[test]
fn player_answers_are_copies_the_caller_cannot_corrupt() {
    let dir = temp_dir("player-copies");
    write_fixture(&dir, "one.json", &agree_pair());
    let player = FixturePlayer::load(&dir).expect("player loads");
    let request = classification_request(&agree(), &shipped_class_list());
    let mut first = player
        .judge("classification", &request)
        .expect("an answer stands");
    first.set("choice", text("manual-edit"));
    let second = player
        .judge("classification", &request)
        .expect("an answer stands");
    assert_eq!(
        second.get("choice").and_then(Value::as_str),
        Some("prompt-rephrase")
    );
}

#[test]
fn fixture_pairs_are_not_mutated_by_validation() {
    let pair = agree_pair();
    let snapshot = pair.clone();
    validate_fixture("pair", &pair).expect("pair validates");
    common::assert_py_eq(&pair, &snapshot);
}
