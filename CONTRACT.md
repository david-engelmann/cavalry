# Contract - cavalry

This contract is the executable spec for cavalry. Every obligation below is stated so it can fail. The full contract is the first build act of this repo and it extends the stub obligations written here.

## Stub obligations

CV-S1. Every recorded rescue carries who intervened, what they did, and which run it touched, and a record missing any of those fields is rejected rather than stored incomplete.

CV-S2. Every rescue is classified into a named class, and a record that cannot be classified is held in an explicit unclassified state that reports itself rather than passing silently.

CV-S3. Every class with at least one recorded rescue carries a drafted hook or rule meant to retire that class, and a class with rescues and no draft is a defect the tests must catch.

CV-S4. Cavalry records human rescues only, and an automatic rescue performed by prod is never written into the cavalry record as a human rescue.

CV-S5. Every fixture and example in this repo is synthetic and built for this repo, and no real trace or session data appears anywhere in the tree.
