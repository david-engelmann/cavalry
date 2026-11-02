import json


def make_record(**overrides):
    record = {
        "id": "rec-001",
        "run_ref": "run-synthetic-001",
        "rescued_role": "support-operator",
        "rescuer_role": "on-call-operator",
        "rescuer_kind": "human",
        "stalled": "The task stalled because the prompt wording sent the worker in a circle",
        "human_action": "The operator rephrased the prompt and restarted the task",
        "occurred_at": "2026-01-01T09:00:00Z",
        "outcome": "The task finished on the next run",
    }
    record.update(overrides)
    return record


def write_input(tmp_path, record, name="input.json"):
    path = tmp_path / name
    path.write_text(json.dumps(record), encoding="utf-8")
    return path
