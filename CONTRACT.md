# Contract - cavalry

This contract is the executable spec for cavalry. Every obligation below is stated so it can fail. The full contract is the first build act of this repo and it extends the stub obligations written here.

## Stub obligations

CV-S1. Every recorded rescue carries who intervened, what they did, and which run it touched, and a record missing any of those fields is rejected rather than stored incomplete.

CV-S2. Every rescue is classified into a named class, and a record that cannot be classified is held in an explicit unclassified state that reports itself rather than passing silently.

CV-S3. Every class with at least one recorded rescue carries a drafted hook or rule meant to retire that class, and a class with rescues and no draft is a defect the tests must catch.

CV-S4. Cavalry records human rescues only, and an automatic rescue performed by prod is never written into the cavalry record as a human rescue.

CV-S5. Every fixture and example in this repo is synthetic and built for this repo, and no real trace or session data appears anywhere in the tree.

## Round 1 obligations

Round 1 records the rescues only a human could make. It builds no model calls, it performs no rescue, it grades no rescuer, and it reimplements no automatic ladder from prod. The decision model material in ROADMAP.md is advisory design for later rounds and creates no Round 1 obligation.

CV-1. Every capture input is one JSON object carrying the fields id and run_ref and rescued_role and rescuer_role and rescuer_kind and stalled and human_action and occurred_at and outcome, plus an optional field named corrects, and no other field. The field stalled states what stalled, human_action states what the human did that the automatic ladder could not do, occurred_at states when the rescue happened as supplied by the capturer, outcome states what happened after the rescue, and run_ref names the run the rescue touched.

CV-2. The fields rescued_role and rescuer_role are role labels and never real names. A role label is one token made of a leading lowercase letter followed by lowercase letters or digits or hyphens, and a record whose role field is not a role label in this shape is refused with that field named.

CV-3. Cavalry records human rescues only. The field rescuer_kind must carry the value human, and a record carrying any other value is refused with rescuer_kind named and is never written into the cavalry record as a human rescue.

CV-4. The capture flow is the capture subcommand of the dependency free package cavalry run as python -m cavalry. Capture reads one record from a JSON argument file, validates it, classifies it, and appends exactly one line holding that record as JSON with sorted keys to the end of a records file held in JSON Lines form. A stored record carries every capture field plus a field named rescue_class added by the tooling.

CV-5. Capture validates a record on the way in. A record missing a required field, or carrying a required field with an empty value, is refused with the missing field named in the error and nothing is written. A record carrying an unknown field is refused with that field named and nothing is written. A record whose id is already present in the records file is refused with id named and nothing is written.

CV-6. Every record is classified into a rescue class from the versioned class list held in the repo at cavalry/classes.json. That file carries a version field and one entry per class, and each entry carries a class id and a keyword list and a drafted proposal text. Classification is deterministic and matches the lowercased combined text of stalled and human_action against the keyword lists in list order, and the first matching class wins.

CV-7. A record that matches no class in the versioned list is stored with rescue_class set to the explicit value unclassified. An unclassified record is never forced into a named class, and the list and propose outputs report unclassified records under that name rather than passing over them silently.

CV-8. A record once written is never edited by the tooling. Capture only ever appends to a records file and never rewrites or deletes an existing line. A correction lands as a new record whose corrects field carries the id of the record it corrects, and a correction whose corrects value names no record already present in the records file is refused with corrects named and nothing is written.

CV-9. The list subcommand prints one line per record in file order, and each printed line carries the record id and its rescue_class and its run_ref and its occurred_at and its outcome, so every record reports its class including the unclassified state.

CV-10. The propose subcommand prints one drafted hook or rule proposal for each distinct class present in a records file, in the order of the versioned class list with unclassified last when present. Each printed proposal names its class and carries the word proposal and states plainly that it is a proposal only. A proposal is never applied anywhere by the tooling, and no proposal output edits any other file or repo.

CV-11. Every output of the tooling is deterministic for a given input. No output uses a wall clock, and every timestamp printed comes from the occurred_at field of the record itself as supplied by the capturer. Repeating a command on an unchanged records file produces byte identical output.

CV-12. Every record, fixture, and example used by Round 1 is synthetic and written for this repo. No real name, real trace, real session data, or real operational detail appears in any record, test, or example in the tree.

CV-13. The examples directory holds a synthetic records file with at least three records spanning at least two named classes plus at least one unclassified record, and a committed proposals file holding the output of the propose subcommand run on that records file. The proposals file regenerates byte for byte with the command python3 -m cavalry propose examples/records.jsonl run from the repo root with its output written to examples/proposals.txt.

## Round 1 amendments

The amendments below tighten Round 1 behaviour after the red team audit and the two axis review of the Round 1 build. Each amendment states the behaviour it changes or adds and leaves the earlier text standing, so the record shows both the original obligation and the tightened one.

CV-14. Capture appends safely to a records file whose last line carries no trailing newline. When the records file is not empty and its last byte is not a newline, capture writes the missing newline first and then writes the new record line, so the bytes already stored stay an untouched prefix of the file and every stored record stays readable by the tooling.

CV-15. Every line read from a records file must hold one stored record. A line that parses as JSON but is not an object, or an object missing any field of the stored field set, or an object carrying a field outside that set, or a field whose value is not text, is refused by every subcommand with the records error naming the defect, and in that case no subcommand writes anything and none leaks a traceback or a bare internal error. The stored field set is every capture field plus rescue_class.

CV-16. The propose subcommand never passes over a stored class in silence. A stored record whose rescue_class names no class in the versioned class list and is not the unclassified value makes propose refuse with the records error naming that class.

CV-17. Keyword matching is whole word matching and ignores case on both sides. A keyword matches only where it appears in the combined text as a whole word or as a whole word with a trailing s, and a longer word that merely contains a keyword inside it never matches. Keywords are lowercased when the class list loads and the combined record text is lowercased before matching. The keyword lists in cavalry/classes.json spell out each word form they intend to catch in place of the stems the first version relied on, and the class list version rises to 2 for this change.

CV-18. A role label must match its shape over the whole value. A role field carrying any extra character around the label, including a trailing newline, is refused with that field named.

CV-19. The class list is validated when it loads. The file must carry its version and its unclassified proposal text, and every entry must carry a class id and a non empty keyword list and a non empty proposal text, and any defect in the file is refused with the records error naming the entry and the field rather than leaking a bare key error.

CV-20. The proposal text for the unclassified state lives in cavalry/classes.json alongside the named classes, and the tooling reads it from that file, so no proposal text is hardcoded in the package.

CV-21. The occurred_at field must carry a timestamp in the shape the examples carry, namely a four digit year and a two digit month and a two digit day joined by the letter T to a two digit hour and a two digit minute and a two digit second, with an optional trailing Z, and a record carrying any other shape is refused with occurred_at named.

CV-22. No field value in a capture input may carry a newline character or a carriage return character, and a record carrying one is refused with that field named, so a stored value can never shape the lines the list subcommand prints.
