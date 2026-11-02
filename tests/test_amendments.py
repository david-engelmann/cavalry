import json
import pathlib
import subprocess
import sys

import pytest

import cavalry.records as records_module
from cavalry.records import (
    RecordError,
    capture,
    load_class_list,
    read_records,
    render_list,
    render_proposals,
)
from record_factory import make_record, write_input

ROOT = pathlib.Path(__file__).resolve().parents[1]


def write_store(tmp_path, text, name="records.jsonl"):
    path = tmp_path / name
    path.write_text(text, encoding="utf-8")
    return path


def stored_record(**overrides):
    record = make_record(**overrides)
    record["rescue_class"] = "prompt-rephrase"
    return record


def valid_class_list_data():
    return {
        "version": 2,
        "unclassified_proposal": "Draft a review. This is a proposal only.",
        "classes": [
            {
                "id": "some-class",
                "keywords": ["something"],
                "proposal": "Draft a rule. This is a proposal only.",
            }
        ],
    }


def class_list_file(tmp_path, data):
    path = tmp_path / "classes.json"
    path.write_text(json.dumps(data), encoding="utf-8")
    return path


# CV-14. Unterminated store.


def test_capture_repairs_an_unterminated_store(tmp_path):
    records_file = tmp_path / "records.jsonl"
    capture(write_input(tmp_path, make_record(), "one.json"), records_file)
    original = records_file.read_bytes()
    assert original.endswith(b"\n")
    records_file.write_bytes(original[:-1])
    capture(
        write_input(
            tmp_path,
            make_record(id="rec-002", occurred_at="2026-01-01T10:00:00Z"),
            "two.json",
        ),
        records_file,
    )
    final = records_file.read_bytes()
    assert final[: len(original) - 1] == original[:-1]
    loaded = read_records(records_file)
    assert [record["id"] for record in loaded] == ["rec-001", "rec-002"]
    listed = render_list(loaded)
    assert "rec-001" in listed
    assert "rec-002" in listed


# CV-15. Stored line shape validation.


@pytest.mark.parametrize("line", ['[1, 2, 3]', '"just a string"', "42"])
def test_non_object_store_line_is_refused(tmp_path, line):
    store = write_store(tmp_path, line + "\n")
    with pytest.raises(RecordError):
        read_records(store)


def test_store_line_missing_a_field_is_refused_with_field_named(tmp_path):
    record = stored_record()
    del record["outcome"]
    store = write_store(tmp_path, json.dumps(record) + "\n")
    with pytest.raises(RecordError) as excinfo:
        read_records(store)
    assert "outcome" in str(excinfo.value)


def test_store_line_missing_rescue_class_is_refused_with_field_named(tmp_path):
    record = stored_record()
    del record["rescue_class"]
    store = write_store(tmp_path, json.dumps(record) + "\n")
    with pytest.raises(RecordError) as excinfo:
        read_records(store)
    assert "rescue_class" in str(excinfo.value)


def test_store_line_with_an_unknown_field_is_refused_with_field_named(tmp_path):
    record = stored_record(surprise="nope")
    store = write_store(tmp_path, json.dumps(record) + "\n")
    with pytest.raises(RecordError) as excinfo:
        read_records(store)
    assert "surprise" in str(excinfo.value)


def test_capture_onto_a_malformed_store_writes_nothing(tmp_path):
    store = write_store(tmp_path, "[1, 2, 3]\n")
    before = store.read_bytes()
    with pytest.raises(RecordError):
        capture(write_input(tmp_path, make_record()), store)
    assert store.read_bytes() == before


def test_cli_list_on_a_malformed_store_exits_2_without_a_traceback(tmp_path):
    store = write_store(tmp_path, '{"id": "rec-x"}\n')
    result = subprocess.run(
        [sys.executable, "-m", "cavalry", "list", str(store)],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 2
    assert "Traceback" not in result.stderr


# CV-16. Unknown stored class in propose.


def test_propose_refuses_an_unknown_stored_class_and_names_it(tmp_path):
    record = stored_record()
    record["rescue_class"] = "bogus-class"
    store = write_store(tmp_path, json.dumps(record) + "\n")
    loaded = read_records(store)
    assert "bogus-class" in render_list(loaded)
    with pytest.raises(RecordError) as excinfo:
        render_proposals(loaded)
    assert "bogus-class" in str(excinfo.value)


# CV-18. Role label over the whole value.


def test_role_label_with_a_trailing_newline_is_refused(tmp_path):
    record = make_record(rescuer_role="operator\n")
    records_file = tmp_path / "records.jsonl"
    with pytest.raises(RecordError) as excinfo:
        capture(write_input(tmp_path, record), records_file)
    assert "rescuer_role" in str(excinfo.value)
    assert not records_file.exists()


# CV-19. Class list validation at load.


def test_class_list_entry_missing_keywords_is_refused(tmp_path, monkeypatch):
    data = valid_class_list_data()
    del data["classes"][0]["keywords"]
    monkeypatch.setattr(
        records_module, "_CLASSES_PATH", class_list_file(tmp_path, data)
    )
    with pytest.raises(RecordError) as excinfo:
        load_class_list()
    assert "some-class" in str(excinfo.value)
    assert "keywords" in str(excinfo.value)


def test_class_list_entry_missing_proposal_is_refused(tmp_path, monkeypatch):
    data = valid_class_list_data()
    del data["classes"][0]["proposal"]
    monkeypatch.setattr(
        records_module, "_CLASSES_PATH", class_list_file(tmp_path, data)
    )
    with pytest.raises(RecordError) as excinfo:
        load_class_list()
    assert "some-class" in str(excinfo.value)
    assert "proposal" in str(excinfo.value)


def test_class_list_missing_unclassified_proposal_is_refused(tmp_path, monkeypatch):
    data = valid_class_list_data()
    del data["unclassified_proposal"]
    monkeypatch.setattr(
        records_module, "_CLASSES_PATH", class_list_file(tmp_path, data)
    )
    with pytest.raises(RecordError) as excinfo:
        load_class_list()
    assert "unclassified_proposal" in str(excinfo.value)


def test_class_list_keywords_are_lowercased_at_load(tmp_path, monkeypatch):
    data = valid_class_list_data()
    data["classes"][0]["keywords"] = ["KILLED"]
    monkeypatch.setattr(
        records_module, "_CLASSES_PATH", class_list_file(tmp_path, data)
    )
    loaded = load_class_list()
    assert loaded["classes"][0]["keywords"] == ["killed"]


# CV-20. Unclassified proposal text lives in the class list.


def test_unclassified_proposal_text_comes_from_the_class_list(tmp_path, monkeypatch):
    data = valid_class_list_data()
    data["unclassified_proposal"] = (
        "Draft a bespoke unclassified review. This is a proposal only."
    )
    monkeypatch.setattr(
        records_module, "_CLASSES_PATH", class_list_file(tmp_path, data)
    )
    record = stored_record()
    record["rescue_class"] = "unclassified"
    output = render_proposals([record])
    assert "bespoke unclassified review" in output


def test_shipped_unclassified_proposal_matches_the_class_list_file():
    class_list = load_class_list()
    record = stored_record()
    record["rescue_class"] = "unclassified"
    assert class_list["unclassified_proposal"] in render_proposals([record])


# CV-21. occurred_at shape.


@pytest.mark.parametrize(
    "bad", ["January 1 2026", "2026-01-01 09:00:00", "2026-01-01", "banana"]
)
def test_occurred_at_in_another_shape_is_refused(tmp_path, bad):
    records_file = tmp_path / "records.jsonl"
    with pytest.raises(RecordError) as excinfo:
        capture(write_input(tmp_path, make_record(occurred_at=bad)), records_file)
    assert "occurred_at" in str(excinfo.value)
    assert not records_file.exists()


def test_occurred_at_without_a_trailing_z_is_accepted(tmp_path):
    records_file = tmp_path / "records.jsonl"
    stored = capture(
        write_input(tmp_path, make_record(occurred_at="2026-01-01T09:00:00")),
        records_file,
    )
    assert stored["occurred_at"] == "2026-01-01T09:00:00"


# CV-22. Newline safety.


def test_field_value_carrying_a_newline_is_refused(tmp_path):
    record = make_record(outcome="The task finished\nand a second line appeared")
    records_file = tmp_path / "records.jsonl"
    with pytest.raises(RecordError) as excinfo:
        capture(write_input(tmp_path, record), records_file)
    assert "outcome" in str(excinfo.value)
    assert not records_file.exists()
