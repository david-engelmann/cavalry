import pytest

from cavalry.records import RecordError, capture, read_records
from record_factory import make_record, write_input


def test_written_record_bytes_unchanged_after_correction_appended(tmp_path):
    records_file = tmp_path / "records.jsonl"
    capture(write_input(tmp_path, make_record(), "one.json"), records_file)
    before = records_file.read_bytes()
    correction = make_record(
        id="rec-002",
        occurred_at="2026-01-01T10:00:00Z",
        outcome="The outcome was corrected on review",
        corrects="rec-001",
    )
    capture(write_input(tmp_path, correction, "two.json"), records_file)
    after = records_file.read_bytes()
    assert after[: len(before)] == before
    assert len(after) > len(before)
    loaded = read_records(records_file)
    assert loaded[0]["id"] == "rec-001"
    assert loaded[0]["outcome"] == "The task finished on the next run"
    assert loaded[1]["corrects"] == "rec-001"


def test_plain_append_also_preserves_existing_bytes(tmp_path):
    records_file = tmp_path / "records.jsonl"
    capture(write_input(tmp_path, make_record(), "one.json"), records_file)
    before = records_file.read_bytes()
    capture(
        write_input(tmp_path, make_record(id="rec-002"), "two.json"),
        records_file,
    )
    assert records_file.read_bytes()[: len(before)] == before


def test_correction_naming_no_existing_record_is_refused(tmp_path):
    records_file = tmp_path / "records.jsonl"
    capture(write_input(tmp_path, make_record(), "one.json"), records_file)
    before = records_file.read_bytes()
    bad = make_record(id="rec-003", corrects="rec-999")
    with pytest.raises(RecordError) as excinfo:
        capture(write_input(tmp_path, bad, "bad.json"), records_file)
    assert "corrects" in str(excinfo.value)
    assert records_file.read_bytes() == before
