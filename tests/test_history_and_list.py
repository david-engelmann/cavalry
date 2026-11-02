"""Pins for CV-36, the history and current list amendment.

The records store is append-only history and the class list is
versioned current data. These pins hold the two apart. A stored
record reads as recorded under any later list. The proposals
document counts each record under its confirmed class when it
stands confirmed and otherwise under its recorded rescue class,
and it refuses when a counted class is not on the current books.
A proposed class is never counted, so a stale one refuses nothing.
A confirmation is judged against the current list, so a record
proposed under a retired class still confirms into a current one.
The list is accepted on its shape alone, whatever number its
version field carries.
"""

import hashlib
import json
import pathlib

import pytest

import cavalry.records as records_module
from cavalry.records import (
    RecordError,
    build_confirmation,
    classify,
    load_class_list,
    read_records,
    render_list,
    render_proposals,
)

ROOT = pathlib.Path(__file__).resolve().parents[1]
RETIRED = "retired-class"


def stored(record_id, rescue_class, **extra):
    record = {
        "id": record_id,
        "run_ref": f"run-{record_id}",
        "rescued_role": "support-operator",
        "rescuer_role": "on-call-operator",
        "rescuer_kind": "human",
        "stalled": "The task stalled because the prompt wording sent the worker in a circle",
        "human_action": "The operator rephrased the prompt and restarted the task",
        "occurred_at": "2026-01-01T09:00:00Z",
        "outcome": "The task finished on the next run",
        "rescue_class": rescue_class,
    }
    record.update(extra)
    return record


def write_store(tmp_path, rows):
    path = tmp_path / "records.jsonl"
    path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows),
        encoding="utf-8",
    )
    return path


def test_a_record_under_a_retired_class_reads_as_recorded(tmp_path):
    path = write_store(tmp_path, [stored("rec-001", RETIRED)])
    loaded = read_records(path)
    assert loaded[0]["rescue_class"] == RETIRED


def test_a_record_under_a_retired_class_lists_byte_as_recorded(tmp_path):
    path = write_store(tmp_path, [stored("rec-001", RETIRED)])
    listing = render_list(read_records(path))
    assert listing == (
        "rec-001 retired-class run-rec-001 2026-01-01T09:00:00Z "
        "The task finished on the next run\n"
    )


def test_propose_refuses_an_unknown_rescue_class_and_names_it(tmp_path):
    path = write_store(tmp_path, [stored("rec-001", RETIRED)])
    with pytest.raises(RecordError) as caught:
        render_proposals(read_records(path))
    assert str(caught.value) == f"records hold an unknown rescue_class: {RETIRED}"


def test_propose_refuses_an_unknown_confirmed_class_and_names_it(tmp_path):
    rows = [
        stored(
            "rec-002",
            "prompt-rephrase",
            review_state="proposed",
            proposed_class="prompt-rephrase",
        ),
        stored(
            "rec-002--confirmed",
            "prompt-rephrase",
            corrects="rec-002",
            review_state="confirmed",
            confirmed_class=RETIRED,
            proposed_class="prompt-rephrase",
        ),
    ]
    path = write_store(tmp_path, rows)
    with pytest.raises(RecordError) as caught:
        render_proposals(read_records(path))
    assert str(caught.value) == f"records hold an unknown rescue_class: {RETIRED}"


def test_propose_names_the_first_unknown_counted_class_in_sorted_order(tmp_path):
    rows = [
        stored("rec-003", "zzz-retired"),
        stored("rec-004", "aaa-retired"),
    ]
    path = write_store(tmp_path, rows)
    with pytest.raises(RecordError) as caught:
        render_proposals(read_records(path))
    assert str(caught.value) == "records hold an unknown rescue_class: aaa-retired"


def test_a_counted_class_the_later_list_keeps_still_proposes(tmp_path):
    later = {
        "version": 3,
        "classes": [
            entry
            for entry in load_class_list()["classes"]
            if entry["id"] != "manual-edit"
        ],
        "unclassified_proposal": load_class_list()["unclassified_proposal"],
    }
    path = write_store(tmp_path, [stored("rec-005", "prompt-rephrase")])
    output = render_proposals(read_records(path), later)
    assert "proposal for class prompt-rephrase" in output
    assert "manual-edit" not in output


def test_a_stale_proposed_class_alone_never_refuses_propose(tmp_path):
    rows = [
        stored(
            "rec-006",
            "prompt-rephrase",
            review_state="proposed",
            proposed_class=RETIRED,
        )
    ]
    path = write_store(tmp_path, rows)
    output = render_proposals(read_records(path))
    assert "proposal for class prompt-rephrase" in output
    plain = render_proposals([stored("rec-006", "prompt-rephrase")])
    assert output == plain


def test_a_record_proposed_under_a_retired_class_confirms_into_a_current_class():
    target = stored(
        "rec-007",
        RETIRED,
        review_state="proposed",
        proposed_class=RETIRED,
    )
    entry = build_confirmation({"record": "rec-007", "class": "manual-edit"}, [target])
    assert entry["id"] == "rec-007--confirmed"
    assert entry["corrects"] == "rec-007"
    assert entry["review_state"] == "confirmed"
    assert entry["confirmed_class"] == "manual-edit"
    assert entry["proposed_class"] == RETIRED
    assert entry["rescue_class"] == classify(entry)
    assert entry["rescue_class"] == "prompt-rephrase"
    assert entry["rescue_class"] != target["rescue_class"]


def test_a_well_formed_list_with_another_version_number_behaves_identically(
    tmp_path, monkeypatch
):
    raw = json.loads(
        (ROOT / "cavalry" / "classes.json").read_text(encoding="utf-8")
    )
    raw["version"] = 7
    changed = tmp_path / "classes.json"
    changed.write_text(json.dumps(raw), encoding="utf-8")
    monkeypatch.setattr(records_module, "_CLASSES_PATH", changed)
    loaded = load_class_list()
    assert loaded["version"] == 7
    sample = {
        "stalled": "The task stalled because the prompt wording sent the worker in a circle",
        "human_action": "The operator rephrased the prompt and restarted the task",
    }
    assert classify(sample, loaded) == "prompt-rephrase"
    records = read_records(ROOT / "examples" / "records.jsonl")
    expected = (ROOT / "examples" / "proposals.txt").read_text(encoding="utf-8")
    assert render_proposals(records, loaded) == expected


def test_the_committed_examples_are_byte_identical():
    records_bytes = (ROOT / "examples" / "records.jsonl").read_bytes()
    proposals_bytes = (ROOT / "examples" / "proposals.txt").read_bytes()
    assert hashlib.sha256(records_bytes).hexdigest() == (
        "e4d2cd027681cd23463683c0fbdfbbbcf4365dbf7ae0608aa0430d25a8d64d59"
    )
    assert hashlib.sha256(proposals_bytes).hexdigest() == (
        "d69e0049f0d512e305a6adc2d52374eef666913337a12cb9d657ce3f9e2a39c6"
    )
