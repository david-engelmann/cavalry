"""CV-24 to CV-27. Requests, the recorded shape, the player, and trust."""

import copy
import json

import pytest

from cavalry.judgment import (
    CONFIDENCE_THRESHOLD,
    FixturePlayer,
    JudgmentError,
    check_answer,
    classification_labels,
    classification_request,
    quality_request,
    trusted,
    validate_fixture,
)
from cavalry.records import load_class_list
from review_factory import AGREE, DIFFER

CLASS_LIST = load_class_list()
LABELS = classification_labels(CLASS_LIST)


def agree_pair():
    return {
        "judgment_point": "classification",
        "request": classification_request(AGREE, CLASS_LIST),
        "response": {
            "kind": "choice",
            "choice": "prompt-rephrase",
            "confidence": 0.9,
            "probabilities": {
                "prompt-rephrase": 0.9,
                "run-interruption": 0.04,
                "manual-edit": 0.03,
                "new-class": 0.03,
            },
        },
        "synthetic": True,
    }


def write_fixture(tmp_path, name, pair):
    path = tmp_path / name
    path.write_text(json.dumps(pair), encoding="utf-8")
    return path


def test_classification_request_is_built_from_the_record_and_the_class_list():
    assert classification_request(AGREE, CLASS_LIST) == {
        "record_id": "fox-001",
        "stalled": AGREE["stalled"],
        "human_action": AGREE["human_action"],
        "classes": ["prompt-rephrase", "run-interruption", "manual-edit"],
    }


def test_quality_request_is_built_from_the_record():
    assert quality_request(DIFFER) == {
        "record_id": "fox-002",
        "stalled": DIFFER["stalled"],
        "human_action": DIFFER["human_action"],
        "outcome": DIFFER["outcome"],
    }


def test_labels_offer_every_class_plus_the_new_class_label():
    assert LABELS == ["prompt-rephrase", "run-interruption", "manual-edit", "new-class"]


def test_player_answers_a_recorded_request_exactly():
    import pathlib

    player = FixturePlayer(pathlib.Path(__file__).resolve().parents[1] / "fixtures")
    answer = player.judge("classification", classification_request(AGREE, CLASS_LIST))
    assert answer["choice"] == "prompt-rephrase"
    assert answer["confidence"] == 0.9


def test_player_returns_no_answer_where_nothing_matches(tmp_path):
    write_fixture(tmp_path, "one.json", agree_pair())
    player = FixturePlayer(tmp_path)
    request = classification_request(AGREE, CLASS_LIST)
    assert player.judge("classification", request) is not None
    shifted = dict(request, stalled=request["stalled"] + " again")
    assert player.judge("classification", shifted) is None
    assert player.judge("quality", quality_request(AGREE)) is None


def test_player_refuses_two_fixtures_for_the_same_request(tmp_path):
    write_fixture(tmp_path, "one.json", agree_pair())
    write_fixture(tmp_path, "two.json", agree_pair())
    with pytest.raises(JudgmentError) as caught:
        FixturePlayer(tmp_path)
    assert "two.json" in str(caught.value)


def test_player_refuses_a_missing_directory(tmp_path):
    with pytest.raises(JudgmentError):
        FixturePlayer(tmp_path / "nope")


def test_validator_refuses_a_response_of_the_wrong_kind_for_the_point():
    pair = agree_pair()
    pair["response"] = {"kind": "score", "score": 0.5, "confidence": 0.9}
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


def test_validator_refuses_a_choice_response_carrying_an_extra_field():
    pair = agree_pair()
    pair["response"]["score"] = 0.5
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


def test_validator_refuses_an_abstention_carrying_a_confidence():
    pair = agree_pair()
    pair["response"] = {"kind": "abstention", "confidence": 0.9}
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


def test_validator_refuses_probabilities_missing_a_label():
    pair = agree_pair()
    del pair["response"]["probabilities"]["new-class"]
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


def test_validator_refuses_probabilities_that_do_not_add_up_to_one():
    pair = agree_pair()
    pair["response"]["probabilities"]["new-class"] = 0.5
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


def test_validator_refuses_a_choice_outside_the_labels_offered():
    pair = agree_pair()
    pair["response"]["choice"] = "some-other-class"
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


def test_validator_refuses_a_fixture_not_marked_synthetic():
    pair = agree_pair()
    pair["synthetic"] = False
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


def test_validator_refuses_a_fixture_carrying_an_extra_field():
    pair = agree_pair()
    pair["note"] = "recorded by hand"
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


def test_validator_refuses_a_score_outside_the_unit_interval():
    pair = {
        "judgment_point": "quality",
        "request": quality_request(AGREE),
        "response": {"kind": "score", "score": 1.4, "confidence": 0.9},
        "synthetic": True,
    }
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


def test_a_malformed_fixture_file_refuses_the_directory_naming_the_file(tmp_path):
    pair = agree_pair()
    del pair["response"]["probabilities"]
    write_fixture(tmp_path, "broken.json", pair)
    with pytest.raises(JudgmentError) as caught:
        FixturePlayer(tmp_path)
    assert "broken.json" in str(caught.value)


def test_trusted_edges_at_the_contract_threshold():
    base = {"kind": "score", "score": 0.5}
    assert trusted(dict(base, confidence=CONFIDENCE_THRESHOLD))
    assert trusted(dict(base, confidence=1.0))
    assert not trusted(dict(base, confidence=0.59))
    assert not trusted({"kind": "abstention"})
    assert not trusted(None)
    assert not trusted(dict(base, confidence="high"))


def test_check_answer_passes_a_well_formed_answer_through():
    answer = {
        "kind": "choice",
        "choice": "manual-edit",
        "confidence": 0.8,
        "probabilities": {
            "prompt-rephrase": 0.1,
            "run-interruption": 0.05,
            "manual-edit": 0.8,
            "new-class": 0.05,
        },
    }
    assert check_answer("classification", answer, LABELS) is answer
    assert check_answer("quality", {"kind": "score", "score": 0.4, "confidence": 0.7})


def test_check_answer_counts_a_malformed_answer_as_no_judgment():
    assert check_answer("classification", {"kind": "score", "score": 0.5, "confidence": 0.9}, LABELS) is None
    assert check_answer("quality", {"kind": "score", "confidence": 0.9}) is None
    assert check_answer("quality", {"kind": "score", "score": "deep", "confidence": 0.9}) is None
    assert check_answer("classification", "not an object", LABELS) is None
    assert check_answer("classification", None, LABELS) is None


def test_check_answer_counts_an_answer_with_an_extra_field_as_no_judgment():
    answer = {"kind": "score", "score": 0.5, "confidence": 0.9, "choice": "recovering"}
    assert check_answer("quality", answer) is None


def test_player_answers_are_copies_the_caller_cannot_corrupt(tmp_path):
    write_fixture(tmp_path, "one.json", agree_pair())
    player = FixturePlayer(tmp_path)
    request = classification_request(AGREE, CLASS_LIST)
    first = player.judge("classification", request)
    first["choice"] = "manual-edit"
    second = player.judge("classification", request)
    assert second["choice"] == "prompt-rephrase"


def test_fixture_pairs_are_not_mutated_by_validation():
    pair = agree_pair()
    snapshot = copy.deepcopy(pair)
    assert validate_fixture(pair, source="pair") is pair
    assert pair == snapshot
