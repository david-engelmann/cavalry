//! CV-25 and CV-26. The committed fixtures and their schema
//! pin, the pins of the Python fixture corpus.

mod common;

use cavalry::judgment::{
    CONFIDENCE_THRESHOLD, FixturePlayer, JudgmentSource, QUALITY_BAR, validate_fixture,
};
use cavalry::pyjson::{self, Value};
use common::fixtures_dir;

fn committed_pairs() -> Vec<(String, Value)> {
    let dir = fixtures_dir();
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("fixtures readable")
        .map(|entry| {
            entry
                .expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with(".json"))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let text = std::fs::read_to_string(dir.join(&name)).expect("fixture readable");
            let pair = pyjson::parse(&text).expect("fixture parses");
            (name, pair)
        })
        .collect()
}

#[test]
fn every_committed_fixture_matches_the_recorded_shape() {
    let pairs = committed_pairs();
    assert!(!pairs.is_empty(), "no committed fixtures found");
    for (name, pair) in &pairs {
        validate_fixture(name, pair)
            .unwrap_or_else(|error| panic!("fixture {name} validates: {error}"));
    }
}

#[test]
fn every_committed_fixture_is_marked_synthetic() {
    for (name, pair) in committed_pairs() {
        assert_eq!(
            pair.get("synthetic"),
            Some(&Value::Bool(true)),
            "fixture {name} is not marked synthetic"
        );
    }
}

#[test]
fn the_player_loads_the_committed_directory() {
    let player = FixturePlayer::load(&fixtures_dir()).expect("player loads");
    for (_, pair) in committed_pairs() {
        let point = pair
            .get("judgment_point")
            .and_then(Value::as_str)
            .unwrap_or("");
        let request = pair.get("request").expect("request stands");
        assert!(player.judge(point, request).is_some());
    }
}

#[test]
fn the_committed_set_covers_both_judgment_points() {
    let pairs = committed_pairs();
    let mut points: Vec<&str> = pairs
        .iter()
        .map(|(_, pair)| {
            pair.get("judgment_point")
                .and_then(Value::as_str)
                .unwrap_or("")
        })
        .collect();
    points.sort();
    points.dedup();
    assert_eq!(points, vec!["classification", "quality"]);
}

#[test]
fn the_classification_set_covers_agreement_difference_and_the_new_class_label() {
    let mut choices: Vec<String> = Vec::new();
    for (_, pair) in committed_pairs() {
        if pair.get("judgment_point").and_then(Value::as_str) != Some("classification") {
            continue;
        }
        let response = pair.get("response").expect("response stands");
        if response.get("kind").and_then(Value::as_str) == Some("choice") {
            choices.push(
                response
                    .get("choice")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            );
        }
    }
    choices.sort();
    choices.dedup();
    // fox-001 agrees with its deterministic class, fox-002
    // proposes a different existing class, and fox-003 proposes
    // the new label.
    assert_eq!(choices, vec!["manual-edit", "new-class", "prompt-rephrase"]);
}

#[test]
fn the_classification_set_has_an_abstention_and_a_below_threshold_case() {
    let pairs = committed_pairs();
    let responses: Vec<&Value> = pairs
        .iter()
        .filter(|(_, pair)| {
            pair.get("judgment_point").and_then(Value::as_str) == Some("classification")
        })
        .map(|(_, pair)| pair.get("response").expect("response stands"))
        .collect();
    assert!(
        responses
            .iter()
            .any(|response| response.get("kind").and_then(Value::as_str) == Some("abstention"))
    );
    assert!(responses.iter().any(|response| {
        response.get("kind").and_then(Value::as_str) == Some("choice")
            && response
                .get("confidence")
                .and_then(|value| match value {
                    Value::Float(number) => Some(*number),
                    _ => None,
                })
                .unwrap_or(1.0)
                < CONFIDENCE_THRESHOLD
    }));
}

#[test]
fn the_quality_set_covers_scores_on_both_sides_of_the_bar_and_an_abstention() {
    let pairs = committed_pairs();
    let responses: Vec<&Value> = pairs
        .iter()
        .filter(|(_, pair)| pair.get("judgment_point").and_then(Value::as_str) == Some("quality"))
        .map(|(_, pair)| pair.get("response").expect("response stands"))
        .collect();
    let scores: Vec<f64> = responses
        .iter()
        .filter(|response| response.get("kind").and_then(Value::as_str) == Some("score"))
        .map(|response| match response.get("score") {
            Some(Value::Float(number)) => *number,
            _ => f64::NAN,
        })
        .collect();
    assert!(scores.iter().any(|score| *score >= QUALITY_BAR));
    assert!(scores.iter().any(|score| *score < QUALITY_BAR));
    assert!(
        responses
            .iter()
            .any(|response| response.get("kind").and_then(Value::as_str) == Some("abstention"))
    );
}
