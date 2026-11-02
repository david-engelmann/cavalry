"""Synthetic records whose judgment requests the committed fixtures answer.

Each record here pairs with fixtures under the repo fixtures
directory. The fixture requests were built from these records by
the judgment request builders, so a change to a record here must
regenerate the matching fixture or the player abstains.
"""

from record_factory import make_record

AGREE = make_record(id="fox-001")
DIFFER = make_record(
    id="fox-002",
    run_ref="run-synthetic-102",
    stalled="The run kept writing to the wrong output and would not stop",
    human_action="The reviewer killed the run and removed the bad output",
    occurred_at="2026-01-02T11:30:00Z",
    outcome="The bad output was removed and the run was queued again",
)
NEW = make_record(
    id="fox-003",
    run_ref="run-synthetic-103",
    stalled="A dashboard number looked odd for a reason nobody could name",
    human_action="The maintainer checked the source by hand and wrote a note for follow up",
    occurred_at="2026-01-04T16:45:00Z",
    outcome="The number was explained in the note and no run was changed",
)
ABSTAIN = make_record(
    id="fox-004",
    run_ref="run-synthetic-104",
    stalled="The generated file carried a wrong heading that the checker passed",
    human_action="The maintainer edited the file by hand and sent it on",
    occurred_at="2026-01-03T14:15:00Z",
    outcome="The file went out with the right heading",
)
FAINT = make_record(
    id="fox-005",
    run_ref="run-synthetic-105",
    stalled="The worker looped on the same step because the prompt wording was vague",
    human_action="The operator rephrased the prompt with plainer steps",
    occurred_at="2026-01-05T08:20:00Z",
    outcome="The task finished once the wording was plain",
)

ALL_RECORDS = (AGREE, DIFFER, NEW, ABSTAIN, FAINT)
