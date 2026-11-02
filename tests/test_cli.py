import json
import pathlib
import subprocess
import sys

from record_factory import make_record

ROOT = pathlib.Path(__file__).resolve().parents[1]


def run_cli(*args):
    return subprocess.run(
        [sys.executable, "-m", "cavalry", *args],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )


def test_capture_list_and_propose_end_to_end(tmp_path):
    input_file = tmp_path / "input.json"
    input_file.write_text(json.dumps(make_record()), encoding="utf-8")
    records_file = tmp_path / "records.jsonl"

    captured = run_cli("capture", str(input_file), str(records_file))
    assert captured.returncode == 0, captured.stderr
    assert "rec-001" in captured.stdout
    assert "prompt-rephrase" in captured.stdout

    listed = run_cli("list", str(records_file))
    assert listed.returncode == 0, listed.stderr
    assert "rec-001" in listed.stdout
    assert "prompt-rephrase" in listed.stdout
    assert "2026-01-01T09:00:00Z" in listed.stdout

    proposed = run_cli("propose", str(records_file))
    assert proposed.returncode == 0, proposed.stderr
    assert "proposal for class prompt-rephrase" in proposed.stdout
    assert "proposal" in proposed.stdout


def test_cli_capture_refusal_names_field_and_writes_nothing(tmp_path):
    record = make_record()
    del record["outcome"]
    input_file = tmp_path / "input.json"
    input_file.write_text(json.dumps(record), encoding="utf-8")
    records_file = tmp_path / "records.jsonl"
    result = run_cli("capture", str(input_file), str(records_file))
    assert result.returncode != 0
    assert "outcome" in result.stderr
    assert not records_file.exists()


def test_cli_output_is_deterministic_across_runs(tmp_path):
    input_file = tmp_path / "input.json"
    input_file.write_text(json.dumps(make_record()), encoding="utf-8")
    records_file = tmp_path / "records.jsonl"
    assert run_cli("capture", str(input_file), str(records_file)).returncode == 0
    first_list = run_cli("list", str(records_file))
    second_list = run_cli("list", str(records_file))
    assert first_list.stdout == second_list.stdout
    first_propose = run_cli("propose", str(records_file))
    second_propose = run_cli("propose", str(records_file))
    assert first_propose.stdout == second_propose.stdout
