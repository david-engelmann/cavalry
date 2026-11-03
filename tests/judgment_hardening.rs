//! CV-35. The hardened validation discipline of the judgment
//! seam, the pins of the Python hardening corpus.
//!
//! One substitution stands in this file and it is recorded
//! here. The Python corpus builds documents with an integer
//! key standing beside a string key, and reads the named extra
//! field off the refusal, which proves the extras are ordered
//! by the printed form of the name and not by the name itself.
//! The value model of this crate keys objects by strings only,
//! so the same proof is made with two string keys whose printed
//! order is the reverse of their own order, the control
//! character key and the space key. Their raw order names the
//! control character first. Their printed order names the
//! space first, since the printed form of a space stands
//! before the backslash of the control character's escape, and
//! the refusals below name the space, which is the assertion
//! the Python corpus makes in its own terms.

mod common;

use cavalry::judgment::{
    FixturePlayer, check_answer, classification_labels, classification_request, quality_request,
    trusted, validate_fixture,
};
use cavalry::pyjson::{self, Value};
use cavalry::records::shipped_class_list;
use common::{agree, temp_dir, text};

fn labels() -> Vec<String> {
    classification_labels(&shipped_class_list())
}

/// The Python corpus's 10**400, an integer a float cannot hold.
fn huge() -> Value {
    Value::Int(format!("1{}", "0".repeat(400)))
}

fn probabilities() -> Value {
    Value::object(vec![
        ("prompt-rephrase".to_string(), Value::Float(0.9)),
        ("run-interruption".to_string(), Value::Float(0.04)),
        ("manual-edit".to_string(), Value::Float(0.03)),
        ("new-class".to_string(), Value::Float(0.03)),
    ])
}

fn choice_response() -> Value {
    Value::object(vec![
        ("kind".to_string(), text("choice")),
        ("choice".to_string(), text("prompt-rephrase")),
        ("confidence".to_string(), Value::Float(0.9)),
        ("probabilities".to_string(), probabilities()),
    ])
}

fn with_change(response: &Value, key: &str, replacement: Value) -> Value {
    common::overridden(response, key, replacement)
}

fn classification_pair(response: Value) -> Value {
    Value::object(vec![
        ("judgment_point".to_string(), text("classification")),
        (
            "request".to_string(),
            classification_request(&agree(), &shipped_class_list()),
        ),
        ("response".to_string(), response),
        ("synthetic".to_string(), Value::Bool(true)),
    ])
}

fn quality_pair(response: Value) -> Value {
    Value::object(vec![
        ("judgment_point".to_string(), text("quality")),
        ("request".to_string(), quality_request(&agree())),
        ("response".to_string(), response),
        ("synthetic".to_string(), Value::Bool(true)),
    ])
}

#[test]
fn load_refuses_a_response_whose_kind_is_not_a_string() {
    let pair = classification_pair(with_change(
        &choice_response(),
        "kind",
        Value::List(vec![text("choice")]),
    ));
    assert!(validate_fixture("mutant", &pair).is_err());
}

#[test]
fn a_directory_holding_a_non_string_kind_fixture_is_refused() {
    let dir = temp_dir("hardening-kind");
    let pair = classification_pair(with_change(
        &choice_response(),
        "kind",
        Value::List(vec![text("choice")]),
    ));
    std::fs::write(dir.join("broken.json"), pyjson::dumps_preserving(&pair))
        .expect("fixture written");
    let message = match FixturePlayer::load(&dir) {
        Ok(_) => panic!("the player must refuse"),
        Err(error) => error.to_string(),
    };
    assert!(message.contains("broken.json"), "message: {message}");
}

#[test]
fn load_refuses_a_confidence_that_is_not_a_finite_number() {
    let pair = quality_pair(Value::object(vec![
        ("kind".to_string(), text("score")),
        ("score".to_string(), Value::Float(0.5)),
        ("confidence".to_string(), huge()),
    ]));
    assert!(validate_fixture("mutant", &pair).is_err());
}

#[test]
fn load_refuses_a_score_that_is_not_a_finite_number() {
    let pair = quality_pair(Value::object(vec![
        ("kind".to_string(), text("score")),
        ("score".to_string(), huge()),
        ("confidence".to_string(), Value::Float(0.9)),
    ]));
    assert!(validate_fixture("mutant", &pair).is_err());
}

#[test]
fn load_refuses_a_probability_that_is_not_a_finite_number() {
    let mut probs = probabilities();
    probs.set("new-class", huge());
    let pair = classification_pair(with_change(&choice_response(), "probabilities", probs));
    assert!(validate_fixture("mutant", &pair).is_err());
}

#[test]
fn load_refuses_a_fixture_envelope_with_mixed_type_extra_fields() {
    // The substitution the header records. The Python corpus
    // pairs an integer key with the note key and the refusal
    // names the note. Here the control key and the space key
    // stand in, and the refusal names the space.
    let mut pair = classification_pair(choice_response());
    pair.set("\u{1}", text("recorded by hand"));
    pair.set(" ", text("recorded by hand"));
    let message = match validate_fixture("mutant", &pair) {
        Ok(()) => panic!("the fixture must refuse"),
        Err(error) => error.to_string(),
    };
    assert!(
        message.contains("carries the field ' '"),
        "message: {message}"
    );
}

#[test]
fn load_refuses_a_response_with_mixed_type_extra_fields() {
    let mut response = choice_response();
    response.set("\u{1}", text("a"));
    response.set(" ", text("b"));
    let pair = classification_pair(response);
    let message = match validate_fixture("mutant", &pair) {
        Ok(()) => panic!("the fixture must refuse"),
        Err(error) => error.to_string(),
    };
    assert!(
        message.contains("carries the field ' '"),
        "message: {message}"
    );
}

#[test]
fn check_answer_counts_a_non_string_kind_as_no_judgment() {
    let answer = with_change(
        &choice_response(),
        "kind",
        Value::List(vec![text("choice")]),
    );
    let labels = labels();
    assert!(check_answer("classification", Some(answer), Some(&labels)).is_none());
}

#[test]
fn check_answer_counts_an_unconvertible_confidence_as_no_judgment() {
    let answer = Value::object(vec![
        ("kind".to_string(), text("score")),
        ("score".to_string(), Value::Float(0.5)),
        ("confidence".to_string(), huge()),
    ]);
    assert!(check_answer("quality", Some(answer), None).is_none());
}

#[test]
fn check_answer_counts_an_unconvertible_score_as_no_judgment() {
    let answer = Value::object(vec![
        ("kind".to_string(), text("score")),
        ("score".to_string(), huge()),
        ("confidence".to_string(), Value::Float(0.9)),
    ]);
    assert!(check_answer("quality", Some(answer), None).is_none());
}

#[test]
fn check_answer_counts_an_unconvertible_probability_as_no_judgment() {
    let mut probs = probabilities();
    probs.set("new-class", huge());
    let answer = with_change(&choice_response(), "probabilities", probs);
    let labels = labels();
    assert!(check_answer("classification", Some(answer), Some(&labels)).is_none());
}

#[test]
fn check_answer_counts_mixed_type_extra_fields_as_no_judgment() {
    let mut answer = Value::object(vec![
        ("kind".to_string(), text("score")),
        ("score".to_string(), Value::Float(0.5)),
        ("confidence".to_string(), Value::Float(0.9)),
    ]);
    answer.set("\u{1}", text("a"));
    answer.set("bogus", text("b"));
    assert!(check_answer("quality", Some(answer), None).is_none());
}

#[test]
fn trusted_never_trusts_an_infinite_confidence() {
    let answer = Value::object(vec![
        ("kind".to_string(), text("score")),
        ("score".to_string(), Value::Float(0.5)),
        ("confidence".to_string(), Value::Float(f64::INFINITY)),
    ]);
    assert!(!trusted(&answer));
}

#[test]
fn trusted_returns_false_for_an_unconvertible_confidence() {
    let answer = Value::object(vec![
        ("kind".to_string(), text("score")),
        ("score".to_string(), Value::Float(0.5)),
        ("confidence".to_string(), huge()),
    ]);
    assert!(!trusted(&answer));
}

#[test]
fn probabilities_within_one_thousandth_of_one_are_accepted() {
    let probs = Value::object(vec![
        ("prompt-rephrase".to_string(), Value::Float(0.4)),
        ("run-interruption".to_string(), Value::Float(0.3)),
        ("manual-edit".to_string(), Value::Float(0.2)),
        ("new-class".to_string(), Value::Float(0.1005)),
    ]);
    let pair = classification_pair(with_change(
        &choice_response(),
        "probabilities",
        probs.clone(),
    ));
    validate_fixture("mutant", &pair).expect("the pair validates");
    let answer = with_change(&choice_response(), "probabilities", probs);
    let labels = labels();
    let checked = check_answer("classification", Some(answer.clone()), Some(&labels));
    common::assert_py_eq(&checked.expect("the answer passes"), &answer);
}

#[test]
fn probabilities_beyond_one_thousandth_of_one_are_refused() {
    let probs = Value::object(vec![
        ("prompt-rephrase".to_string(), Value::Float(0.4)),
        ("run-interruption".to_string(), Value::Float(0.3)),
        ("manual-edit".to_string(), Value::Float(0.2)),
        ("new-class".to_string(), Value::Float(0.102)),
    ]);
    let pair = classification_pair(with_change(
        &choice_response(),
        "probabilities",
        probs.clone(),
    ));
    assert!(validate_fixture("mutant", &pair).is_err());
    let answer = with_change(&choice_response(), "probabilities", probs);
    let labels = labels();
    assert!(check_answer("classification", Some(answer), Some(&labels)).is_none());
}
