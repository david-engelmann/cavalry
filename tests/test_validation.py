import pytest

from cavalry.records import REQUIRED_FIELDS, RecordError, capture, read_records
from record_factory import make_record, write_input


def test_complete_record_validates_and_round_trips(tmp_path):
    records_file = tmp_path / "records.jsonl"
    stored = capture(write_input(tmp_path, make_record()), records_file)
    assert stored["rescue_class"] == "prompt-rephrase"
    loaded = read_records(records_file)
    assert loaded == [stored]
    for field in REQUIRED_FIELDS:
        assert loaded[0][field] == make_record()[field]


@pytest.mark.parametrize("field", REQUIRED_FIELDS)
def test_record_missing_each_required_field_is_refused_with_field_named(tmp_path, field):
    record = make_record()
    del record[field]
    records_file = tmp_path / "records.jsonl"
    with pytest.raises(RecordError) as excinfo:
        capture(write_input(tmp_path, record), records_file)
    assert field in str(excinfo.value)
    assert not records_file.exists()


@pytest.mark.parametrize("field", REQUIRED_FIELDS)
def test_record_with_empty_required_field_is_refused_with_field_named(tmp_path, field):
    record = make_record(**{field: "  "})
    records_file = tmp_path / "records.jsonl"
    with pytest.raises(RecordError) as excinfo:
        capture(write_input(tmp_path, record), records_file)
    assert field in str(excinfo.value)
    assert not records_file.exists()


def test_unknown_field_is_refused_with_field_named(tmp_path):
    record = make_record(surprise="nope")
    records_file = tmp_path / "records.jsonl"
    with pytest.raises(RecordError) as excinfo:
        capture(write_input(tmp_path, record), records_file)
    assert "surprise" in str(excinfo.value)
    assert not records_file.exists()


def test_real_name_shaped_role_is_refused_with_field_named(tmp_path):
    record = make_record(rescuer_role="Jane Smith")
    records_file = tmp_path / "records.jsonl"
    with pytest.raises(RecordError) as excinfo:
        capture(write_input(tmp_path, record), records_file)
    assert "rescuer_role" in str(excinfo.value)
    assert not records_file.exists()


def test_duplicate_id_is_refused_and_file_keeps_one_record(tmp_path):
    records_file = tmp_path / "records.jsonl"
    capture(write_input(tmp_path, make_record()), records_file)
    with pytest.raises(RecordError) as excinfo:
        capture(write_input(tmp_path, make_record()), records_file)
    assert "id" in str(excinfo.value)
    assert len(read_records(records_file)) == 1


def test_automatic_rescue_is_refused_as_not_human(tmp_path):
    record = make_record(rescuer_kind="automatic")
    records_file = tmp_path / "records.jsonl"
    with pytest.raises(RecordError) as excinfo:
        capture(write_input(tmp_path, record), records_file)
    assert "rescuer_kind" in str(excinfo.value)
    assert not records_file.exists()
