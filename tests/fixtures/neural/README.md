# Neural-disambiguation fixtures

Phase 1 fixtures for the candidate-coverage harness. They are not model
training data.

- `ud_sample.conllu` — hand-authored CoNLL-U smoke corpus. MIT OR
  Apache-2.0, like the rest of the repository. It is not copied from UD; it
  exercises NOUN, VERB, NUM, PRON, ADP, AUX and PUNCT.
- `manual_ambiguity.json` — initial manual ambiguity format. Labels are
  candidate signatures, not reading indices. A resolver must not assume that a
  preferred reading identifies a unique lemma.

Run:

```bash
cargo run --release -p torille-tools --bin ud_coverage -- \
  --file tests/fixtures/neural/ud_sample.conllu \
  --ambiguity tests/fixtures/neural/manual_ambiguity.json
```

The mapping version is `fi-ud-tags-1` (`tools/src/ud/mapping.rs`). A mapping
change bumps the version and should update this fixture.
