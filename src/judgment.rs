//! The judgment seam.
//!
//! Cavalry records the rescues only a human could make, and the
//! judgment a record carries comes from outside the tool, from a
//! source the operator supplies. This module is the seam that
//! judgment crosses. It states the two judgment points, the
//! classification point and the quality point, the fixture
//! document a judgment is recorded in, the validation a recorded
//! judgment must survive before anything trusts it, and the
//! fixture player that answers from a directory of recorded
//! judgments. The tool itself judges nothing here. It validates
//! what a source claims, decides whether the claim is trusted,
//! and writes down what stands.
//!
//! A fixture file is a JSON document with exactly four fields,
//! the judgment point, the request, the response, and a mark
//! that the judgment is synthetic. A response is an abstention,
//! a score with its confidence, or a choice with its confidence
//! and a probability for every label offered, and the fields of
//! a response are exact for its kind, because a response
//! carrying a field its kind does not carry is a document the
//! seam cannot read as the answer it claims to be. A claim is
//! trusted exactly when it is a score or a choice whose
//! confidence reaches the threshold.

use crate::pyjson::{self, ParseError, Value};
use std::collections::HashMap;
use std::path::Path;

/// The classification judgment point, which answers with a
/// choice among the labels offered or an abstention.
pub const CLASSIFICATION: &str = "classification";

/// The quality judgment point, which answers with a score or an
/// abstention.
pub const QUALITY: &str = "quality";

/// The confidence a judgment must reach before the tool trusts
/// it enough to write a proposal or a flag from it.
pub const CONFIDENCE_THRESHOLD: f64 = 0.6;

/// The quality score below which a trusted quality judgment
/// flags the record it judged.
pub const QUALITY_BAR: f64 = 0.5;

/// The label a classification judgment gives a rescue it reads
/// as belonging to no class on the books.
pub const NEW_CLASS_LABEL: &str = "new-class";

/// The deepest container path a fixture's request key document
/// may carry, measured on the Python implementation the port
/// mirrors. The key document wraps the request beside the
/// point, and its building in the Python player fails uncaught
/// past a path of 9991 containers.
const KEY_MAX_PATH: usize = 9991;

const FIXTURE_FIELDS: [&str; 4] = ["judgment_point", "request", "response", "synthetic"];
const ABSTENTION_FIELDS: [&str; 1] = ["kind"];
const SCORE_FIELDS: [&str; 3] = ["kind", "score", "confidence"];
const CHOICE_FIELDS: [&str; 4] = ["kind", "choice", "confidence", "probabilities"];

/// A judgment refusal, in the two kinds the Python seam's
/// failures divide into at the tool's surfaces.
///
/// A refusal is a defect the seam names in its own words, which
/// the command line answers at status 2. The uncaught kind is
/// the nesting failure the Python player's key building never
/// catches, which the command line answers at status 1 in the
/// port's own words.
#[derive(Debug, thiserror::Error)]
pub enum JudgmentError {
    #[error("{0}")]
    Refused(String),
    #[error("{0}")]
    Uncaught(String),
}

/// The labels a classification choice offers, the class
/// identifiers of the list given and the new class label last.
pub fn classification_labels(class_list: &crate::records::ClassList) -> Vec<String> {
    class_list.labels()
}

/// A judgment request for one record at the classification
/// point, naming the classes on the books. The classes named
/// are the class identifiers only. The new class label is a
/// label a choice may answer with, and it is never a class the
/// request names.
pub fn classification_request(record: &Value, class_list: &crate::records::ClassList) -> Value {
    Value::object(vec![
        (
            "record_id".to_string(),
            record.get("id").cloned().unwrap_or(Value::Null),
        ),
        (
            "stalled".to_string(),
            record.get("stalled").cloned().unwrap_or(Value::Null),
        ),
        (
            "human_action".to_string(),
            record.get("human_action").cloned().unwrap_or(Value::Null),
        ),
        (
            "classes".to_string(),
            Value::List(
                class_list
                    .classes
                    .iter()
                    .map(|entry| Value::Str(entry.id.clone()))
                    .collect(),
            ),
        ),
    ])
}

/// A judgment request for one record at the quality point.
pub fn quality_request(record: &Value) -> Value {
    Value::object(vec![
        (
            "record_id".to_string(),
            record.get("id").cloned().unwrap_or(Value::Null),
        ),
        (
            "stalled".to_string(),
            record.get("stalled").cloned().unwrap_or(Value::Null),
        ),
        (
            "human_action".to_string(),
            record.get("human_action").cloned().unwrap_or(Value::Null),
        ),
        (
            "outcome".to_string(),
            record.get("outcome").cloned().unwrap_or(Value::Null),
        ),
    ])
}

/// The request key a judgment stands under, the Python dumps
/// form of the document naming the point and the request with
/// sorted keys. Two judgments answer the same request exactly
/// when their keys are the same text.
///
/// Building the key refuses a key document nested past the
/// measured ceiling with the uncaught kind, as the Python
/// player's key building fails.
fn request_key(point: &str, request: &Value) -> Result<String, JudgmentError> {
    let document = Value::object(vec![
        ("point".to_string(), Value::Str(point.to_string())),
        ("request".to_string(), request.clone()),
    ]);
    if pyjson::deepest_path(&document) > KEY_MAX_PATH {
        return Err(JudgmentError::Uncaught(
            "the request is nested too deeply".to_string(),
        ));
    }
    Ok(pyjson::dumps_compact(&document))
}

/// The key of one fixture at its load, where the uncaught
/// nesting failure names the fixture file it was building for.
fn fixture_key(point: &str, request: &Value, display: &str) -> Result<String, JudgmentError> {
    request_key(point, request).map_err(|_| {
        JudgmentError::Uncaught(format!("fixture file is nested too deeply, {display}"))
    })
}

/// Whether a value is a number for the seam's readings, which
/// is a finite float or an integer a float can hold. A boolean
/// is not a number, and an integer too large for a float is not
/// one either, as the Python conversion of it fails.
fn as_number(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Int(text)) => match text.parse::<f64>() {
            Ok(number) if number.is_finite() => Some(number),
            _ => None,
        },
        Some(Value::Float(number)) if number.is_finite() => Some(*number),
        _ => None,
    }
}

fn refused<T>(source: &str, defect: String) -> Result<T, JudgmentError> {
    Err(JudgmentError::Refused(format!("{source} {defect}")))
}

/// The first field in an object the allowed set does not name,
/// in the order refusals name one, which is the order of the
/// printed forms of the names. A non-string key is not
/// expressible in the value model, so the callers refuse those
/// documents at their own shape checks before this walk.
fn first_extra(object: &Value, allowed: &[&str]) -> Option<String> {
    let entries = match object {
        Value::Object(entries) => entries,
        _ => return None,
    };
    let mut extra: Vec<&str> = entries
        .iter()
        .map(|(key, _)| key.as_str())
        .filter(|key| !allowed.contains(key))
        .collect();
    extra.sort_by_key(|a| pyjson::py_repr_str(a));
    extra.first().map(|key| key.to_string())
}

fn check_text(source: &str, request: &Value, field: &str) -> Result<(), JudgmentError> {
    match request.get(field) {
        Some(Value::Str(text)) if !text.is_empty() => Ok(()),
        _ => refused(
            source,
            format!("request field '{field}' is not a non-empty string"),
        ),
    }
}

fn check_number(source: &str, field: &str, value: Option<&Value>) -> Result<f64, JudgmentError> {
    match as_number(value) {
        Some(number) if (0.0..=1.0).contains(&number) => Ok(number),
        _ => refused(
            source,
            format!("field '{field}' is not a number between 0 and 1"),
        ),
    }
}

fn check_unit_interval(source: &str, response: &Value, field: &str) -> Result<f64, JudgmentError> {
    check_number(source, field, response.get(field))
}

fn validate_classification_request(
    source: &str,
    request: &Value,
) -> Result<Vec<String>, JudgmentError> {
    check_text(source, request, "record_id")?;
    check_text(source, request, "stalled")?;
    check_text(source, request, "human_action")?;
    let classes = request.get("classes");
    let items = match classes {
        Some(Value::List(items)) if !items.is_empty() => items,
        _ => {
            return refused(
                source,
                "request field 'classes' is not a list of class identifiers".to_string(),
            );
        }
    };
    let mut labels = Vec::new();
    for item in items {
        match item {
            Value::Str(text) if !text.is_empty() => labels.push(text.clone()),
            _ => {
                return refused(
                    source,
                    "request field 'classes' is not a list of class identifiers".to_string(),
                );
            }
        }
    }
    let mut seen: Vec<&String> = Vec::new();
    for label in &labels {
        if seen.contains(&label) {
            return refused(
                source,
                "request field 'classes' names a class twice".to_string(),
            );
        }
        seen.push(label);
    }
    Ok(labels)
}

fn validate_quality_request(source: &str, request: &Value) -> Result<(), JudgmentError> {
    check_text(source, request, "record_id")?;
    check_text(source, request, "stalled")?;
    check_text(source, request, "human_action")?;
    check_text(source, request, "outcome")?;
    Ok(())
}

/// Validate one response, where the labels are the class labels
/// the classification point offered, present only at that point.
fn validate_response(
    source: &str,
    point: &str,
    response: &Value,
    labels: Option<&[String]>,
) -> Result<(), JudgmentError> {
    if !response.is_object() {
        return refused(source, "response is not an object".to_string());
    }
    let kind = match response.get("kind") {
        Some(Value::Str(text)) => text.clone(),
        _ => {
            return refused(
                source,
                "response field 'kind' is not a response kind".to_string(),
            );
        }
    };
    let fields: Option<&[&str]> = match kind.as_str() {
        "abstention" => Some(&ABSTENTION_FIELDS),
        "score" => Some(&SCORE_FIELDS),
        "choice" => Some(&CHOICE_FIELDS),
        _ => None,
    };
    if let Some(fields) = fields
        && let Some(extra) = first_extra(response, fields)
    {
        return refused(
            source,
            format!(
                "response carries the field {}, which a {} response does not carry",
                pyjson::py_repr_str(&extra),
                kind
            ),
        );
    }
    if kind == "abstention" {
        return Ok(());
    }
    let expected = if point == CLASSIFICATION {
        "choice"
    } else {
        "score"
    };
    if kind != expected {
        return refused(
            source,
            format!(
                "response kind {} does not answer the {} point, which takes a {} or an abstention",
                pyjson::py_repr_str(&kind),
                point,
                expected
            ),
        );
    }
    check_unit_interval(source, response, "confidence")?;
    if kind == "score" {
        check_unit_interval(source, response, "score")?;
        return Ok(());
    }
    let labels = match labels {
        Some(labels) => labels,
        None => {
            return refused(
                source,
                "choice response cannot be checked without its labels".to_string(),
            );
        }
    };
    let choice_ok = match response.get("choice") {
        Some(Value::Str(text)) => labels.iter().any(|label| label == text),
        _ => false,
    };
    if !choice_ok {
        return refused(
            source,
            "response field 'choice' is not one of the labels offered".to_string(),
        );
    }
    let probabilities = match response.get("probabilities") {
        Some(probabilities @ Value::Object(_)) => probabilities,
        _ => {
            return refused(
                source,
                "response field 'probabilities' is not an object".to_string(),
            );
        }
    };
    let offered: Vec<&str> = match probabilities {
        Value::Object(entries) => entries.iter().map(|(key, _)| key.as_str()).collect(),
        _ => Vec::new(),
    };
    let covers = offered.len() == labels.len()
        && labels.iter().all(|label| offered.contains(&label.as_str()));
    if !covers {
        return refused(
            source,
            "response probabilities do not cover exactly the labels offered".to_string(),
        );
    }
    let mut total = 0.0f64;
    for label in labels {
        let part = check_number(
            source,
            &format!("probabilities.{label}"),
            probabilities.get(label),
        )?;
        total += part;
    }
    if (total - 1.0).abs() > 0.001 {
        return refused(
            source,
            "response probabilities do not add up to 1".to_string(),
        );
    }
    Ok(())
}

/// Validate a fixture document read from the source named. The
/// source is the name a refusal speaks in, the fixture's own
/// file name for a file and the plain word answer for a live
/// answer.
pub fn validate_fixture(source: &str, data: &Value) -> Result<(), JudgmentError> {
    if !data.is_object() {
        return refused(source, "is not an object".to_string());
    }
    if let Some(extra) = first_extra(data, &FIXTURE_FIELDS) {
        return refused(
            source,
            format!(
                "carries the field {}, which a fixture does not carry",
                pyjson::py_repr_str(&extra)
            ),
        );
    }
    if data.get("synthetic") != Some(&Value::Bool(true)) {
        return refused(source, "is not marked synthetic".to_string());
    }
    let point = match data.get("judgment_point") {
        Some(Value::Str(text)) if text == CLASSIFICATION || text == QUALITY => text.clone(),
        _ => {
            return refused(
                source,
                "field 'judgment_point' is not a judgment point".to_string(),
            );
        }
    };
    let request = match data.get("request") {
        Some(request @ Value::Object(_)) => request,
        _ => return refused(source, "field 'request' is not an object".to_string()),
    };
    let mut labels = None;
    if point == CLASSIFICATION {
        let mut offered = validate_classification_request(source, request)?;
        offered.push(NEW_CLASS_LABEL.to_string());
        labels = Some(offered);
    } else {
        validate_quality_request(source, request)?;
    }
    let response = match data.get("response") {
        Some(response) => response,
        None => return refused(source, "carries no response".to_string()),
    };
    validate_response(source, &point, response, labels.as_deref())
}

/// Read and validate one fixture file under the display path
/// given, which is the path the load's own lines name. The file
/// is read as UTF-8 text, parsed as JSON, and validated, and
/// each failure is refused in the fixture discipline's words,
/// except the nesting failures the Python reader never catches.
fn load_fixture_file(path: &Path, display: &str) -> Result<(String, Value, Value), JudgmentError> {
    let bytes = std::fs::read(path)
        .map_err(|_| JudgmentError::Refused(format!("fixture file cannot be read, {display}")))?;
    let text = String::from_utf8(bytes).map_err(|_| {
        JudgmentError::Refused(format!("fixture file is not valid UTF-8, {display}"))
    })?;
    let data = pyjson::parse(&text).map_err(|error| match error {
        ParseError::Syntax(_) => {
            JudgmentError::Refused(format!("fixture file is not valid JSON, {display}"))
        }
        ParseError::TooDeep => {
            JudgmentError::Uncaught(format!("fixture file is nested too deeply, {display}"))
        }
        ParseError::IntTooLong => JudgmentError::Uncaught(format!(
            "fixture file holds an integer with too many digits, {display}"
        )),
    })?;
    let name = path
        .file_name()
        .map(|name| pyjson::py_path_display(Path::new(name)))
        .unwrap_or_default();
    let source = format!("fixture {name}");
    validate_fixture(&source, &data)?;
    let point = data
        .get("judgment_point")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let request = data.get("request").cloned().unwrap_or(Value::Null);
    let response = data.get("response").cloned().unwrap_or(Value::Null);
    Ok((point, request, response))
}

/// A judgment source, the shape a supplier of judgments takes.
/// A source answers a judgment point and a request with a
/// response document, or with nothing when it holds no judgment
/// for that request.
pub trait JudgmentSource {
    /// Answer one point and request, or nothing.
    fn judge(&self, point: &str, request: &Value) -> Option<Value>;
}

/// The fixture player, a judgment source answering from a
/// directory of recorded judgments.
///
/// A directory that cannot be read is refused. Every JSON file
/// in the directory loads in sorted path order, and two
/// fixtures recording the same request are refused, because the
/// player could not say which of them answers. A judgment the
/// player holds is returned as a copy, exactly as it was
/// recorded, and a request the player holds no judgment for is
/// answered with nothing.
pub struct FixturePlayer {
    answers: HashMap<String, Value>,
}

impl FixturePlayer {
    /// Load a directory of fixtures, refusing what the seam
    /// refuses. The directory argument is rendered in the
    /// refusal line exactly as it was given.
    pub fn load(directory: &Path) -> Result<FixturePlayer, JudgmentError> {
        if !directory.is_dir() {
            return Err(JudgmentError::Refused(format!(
                "fixtures directory cannot be read, {}",
                pyjson::py_path_display(directory)
            )));
        }
        let mut paths: Vec<std::path::PathBuf> = Vec::new();
        let read = std::fs::read_dir(directory).map_err(|_| {
            JudgmentError::Refused(format!(
                "fixtures directory cannot be read, {}",
                pyjson::py_path_display(directory)
            ))
        })?;
        for entry in read {
            let entry = entry.map_err(|_| {
                JudgmentError::Refused(format!(
                    "fixtures directory cannot be read, {}",
                    pyjson::py_path_display(directory)
                ))
            })?;
            let path = entry.path();
            // The glob takes every name ending in the JSON
            // suffix, including a name that is the suffix alone,
            // which the extension reading of a path would miss.
            let named = path
                .file_name()
                .map(|name| name.to_string_lossy().ends_with(".json"))
                .unwrap_or(false);
            if named {
                paths.push(path);
            }
        }
        // The glob order is the sorted order of the joined
        // paths, which the path library's display forms order
        // as the Python glob orders its results.
        paths.sort_by(|a, b| {
            let name_a = a.file_name().unwrap_or_default();
            let name_b = b.file_name().unwrap_or_default();
            pyjson::py_path_join_display(directory, name_a)
                .cmp(&pyjson::py_path_join_display(directory, name_b))
        });
        let mut answers: HashMap<String, Value> = HashMap::new();
        for path in &paths {
            let name = path.file_name().unwrap_or_default();
            let display = pyjson::py_path_join_display(directory, name);
            let (point, request, response) = load_fixture_file(path, &display)?;
            let key = fixture_key(&point, &request, &display)?;
            if answers.contains_key(&key) {
                return Err(JudgmentError::Refused(format!(
                    "fixture {} records the same request as an earlier fixture",
                    pyjson::py_path_display(Path::new(name))
                )));
            }
            answers.insert(key, response);
        }
        Ok(FixturePlayer { answers })
    }
}

impl JudgmentSource for FixturePlayer {
    fn judge(&self, point: &str, request: &Value) -> Option<Value> {
        let key = request_key(point, request).ok()?;
        let response = self.answers.get(&key)?;
        // The answer returns as a copy, through the dumps and
        // loads round trip the Python player copies with. A
        // validated response stands at most three containers
        // deep, so the copy never nears a ceiling.
        pyjson::parse(&pyjson::dumps_preserving(response)).ok()
    }
}

/// Check a judgment a source answered with, in context.
///
/// The labels are the labels the classification point offered,
/// the class labels and the new class label together, and the
/// check names the plain word answer as its source. A judgment
/// that validates is returned as it stands, and any other
/// answer, including no answer, returns nothing, because a
/// source's malformed claim is ignored rather than answered,
/// exactly as a source's silence is.
pub fn check_answer(
    point: &str,
    response: Option<Value>,
    labels: Option<&[String]>,
) -> Option<Value> {
    let response = response?;
    match validate_response("answer", point, &response, labels) {
        Ok(()) => Some(response),
        Err(_) => None,
    }
}

/// Whether a checked judgment is trusted, which is a score or a
/// choice whose confidence is a number reaching the threshold.
pub fn trusted(response: &Value) -> bool {
    let kind = response.get("kind").and_then(Value::as_str);
    if !matches!(kind, Some("score") | Some("choice")) {
        return false;
    }
    match as_number(response.get("confidence")) {
        Some(confidence) => confidence >= CONFIDENCE_THRESHOLD,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A request standing in a chain of nested lists, one
    /// list per step, with nothing at the foot, so the
    /// chain's own container depth is the count of its lists.
    fn nested_request(lists: usize) -> Value {
        let mut value = Value::Null;
        for _ in 0..lists {
            value = Value::List(vec![value]);
        }
        value
    }

    #[test]
    fn request_key_carries_at_the_ceiling_and_refuses_past_it() {
        // The key document names the point and the request
        // over the request itself, so its deepest container
        // path is the request's own depth plus one for the
        // document's root. A chain of 9990 lists therefore
        // stands at path 9991, the ceiling CV-42 states, and
        // its key builds; a chain of 9991 lists stands at
        // path 9992 and the key building refuses with the
        // uncaught kind. No command line reaches this
        // ceiling, since load validation refuses a deep
        // request's shape first, so the pin stands at the
        // key building itself. The building recurses as the
        // tool's walks do, so it runs on a stack sized as
        // the binary's own is.
        std::thread::Builder::new()
            .stack_size(512 * 1024 * 1024)
            .spawn(|| {
                let carried = request_key("classification", &nested_request(9990));
                assert!(carried.is_ok(), "a key at the ceiling builds");
                let refused = request_key("classification", &nested_request(9991));
                assert!(
                    matches!(refused, Err(JudgmentError::Uncaught(_))),
                    "a key past the ceiling refuses uncaught"
                );
            })
            .expect("the thread stands")
            .join()
            .expect("the key building answers");
    }
}
