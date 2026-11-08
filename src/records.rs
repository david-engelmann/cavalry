//! The records discipline.
//!
//! Cavalry records the rescues only a human could make. A
//! rescue a person performed by hand, when an automated run had
//! stalled, is written down as one record in a records file, one
//! JSON object per line, and the record is never edited after it
//! lands. This module is that discipline. It states what a
//! record is, validates a new record against the records already
//! on the books, classifies the rescue against the versioned
//! class list, reads a records file and refuses one that does
//! not hold, appends a record without touching the bytes already
//! stored, builds the confirmation entry that closes a review,
//! and renders the book as a list and as the proposals a review
//! works from. Cavalry performs no rescue. It keeps the record
//! of the ones a human performed, and the ladder westering climbs
//! to rescue automatically is westering's own and is not
//! reimplemented here.

use crate::judgment::{
    CLASSIFICATION, JudgmentSource, NEW_CLASS_LABEL, QUALITY, QUALITY_BAR, check_answer,
    classification_request, quality_request, trusted,
};
use crate::pyjson::{self, ParseError, Value};
use crate::pytext;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// The fields every rescue record carries, in the order the
/// validation walks them. A record is the who and the what of a
/// rescue a human performed, the moment it happened, and what
/// came of it.
pub const REQUIRED_FIELDS: [&str; 9] = [
    "id",
    "run_ref",
    "rescued_role",
    "rescuer_role",
    "rescuer_kind",
    "stalled",
    "human_action",
    "occurred_at",
    "outcome",
];

/// The one field a record may carry beyond the required set
/// before it is stored, naming the record it corrects.
pub const OPTIONAL_FIELDS: [&str; 1] = ["corrects"];

/// The fields that name a role, which must read as role labels.
pub const ROLE_FIELDS: [&str; 2] = ["rescued_role", "rescuer_role"];

/// The fields the review discipline adds to a record, in the
/// order the stored shape walks them.
pub const REVIEW_FIELDS: [&str; 4] = [
    "review_state",
    "proposed_class",
    "quality_flag",
    "confirmed_class",
];

/// The review states a stored record may carry, beside carrying
/// none, which reads as unreviewed.
pub const REVIEW_STATES: [&str; 3] = ["unreviewed", "proposed", "confirmed"];

/// Every field a stored record may carry.
pub const STORED_FIELDS: [&str; 15] = [
    "id",
    "run_ref",
    "rescued_role",
    "rescuer_role",
    "rescuer_kind",
    "stalled",
    "human_action",
    "occurred_at",
    "outcome",
    "corrects",
    "rescue_class",
    "review_state",
    "proposed_class",
    "quality_flag",
    "confirmed_class",
];

/// The rescue class of a record no class on the books claims.
pub const UNCLASSIFIED: &str = "unclassified";

/// The suffix a confirmation entry's id carries past the id of
/// the record it confirms.
pub const CONFIRM_SUFFIX: &str = "--confirmed";

/// The quality flag a trusted quality judgment sets when the
/// score it gives a rescue falls below the bar.
pub const BELOW_BAR_FLAG: &str = "below-bar";

/// A records refusal, in the kinds the Python discipline's
/// failures divide into at the tool's surfaces.
///
/// A refusal is a defect the discipline names in its own words,
/// which the command line answers at status 2. The uncaught kind
/// is a failure the Python tool never catches, an unreadable
/// depth, an integer past its limit, a file that is not UTF-8,
/// which the command line answers at status 1 in the port's own
/// words. The system kind is a failure of the filesystem itself,
/// which the command line answers at status 2 with the platform
/// error line the Python tool prints for it.
#[derive(Debug, thiserror::Error)]
pub enum RecordError {
    #[error("{0}")]
    Refused(String),
    #[error("{0}")]
    Uncaught(String),
    #[error("{}", system_line(*errno, path.as_deref()))]
    System {
        errno: i32,
        path: Option<std::path::PathBuf>,
    },
}

/// The platform error line for one system error, the form the
/// Python tool prints for the errors it catches, the error
/// number and the platform's own text for it, with the file
/// named in the repr of its folded path when a file stands in
/// the failure.
pub fn system_line(errno: i32, path: Option<&Path>) -> String {
    let raw = std::io::Error::from_raw_os_error(errno).to_string();
    let suffix = format!(" (os error {errno})");
    let message = raw.strip_suffix(&suffix).unwrap_or(&raw);
    match path {
        Some(path) => format!("[Errno {errno}] {message}: {}", pyjson::py_path_repr(path)),
        None => format!("[Errno {errno}] {message}"),
    }
}

fn refused<T>(message: String) -> Result<T, RecordError> {
    Err(RecordError::Refused(message))
}

fn system<T>(error: &std::io::Error, path: &Path) -> Result<T, RecordError> {
    Err(RecordError::System {
        errno: error.raw_os_error().unwrap_or(1),
        path: Some(path.to_path_buf()),
    })
}

/// One rescue class on the books, its identifier, the keywords
/// that claim a rescue for it, in the order they are tested,
/// and the proposal text a review of that class works from. The
/// keywords stand lowercased, as the class list load leaves
/// them.
#[derive(Clone, Debug, PartialEq)]
pub struct ClassEntry {
    pub id: String,
    pub keywords: Vec<String>,
    pub proposal: String,
}

/// The class list in force, its version, its classes in list
/// order, and the proposal text for the unclassified value.
#[derive(Clone, Debug, PartialEq)]
pub struct ClassList {
    /// The version of the list, held as the integer text the
    /// list carries it in, so a version of any size survives
    /// exactly.
    pub version: String,
    pub classes: Vec<ClassEntry>,
    pub unclassified_proposal: String,
}

impl ClassList {
    /// The labels the classification point offers, the class
    /// identifiers in list order and the new class label last.
    pub fn labels(&self) -> Vec<String> {
        let mut labels: Vec<String> = self.classes.iter().map(|entry| entry.id.clone()).collect();
        labels.push(NEW_CLASS_LABEL.to_string());
        labels
    }

    /// The proposal text for one class on the books, or for the
    /// unclassified value.
    pub fn proposal_for(&self, class_id: &str) -> Option<&str> {
        if class_id == UNCLASSIFIED {
            return Some(&self.unclassified_proposal);
        }
        self.classes
            .iter()
            .find(|entry| entry.id == class_id)
            .map(|entry| entry.proposal.as_str())
    }
}

const SHIPPED_CLASS_LIST: &str = include_str!("../cavalry/classes.json");

/// Load a class list from its JSON text.
///
/// The reading is the one loading path for the shipped list and
/// for a list supplied from any other path, so the readings the
/// contract states for a supplied list hold identically for the
/// shipped one. A defect in the document is refused in the class
/// list's own words, with the JSON reader's defect text standing
/// inside the invalid JSON refusal.
pub fn load_class_list(text: &str) -> Result<ClassList, RecordError> {
    let data = pyjson::parse(text).map_err(|error| match error {
        ParseError::Syntax(message) => {
            RecordError::Refused(format!("class list holds invalid JSON: {message}"))
        }
        ParseError::TooDeep => RecordError::Uncaught("class list is nested too deeply".to_string()),
        ParseError::IntTooLong => {
            RecordError::Uncaught("class list holds an integer with too many digits".to_string())
        }
    })?;
    if !data.is_object() {
        return refused("class list must be a JSON object".to_string());
    }
    let version = match data.get("version") {
        None => return refused("class list is missing field: version".to_string()),
        Some(Value::Int(text)) => text.clone(),
        Some(_) => return refused("class list version must be a whole number".to_string()),
    };
    let raw_classes = match data.get("classes") {
        Some(Value::List(items)) => items.clone(),
        _ => return refused("class list is missing field: classes".to_string()),
    };
    let unclassified_proposal = match data.get("unclassified_proposal") {
        Some(Value::Str(text)) if !pytext::is_blank(text) => text.clone(),
        _ => return refused("class list is missing field: unclassified_proposal".to_string()),
    };
    let mut classes = Vec::new();
    for (index, entry) in raw_classes.iter().enumerate() {
        if !entry.is_object() {
            return refused(format!("class list entry {index} must be an object"));
        }
        let id = match entry.get("id") {
            Some(Value::Str(text)) if !pytext::is_blank(text) => text.clone(),
            _ => return refused(format!("class list entry {index} is missing field: id")),
        };
        if id == UNCLASSIFIED {
            return refused(format!(
                "class list names a class with the unclassified value: {id}"
            ));
        }
        if classes.iter().any(|prior: &ClassEntry| prior.id == id) {
            return refused(format!("class list holds two classes with the id: {id}"));
        }
        let keywords = match entry.get("keywords") {
            Some(Value::List(items)) if !items.is_empty() => {
                let mut words = Vec::new();
                let mut valid = true;
                for item in items {
                    match item {
                        Value::Str(text) if !pytext::is_blank(text) => {
                            words.push(pytext::lower(text))
                        }
                        _ => {
                            valid = false;
                            break;
                        }
                    }
                }
                if !valid {
                    return refused(format!("class list entry {id} is missing field: keywords"));
                }
                words
            }
            _ => return refused(format!("class list entry {id} is missing field: keywords")),
        };
        let proposal = match entry.get("proposal") {
            Some(Value::Str(text)) if !pytext::is_blank(text) => text.clone(),
            _ => return refused(format!("class list entry {id} is missing field: proposal")),
        };
        classes.push(ClassEntry {
            id,
            keywords,
            proposal,
        });
    }
    Ok(ClassList {
        version,
        classes,
        unclassified_proposal,
    })
}

/// Load a class list from a file. This is the seam a supplied
/// list enters through, standing where the Python discipline's
/// path seam stood.
pub fn load_class_list_at(path: &Path) -> Result<ClassList, RecordError> {
    let bytes = std::fs::read(path).map_err(|error| RecordError::System {
        errno: error.raw_os_error().unwrap_or(1),
        path: Some(path.to_path_buf()),
    })?;
    let text = String::from_utf8(bytes)
        .map_err(|_| RecordError::Uncaught("class list is not valid UTF-8".to_string()))?;
    load_class_list(&text)
}

/// The shipped class list, loaded through the one loading path.
/// The shipped list is product data pinned by the test suite, so
/// a defect in it is a defect in the build and stops the tool.
pub fn shipped_class_list() -> ClassList {
    load_class_list(SHIPPED_CLASS_LIST)
        .expect("the shipped class list must load, as the test suite pins it")
}

/// Whether a text matches where a keyword pattern matches,
/// which is the keyword standing in the text as a whole word or
/// as a whole word with one trailing s, under the Python word
/// boundary rule. The pattern tests the keyword and then the
/// keyword with its optional s, in that order, at every start
/// the scan reaches, exactly as the Python pattern searches.
fn keyword_matches(text: &[char], keyword: &[char]) -> bool {
    fn boundary(text: &[char], at: usize) -> bool {
        let before = at > 0 && pytext::is_word_char(text[at - 1]);
        let after = at < text.len() && pytext::is_word_char(text[at]);
        before != after
    }
    let length = text.len();
    for start in 0..=length {
        if !boundary(text, start) {
            continue;
        }
        if start + keyword.len() > length {
            continue;
        }
        if &text[start..start + keyword.len()] != keyword {
            continue;
        }
        let end = start + keyword.len();
        if end < length && text[end] == 's' && boundary(text, end + 1) {
            return true;
        }
        if boundary(text, end) {
            return true;
        }
    }
    false
}

/// Classify a rescue against a class list. The text judged is
/// the stalled account and the human action together,
/// lowercased as the Python lowercase lowercases them, and the
/// first class in list order with a keyword standing in that
/// text as a whole word, or a whole word with a trailing s,
/// claims the rescue. A rescue no class claims is unclassified.
pub fn classify(record: &Value, class_list: &ClassList) -> String {
    let stalled = record.get("stalled").and_then(Value::as_str).unwrap_or("");
    let action = record
        .get("human_action")
        .and_then(Value::as_str)
        .unwrap_or("");
    let text: Vec<char> = pytext::lower(&format!("{stalled} {action}"))
        .chars()
        .collect();
    for entry in &class_list.classes {
        for keyword in &entry.keywords {
            // The match lowercases the keyword as it tests it,
            // as the Python pattern builds from the lowered
            // keyword, so a list carrying a cased keyword still
            // classifies. The load's own lowering and this one
            // agree, both reading the keyword on its own.
            let word: Vec<char> = pytext::lower(keyword).chars().collect();
            if keyword_matches(&text, &word) {
                return entry.id.clone();
            }
        }
    }
    UNCLASSIFIED.to_string()
}

/// Whether a value reads as missing, which is an absent field,
/// a null, or a text the strip leaves empty.
fn is_missing(value: Option<&Value>) -> bool {
    match value {
        None => true,
        Some(Value::Null) => true,
        Some(Value::Str(text)) => pytext::is_blank(text),
        Some(_) => false,
    }
}

/// Whether a text is a role label, a lowercase letter and then
/// lowercase letters and digits and hyphens, the whole text.
fn is_role_label(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

/// Whether a text is a timestamp of the contracted shape, four
/// digits and a dash and two digits and a dash and two digits
/// and the letter T and the hour and minute and second in
/// pairs, and an optional trailing Z, where a digit is any
/// Unicode decimal digit, as the Python pattern reads one.
fn is_timestamp(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() != 19 && chars.len() != 20 {
        return false;
    }
    if chars.len() == 20 && chars[19] != 'Z' {
        return false;
    }
    for (index, ch) in chars.iter().take(19).enumerate() {
        let ok = match index {
            4 | 7 => *ch == '-',
            10 => *ch == 'T',
            13 | 16 => *ch == ':',
            _ => pyjson::decimal_digit_value(*ch).is_some(),
        };
        if !ok {
            return false;
        }
    }
    true
}

/// The record identifiers a set of records carries.
fn record_ids(records: &[Value]) -> Vec<&str> {
    records
        .iter()
        .filter_map(|record| record.get("id").and_then(Value::as_str))
        .collect()
}

/// The first field of an object outside the allowed set, in
/// plain sorted order, the order the unknown field refusals
/// name one in.
fn first_unknown(data: &Value, allowed: &[&str]) -> Option<String> {
    let entries = match data {
        Value::Object(entries) => entries,
        _ => return None,
    };
    let mut unknown: Vec<&str> = entries
        .iter()
        .map(|(key, _)| key.as_str())
        .filter(|key| !allowed.contains(key))
        .collect();
    unknown.sort();
    unknown.first().map(|key| key.to_string())
}

/// Validate a new rescue record against the records on the
/// books, in the order the discipline checks, refusing the
/// first defect found in the discipline's own words.
pub fn validate_input(data: &Value, existing: &[Value]) -> Result<(), RecordError> {
    if !data.is_object() {
        return refused("record must be a JSON object".to_string());
    }
    let mut allowed: Vec<&str> = REQUIRED_FIELDS.to_vec();
    allowed.extend(OPTIONAL_FIELDS);
    if let Some(name) = first_unknown(data, &allowed) {
        return refused(format!("unknown field: {name}"));
    }
    for field in REQUIRED_FIELDS {
        let value = data.get(field);
        if is_missing(value) {
            return refused(format!("missing field: {field}"));
        }
        if !matches!(value, Some(Value::Str(_))) {
            return refused(format!("invalid field: {field} must be text"));
        }
    }
    for field in REQUIRED_FIELDS.iter().chain(OPTIONAL_FIELDS.iter()) {
        if let Some(Value::Str(text)) = data.get(field)
            && (text.contains('\n') || text.contains('\r'))
        {
            return refused(format!("invalid field: {field} must not contain a newline"));
        }
    }
    for field in ROLE_FIELDS {
        let text = data.get(field).and_then(Value::as_str).unwrap_or("");
        if !is_role_label(text) {
            return refused(format!("invalid field: {field} must be a role label"));
        }
    }
    if data.get("rescuer_kind").and_then(Value::as_str) != Some("human") {
        return refused("invalid field: rescuer_kind must be human".to_string());
    }
    let occurred = data
        .get("occurred_at")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !is_timestamp(occurred) {
        return refused("invalid field: occurred_at must be a timestamp".to_string());
    }
    if let Some(corrects) = data.get("corrects") {
        let named = match corrects {
            Value::Str(text) if !pytext::is_blank(text) => text.as_str(),
            _ => return refused("invalid field: corrects must name a record id".to_string()),
        };
        if !record_ids(existing).contains(&named) {
            return refused("invalid field: corrects names no existing record".to_string());
        }
    }
    let id = data.get("id").and_then(Value::as_str).unwrap_or("");
    if record_ids(existing).contains(&id) {
        return refused("duplicate field: id is already present".to_string());
    }
    Ok(())
}

/// The review fields a new record is stored with, judged by the
/// source given, if any.
///
/// A record enters unreviewed. A trusted classification gives
/// it a proposed class and the proposed state, and the
/// deterministic class is never replaced by the proposal. A
/// trusted quality score below the bar flags the record, and
/// quality alone never moves the review state. With no source,
/// or no trusted judgment, the record simply stands unreviewed.
fn review_fields(
    source: Option<&dyn JudgmentSource>,
    record: &Value,
    class_list: &ClassList,
) -> Vec<(String, Value)> {
    let mut fields = vec![(
        "review_state".to_string(),
        Value::Str("unreviewed".to_string()),
    )];
    let source = match source {
        Some(source) => source,
        None => return fields,
    };
    let labels = class_list.labels();
    let request = classification_request(record, class_list);
    let answer = check_answer(
        CLASSIFICATION,
        source.judge(CLASSIFICATION, &request),
        Some(&labels),
    );
    if let Some(answer) = &answer
        && trusted(answer)
    {
        if let Some(choice) = answer.get("choice") {
            fields.push(("proposed_class".to_string(), choice.clone()));
        }
        fields[0].1 = Value::Str("proposed".to_string());
    }
    let request = quality_request(record);
    let answer = check_answer(QUALITY, source.judge(QUALITY, &request), None);
    if let Some(answer) = &answer
        && trusted(answer)
    {
        let score = answer.get("score").and_then(|value| match value {
            Value::Int(text) => text.parse::<f64>().ok(),
            Value::Float(number) => Some(*number),
            _ => None,
        });
        if let Some(score) = score
            && score < QUALITY_BAR
        {
            fields.push((
                "quality_flag".to_string(),
                Value::Str(BELOW_BAR_FLAG.to_string()),
            ));
        }
    }
    fields
}

/// Build the stored form of a new record, validated, classified,
/// and carrying the review fields the source given judges it
/// with. A class list may be supplied, and the shipped list
/// loads when none is.
pub fn build_stored_record(
    data: &Value,
    existing: &[Value],
    class_list: Option<&ClassList>,
    source: Option<&dyn JudgmentSource>,
) -> Result<Value, RecordError> {
    validate_input(data, existing)?;
    let shipped;
    let class_list = match class_list {
        Some(list) => list,
        None => {
            shipped = shipped_class_list();
            &shipped
        }
    };
    let mut stored = data.clone();
    let class = classify(&stored, class_list);
    stored.set("rescue_class", Value::Str(class));
    for (field, value) in review_fields(source, data, class_list) {
        stored.set(&field, value);
    }
    Ok(stored)
}

/// Validate one stored record read from a records file, where
/// the defect returned is the bare defect the caller wraps in
/// the line's place. A record carrying no review state predates
/// the review discipline and reads as unreviewed.
fn validate_stored(data: &Value) -> Result<(), String> {
    if !data.is_object() {
        return Err("line is not a JSON object".to_string());
    }
    if let Some(name) = first_unknown(data, &STORED_FIELDS) {
        return Err(format!("unknown field: {name}"));
    }
    let mut required: Vec<&str> = REQUIRED_FIELDS.to_vec();
    required.push("rescue_class");
    for field in required {
        match data.get(field) {
            None => return Err(format!("missing field: {field}")),
            Some(Value::Str(_)) => {}
            Some(_) => return Err(format!("invalid field: {field} must be text")),
        }
    }
    if let Some(corrects) = data.get("corrects")
        && !matches!(corrects, Value::Str(_))
    {
        return Err("invalid field: corrects must be text".to_string());
    }
    for field in REVIEW_FIELDS {
        if let Some(value) = data.get(field)
            && !matches!(value, Value::Str(_))
        {
            return Err(format!("invalid field: {field} must be text"));
        }
    }
    if let Some(state) = data.get("review_state") {
        let state = state.as_str().unwrap_or_default();
        if !REVIEW_STATES.contains(&state) {
            return Err("invalid field: review_state holds an unknown state".to_string());
        }
        if state == "proposed" && data.get("proposed_class").is_none() {
            return Err("missing field: proposed_class".to_string());
        }
        if state == "confirmed" && data.get("confirmed_class").is_none() {
            return Err("missing field: confirmed_class".to_string());
        }
    }
    Ok(())
}

/// Read a records file into the records it holds.
///
/// A file that does not stand is refused as not found. A line
/// that does not parse is refused without its number, carrying
/// the JSON reader's own defect text, and a line that parses
/// but does not hold the stored shape is refused with the
/// line's number standing in the refusal. A repeated record
/// identifier is refused at the line that repeats it. Blank
/// lines are skipped, and the line numbers count every line,
/// blank or not.
pub fn read_records(path: &Path) -> Result<Vec<Value>, RecordError> {
    let open = pyjson::py_path_folded(path);
    if !open.exists() {
        return refused(format!(
            "records file not found: {}",
            pyjson::py_path_normalized(path)
        ));
    }
    let bytes = match std::fs::read(&open) {
        Ok(bytes) => bytes,
        Err(error) => return system(&error, path),
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => {
            return Err(RecordError::Uncaught(
                "records file is not valid UTF-8".to_string(),
            ));
        }
    };
    let mut records = Vec::new();
    for (index, line) in pyjson::py_splitlines(&text).iter().enumerate() {
        let number = index + 1;
        if pytext::is_blank(line) {
            continue;
        }
        let data = match pyjson::parse(line) {
            Ok(data) => data,
            Err(ParseError::Syntax(message)) => {
                return refused(format!("records file holds an invalid line: {message}"));
            }
            Err(ParseError::TooDeep) => {
                return Err(RecordError::Uncaught(
                    "records file is nested too deeply".to_string(),
                ));
            }
            Err(ParseError::IntTooLong) => {
                return Err(RecordError::Uncaught(
                    "records file holds an integer with too many digits".to_string(),
                ));
            }
        };
        if let Err(defect) = validate_stored(&data) {
            return refused(format!(
                "records file holds an invalid line {number}: {defect}"
            ));
        }
        let id = data.get("id").and_then(Value::as_str).unwrap_or("");
        if record_ids(&records).contains(&id) {
            return refused(format!(
                "records file holds an invalid line {number}: duplicate record id: {id}"
            ));
        }
        records.push(data);
    }
    Ok(records)
}

/// Append one record to a records file as its stored line.
///
/// Stored bytes are never rewritten. A file not ending in a
/// newline has one written first, so the new line never joins
/// the line before it, and a file that does not stand is
/// created.
pub fn append_record(path: &Path, record: &Value) -> Result<(), RecordError> {
    let mut line = pyjson::dumps_compact(record);
    line.push('\n');
    let open = pyjson::py_path_folded(path);
    let mut separator = false;
    if open.exists() {
        let metadata = match std::fs::metadata(&open) {
            Ok(metadata) => metadata,
            Err(error) => return system(&error, path),
        };
        if metadata.len() > 0 {
            let mut file = match std::fs::File::open(&open) {
                Ok(file) => file,
                Err(error) => return system(&error, path),
            };
            if file.seek(SeekFrom::End(-1)).is_err() {
                // A file that cannot be sought is read whole for
                // its last byte, which a short records file
                // always allows.
                let mut content = Vec::new();
                if let Err(error) = file.read_to_end(&mut content) {
                    return system(&error, path);
                }
                separator = content.last().map(|byte| *byte != b'\n').unwrap_or(false);
            } else {
                let mut byte = [0u8; 1];
                match file.read_exact(&mut byte) {
                    Ok(()) => separator = byte[0] != b'\n',
                    Err(error) => return system(&error, path),
                }
            }
        }
    }
    let mut file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&open)
    {
        Ok(file) => file,
        Err(error) => return system(&error, path),
    };
    if separator && let Err(error) = file.write_all(b"\n") {
        return system(&error, path);
    }
    match file.write_all(line.as_bytes()) {
        Ok(()) => Ok(()),
        Err(error) => system(&error, path),
    }
}

/// Read a JSON document from an input file, with the failures
/// the input surfaces share. The capture input and the
/// confirmation input are both read this way, and both are
/// named the input file in the discipline's lines.
fn read_input(path: &Path) -> Result<Value, RecordError> {
    let bytes = match std::fs::read(pyjson::py_path_folded(path)) {
        Ok(bytes) => bytes,
        Err(error) => return system(&error, path),
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => {
            return Err(RecordError::Uncaught(
                "input file is not valid UTF-8".to_string(),
            ));
        }
    };
    pyjson::parse(&text).map_err(|error| match error {
        ParseError::Syntax(message) => {
            RecordError::Refused(format!("input file holds invalid JSON: {message}"))
        }
        ParseError::TooDeep => RecordError::Uncaught("input file is nested too deeply".to_string()),
        ParseError::IntTooLong => {
            RecordError::Uncaught("input file holds an integer with too many digits".to_string())
        }
    })
}

/// Capture one rescue record from an input file into a records
/// file, judged by the source given, if any, and return the
/// record as stored.
pub fn capture(
    input_path: &Path,
    records_path: &Path,
    source: Option<&dyn JudgmentSource>,
) -> Result<Value, RecordError> {
    let data = read_input(input_path)?;
    let existing = if pyjson::py_path_folded(records_path).exists() {
        read_records(records_path)?
    } else {
        Vec::new()
    };
    let stored = build_stored_record(&data, &existing, None, source)?;
    append_record(records_path, &stored)?;
    Ok(stored)
}

/// Build the confirmation entry a confirmation names, against
/// the records on the books. A class list may be supplied, and
/// the shipped list loads when none is.
///
/// The entry stands as a new record beside the one it confirms,
/// carrying that record's own account of the rescue, its
/// identifier with the confirmation suffix, the confirmed class
/// as its confirmed class, and the proposal and flag under
/// review copied over, so the book holds both what was proposed
/// and what the review settled.
pub fn build_confirmation(
    data: &Value,
    existing: &[Value],
    class_list: Option<&ClassList>,
) -> Result<Value, RecordError> {
    if !data.is_object() {
        return refused("confirmation must be a JSON object".to_string());
    }
    if let Some(name) = first_unknown(data, &["record", "class"]) {
        return refused(format!("unknown field: {name}"));
    }
    for field in ["record", "class"] {
        match data.get(field) {
            Some(Value::Str(text)) if !pytext::is_blank(text) => {}
            _ => return refused(format!("missing field: {field}")),
        }
    }
    let target_id = data
        .get("record")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let class_name = data
        .get("class")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut target: Option<&Value> = None;
    for record in existing {
        if record.get("id").and_then(Value::as_str) == Some(target_id.as_str()) {
            target = Some(record);
        }
    }
    let target = match target {
        Some(target) => target,
        None => return refused(format!("record not found: {target_id}")),
    };
    let already = existing.iter().any(|record| {
        record.get("corrects").and_then(Value::as_str) == Some(target_id.as_str())
            && record.get("review_state").and_then(Value::as_str) == Some("confirmed")
    });
    if already {
        return refused(format!("record already confirmed: {target_id}"));
    }
    if target.get("review_state").and_then(Value::as_str) != Some("proposed") {
        return refused(format!(
            "record carries no proposal under review: {target_id}"
        ));
    }
    if class_name == NEW_CLASS_LABEL {
        return refused(format!("class names no class on the books: {class_name}"));
    }
    let shipped;
    let class_list = match class_list {
        Some(list) => list,
        None => {
            shipped = shipped_class_list();
            &shipped
        }
    };
    let known = class_name == UNCLASSIFIED
        || class_list
            .classes
            .iter()
            .any(|entry| entry.id == class_name);
    if !known {
        return refused(format!("unknown class: {class_name}"));
    }
    let new_id = format!("{target_id}{CONFIRM_SUFFIX}");
    if record_ids(existing).contains(&new_id.as_str()) {
        return refused(format!("record already confirmed: {target_id}"));
    }
    let mut entry = Value::object(Vec::new());
    for field in REQUIRED_FIELDS {
        if let Some(value) = target.get(field) {
            entry.set(field, value.clone());
        }
    }
    entry.set("id", Value::Str(new_id));
    entry.set("corrects", Value::Str(target_id.clone()));
    let class = classify(&entry, class_list);
    entry.set("rescue_class", Value::Str(class));
    entry.set("review_state", Value::Str("confirmed".to_string()));
    entry.set("confirmed_class", Value::Str(class_name));
    if let Some(proposed) = target.get("proposed_class") {
        entry.set("proposed_class", proposed.clone());
    }
    if let Some(flag) = target.get("quality_flag") {
        entry.set("quality_flag", flag.clone());
    }
    Ok(entry)
}

/// Confirm one record from a confirmation file into a records
/// file, and return the confirmation entry as stored.
pub fn confirm(confirmation_path: &Path, records_path: &Path) -> Result<Value, RecordError> {
    let data = read_input(confirmation_path)?;
    let existing = read_records(records_path)?;
    let entry = build_confirmation(&data, &existing, None)?;
    append_record(records_path, &entry)?;
    Ok(entry)
}

/// Render the book as a list, one line per record naming the
/// record, its class, its run, its moment, and its outcome. An
/// empty book renders as the empty text.
pub fn render_list(records: &[Value]) -> String {
    let mut out = String::new();
    for record in records {
        let field = |name: &str| record.get(name).and_then(Value::as_str).unwrap_or("");
        out.push_str(&format!(
            "{} {} {} {} {}\n",
            field("id"),
            field("rescue_class"),
            field("run_ref"),
            field("occurred_at"),
            field("outcome")
        ));
    }
    out
}

/// Collect the settled set, the ids that records in the
/// confirmed state name through their corrects field.
///
/// The queue rendering and the proposals rendering both read
/// the book under this one supersession, and CV-57 holds the
/// scan to this single place so the two renderings can never
/// read the settled set apart.
fn settled_ids(records: &[Value]) -> Vec<&str> {
    let mut settled: Vec<&str> = Vec::new();
    for record in records {
        if record.get("review_state").and_then(Value::as_str) == Some("confirmed")
            && let Some(corrects) = record.get("corrects").and_then(Value::as_str)
        {
            settled.push(corrects);
        }
    }
    settled
}

/// Render the queue, one line for each record whose current
/// state still waits for a human decision.
///
/// A record a confirmation has settled stands settled through
/// the record that confirms it, under the same supersession
/// the proposals document counts by, and a record in the
/// confirmed state stands settled in its own right, so neither
/// prints. Every other record prints when its review state is
/// proposed or unreviewed, and a record carrying no review
/// state reads as unreviewed, as it reads everywhere in the
/// discipline. The line carries the five fields the list line
/// carries and then the reason the record waits, the proposed
/// class it awaits confirmation as or the review it awaits,
/// and a record carrying the below-bar quality flag carries
/// that standing as the tail of its reason. The flag alone
/// never brings a record into the queue, and a book in which
/// no record waits renders as the empty text.
pub fn render_queue(records: &[Value]) -> String {
    let settled = settled_ids(records);
    let mut out = String::new();
    for record in records {
        let field = |name: &str| record.get(name).and_then(Value::as_str).unwrap_or("");
        if settled.contains(&field("id")) {
            continue;
        }
        let mut reason = match record.get("review_state").and_then(Value::as_str) {
            Some("proposed") => format!("awaiting confirmation as {}", field("proposed_class")),
            Some("confirmed") => continue,
            _ => "awaiting review".to_string(),
        };
        if record.get("quality_flag").and_then(Value::as_str) == Some(BELOW_BAR_FLAG) {
            reason.push_str(" and flagged below-bar");
        }
        out.push_str(&format!(
            "{} {} {} {} {} {}\n",
            field("id"),
            field("rescue_class"),
            field("run_ref"),
            field("occurred_at"),
            field("outcome"),
            reason
        ));
    }
    out
}

/// Render the proposals a review works from, counting the book
/// under the class list given.
///
/// A record in the confirmed state counts under its confirmed
/// class. A record a confirmation names is counted only through
/// that confirmation, and every other record counts under the
/// class it was stored with, so a class a stale proposal names
/// is never counted and never refused. A counted class the list
/// does not know refuses the whole rendering. The blocks print
/// in class list order with the unclassified block last, and a
/// book counting nothing renders as the empty text.
pub fn render_proposals(records: &[Value], class_list: &ClassList) -> Result<String, RecordError> {
    let confirmed = settled_ids(records);
    let mut present: Vec<String> = Vec::new();
    for record in records {
        let counted = if record.get("review_state").and_then(Value::as_str) == Some("confirmed") {
            record.get("confirmed_class").and_then(Value::as_str)
        } else if confirmed.contains(&record.get("id").and_then(Value::as_str).unwrap_or("")) {
            None
        } else {
            record.get("rescue_class").and_then(Value::as_str)
        };
        if let Some(class) = counted
            && !present.iter().any(|held| held == class)
        {
            present.push(class.to_string());
        }
    }
    let mut unknown: Vec<&str> = present
        .iter()
        .map(|class| class.as_str())
        .filter(|class| {
            *class != UNCLASSIFIED && !class_list.classes.iter().any(|entry| entry.id == *class)
        })
        .collect();
    unknown.sort();
    if let Some(name) = unknown.first() {
        return refused(format!("records hold an unknown rescue_class: {name}"));
    }
    let mut blocks = Vec::new();
    for entry in &class_list.classes {
        if present.contains(&entry.id) {
            blocks.push(format!(
                "proposal for class {}\n{}",
                entry.id, entry.proposal
            ));
        }
    }
    if present.iter().any(|class| class == UNCLASSIFIED) {
        blocks.push(format!(
            "proposal for class {}\n{}",
            UNCLASSIFIED, class_list.unclassified_proposal
        ));
    }
    if blocks.is_empty() {
        return Ok(String::new());
    }
    Ok(blocks.join("\n\n") + "\n")
}
