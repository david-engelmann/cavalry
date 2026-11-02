from cavalry.records import render_proposals


def stored(record_id, rescue_class):
    return {
        "id": record_id,
        "rescue_class": rescue_class,
        "run_ref": f"run-{record_id}",
        "occurred_at": "2026-01-01T09:00:00Z",
        "outcome": "Done",
    }


def blocks(output):
    return [block for block in output.strip().split("\n\n") if block]


def test_proposals_produced_per_class_present():
    records = [
        stored("rec-001", "prompt-rephrase"),
        stored("rec-002", "prompt-rephrase"),
        stored("rec-003", "run-interruption"),
    ]
    found = blocks(render_proposals(records))
    assert len(found) == 2


def test_each_proposal_names_its_class_and_carries_the_word_proposal():
    records = [
        stored("rec-001", "manual-edit"),
        stored("rec-002", "prompt-rephrase"),
        stored("rec-003", "unclassified"),
    ]
    output = render_proposals(records)
    found = blocks(output)
    assert len(found) == 3
    for block in found:
        assert "proposal" in block
    assert "proposal for class prompt-rephrase" in found[0]
    assert "proposal for class manual-edit" in found[1]
    assert "proposal for class unclassified" in found[2]
    assert "never applied" in output


def test_class_with_no_records_gets_no_proposal():
    output = render_proposals([stored("rec-001", "run-interruption")])
    assert "run-interruption" in output
    assert "manual-edit" not in output
    assert "prompt-rephrase" not in output


def test_no_records_produces_no_proposals():
    assert render_proposals([]) == ""
