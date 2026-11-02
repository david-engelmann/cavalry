import pytest

from cavalry.records import UNCLASSIFIED, classify, load_class_list


def record(stalled, human_action):
    return {"stalled": stalled, "human_action": human_action}


def test_class_list_is_versioned_and_held_in_repo():
    class_list = load_class_list()
    # The version rose to 2 as the mechanical consequence of the CV-17
    # amendment, which replaced the keyword stems with spelled out forms.
    assert class_list["version"] == 2
    ids = [entry["id"] for entry in class_list["classes"]]
    assert ids == ["prompt-rephrase", "run-interruption", "manual-edit"]
    for entry in class_list["classes"]:
        assert entry["keywords"]
        assert "proposal" in entry["proposal"]


@pytest.mark.parametrize(
    "stalled,human_action,expected",
    [
        (
            "The task stalled because the prompt wording confused the worker",
            "The operator rephrased the prompt",
            "prompt-rephrase",
        ),
        (
            "The run kept writing the wrong output and would not stop",
            "The reviewer killed the run",
            "run-interruption",
        ),
        (
            "The generated file carried a wrong heading",
            "The maintainer edited the file by hand",
            "manual-edit",
        ),
    ],
)
def test_matching_record_lands_in_named_class(stalled, human_action, expected):
    assert classify(record(stalled, human_action)) == expected


def test_record_matching_nothing_lands_in_unclassified():
    result = classify(
        record(
            "A dashboard number looked odd for a reason nobody could name",
            "The operator checked the source by hand and wrote a note",
        )
    )
    assert result == UNCLASSIFIED
    assert result == "unclassified"


def test_first_matching_class_in_list_order_wins():
    result = classify(
        record(
            "The prompt wording stalled the task",
            "The operator rephrased the prompt and then killed the run",
        )
    )
    assert result == "prompt-rephrase"


@pytest.mark.parametrize(
    "stalled,human_action",
    [
        (
            "The task needed a skilled hand to finish it",
            "A skilled operator restarted the service by hand",
        ),
        (
            "The label run had not finished printing",
            "The operator dispatched a courier with the printed label",
        ),
        (
            "The account balance was wrong after the import",
            "The operator credited the account with the missing amount",
        ),
    ],
)
def test_keyword_inside_a_longer_word_does_not_classify(stalled, human_action):
    assert classify(record(stalled, human_action)) == UNCLASSIFIED


@pytest.mark.parametrize(
    "stalled,human_action,expected",
    [
        (
            "The wording tangled the worker",
            "The operator is rephrasing the prompt now",
            "prompt-rephrase",
        ),
        (
            "The run would not halt on its own",
            "The operator interrupted the run",
            "run-interruption",
        ),
        (
            "The loop kept running unchecked",
            "An interruption by the operator ended it",
            "run-interruption",
        ),
    ],
)
def test_spelled_out_word_forms_still_classify(stalled, human_action, expected):
    assert classify(record(stalled, human_action)) == expected


def test_keyword_case_is_ignored_on_the_keyword_side():
    class_list = {
        "version": 2,
        "classes": [
            {
                "id": "loud-class",
                "keywords": ["KILLED"],
                "proposal": "Draft a test proposal only",
            }
        ],
        "unclassified_proposal": "Draft a test proposal only",
    }
    result = classify(
        record("The run kept going", "The operator killed the run"), class_list
    )
    assert result == "loud-class"
