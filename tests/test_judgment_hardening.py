"""CV-35. The hardened validation discipline of the judgment seam."""

import json
import math

import pytest

from cavalry.judgment import (
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
from review_factory import AGREE

CLASS_LIST = load_class_list()
LABELS = classification_labels(CLASS_LIST)
HUGE = 10**400


def choice_response(**changes):
    response = {
        "kind": "choice",
        "choice": "prompt-rephrase",
        "confidence": 0.9,
        "probabilities": {
            "prompt-rephrase": 0.9,
            "run-interruption": 0.04,
            "manual-edit": 0.03,
            "new-class": 0.03,
        },
    }
    response.update(changes)
    return response


def classification_pair(response):
    return {
        "judgment_point": "classification",
        "request": classification_request(AGREE, CLASS_LIST),
        "response": response,
        "synthetic": True,
    }


def quality_pair(response):
    return {
        "judgment_point": "quality",
        "request": quality_request(AGREE),
        "response": response,
        "synthetic": True,
    }


def test_load_refuses_a_response_whose_kind_is_not_a_string():
    pair = classification_pair(choice_response(kind=["choice"]))
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


def test_a_directory_holding_a_non_string_kind_fixture_is_refused(tmp_path):
    pair = classification_pair(choice_response(kind=["choice"]))
    (tmp_path / "broken.json").write_text(json.dumps(pair), encoding="utf-8")
    with pytest.raises(JudgmentError) as caught:
        FixturePlayer(tmp_path)
    assert "broken.json" in str(caught.value)


def test_load_refuses_a_confidence_that_is_not_a_finite_number():
    pair = quality_pair({"kind": "score", "score": 0.5, "confidence": HUGE})
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


def test_load_refuses_a_score_that_is_not_a_finite_number():
    pair = quality_pair({"kind": "score", "score": HUGE, "confidence": 0.9})
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


def test_load_refuses_a_probability_that_is_not_a_finite_number():
    probabilities = {
        "prompt-rephrase": 0.9,
        "run-interruption": 0.04,
        "manual-edit": 0.03,
        "new-class": HUGE,
    }
    pair = classification_pair(choice_response(probabilities=probabilities))
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")


def test_load_refuses_a_fixture_envelope_with_mixed_type_extra_fields():
    pair = classification_pair(choice_response())
    pair[7] = "recorded by hand"
    pair["note"] = "recorded by hand"
    with pytest.raises(JudgmentError) as caught:
        validate_fixture(pair, source="mutant")
    assert "note" in str(caught.value)


def test_load_refuses_a_response_with_mixed_type_extra_fields():
    response = choice_response()
    response[7] = "a"
    response["bogus"] = "b"
    pair = classification_pair(response)
    with pytest.raises(JudgmentError) as caught:
        validate_fixture(pair, source="mutant")
    assert "bogus" in str(caught.value)


def test_check_answer_counts_a_non_string_kind_as_no_judgment():
    answer = choice_response(kind=["choice"])
    assert check_answer("classification", answer, LABELS) is None


def test_check_answer_counts_an_unconvertible_confidence_as_no_judgment():
    answer = {"kind": "score", "score": 0.5, "confidence": HUGE}
    assert check_answer("quality", answer) is None


def test_check_answer_counts_an_unconvertible_score_as_no_judgment():
    answer = {"kind": "score", "score": HUGE, "confidence": 0.9}
    assert check_answer("quality", answer) is None


def test_check_answer_counts_an_unconvertible_probability_as_no_judgment():
    probabilities = {
        "prompt-rephrase": 0.9,
        "run-interruption": 0.04,
        "manual-edit": 0.03,
        "new-class": HUGE,
    }
    answer = choice_response(probabilities=probabilities)
    assert check_answer("classification", answer, LABELS) is None


def test_check_answer_counts_mixed_type_extra_fields_as_no_judgment():
    answer = {"kind": "score", "score": 0.5, "confidence": 0.9, 7: "a", "bogus": "b"}
    assert check_answer("quality", answer) is None


def test_trusted_never_trusts_an_infinite_confidence():
    answer = {"kind": "score", "score": 0.5, "confidence": math.inf}
    assert not trusted(answer)


def test_trusted_returns_false_for_an_unconvertible_confidence():
    answer = {"kind": "score", "score": 0.5, "confidence": HUGE}
    assert not trusted(answer)


def test_probabilities_within_one_thousandth_of_one_are_accepted():
    probabilities = {
        "prompt-rephrase": 0.4,
        "run-interruption": 0.3,
        "manual-edit": 0.2,
        "new-class": 0.1005,
    }
    pair = classification_pair(choice_response(probabilities=probabilities))
    assert validate_fixture(pair, source="mutant") is pair
    answer = choice_response(probabilities=probabilities)
    assert check_answer("classification", answer, LABELS) is answer


def test_probabilities_beyond_one_thousandth_of_one_are_refused():
    probabilities = {
        "prompt-rephrase": 0.4,
        "run-interruption": 0.3,
        "manual-edit": 0.2,
        "new-class": 0.102,
    }
    pair = classification_pair(choice_response(probabilities=probabilities))
    with pytest.raises(JudgmentError):
        validate_fixture(pair, source="mutant")
    answer = choice_response(probabilities=probabilities)
    assert check_answer("classification", answer, LABELS) is None
