#!/usr/bin/env python3
"""Run the Rust detector against annotated JSONL; report aggregate counts only."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
from source_digest import ROOT, source_digest

corpus = Path(sys.argv[1] if len(sys.argv) > 1 else "benchmarks/held-out.jsonl").resolve()
if not corpus.is_file():
    sys.exit("Supply an annotated corpus. See benchmarks/README.md; a smoke fixture is not release evidence.")
output = Path(sys.argv[2] if len(sys.argv) > 2 else "benchmarks/report.json")
result = subprocess.check_output(["cargo", "run", "--quiet", "-p", "codeswitch-engine", "--bin", "benchmark", "--", str(corpus)], cwd=ROOT)
report = json.loads(result)
report["source_sha256"] = source_digest()
report["corpus"] = corpus.relative_to(ROOT).as_posix()
report["corpus_sha256"] = hashlib.sha256(corpus.read_bytes()).hexdigest()
metadata = corpus.with_suffix(".meta.json")
report["metadata_sha256"] = hashlib.sha256(metadata.read_bytes()).hexdigest() if metadata.exists() else None
output.write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
