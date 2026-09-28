#!/usr/bin/env python3
"""Fail-closed style gate for table/figure captions in the SAGE manuscript.

Two failure modes matter and they pull in opposite directions:

  1. Captions that are paragraphs. An IEEE caption is a noun phrase naming what
     the float shows. Interpretation and methodology belong in body text.
  2. Compression that silently deletes a scope caveat. Several honest caveats
     currently live inside over-long captions; shortening a caption must move
     such a caveat into the body, never drop it from the manuscript.

This gate checks BOTH: caption length/shape, and whole-document survival of the
caveat vocabulary that the captions currently carry. It is deliberately
whole-document for caveats (a caveat relocated into body text still passes) and
per-caption for length.

Usage:
    verify_caption_style.py PAPER.tex           # gate
    verify_caption_style.py PAPER.tex --list    # show every caption + length
    verify_caption_style.py --self-test         # prove the gate has teeth
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# IEEE-ish budget. Captions above HARD are paragraphs, not captions.
SOFT_WORDS = 25
HARD_WORDS = 40
# A small number of data-dense tables legitimately need a column legend.
ALLOWED_OVER_SOFT = 8

# Scope caveats that must survive SOMEWHERE in the manuscript. These are the
# claims that keep the paper honest; caption compression must relocate, not drop.
CAVEATS: list[tuple[str, str, list[str]]] = [
    (
        "C01",
        "serialized simulator cannot realize a concurrent fork",
        [
            r"cannot\s+realize\s+a?\s*concurrent",
            r"serialized[^.]{0,80}cannot\s+(realize|fork)",
            r"one\s+proposer\s+per\s+height",
        ],
    ),
    (
        "C02",
        "exposure is a precondition, not an observed violation",
        [
            r"exposure\s+is\s+a\s+(structural\s+)?precondition",
            r"precondition[^.]{0,60}not\s+an?\s+observed",
            r"structural\s+precondition",
        ],
    ),
    (
        "C03",
        "fixed constructed schedule, not an operational distribution",
        [
            r"constructed\s+schedule[^.]{0,80}not[^.]{0,40}(sample|distribution)",
            r"not\s+samples?\s+of\s+an\s+operational",
            r"conformance\s+(evidence|observations|sweep)[^.]{0,80}(not|rather than)",
        ],
    ),
    (
        "C04",
        "injected latency on same-region hosts, not physical geo-separation",
        [
            r"injected[- ]latency[^.]{0,80}not[^.]{0,40}(physical|geo)",
            r"not\s+claim\s+physical\s+geo-separation",
            r"injected[^.]{0,60}same-region",
        ],
    ),
    (
        "C05",
        "Cox arms are mechanism transplants, not reimplementations",
        [
            r"transplants?,?\s+not\s+(a\s+)?Cox\s+reimplementation",
            r"not\s+a\s+Cox[- ](faithful|threshold)?\s*(reimplementation|implementation)",
            r"Cox-inspired[^.]{0,80}not\s+a\s+Cox",
        ],
    ),
    (
        "C06",
        "gate-closure evidence, not migration liveness",
        [
            r"gate[- ]closure\s+evidence[^.]{0,60}not\s+migration\s+liveness",
            r"not\s+migration\s+liveness",
            r"gate\s+stayed\s+closed",
        ],
    ),
    (
        "C07",
        "safety compared on finalized state roots (not block hashes)",
        [
            r"finalized\s+state[- ]root",
            r"state[- ]root\s+conflict",
        ],
    ),
    (
        "C08",
        "neither panel measures distributed quorum latency",
        [
            r"(neither|not)[^.]{0,60}measures?\s+distributed\s+quorum\s+latency",
            r"not\s+distributed\s+quorum\s+latency",
        ],
    ),
]

# Implementation-layer vocabulary that must never re-enter the paper.
FORBIDDEN = [
    (r"[A-Za-z0-9_./-]+\.(sh|py|tla|rs|toml|csv)\b", "file path / script name"),
    (r"\b(scripts|crates|formal|results)/", "repository directory"),
    (r"\\texttt\{[a-z]+_[a-z_]+\}", "snake_case identifier"),
    (r"\\texttt\{[A-Z][a-z]+[A-Z][A-Za-z]*\}", "CamelCase identifier"),
]

INPUT_RE = re.compile(r"\\(?:input|include)\{([^}]+)\}")


def expand_source(path: Path, root: Path, seen: set[Path] | None = None) -> str:
    """Expand local LaTeX inputs so a split manuscript is audited as one document."""
    seen = set() if seen is None else seen
    path = path.resolve()
    if path in seen:
        return ""
    seen.add(path)
    source = path.read_text(encoding="utf-8")
    parts: list[str] = []
    cursor = 0
    for match in INPUT_RE.finditer(source):
        parts.append(source[cursor : match.start()])
        # TeX resolves inputs from the compilation working directory, not
        # from the including section's directory (sections -> tables).
        child = root / match.group(1)
        if child.suffix == "":
            child = child.with_suffix(".tex")
        try:
            child.resolve().relative_to(root.resolve())
        except ValueError as error:
            raise ValueError(f"Outside manuscript root: {child}") from error
        if not child.is_file():
            raise ValueError(f"Missing manuscript input: {child}")
        parts.append(expand_source(child, root, seen))
        cursor = match.end()
    parts.append(source[cursor:])
    return "".join(parts)


def strip_comments(tex: str) -> str:
    out = []
    for line in tex.splitlines():
        idx, esc = None, False
        for i, ch in enumerate(line):
            if ch == "\\" and not esc:
                esc = True
                continue
            if ch == "%" and not esc:
                idx = i
                break
            esc = False
        out.append(line if idx is None else line[:idx])
    return "\n".join(out)


def normalize(tex: str) -> str:
    body = strip_comments(tex).replace("~", " ")
    body = re.sub(r"\\(emph|textbf|texttt|textit)\{([^{}]*)\}", r"\2", body)
    return re.sub(r"\s+", " ", body)


def extract_captions(tex: str) -> list[tuple[int, str]]:
    """Return (line_number, caption_text) for every \\caption{...}."""
    body = strip_comments(tex)
    out = []
    for m in re.finditer(r"\\caption\{", body):
        depth, i = 1, m.end()
        while i < len(body) and depth:
            if body[i] == "{" and body[i - 1] != "\\":
                depth += 1
            elif body[i] == "}" and body[i - 1] != "\\":
                depth -= 1
            i += 1
        raw = body[m.end() : i - 1]
        line = body[: m.start()].count("\n") + 1
        text = re.sub(r"\\(emph|textbf|texttt|textit|ref|cite)\{[^{}]*\}", " ", raw)
        text = re.sub(r"\$[^$]*\$", " ", text)
        text = re.sub(r"\\[a-zA-Z]+", " ", text)
        text = re.sub(r"[{}~\\]", " ", text)
        out.append((line, re.sub(r"\s+", " ", text).strip()))
    return out


def word_count(text: str) -> int:
    return len([w for w in re.split(r"\s+", text) if re.search(r"[A-Za-z0-9]", w)])


def gate(tex: str) -> list[str]:
    failures: list[str] = []
    caps = extract_captions(tex)
    if not caps:
        return ["no captions found (parser or file problem)"]

    over_hard = [(ln, word_count(t), t) for ln, t in caps if word_count(t) > HARD_WORDS]
    over_soft = [(ln, word_count(t), t) for ln, t in caps if word_count(t) > SOFT_WORDS]
    for ln, n, t in over_hard:
        failures.append(f"caption at line {ln} is {n} words (>{HARD_WORDS}): {t[:60]!r}")
    if len(over_soft) > ALLOWED_OVER_SOFT:
        failures.append(
            f"{len(over_soft)} captions exceed {SOFT_WORDS} words "
            f"(at most {ALLOWED_OVER_SOFT} allowed)"
        )

    body = normalize(tex)
    for cid, desc, pats in CAVEATS:
        if not any(re.search(p, body, re.IGNORECASE) for p in pats):
            failures.append(f"CAVEAT LOST {cid}: {desc}")

    for pat, label in FORBIDDEN:
        for m in re.finditer(pat, strip_comments(tex)):
            if "bibitem" in strip_comments(tex)[max(0, m.start() - 200) : m.start()]:
                continue
            failures.append(f"implementation leak ({label}): {m.group(0)!r}")
    return failures


def _redact_caveat(body: str, cid: str) -> str:
    """Remove EVERY surface form of one caveat from already-normalized text.

    Used only by the self-test. Redacting a single alternative would leave the
    others matching, so the test would pass while proving nothing.
    """
    for this_id, _desc, pats in CAVEATS:
        if this_id != cid:
            continue
        for pat in pats:
            body = re.sub(pat, "REDACTED", body, flags=re.IGNORECASE)
    return body


def self_test() -> int:
    """A gate that cannot fail proves nothing."""
    base = Path(__file__).resolve().parents[1] / "docs/paper/SAGE_ThaiCong_IEEE/main.tex"
    if not base.is_file():
        print(f"SELF-TEST SKIPPED: {base} not found", file=sys.stderr)
        return 2
    tex = expand_source(base, base.parent)
    ok = True

    long_cap = "\\caption{" + " word" * 60 + "}"
    cases = [
        ("paragraph-length caption is rejected", tex + "\n" + long_cap, "is 6"),
        (
            "a dropped caveat is detected",
            # Redact on NORMALIZED text. gate() normalizes before searching
            # (collapsing whitespace and unwrapping \emph{}), so redacting the
            # raw source leaves line-break-spanning and markup-wrapped phrasings
            # intact -- the case then passes vacuously and proves nothing.
            _redact_caveat(normalize(tex), "C01"),
            "CAVEAT LOST C01",
        ),
        (
            "a re-introduced script path is detected",
            tex + "\n\\caption{See scripts/run_it.sh here.}",
            "implementation leak",
        ),
    ]
    for label, mutated, expect in cases:
        hits = gate(mutated)
        if any(expect in h for h in hits):
            print(f"  [OK] {label}")
        else:
            print(f"  [MISS] {label} -- expected {expect!r}")
            ok = False

    print()
    print("SELF-TEST: PASS" if ok else "SELF-TEST: FAIL")
    return 0 if ok else 1


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("paper", nargs="?")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return self_test()
    if not args.paper:
        print("FAIL: provide a paper path or --self-test", file=sys.stderr)
        return 2
    path = Path(args.paper)
    if not path.is_file():
        print(f"FAIL: missing {path}", file=sys.stderr)
        return 2
    tex = expand_source(path, path.parent)

    if args.list:
        caps = sorted(
            ((ln, word_count(t), t) for ln, t in extract_captions(tex)),
            key=lambda r: -r[1],
        )
        for ln, n, t in caps:
            flag = "TOO-LONG" if n > HARD_WORDS else ("long" if n > SOFT_WORDS else "ok")
            print(f"  {n:4d}w  L{ln:5d}  [{flag}] {t[:88]}")
        print(f"\n  total {len(caps)}  over{SOFT_WORDS}={sum(1 for _, n, _ in caps if n > SOFT_WORDS)}"
              f"  over{HARD_WORDS}={sum(1 for _, n, _ in caps if n > HARD_WORDS)}")
        return 0

    failures = gate(tex)
    if failures:
        print("CAPTION GATE: FAIL")
        for f in failures:
            print(f"  !! {f}")
        return 1
    print("CAPTION GATE: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
