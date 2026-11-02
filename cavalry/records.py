import io
import json
import pathlib
import re

REQUIRED_FIELDS = (
    "id",
    "run_ref",
    "rescued_role",
    "rescuer_role",
    "rescuer_kind",
    "stalled",
    "human_action",
    "occurred_at",
    "outcome",
)
OPTIONAL_FIELDS = ("corrects",)
STORED_FIELDS = REQUIRED_FIELDS + OPTIONAL_FIELDS + ("rescue_class",)
ROLE_FIELDS = ("rescued_role", "rescuer_role")
ROLE_LABEL = re.compile(r"[a-z][a-z0-9-]*")
TIMESTAMP_SHAPE = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z?")
UNCLASSIFIED = "unclassified"

_CLASSES_PATH = pathlib.Path(__file__).resolve().with_name("classes.json")


class RecordError(ValueError):
    """A record or records file that the tooling refuses."""


def load_class_list():
    try:
        data = json.loads(_CLASSES_PATH.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise RecordError(f"class list holds invalid JSON: {exc}") from exc
    if not isinstance(data, dict):
        raise RecordError("class list must be a JSON object")
    if "version" not in data:
        raise RecordError("class list is missing field: version")
    if not isinstance(data.get("classes"), list):
        raise RecordError("class list is missing field: classes")
    if not isinstance(data.get("unclassified_proposal"), str) or not data[
        "unclassified_proposal"
    ].strip():
        raise RecordError("class list is missing field: unclassified_proposal")
    classes = []
    for index, entry in enumerate(data["classes"]):
        if not isinstance(entry, dict):
            raise RecordError(f"class list entry {index} must be an object")
        if not isinstance(entry.get("id"), str) or not entry["id"].strip():
            raise RecordError(f"class list entry {index} is missing field: id")
        entry_id = entry["id"]
        keywords = entry.get("keywords")
        if (
            not isinstance(keywords, list)
            or not keywords
            or any(not isinstance(word, str) or not word.strip() for word in keywords)
        ):
            raise RecordError(
                f"class list entry {entry_id} is missing field: keywords"
            )
        if not isinstance(entry.get("proposal"), str) or not entry["proposal"].strip():
            raise RecordError(
                f"class list entry {entry_id} is missing field: proposal"
            )
        classes.append(
            {
                "id": entry_id,
                "keywords": [word.lower() for word in keywords],
                "proposal": entry["proposal"],
            }
        )
    return {
        "version": data["version"],
        "classes": classes,
        "unclassified_proposal": data["unclassified_proposal"],
    }


def _keyword_found(keyword, haystack):
    pattern = r"\b" + re.escape(keyword.lower()) + r"s?\b"
    return re.search(pattern, haystack) is not None


def classify(record, class_list=None):
    if class_list is None:
        class_list = load_class_list()
    haystack = (record["stalled"] + " " + record["human_action"]).lower()
    for entry in class_list["classes"]:
        for keyword in entry["keywords"]:
            if _keyword_found(keyword, haystack):
                return entry["id"]
    return UNCLASSIFIED


def validate_input(data, existing_ids=()):
    if not isinstance(data, dict):
        raise RecordError("record must be a JSON object")
    allowed = set(REQUIRED_FIELDS) | set(OPTIONAL_FIELDS)
    unknown = sorted(set(data) - allowed)
    if unknown:
        raise RecordError(f"unknown field: {unknown[0]}")
    for field in REQUIRED_FIELDS:
        if field not in data or data[field] is None or (
            isinstance(data[field], str) and not data[field].strip()
        ):
            raise RecordError(f"missing field: {field}")
        if not isinstance(data[field], str):
            raise RecordError(f"invalid field: {field} must be text")
    for field in REQUIRED_FIELDS + OPTIONAL_FIELDS:
        value = data.get(field)
        if isinstance(value, str) and ("\n" in value or "\r" in value):
            raise RecordError(f"invalid field: {field} must not contain a newline")
    for field in ROLE_FIELDS:
        if not ROLE_LABEL.fullmatch(data[field]):
            raise RecordError(f"invalid field: {field} must be a role label")
    if data["rescuer_kind"] != "human":
        raise RecordError("invalid field: rescuer_kind must be human")
    if not TIMESTAMP_SHAPE.fullmatch(data["occurred_at"]):
        raise RecordError("invalid field: occurred_at must be a timestamp")
    if "corrects" in data:
        if not isinstance(data["corrects"], str) or not data["corrects"].strip():
            raise RecordError("invalid field: corrects must name a record id")
        if data["corrects"] not in set(existing_ids):
            raise RecordError("invalid field: corrects names no existing record")
    if data["id"] in set(existing_ids):
        raise RecordError("duplicate field: id is already present")
    return data


def build_stored_record(data, existing_ids=()):
    validate_input(data, existing_ids)
    stored = dict(data)
    stored["rescue_class"] = classify(data)
    return stored


def _validate_stored(record, line_number):
    where = f"records file holds an invalid line {line_number}"
    if not isinstance(record, dict):
        raise RecordError(f"{where}: line is not a JSON object")
    unknown = sorted(set(record) - set(STORED_FIELDS))
    if unknown:
        raise RecordError(f"{where}: unknown field: {unknown[0]}")
    for field in REQUIRED_FIELDS + ("rescue_class",):
        if field not in record:
            raise RecordError(f"{where}: missing field: {field}")
        if not isinstance(record[field], str):
            raise RecordError(f"{where}: invalid field: {field} must be text")
    if "corrects" in record and not isinstance(record["corrects"], str):
        raise RecordError(f"{where}: invalid field: corrects must be text")
    return record


def read_records(records_path):
    path = pathlib.Path(records_path)
    if not path.exists():
        raise RecordError(f"records file not found: {path}")
    records = []
    for number, line in enumerate(
        path.read_text(encoding="utf-8").splitlines(), start=1
    ):
        if not line.strip():
            continue
        try:
            parsed = json.loads(line)
        except json.JSONDecodeError as exc:
            raise RecordError(f"records file holds an invalid line: {exc}") from exc
        records.append(_validate_stored(parsed, number))
    return records


def capture(input_path, records_path):
    input_file = pathlib.Path(input_path)
    try:
        data = json.loads(input_file.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise RecordError(f"input file holds invalid JSON: {exc}") from exc
    path = pathlib.Path(records_path)
    existing = read_records(path) if path.exists() else []
    existing_ids = [record.get("id") for record in existing]
    stored = build_stored_record(data, existing_ids)
    separator = ""
    if path.exists() and path.stat().st_size > 0:
        with path.open("rb") as handle:
            handle.seek(-1, io.SEEK_END)
            if handle.read(1) != b"\n":
                separator = "\n"
    with path.open("a", encoding="utf-8") as handle:
        handle.write(separator + json.dumps(stored, sort_keys=True) + "\n")
    return stored


def render_list(records):
    lines = []
    for record in records:
        lines.append(
            " ".join(
                [
                    record["id"],
                    record["rescue_class"],
                    record["run_ref"],
                    record["occurred_at"],
                    record["outcome"],
                ]
            )
        )
    if not lines:
        return ""
    return "\n".join(lines) + "\n"


def render_proposals(records, class_list=None):
    if class_list is None:
        class_list = load_class_list()
    present = {record["rescue_class"] for record in records}
    known = {entry["id"] for entry in class_list["classes"]} | {UNCLASSIFIED}
    unknown = sorted(present - known)
    if unknown:
        raise RecordError(f"records hold an unknown rescue_class: {unknown[0]}")
    ordered = []
    proposals = {}
    for entry in class_list["classes"]:
        proposals[entry["id"]] = entry["proposal"]
        if entry["id"] in present:
            ordered.append(entry["id"])
    if UNCLASSIFIED in present:
        ordered.append(UNCLASSIFIED)
        proposals[UNCLASSIFIED] = class_list["unclassified_proposal"]
    blocks = []
    for class_id in ordered:
        blocks.append(
            "\n".join(
                [
                    f"proposal for class {class_id}",
                    proposals[class_id],
                ]
            )
        )
    if not blocks:
        return ""
    return "\n\n".join(blocks) + "\n"
