"""CV-25 and CV-26. The committed fixtures and their schema pin."""

import json
import pathlib

from cavalry.judgment import CONFIDENCE_THRESHOLD, FixturePlayer, validate_fixture

ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "fixtures"


def committed_pairs():
    pairs = []
    for path in sorted(FIXTURES.glob("*.json")):
        pairs.append((path.name, json.loads(path.read_text(encoding="utf-8"))))
    return pairs


def test_every_committed_fixture_matches_the_recorded_shape():
    pairs = committed_pairs()
    assert pairs, "no committed fixtures found"
    for name, pair in pairs:
        assert validate_fixture(pair, source=name) is pair


def test_every_committed_fixture_is_marked_synthetic():
    for name, pair in committed_pairs():
        assert pair.get("synthetic") is True, f"fixture {name} is not marked synthetic"


def test_the_player_loads_the_committed_directory():
    player = FixturePlayer(FIXTURES)
    for _, pair in committed_pairs():
        assert player.judge(pair["judgment_point"], pair["request"]) is not None


def test_the_committed_set_covers_both_judgment_points():
    points = {pair["judgment_point"] for _, pair in committed_pairs()}
    assert points == {"classification", "quality"}


def test_the_classification_set_covers_agreement_difference_and_the_new_class_label():
    choices = {
        pair["response"]["choice"]
        for _, pair in committed_pairs()
        if pair["judgment_point"] == "classification"
        and pair["response"].get("kind") == "choice"
    }
    # fox-001 agrees with its deterministic class, fox-002 proposes a
    # different existing class, and fox-003 proposes the new label.
    assert choices == {"prompt-rephrase", "manual-edit", "new-class"}


def test_the_classification_set_has_an_abstention_and_a_below_threshold_case():
    responses = [
        pair["response"]
        for _, pair in committed_pairs()
        if pair["judgment_point"] == "classification"
    ]
    assert any(response.get("kind") == "abstention" for response in responses)
    assert any(
        response.get("kind") == "choice"
        and response.get("confidence", 1.0) < CONFIDENCE_THRESHOLD
        for response in responses
    )


def test_the_quality_set_covers_scores_on_both_sides_of_the_bar_and_an_abstention():
    from cavalry.judgment import QUALITY_BAR

    responses = [
        pair["response"]
        for _, pair in committed_pairs()
        if pair["judgment_point"] == "quality"
    ]
    scores = [response["score"] for response in responses if response.get("kind") == "score"]
    assert any(score >= QUALITY_BAR for score in scores)
    assert any(score < QUALITY_BAR for score in scores)
    assert any(response.get("kind") == "abstention" for response in responses)
