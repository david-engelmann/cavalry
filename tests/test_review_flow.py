"""CV-27 to CV-33. Proposals, review states, quality flags, confirmation."""

import json
import pathlib

import pytest

from cavalry.judgment import FixturePlayer
from cavalry.records import (
    RecordError,
    build_stored_record,
    capture,
    confirm,
    read_records,
    render_list,
    render_proposals,
)
from record_factory import make_record, write_input
from review_factory import ABSTAIN, AGREE, DIFFER, FAINT, NEW

ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "fixtures"
REVIEW_FIELDS = ("review_state", "proposed_class", "quality_flag", "confirmed_class")


def player():
    return FixturePlayer(FIXTURES)


class StubSource:
    """A judgment source that is not the fixture player."""

    def __init__(self, answers):
        self._answers = answers

    def judge(self, point, request):
        return self._answers.get(point)


def capture_one(tmp_path, record, judgments=None, name="records.jsonl"):
    records_path = tmp_path / name
    stored = capture(write_input(tmp_path, record), records_path, judgments=judgments)
    return stored, records_path


def write_confirmation(tmp_path, payload, name="confirmation.json"):
    path = tmp_path / name
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def test_a_proposal_attaches_and_the_deterministic_class_is_unchanged(tmp_path):
    stored, _ = capture_one(tmp_path, DIFFER, judgments=player())
    assert stored["rescue_class"] == "run-interruption"
    assert stored["proposed_class"] == "manual-edit"
    assert stored["review_state"] == "proposed"


def test_an_agreeing_proposal_still_attaches_as_a_second_opinion(tmp_path):
    stored, _ = capture_one(tmp_path, AGREE, judgments=player())
    assert stored["rescue_class"] == "prompt-rephrase"
    assert stored["proposed_class"] == "prompt-rephrase"
    assert stored["review_state"] == "proposed"


def test_a_new_class_proposal_attaches_to_an_unclassified_record(tmp_path):
    stored, _ = capture_one(tmp_path, NEW, judgments=player())
    assert stored["rescue_class"] == "unclassified"
    assert stored["proposed_class"] == "new-class"
    assert stored["review_state"] == "proposed"


def test_no_source_leaves_the_record_in_the_unreviewed_state(tmp_path):
    stored, _ = capture_one(tmp_path, AGREE)
    assert stored["review_state"] == "unreviewed"
    assert "proposed_class" not in stored
    assert "quality_flag" not in stored


def test_an_abstention_leaves_the_record_in_the_unreviewed_state(tmp_path):
    stored, _ = capture_one(tmp_path, ABSTAIN, judgments=player())
    assert stored["rescue_class"] == "manual-edit"
    assert stored["review_state"] == "unreviewed"
    assert "proposed_class" not in stored


def test_a_below_threshold_answer_leaves_the_record_in_the_unreviewed_state(tmp_path):
    stored, _ = capture_one(tmp_path, FAINT, judgments=player())
    assert stored["rescue_class"] == "prompt-rephrase"
    assert stored["review_state"] == "unreviewed"
    assert "proposed_class" not in stored


def test_a_malformed_source_answer_counts_as_no_judgment_and_never_fails_capture(tmp_path):
    stub = StubSource(
        {
            "classification": {"kind": "score", "score": 0.9, "confidence": 0.99},
            "quality": "not even an object",
        }
    )
    stored, _ = capture_one(tmp_path, AGREE, judgments=stub)
    assert stored["review_state"] == "unreviewed"
    assert "proposed_class" not in stored
    assert "quality_flag" not in stored


def test_the_human_fields_are_byte_identical_with_and_without_a_source(tmp_path):
    plain, plain_path = capture_one(tmp_path, ABSTAIN, name="plain.jsonl")
    judged, judged_path = capture_one(tmp_path, ABSTAIN, judgments=player(), name="judged.jsonl")
    assert judged_path.read_bytes() == plain_path.read_bytes()
    assert judged == plain

    proposed, _ = capture_one(tmp_path, AGREE, judgments=player(), name="proposed.jsonl")
    baseline, _ = capture_one(tmp_path, AGREE, name="baseline.jsonl")
    stripped = {k: v for k, v in proposed.items() if k not in REVIEW_FIELDS}
    stripped_baseline = {k: v for k, v in baseline.items() if k not in REVIEW_FIELDS}
    assert stripped == stripped_baseline


def test_a_below_bar_quality_score_flags_the_record(tmp_path):
    stored, _ = capture_one(tmp_path, NEW, judgments=player())
    assert stored["quality_flag"] == "below-bar"


def test_an_above_bar_quality_score_records_no_flag(tmp_path):
    stored, _ = capture_one(tmp_path, AGREE, judgments=player())
    assert "quality_flag" not in stored


def test_a_quality_flag_never_changes_the_review_state(tmp_path):
    stub = StubSource(
        {
            "classification": {"kind": "abstention"},
            "quality": {"kind": "score", "score": 0.2, "confidence": 0.95},
        }
    )
    stored, _ = capture_one(tmp_path, AGREE, judgments=stub)
    assert stored["review_state"] == "unreviewed"
    assert stored["quality_flag"] == "below-bar"


def test_the_round_1_examples_still_read_list_and_propose():
    records = read_records(ROOT / "examples" / "records.jsonl")
    assert len(records) == 4
    assert all("review_state" not in record for record in records)
    assert render_list(records)
    expected = (ROOT / "examples" / "proposals.txt").read_text(encoding="utf-8")
    assert render_proposals(records) == expected


def _write_lines(tmp_path, records):
    path = tmp_path / "records.jsonl"
    path.write_text(
        "".join(json.dumps(record, sort_keys=True) + "\n" for record in records),
        encoding="utf-8",
    )
    return path


def test_a_round_1_stored_line_still_reads(tmp_path):
    stored = build_stored_record(make_record())
    for field in REVIEW_FIELDS:
        stored.pop(field, None)
    path = _write_lines(tmp_path, [stored])
    assert read_records(path)[0]["id"] == "rec-001"


def test_a_stored_record_with_an_unknown_review_state_is_refused(tmp_path):
    stored = build_stored_record(make_record())
    stored["review_state"] = "pending"
    path = _write_lines(tmp_path, [stored])
    with pytest.raises(RecordError):
        read_records(path)


def test_a_stored_proposal_without_a_proposed_class_is_refused(tmp_path):
    stored = build_stored_record(make_record())
    stored["review_state"] = "proposed"
    path = _write_lines(tmp_path, [stored])
    with pytest.raises(RecordError):
        read_records(path)


def test_a_stored_confirmation_without_a_confirmed_class_is_refused(tmp_path):
    stored = build_stored_record(make_record())
    stored["review_state"] = "confirmed"
    path = _write_lines(tmp_path, [stored])
    with pytest.raises(RecordError):
        read_records(path)


def test_a_stored_review_field_that_is_not_text_is_refused(tmp_path):
    stored = build_stored_record(make_record())
    stored["proposed_class"] = 7
    stored["review_state"] = "proposed"
    path = _write_lines(tmp_path, [stored])
    with pytest.raises(RecordError):
        read_records(path)


def capture_differ_with_proposal(tmp_path):
    _, records_path = capture_one(tmp_path, DIFFER, judgments=player())
    return records_path


def test_confirmation_appends_one_record_and_edits_no_existing_line(tmp_path):
    records_path = capture_differ_with_proposal(tmp_path)
    before = records_path.read_bytes()
    entry = confirm(
        write_confirmation(tmp_path, {"record": "fox-002", "class": "manual-edit"}),
        records_path,
    )
    after = records_path.read_bytes()
    assert after.startswith(before)
    assert len(read_records(records_path)) == 2
    assert entry["id"] == "fox-002--confirmed"


def test_the_confirmation_entry_carries_the_confirmed_state(tmp_path):
    records_path = capture_differ_with_proposal(tmp_path)
    entry = confirm(
        write_confirmation(tmp_path, {"record": "fox-002", "class": "manual-edit"}),
        records_path,
    )
    assert entry["corrects"] == "fox-002"
    assert entry["review_state"] == "confirmed"
    assert entry["confirmed_class"] == "manual-edit"
    assert entry["proposed_class"] == "manual-edit"
    assert entry["rescue_class"] == "run-interruption"
    assert entry["stalled"] == DIFFER["stalled"]
    assert entry["human_action"] == DIFFER["human_action"]


def test_the_class_history_counts_the_confirmed_class_in_place_of_the_record(tmp_path):
    records_path = capture_differ_with_proposal(tmp_path)
    before = render_proposals(read_records(records_path))
    assert "proposal for class run-interruption" in before
    assert "manual-edit" not in before
    confirm(
        write_confirmation(tmp_path, {"record": "fox-002", "class": "manual-edit"}),
        records_path,
    )
    after = render_proposals(read_records(records_path))
    assert "proposal for class manual-edit" in after
    assert "run-interruption" not in after


def test_an_unconfirmed_proposal_counts_nowhere_in_the_class_history(tmp_path):
    _, records_path = capture_one(tmp_path, NEW, judgments=player())
    output = render_proposals(read_records(records_path))
    assert "proposal for class unclassified" in output
    assert "new-class" not in output


def test_a_confirmation_can_settle_a_record_as_unclassified(tmp_path):
    records_path = capture_differ_with_proposal(tmp_path)
    confirm(
        write_confirmation(tmp_path, {"record": "fox-002", "class": "unclassified"}),
        records_path,
    )
    output = render_proposals(read_records(records_path))
    assert "proposal for class unclassified" in output
    assert "run-interruption" not in output


def test_confirmation_refuses_an_object_with_an_unknown_field(tmp_path):
    records_path = capture_differ_with_proposal(tmp_path)
    with pytest.raises(RecordError):
        confirm(
            write_confirmation(
                tmp_path, {"record": "fox-002", "class": "manual-edit", "note": "ok"}
            ),
            records_path,
        )


def test_confirmation_refuses_a_missing_class(tmp_path):
    records_path = capture_differ_with_proposal(tmp_path)
    with pytest.raises(RecordError):
        confirm(write_confirmation(tmp_path, {"record": "fox-002"}), records_path)


def test_confirmation_refuses_a_record_that_does_not_exist(tmp_path):
    records_path = capture_differ_with_proposal(tmp_path)
    with pytest.raises(RecordError):
        confirm(
            write_confirmation(tmp_path, {"record": "fox-999", "class": "manual-edit"}),
            records_path,
        )


def test_confirmation_refuses_a_record_with_no_proposal_under_review(tmp_path):
    _, records_path = capture_one(tmp_path, AGREE)
    with pytest.raises(RecordError):
        confirm(
            write_confirmation(tmp_path, {"record": "fox-001", "class": "prompt-rephrase"}),
            records_path,
        )


def test_confirmation_refuses_the_new_class_label(tmp_path):
    records_path = capture_differ_with_proposal(tmp_path)
    with pytest.raises(RecordError):
        confirm(
            write_confirmation(tmp_path, {"record": "fox-002", "class": "new-class"}),
            records_path,
        )


def test_confirmation_refuses_an_unknown_class(tmp_path):
    records_path = capture_differ_with_proposal(tmp_path)
    with pytest.raises(RecordError):
        confirm(
            write_confirmation(tmp_path, {"record": "fox-002", "class": "hovercraft"}),
            records_path,
        )


def test_confirmation_refuses_a_record_that_already_stands_confirmed(tmp_path):
    records_path = capture_differ_with_proposal(tmp_path)
    payload = {"record": "fox-002", "class": "manual-edit"}
    confirm(write_confirmation(tmp_path, payload), records_path)
    with pytest.raises(RecordError):
        confirm(write_confirmation(tmp_path, payload, name="again.json"), records_path)


def test_a_refused_confirmation_writes_nothing(tmp_path):
    records_path = capture_differ_with_proposal(tmp_path)
    before = records_path.read_bytes()
    with pytest.raises(RecordError):
        confirm(
            write_confirmation(tmp_path, {"record": "fox-002", "class": "new-class"}),
            records_path,
        )
    assert records_path.read_bytes() == before
