# Detection benchmark

The smoke cases validate implementation behavior only. They are **not held out**
and do not establish real-world precision. No public precision claim is made.

The committed engineering evaluation uses original UD English EWT and Russian
SynTagRus text with synthetic wrong-layout positives. See `held-out.meta.json`
and upstream attributions in `licenses/`. It is not an independently reviewed
natural-typing corpus and cannot alone unlock public correction. Reproduce it
with `python3 scripts/import_evaluation_corpus.py` (network access required).
The initial 9,000-case sample found a punctuation/inflection bug and is retained
as `development.jsonl` with its historical v1 baseline. The final evaluation
uses previously unused source files and excludes all development vocabulary.

Run `python3 scripts/benchmark.py benchmarks/smoke.jsonl benchmarks/smoke-report.json`.

For release validation, add `benchmarks/held-out.jsonl` containing independently
annotated words/runs from representative English/Russian prose, names, brands,
code-like identifiers, URLs, and genuine wrong-layout input. Do not sample only
from the frequency resources, generate only easy remapped dictionary words, or
tune the detector on this corpus. Retain redistribution permission and provenance.

Each JSONL line has exactly these keys:

```json
{"input":"ghbdtn","layout":"english","expected":"привет"}
{"input":"hello","layout":"english","expected":null}
```

`input` is a completed non-whitespace run, without its trailing space. `layout`
is `english` or `russian`. `expected: null` means any correction is an error.
An incorrect replacement counts as both a false positive and a false negative.
The report includes misses/recall as well as precision. No individual cases are
printed in reports.

Add `held-out.meta.json` with `held_out: true`, `independently_annotated: true`,
and a nonempty `provenance` string describing collection, annotation and rights.
Then run `npm run benchmark`. Use at least 2,500 negative cases and 100 true
positive detections **per input layout**. Require at least 99.5% precision overall
and separately in each direction. Review recall before approval; conservative
misses are intentionally preferable to unwanted rewrites.

After both platform checklists pass, record approvals in `release/readiness.json`
and run `npm run check:release`. The gate checks current source/resource hashes,
corpus and metadata hashes, coverage, precision, policy and manual approvals.
Any implementation change requires rerunning validation. Release builds that do
not pass the gate cannot enable system-wide automatic correction. Debug builds
can opt into an explicitly acknowledged development preview.
