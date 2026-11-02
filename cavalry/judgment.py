"""Recorded decision model judgments, played back from fixtures.

The roadmap gives cavalry two judgment points in the record
pipeline. Classification review asks which rescue class a record
belongs to and answers with a choice over the class identifiers in
the versioned class list plus one further label, new-class, that
names a class not yet on the books. Record quality asks how
classifiable a record is and answers with a score. A judgment is
advisory in both places. It never edits the record a human made,
never rejects one, and never replaces the deterministic
classification. A trusted classification answer is stored as a
proposal beside that classification, and only a human confirmation
settles it into the class history.

A judgment is recorded as a fixture that pairs one request with one
response in the shape a decision model returns. A choice response
carries the chosen label, a confidence, and a probability for every
label offered. A score response carries a numeric score and a
confidence. An abstention response carries neither. The fixture
player answers a request only from a recorded fixture whose request
matches it exactly, and returns no answer where no fixture matches.
Everything here is offline. The player reads files from one
directory and nothing else.
"""

import json
import math
from pathlib import Path

CONFIDENCE_THRESHOLD = 0.6
QUALITY_BAR = 0.5
NEW_CLASS_LABEL = "new-class"

JUDGMENT_POINTS = ("classification", "quality")
REVIEW_STATES = ("unreviewed", "proposed", "confirmed")

_KIND_FOR_POINT = {"classification": "choice", "quality": "score"}
_FIELDS_FOR_KIND = {
    "abstention": {"kind"},
    "score": {"kind", "score", "confidence"},
    "choice": {"kind", "choice", "confidence", "probabilities"},
}
_FIXTURE_FIELDS = {"judgment_point", "request", "response", "synthetic"}


class JudgmentError(Exception):
    """A fixture file or fixtures directory does not match the format."""


def _fail(source, defect):
    raise JudgmentError(f"{source} {defect}")


def _is_number(value):
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return False
    try:
        return math.isfinite(float(value))
    except OverflowError:
        return False


def _check_unit_interval(source, field, value):
    if not _is_number(value) or not 0.0 <= float(value) <= 1.0:
        _fail(source, f"field '{field}' is not a number between 0 and 1")


def _check_text(source, request, field):
    value = request.get(field)
    if not isinstance(value, str) or not value:
        _fail(source, f"request field '{field}' is not a non-empty string")


def classification_labels(class_list):
    """The labels a classification choice offers, new-class last."""
    return [entry["id"] for entry in class_list["classes"]] + [NEW_CLASS_LABEL]


def classification_request(record, class_list):
    """The classification request the pipeline asks about one record."""
    return {
        "record_id": record["id"],
        "stalled": record["stalled"],
        "human_action": record["human_action"],
        "classes": [entry["id"] for entry in class_list["classes"]],
    }


def quality_request(record):
    """The quality request the pipeline asks about one record."""
    return {
        "record_id": record["id"],
        "stalled": record["stalled"],
        "human_action": record["human_action"],
        "outcome": record["outcome"],
    }


def _validate_classification_request(source, request):
    _check_text(source, request, "record_id")
    _check_text(source, request, "stalled")
    _check_text(source, request, "human_action")
    classes = request.get("classes")
    if (
        not isinstance(classes, list)
        or not classes
        or any(not isinstance(item, str) or not item for item in classes)
    ):
        _fail(source, "request field 'classes' is not a list of class identifiers")
    if len(set(classes)) != len(classes):
        _fail(source, "request field 'classes' names a class twice")


def _validate_quality_request(source, request):
    _check_text(source, request, "record_id")
    _check_text(source, request, "stalled")
    _check_text(source, request, "human_action")
    _check_text(source, request, "outcome")


def _validate_response(source, point, response, labels=None):
    if not isinstance(response, dict):
        _fail(source, "response is not an object")
    kind = response.get("kind")
    if not isinstance(kind, str):
        _fail(source, "response field 'kind' is not a response kind")
    if kind in _FIELDS_FOR_KIND:
        extra = set(response) - _FIELDS_FOR_KIND[kind]
        if extra:
            _fail(
                source,
                f"response carries the field {sorted(extra, key=repr)[0]!r}, "
                f"which a {kind} response does not carry",
            )
    if kind == "abstention":
        return
    expected = _KIND_FOR_POINT[point]
    if kind != expected:
        _fail(
            source,
            f"response kind {kind!r} does not answer the {point} point, "
            f"which takes a {expected} or an abstention",
        )
    _check_unit_interval(source, "confidence", response.get("confidence"))
    if kind == "score":
        _check_unit_interval(source, "score", response.get("score"))
        return
    if labels is None:
        _fail(source, "choice response cannot be checked without its labels")
    choice = response.get("choice")
    if choice not in labels:
        _fail(source, "response field 'choice' is not one of the labels offered")
    probabilities = response.get("probabilities")
    if not isinstance(probabilities, dict):
        _fail(source, "response field 'probabilities' is not an object")
    if set(probabilities) != set(labels):
        _fail(source, "response probabilities do not cover exactly the labels offered")
    total = 0.0
    for label in labels:
        _check_unit_interval(source, f"probabilities.{label}", probabilities[label])
        total += float(probabilities[label])
    if abs(total - 1.0) > 0.001:
        _fail(source, "response probabilities do not add up to 1")


def validate_fixture(data, source="fixture"):
    """One recorded fixture pair, checked against the recorded shape.

    Returns the pair unchanged when it is well formed and raises
    JudgmentError naming the source and the defect when it is not.
    """
    if not isinstance(data, dict):
        _fail(source, "is not an object")
    extra = set(data) - _FIXTURE_FIELDS
    if extra:
        _fail(source, f"carries the field {sorted(extra, key=repr)[0]!r}, which a fixture does not carry")
    if data.get("synthetic") is not True:
        _fail(source, "is not marked synthetic")
    point = data.get("judgment_point")
    if point not in JUDGMENT_POINTS:
        _fail(source, "field 'judgment_point' is not a judgment point")
    request = data.get("request")
    if not isinstance(request, dict):
        _fail(source, "field 'request' is not an object")
    labels = None
    if point == "classification":
        _validate_classification_request(source, request)
        labels = list(request["classes"]) + [NEW_CLASS_LABEL]
    else:
        _validate_quality_request(source, request)
    if "response" not in data:
        _fail(source, "carries no response")
    _validate_response(source, point, data["response"], labels)
    return data


def _request_key(point, request):
    return json.dumps({"point": point, "request": request}, sort_keys=True)


def load_fixture_file(path):
    target = Path(path)
    try:
        text = target.read_text(encoding="utf-8")
    except OSError as exc:
        raise JudgmentError(f"fixture file cannot be read, {path}") from exc
    except UnicodeDecodeError as exc:
        raise JudgmentError(f"fixture file is not valid UTF-8, {path}") from exc
    try:
        data = json.loads(text)
    except json.JSONDecodeError as exc:
        raise JudgmentError(f"fixture file is not valid JSON, {path}") from exc
    return validate_fixture(data, source=f"fixture {target.name}")


class FixturePlayer:
    """Answers judgment requests from the recorded fixtures in a directory.

    Every JSON file in the directory holds one fixture pair. Pairs
    are validated as they load, so a response that changes shape
    fails here, loudly, and never reaches the record pipeline.
    """

    def __init__(self, directory):
        target = Path(directory)
        if not target.is_dir():
            raise JudgmentError(f"fixtures directory cannot be read, {directory}")
        self._answers = {}
        for path in sorted(target.glob("*.json")):
            pair = load_fixture_file(path)
            key = _request_key(pair["judgment_point"], pair["request"])
            if key in self._answers:
                raise JudgmentError(
                    f"fixture {path.name} records the same request as an earlier fixture"
                )
            self._answers[key] = pair["response"]

    def judge(self, point, request):
        """The recorded response for this request, or None to abstain."""
        response = self._answers.get(_request_key(point, request))
        if response is None:
            return None
        return json.loads(json.dumps(response))


def check_answer(point, response, labels=None):
    """The answer itself when it is well formed for its point, else None.

    A judgment source that is not the fixture player can hand back
    anything. An answer outside the recorded shape for its point is
    treated as no judgment at all, never trusted and never recorded,
    so a malformed answer cannot fail the capture or reach a record.
    """
    if response is None:
        return None
    try:
        _validate_response("answer", point, response, labels)
    except JudgmentError:
        return None
    return response


def trusted(response):
    """Whether a judgment answer may be used at all.

    An abstention and an answer whose confidence falls below the
    threshold in the contract both count as no judgment at all.
    """
    if not isinstance(response, dict):
        return False
    if response.get("kind") not in ("score", "choice"):
        return False
    confidence = response.get("confidence")
    return _is_number(confidence) and float(confidence) >= CONFIDENCE_THRESHOLD
