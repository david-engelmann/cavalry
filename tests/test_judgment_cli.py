"""CV-34. The seam at the command line, capture flag and confirm."""

import json
import pathlib
import subprocess
import sys

from record_factory import write_input
from review_factory import AGREE, DIFFER

ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "fixtures"


def run_cli(*args):
    return subprocess.run(
        [sys.executable, "-m", "cavalry", *args],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )


def stored_lines(records_path):
    return [
        json.loads(line)
        for line in records_path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


def test_capture_with_fixtures_stores_the_proposal(tmp_path):
    records_path = tmp_path / "records.jsonl"
    result = run_cli(
        "capture",
        str(write_input(tmp_path, AGREE)),
        str(records_path),
        "--fixtures",
        str(FIXTURES),
    )
    assert result.returncode == 0, result.stderr
    assert result.stdout == "stored fox-001 as prompt-rephrase\n"
    (stored,) = stored_lines(records_path)
    assert stored["proposed_class"] == "prompt-rephrase"
    assert stored["review_state"] == "proposed"


def test_capture_without_the_flag_behaves_as_round_1_behaved(tmp_path):
    records_path = tmp_path / "records.jsonl"
    result = run_cli("capture", str(write_input(tmp_path, AGREE)), str(records_path))
    assert result.returncode == 0, result.stderr
    (stored,) = stored_lines(records_path)
    assert stored["review_state"] == "unreviewed"
    assert "proposed_class" not in stored


def test_capture_with_a_missing_fixtures_directory_writes_nothing(tmp_path):
    records_path = tmp_path / "records.jsonl"
    result = run_cli(
        "capture",
        str(write_input(tmp_path, AGREE)),
        str(records_path),
        "--fixtures",
        str(tmp_path / "nope"),
    )
    assert result.returncode == 2
    assert result.stderr.startswith("error:")
    assert result.stderr.count("\n") == 1
    assert not records_path.exists()


def test_capture_with_a_broken_fixture_writes_nothing(tmp_path):
    broken = tmp_path / "broken-fixtures"
    broken.mkdir()
    (broken / "bad.json").write_text(
        json.dumps(
            {
                "judgment_point": "quality",
                "request": {
                    "record_id": "fox-001",
                    "stalled": "s",
                    "human_action": "h",
                    "outcome": "o",
                },
                "response": {"kind": "score", "score": 2.0, "confidence": 0.9},
                "synthetic": True,
            }
        ),
        encoding="utf-8",
    )
    records_path = tmp_path / "records.jsonl"
    result = run_cli(
        "capture",
        str(write_input(tmp_path, AGREE)),
        str(records_path),
        "--fixtures",
        str(broken),
    )
    assert result.returncode == 2
    assert "bad.json" in result.stderr
    assert not records_path.exists()


def test_confirm_end_to_end_moves_the_class_history(tmp_path):
    records_path = tmp_path / "records.jsonl"
    captured = run_cli(
        "capture",
        str(write_input(tmp_path, DIFFER)),
        str(records_path),
        "--fixtures",
        str(FIXTURES),
    )
    assert captured.returncode == 0, captured.stderr
    confirmation = tmp_path / "confirmation.json"
    confirmation.write_text(
        json.dumps({"record": "fox-002", "class": "manual-edit"}), encoding="utf-8"
    )
    result = run_cli("confirm", str(confirmation), str(records_path))
    assert result.returncode == 0, result.stderr
    assert result.stdout == "confirmed fox-002 as manual-edit\n"
    proposed = run_cli("propose", str(records_path))
    assert proposed.returncode == 0, proposed.stderr
    assert "proposal for class manual-edit" in proposed.stdout
    assert "run-interruption" not in proposed.stdout


def test_confirm_refusal_exits_two_with_one_error_line(tmp_path):
    records_path = tmp_path / "records.jsonl"
    captured = run_cli("capture", str(write_input(tmp_path, AGREE)), str(records_path))
    assert captured.returncode == 0, captured.stderr
    confirmation = tmp_path / "confirmation.json"
    confirmation.write_text(
        json.dumps({"record": "fox-001", "class": "prompt-rephrase"}),
        encoding="utf-8",
    )
    result = run_cli("confirm", str(confirmation), str(records_path))
    assert result.returncode == 2
    assert result.stderr.startswith("error:")
    assert result.stderr.count("\n") == 1
    assert len(stored_lines(records_path)) == 1
