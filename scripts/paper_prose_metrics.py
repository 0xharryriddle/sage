#!/usr/bin/env python3
"""Report approximate prose metrics for the SAGE paper.

The optional --gate checks the historical numeric thresholds encoded below.
This editorial diagnostic is not a policy, proof, or submission-readiness gate.
LaTeX stripping and sentence counts are approximate.

It reports, and with --gate flags:
  * abstract word count outside [MIN_ABSTRACT_WORDS, MAX_ABSTRACT_WORDS]
  * any \\cite or \\ref inside the abstract (belongs in Related Work)
  * sentences longer than HARD_SENTENCE_WORDS
  * hedge density above MAX_HEDGE_PER_100W
  * any subsection above MAX_SUBSECTION_WORDS

Usage:
    python3 scripts/paper_prose_metrics.py                 # report
    python3 scripts/paper_prose_metrics.py --gate           # report + fail closed
    python3 scripts/paper_prose_metrics.py --json           # machine readable
    python3 scripts/paper_prose_metrics.py --self-test      # verify the gates bite
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

DEFAULT_PAPER = Path("docs/paper/SAGE_ThaiCong_IEEE/main.tex")

# Historical editorial thresholds for the optional local diagnostic gate.
MIN_ABSTRACT_WORDS = 150
MAX_ABSTRACT_WORDS = 200
HARD_SENTENCE_WORDS = 45
SOFT_SENTENCE_WORDS = 35
MAX_SOFT_SENTENCES = 15
MAX_HEDGE_PER_100W = 0.60
MAX_SUBSECTION_WORDS = 900

# Defensive-language markers. These are not banned outright: a paper about an
# unfinished prototype legitimately scopes its claims. The gate bounds their
# DENSITY so scope language stays load-bearing instead of becoming the voice of
# the paper.
HEDGES = (
    "only",
    "does not",
    "do not",
    "cannot",
    "not implemented",
    "not attested",
    "not authenticated",
    "not evidence",
    "future work",
    "remains future",
    "remains open",
    "unknown",
    "no claim",
    "we do not claim",
)


def expand_local_inputs(text: str, base_dir: Path, _seen: set[Path] | None = None) -> str:
    r"""Expand local ``\input{...}`` files before measuring the manuscript."""
    seen = set() if _seen is None else set(_seen)
    pattern = re.compile(r"\\input\{([^}]+)\}")

    def replace(match: re.Match[str]) -> str:
        candidate = (base_dir / match.group(1)).with_suffix(".tex")
        if not candidate.is_file() or candidate.resolve() in seen:
            return match.group(0)
        return expand_local_inputs(
            candidate.read_text(encoding="utf-8"),
            candidate.parent,
            seen | {candidate.resolve()},
        )

    return pattern.sub(replace, text)


def strip_latex(text: str) -> str:
    """Reduce LaTeX to approximate prose.

    Removes math, comments, and the environments that are not prose (figures,
    tables, algorithms, code, bibliography). Sentence and word statistics over
    raw LaTeX are dominated by macro noise: an earlier measurement reported a
    110-word "sentence" that was entirely preamble macros.
    """
    text = re.sub(r"(?<!\\)%.*", "", text)
    for env in (
        "figure",
        "figure\\*",
        "table",
        "table\\*",
        "algorithm",
        "algorithmic",
        "lstlisting",
        "verbatim",
        "tikzpicture",
        "thebibliography",
        "IEEEkeywords",
    ):
        text = re.sub(
            rf"\\begin\{{{env}\}}.*?\\end\{{{env}\}}", " ", text, flags=re.S
        )
    text = re.sub(r"\$\$.*?\$\$", " ", text, flags=re.S)
    text = re.sub(r"\$.*?\$", " ", text, flags=re.S)
    text = re.sub(r"\\\[.*?\\\]", " ", text, flags=re.S)
    text = re.sub(r"\\(cite|ref|eqref|label|autoref)\{[^}]*\}", " ", text)
    text = re.sub(r"\\[a-zA-Z@]+\*?", " ", text)
    text = re.sub(r"[{}\\&~^_]", " ", text)
    return re.sub(r"\s+", " ", text)


def sentences(prose: str) -> list[str]:
    parts = re.split(r"(?<=[.!?])\s+", prose)
    return [p.strip() for p in parts if len(p.split()) >= 3]


def words(text: str) -> int:
    return len(text.split())


def extract_abstract(raw: str) -> str | None:
    match = re.search(r"\\begin\{abstract\}(.*?)\\end\{abstract\}", raw, flags=re.S)
    return match.group(1) if match else None


def section_map(raw: str) -> list[dict[str, object]]:
    """Split the body into (sub)sections with prose word counts."""
    pattern = re.compile(r"\\(section|subsection)\*?\{([^}]*)\}")
    marks = [(m.start(), m.group(1), m.group(2)) for m in pattern.finditer(raw)]
    out: list[dict[str, object]] = []
    for index, (start, level, title) in enumerate(marks):
        end = marks[index + 1][0] if index + 1 < len(marks) else len(raw)
        prose = strip_latex(raw[start:end])
        sents = sentences(prose)
        long_sents = [s for s in sents if words(s) > SOFT_SENTENCE_WORDS]
        out.append(
            {
                "level": level,
                "title": title,
                "words": words(prose),
                "sentences": len(sents),
                "over_soft": len(long_sents),
                "over_hard": len([s for s in sents if words(s) > HARD_SENTENCE_WORDS]),
            }
        )
    return out


def count_hedges(prose: str) -> dict[str, int]:
    lowered = prose.lower()
    return {h: lowered.count(h) for h in HEDGES if lowered.count(h)}


def measure(raw: str) -> dict[str, object]:
    abstract_raw = extract_abstract(raw)
    abstract_prose = strip_latex(abstract_raw) if abstract_raw else ""
    body = raw
    if abstract_raw:
        body = raw.replace(abstract_raw, " ")
    prose = strip_latex(body)
    sents = sentences(prose)
    hedges = count_hedges(prose)
    total_hedges = sum(hedges.values())
    total_words = words(prose)
    return {
        "abstract_present": abstract_raw is not None,
        "abstract_words": words(abstract_prose),
        "abstract_cites": len(re.findall(r"\\cite\{", abstract_raw or "")),
        "abstract_refs": len(re.findall(r"\\ref\{", abstract_raw or "")),
        "body_words": total_words,
        "sentences": len(sents),
        "over_soft": [s for s in sents if words(s) > SOFT_SENTENCE_WORDS],
        "over_hard": [s for s in sents if words(s) > HARD_SENTENCE_WORDS],
        "hedges": hedges,
        "hedge_total": total_hedges,
        "hedge_per_100w": round(total_hedges / max(total_words, 1) * 100, 3),
        "sections": section_map(body),
    }


def report(m: dict[str, object]) -> None:
    print("=== SAGE paper prose metrics ===")
    if not m["abstract_present"]:
        print("  abstract: ABSENT")
    else:
        print(
            f"  abstract words       {m['abstract_words']}"
            f"   (target {MIN_ABSTRACT_WORDS}-{MAX_ABSTRACT_WORDS})"
        )
        print(f"  abstract cite/ref    {m['abstract_cites']}/{m['abstract_refs']}   (target 0/0)")
    print(f"  body prose words     {m['body_words']}")
    print(f"  sentences            {m['sentences']}")
    print(f"  sentences >{SOFT_SENTENCE_WORDS}w        {len(m['over_soft'])}   (target <={MAX_SOFT_SENTENCES})")
    print(f"  sentences >{HARD_SENTENCE_WORDS}w        {len(m['over_hard'])}   (target 0)")
    print(
        f"  hedge density        {m['hedge_per_100w']}/100w"
        f"   (target <={MAX_HEDGE_PER_100W})  total={m['hedge_total']}"
    )
    over = [s for s in m["sections"] if s["level"] == "subsection" and s["words"] > MAX_SUBSECTION_WORDS]
    print(f"  subsections >{MAX_SUBSECTION_WORDS}w     {len(over)}   (target 0)")
    for s in over:
        print(f"      {s['words']:5d}w  {s['title']}")
    if m["over_hard"]:
        print(f"\n  --- {len(m['over_hard'])} sentence(s) over {HARD_SENTENCE_WORDS} words ---")
        for s in m["over_hard"][:10]:
            print(f"      [{words(s)}w] {s[:110]}")


def gate(m: dict[str, object]) -> list[str]:
    failures: list[str] = []
    if not m["abstract_present"]:
        failures.append("abstract missing")
    else:
        if not MIN_ABSTRACT_WORDS <= m["abstract_words"] <= MAX_ABSTRACT_WORDS:
            failures.append(
                f"abstract {m['abstract_words']}w outside "
                f"[{MIN_ABSTRACT_WORDS},{MAX_ABSTRACT_WORDS}]"
            )
        if m["abstract_cites"] or m["abstract_refs"]:
            failures.append(
                f"abstract contains {m['abstract_cites']} cite / {m['abstract_refs']} ref"
            )
    if m["over_hard"]:
        failures.append(f"{len(m['over_hard'])} sentence(s) over {HARD_SENTENCE_WORDS} words")
    if len(m["over_soft"]) > MAX_SOFT_SENTENCES:
        failures.append(
            f"{len(m['over_soft'])} sentences over {SOFT_SENTENCE_WORDS}w "
            f"(max {MAX_SOFT_SENTENCES})"
        )
    if m["hedge_per_100w"] > MAX_HEDGE_PER_100W:
        failures.append(
            f"hedge density {m['hedge_per_100w']}/100w exceeds {MAX_HEDGE_PER_100W}"
        )
    for s in m["sections"]:
        if s["level"] == "subsection" and s["words"] > MAX_SUBSECTION_WORDS:
            failures.append(f"subsection '{s['title']}' is {s['words']}w > {MAX_SUBSECTION_WORDS}")
    return failures


def self_test() -> int:
    """Confirm each gate actually fires on a synthetic violation."""
    checks: list[tuple[str, str, str]] = [
        (
            "long abstract fails",
            "\\begin{abstract}" + ("word " * 400) + "\\end{abstract}\\section{S}Body text here.",
            "outside",
        ),
        (
            "cite in abstract fails",
            "\\begin{abstract}"
            + ("word " * 170)
            + "\\cite{x}\\end{abstract}\\section{S}Body text here.",
            "cite",
        ),
        (
            "over-long sentence fails",
            "\\begin{abstract}"
            + ("word " * 170)
            + "\\end{abstract}\\section{S}"
            + ("alpha " * 60)
            + ".",
            "over 45 words",
        ),
        (
            "hedge density fails",
            "\\begin{abstract}"
            + ("word " * 170)
            + "\\end{abstract}\\section{S}"
            + ("This does not hold. " * 30),
            "hedge density",
        ),
        (
            "oversized subsection fails",
            "\\begin{abstract}"
            + ("word " * 170)
            + "\\end{abstract}\\subsection{Big}"
            + ("alpha beta gamma delta. " * 300),
            "> 900",
        ),
    ]
    ok = True
    for name, doc, expect in checks:
        failures = gate(measure(doc))
        hit = any(expect.replace(" 900", "900") in f or expect in f for f in failures)
        print(f"  [{'OK' if hit else 'MISS'}] {name}")
        if not hit:
            print(f"        expected a failure mentioning {expect!r}, got {failures}")
            ok = False
    # A clean document must pass every gate.
    clean = (
        "\\begin{abstract}" + ("word " * 170) + "\\end{abstract}\\section{S}Short body sentence."
    )
    clean_failures = gate(measure(clean))
    print(f"  [{'OK' if not clean_failures else 'MISS'}] clean document passes")
    if clean_failures:
        print(f"        unexpected failures: {clean_failures}")
        ok = False
    print(f"SELF-TEST: {'PASS' if ok else 'FAIL'}")
    return 0 if ok else 1


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("paper", nargs="?", default=str(DEFAULT_PAPER))
    ap.add_argument("--gate", action="store_true", help="fail closed on target violations")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return self_test()

    path = Path(args.paper)
    if not path.is_file():
        print(f"PAPER METRICS FAIL: missing {path}", file=sys.stderr)
        return 2
    raw = path.read_text(encoding="utf-8")
    raw = expand_local_inputs(raw, path.parent, {path.resolve()})
    m = measure(raw)

    if args.json:
        printable = {k: v for k, v in m.items() if k not in {"over_soft", "over_hard"}}
        printable["over_soft"] = len(m["over_soft"])
        printable["over_hard"] = len(m["over_hard"])
        print(json.dumps(printable, indent=2))
    else:
        report(m)

    if args.gate:
        failures = gate(m)
        if failures:
            print("\nPAPER METRICS GATE: FAIL")
            for f in failures:
                print(f"  !! {f}")
            return 1
        print("\nPAPER METRICS GATE: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
