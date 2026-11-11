# cavalry

The troop that rides in after the fact and writes down what a human responder actually did, because the agent log alone cannot be trusted.

Cavalry is a command line tool that keeps the book of human rescues for a fleet of agents. When an automated run stalls and a person steps in, the rescuer writes the rescue down as one JSON object, and the tool validates it, classifies it against a versioned class list, and appends it to an append only records file. The book then answers as a list of records, as the drafted hook or rule each class carries, and as a queue of the records that still wait for a human decision. Westering rescues automatically on its graded ladder and cavalry records only the rescues a human made, and the kill switch arvin holds is the last rung both point at and cavalry never reimplements.

[![CI](https://github.com/david-engelmann/cavalry/actions/workflows/ci.yml/badge.svg)](https://github.com/david-engelmann/cavalry/actions/workflows/ci.yml) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

```console
$ ./target/debug/cavalry list examples/records.jsonl
rec-001 prompt-rephrase run-synthetic-001 2026-01-01T09:00:00Z The task finished on the next run
rec-002 run-interruption run-synthetic-002 2026-01-02T11:30:00Z The bad output was removed and the run was queued again
rec-003 manual-edit run-synthetic-003 2026-01-03T14:15:00Z The file went out with the right heading
rec-004 unclassified run-synthetic-004 2026-01-04T16:45:00Z The number was explained in the note and no run was changed
```

## Install

Cavalry is a Rust crate that builds from source with cargo.

```sh
git clone https://github.com/david-engelmann/cavalry.git
cd cavalry
cargo build
```

The binary lands at target/debug/cavalry, and the test suite runs with `cargo test`.

## Quickstart

From the repository root, with the committed examples.

```sh
./target/debug/cavalry list examples/records.jsonl
```

The command prints the book at the top of this page, one line per record with its id, its class, the run it touched, when it happened, and what happened after the rescue.

## Usage

### capture

`cavalry capture <input-file> <records-file>` stores one rescue record. The input is one JSON object naming who was rescued, who rescued, what stalled, what the human did, when it happened, and what happened after. The tool validates the record, classifies it against the class list at cavalry/classes.json, and appends it to the records file without touching the bytes already stored. A record whose rescuer is not a human is refused, and nothing is written.

```console
$ cat examples/rescue.json
{"human_action":"The operator rephrased the prompt and restarted the task","id":"rec-101","occurred_at":"2026-01-05T09:30:00Z","outcome":"The task finished on the next run","rescued_role":"support-operator","rescuer_kind":"human","rescuer_role":"on-call-operator","run_ref":"run-synthetic-101","stalled":"The task stalled because the prompt wording sent the worker in a circle"}
$ ./target/debug/cavalry capture examples/rescue.json book.jsonl
stored rec-101 as prompt-rephrase
$ cat book.jsonl
{"human_action": "The operator rephrased the prompt and restarted the task", "id": "rec-101", "occurred_at": "2026-01-05T09:30:00Z", "outcome": "The task finished on the next run", "rescue_class": "prompt-rephrase", "rescued_role": "support-operator", "rescuer_kind": "human", "rescuer_role": "on-call-operator", "review_state": "unreviewed", "run_ref": "run-synthetic-101", "stalled": "The task stalled because the prompt wording sent the worker in a circle"}
```

With `--fixtures <directory>` the capture also runs the recorded judgment seam. A trusted classification answer attaches a proposed class to the record and moves it to the proposed state, and a trusted quality score below the bar sets the record's quality flag, and quality alone never moves the review state. An abstention or an untrusted answer at the classification point leaves the record unreviewed, exactly as a capture with no fixtures leaves it.

### list

`cavalry list <records-file>` prints the book, one line per record in file order, as the block at the top of this page shows.

### propose

`cavalry propose <records-file>` prints the proposals document. Each class with at least one counted record prints its drafted hook or rule, and a record a confirmation supersedes counts only through the record that confirms it.

```console
$ ./target/debug/cavalry propose examples/records.jsonl
proposal for class prompt-rephrase
Draft a hook that checks a prompt for the wording pattern in these records before a run starts and suggests clearer wording in its place. This is a proposal only and is never applied anywhere.

proposal for class run-interruption
Draft a rule that flags a run showing the pattern in these records early enough that a human no longer has to stop it by hand. This is a proposal only and is never applied anywhere.

proposal for class manual-edit
Draft a hook that validates the generated output before handover so the manual edit in these records is no longer needed. This is a proposal only and is never applied anywhere.

proposal for class unclassified
Draft a review that reads each unclassified record and proposes either a new class or wider keywords for an existing class. This is a proposal only and is never applied anywhere.
```

A proposal is a draft only. It is never applied anywhere, and no subcommand edits another repo or settles a review.

### queue

`cavalry queue <records-file>` prints only the records whose current state still waits for a human, each with the reason it waits, so a reviewer works the book from the queue instead of reading every stored line to find the open decisions.

```console
$ ./target/debug/cavalry queue examples/records.jsonl
rec-001 prompt-rephrase run-synthetic-001 2026-01-01T09:00:00Z The task finished on the next run awaiting review
rec-002 run-interruption run-synthetic-002 2026-01-02T11:30:00Z The bad output was removed and the run was queued again awaiting review
rec-003 manual-edit run-synthetic-003 2026-01-03T14:15:00Z The file went out with the right heading awaiting review
rec-004 unclassified run-synthetic-004 2026-01-04T16:45:00Z The number was explained in the note and no run was changed awaiting review
```

### confirm

`cavalry confirm <confirmation-file> <records-file>` stores the confirmation a review settled. The confirmation file names a record that stands under review and the class it was confirmed into, and the tool appends one new record that confirms it and prints one line naming the record and the class it was confirmed into. A record carrying no proposal under review is refused, and no existing line is ever edited.

## Status

The tool is complete at the surface its contract describes, with no stubbed or held back part, and the full CI gate runs on every change. Round 10 landed in PR 23, guarding the help short on every command in the tree and closing the root table gap in the long pin. Round 11 landed in PR 25, firing the help spellings behind every command in the committed corpus. Round 12 landed in PR 27, firing the queue behind its states in the committed corpus. Round 13 landed in PR 29, firing the propose command behind its counted classes in the committed corpus. Round 14 landed in PR 31, pinning the argument walk's branches by direct unit tests in the binary's test module. Round 15 landed in PR 34, firing the fixture player's load and validation families behind capture in the committed corpus. Closed rounds 15 of 100.

## License

MIT. See [LICENSE](LICENSE).
