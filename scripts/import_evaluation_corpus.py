#!/usr/bin/env python3
"""Reproducible evaluation from upstream UD test splits; never consult the detector.

Published tokens supply clean negatives. Disjoint tokens are keyboard-remapped to
produce synthetic wrong-layout positives. This is an engineering evaluation, not
a replacement for naturally occurring bilingual typing or independent review.
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SOURCES = [
    ("english", "UD_English-EWT", "4a4d77f599ea53cc405f85d0cec4b2f14f81d42b", "en_ewt-ud-train.conllu"),
    ("russian", "UD_Russian-SynTagRus", "c5266a6f6f4aaa94f4dfa9b6cc863b94a5358afa", "ru_syntagrus-ud-train-a.conllu"),
]
en = "`qwertyuiop[]asdfghjkl;'zxcvbnm,./~QWERTYUIOP{}ASDFGHJKL:\"ZXCVBNM<>?"
ru = "ёйцукенгшщзхъфывапролджэячсмитьбю.ЁЙЦУКЕНГШЩЗХЪФЫВАПРОЛДЖЭЯЧСМИТЬБЮ,"
cases = []
provenance = []
licenses = ROOT / "benchmarks" / "licenses"
licenses.mkdir(exist_ok=True)
smoke = [json.loads(line) for line in (ROOT / "benchmarks/smoke.jsonl").read_text(encoding="utf-8").splitlines() if line]
excluded = {word.casefold() for case in smoke for word in [case["input"], case["expected"] or ""]}
development = ROOT / "benchmarks/development.jsonl"
if development.exists():
    for line in development.read_text(encoding="utf-8").splitlines():
        case = json.loads(line)
        excluded.update(word.casefold() for word in [case["input"], case["expected"] or ""])
for layout, repository, revision, filename in SOURCES:
    base = f"https://raw.githubusercontent.com/UniversalDependencies/{repository}/{revision}/"
    def fetch(name):
        return subprocess.check_output(["curl", "--fail", "--silent", "--show-error", "--location", base + name])
    raw = fetch(filename)
    license_text = fetch("LICENSE.txt")
    readme = fetch("README.md")
    (licenses / f"{repository}-LICENSE.txt").write_bytes(license_text)
    (licenses / f"{repository}-README.md").write_bytes(readme)
    tokens = set()
    for line in raw.decode("utf-8").splitlines():
        parts = line.split("\t")
        if len(parts) == 10 and parts[0].isdigit():
            word = parts[1]
            if not any(c.isspace() for c in word) and word.casefold() not in excluded:
                tokens.add(word)
    ordered = sorted(tokens, key=lambda word: hashlib.sha256(("codeswitch-evaluation-v1:" + word).encode()).digest())
    correct = re.compile(r"[a-zA-Z]{3,64}" if layout == "english" else r"[а-яА-ЯёЁ]{3,64}")
    positives = [word for word in ordered if correct.fullmatch(word) and (word.islower() or word.isupper() or word.istitle())][:1500]
    positive_set = set(positives)
    negatives = [word for word in ordered if word not in positive_set][:3000]
    for word in negatives:
        cases.append({"input": word, "layout": layout, "expected": None})
    table = str.maketrans(en, ru) if layout == "english" else str.maketrans(ru, en)
    for word in positives:
        cases.append({"input": word.translate(table), "layout": "russian" if layout == "english" else "english", "expected": word})
    provenance.append({"repository": f"https://github.com/UniversalDependencies/{repository}", "revision": revision, "url": base + filename, "sha256": hashlib.sha256(raw).hexdigest(), "negative_cases": len(negatives), "synthetic_positive_cases": len(positives), "record_range": [len(cases) - len(negatives) - len(positives) + 1, len(cases)], "license": "CC BY-NC-SA 4.0" if layout == "russian" else "CC BY-SA 4.0"})
destination = ROOT / "benchmarks" / "held-out.jsonl"
destination.write_text("".join(json.dumps(case, ensure_ascii=False) + "\n" for case in cases), encoding="utf-8")
metadata = {
    "held_out": True,
    "independently_annotated": False,
    "provenance": "Deterministic samples from previously unused upstream Universal Dependencies train splits, pinned by commit. These files are used only for CodeSwitch evaluation, never detector training. Original corpus authors supply the text. Selection does not consult CodeSwitch or wordfreq. All smoke and development input/expected vocabulary is excluded case-insensitively, and positive/negative source tokens are disjoint. Negative labels preserve published tokens; positive labels are synthetic keyboard remappings, not independently reviewed natural wrong-layout examples.",
    "annotation_method": "source-derived negatives and synthetic keyboard-remapped positives",
    "license": "Collection: English-EWT-derived records retain CC BY-SA 4.0; Russian-SynTagRus-derived records retain CC BY-NC-SA 4.0. See per-source record ranges and benchmarks/licenses. Not bundled with the application.",
    "limitations": "Token vocabulary sample, not an estimate of live bilingual typing prevalence. No surrounding sentence context or independent human review of correction labels. Does not by itself unlock public automatic correction.",
    "sources": provenance,
}
destination.with_suffix(".meta.json").write_text(json.dumps(metadata, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
print(f"Prepared {len(cases):,} evaluation cases without consulting detector results.")
