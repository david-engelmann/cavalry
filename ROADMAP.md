# Roadmap - cavalry

Cavalry records the rescues only a human could make. The setup slice is landed and the contract stub stands at CV-S1 to CV-S5. Round 1 builds the rescue record format, the manual capture flow, and the first rescue classes.

## Decision models in cavalry

Judgment enters at two points in the record pipeline, and nowhere else.

The first point is classification review. When a human rescue record arrives, someone has to judge whether it matches a rescue class already on the books or whether it is the first instance of a new class. That judgment is a candidate for a decision model call shaped as a choice over the current class identifiers plus one further label for a new class. The second point is record quality. A record can be complete enough to classify from or too thin to teach anything, and that judgment is a candidate for a score call that rates how classifiable the record is.

What the model decides stays advisory in both places. The record a human made stands exactly as written even when the model disagrees. A classification the model proposes is a proposal attached to the record, and a human confirms or replaces it before it becomes part of the class history. A low quality score flags the record for a follow-up conversation with the rescuer. It never edits the record and never rejects it.

What stays deterministic is the boundary itself and everything around the calls. Cavalry never performs a rescue and never grades one for the rescuer's employer or anyone else. The capture flow, the record format, the class list, and the rule that a drafted hook or rule leaves this repo only as a proposal are fixed rules with no model inside them. A model abstention or a confidence below the threshold set in the contract routes the record to the unreviewed queue, which is where records waited before the model existed.

The candidate model is Typesafe's Jev, called through the program's Typesafe connector, with the OpenAI decision model as the comparison point. Both are evaluated against the same recorded fixtures before either is adopted at a decision point, and the deterministic core of this repo never depends on a provider being reachable.

Tests stay offline. Live answers are captured once, in a bounded recording session, as request and response fixture pairs built from synthetic rescue records, and the suite plays those fixtures back through a fixture player. No test calls the network. A schema test pins the fixture shape so a provider response that changes shape fails loudly in the recording session instead of silently in production.

The live-call budget for this repo is set as an envelope and owned by this arc. Trivial verification is capped at 5 calls. Fixture recording is capped at 150 calls in one designated session per round that needs new fixtures. A comparison evaluation is capped at 300 calls per model when a decision point is being adopted. Every call carries synthetic state only, every session is logged in the round record with its call count, and raising the envelope is a contract change recorded in this repo, never a silent overrun.
