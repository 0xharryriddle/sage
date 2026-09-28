#!/usr/bin/env python3
r"""Report per-section prose word counts for the manuscript.

Why this exists as a checked-in tool rather than an ad-hoc snippet: a naive
"count from this heading to the next heading" measurement silently attributes
the bibliography to whichever section precedes it. That produced a 1278-word
"Conclusion" (actual: 226) and a wrong section ranking, which in turn would
have sent an editing pass at the wrong target.

Correct boundaries:
  * a section ends at the next \section OR \subsection, whichever comes first;
  * everything from \begin{thebibliography} onward is EXCLUDED from prose;
  * float bodies (table/figure/algorithm) are excluded -- captions are measured
    separately by verify_caption_style.py;
  * \appendices resets nothing, but appendix sections are labelled as such.

Usage:
    paper_section_sizes.py [PAPER.tex] [--top N] [--json]
    paper_section_sizes.py --self-test
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

DEFAULT_PAPER = Path(__file__).resolve().parents[1] / "docs/paper/SAGE_ThaiCong_IEEE/main.tex"

FLOAT_ENVS = ("table*", "table", "figure*", "figure", "algorithm")


def expand_local_inputs(tex: str, base_dir: Path, _seen: set[Path] | None = None) -> str:
    r"""Expand local ``\input{...}`` files for source-level metrics."""
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

    return pattern.sub(replace, tex)


def strip_comments(tex: str) -> str:
    out = []
    for line in tex.splitlines():
        idx = 0
        while True:
            idx = line.find("%", idx)
            if idx == -1:
                break
            if idx > 0 and line[idx - 1] == "\\":
                idx += 1
                continue
            line = line[:idx]
            break
        out.append(line)
    return "\n".join(out)


def cut_bibliography(tex: str) -> str:
    """Everything from the bibliography onward is not section prose."""
    marker = tex.find(r"\begin{thebibliography}")
    return tex if marker == -1 else tex[:marker]


def cut_floats(tex: str) -> str:
    for env in FLOAT_ENVS:
        tex = re.sub(
            r"\\begin\{" + re.escape(env) + r"\}.*?\\end\{" + re.escape(env) + r"\}",
            " ",
            tex,
            flags=re.DOTALL,
        )
    return tex


def prose_words(chunk: str) -> int:
    body = cut_floats(chunk)
    body = re.sub(r"\\(label|ref|cite|eqref|autoref)\{[^{}]*\}", " ", body)
    body = re.sub(r"\$[^$]*\$", " ", body)
    body = re.sub(r"\\[A-Za-z@]+\*?", " ", body)
    body = re.sub(r"[{}\\&~^_]", " ", body)
    return len(re.findall(r"[A-Za-z][A-Za-z'\-]+", body))


def sections(tex: str) -> list[tuple[str, str, int, int]]:
    """Return (kind, title, start_line, word_count) with CORRECT boundaries."""
    body = cut_bibliography(strip_comments(tex))
    lines = body.splitlines()
    heads: list[tuple[int, str, str]] = []
    in_appendix = False
    for i, line in enumerate(lines):
        if r"\appendices" in line:
            in_appendix = True
        m = re.match(r"\s*\\(section|subsection)\{(.+?)\}", line)
        if m:
            kind = m.group(1)
            if in_appendix:
                kind = "appendix-" + kind
            heads.append((i, kind, m.group(2)))

    out = []
    for idx, (start, kind, title) in enumerate(heads):
        end = heads[idx + 1][0] if idx + 1 < len(heads) else len(lines)
        chunk = "\n".join(lines[start + 1 : end])
        out.append((kind, title, start + 1, prose_words(chunk)))
    return out


def self_test() -> int:
    """A measurement tool that cannot be wrong proves nothing."""
    ok = True

    # The defect this tool exists to prevent: bibliography attributed to the
    # last section before it.
    doc = (
        "\\section{Conclusion}\nAlpha beta gamma delta.\n"
        "\\begin{thebibliography}{99}\n"
        + "\n".join(f"\\bibitem{{k{i}}} Some Author, some long paper title here." for i in range(40))
        + "\n\\end{thebibliography}\n"
    )
    res = {t: w for _k, t, _l, w in sections(doc)}
    if res.get("Conclusion", 0) > 10:
        print(f"  [MISS] bibliography leaked into Conclusion ({res.get('Conclusion')} words)")
        ok = False
    else:
        print("  [OK] bibliography is excluded from the preceding section")

    # A subsection must terminate the enclosing section's count.
    doc2 = (
        "\\section{Outer}\none two three four five\n"
        "\\subsection{Inner}\nsix seven eight nine ten eleven twelve\n"
    )
    res2 = {t: w for _k, t, _l, w in sections(doc2)}
    if res2.get("Outer") != 5:
        print(f"  [MISS] section did not stop at the subsection (got {res2.get('Outer')})")
        ok = False
    else:
        print("  [OK] a section stops counting at the next subsection")

    # Float bodies must not inflate prose.
    doc3 = (
        "\\section{WithFloat}\nreal prose here\n"
        "\\begin{table}[t]\n" + " ".join(["filler"] * 200) + "\n\\end{table}\n"
    )
    res3 = {t: w for _k, t, _l, w in sections(doc3)}
    if res3.get("WithFloat", 0) > 20:
        print(f"  [MISS] float body counted as prose ({res3.get('WithFloat')} words)")
        ok = False
    else:
        print("  [OK] float bodies are excluded from prose")

    print()
    print("SELF-TEST: PASS" if ok else "SELF-TEST: FAIL")
    return 0 if ok else 1


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("paper", nargs="?", default=str(DEFAULT_PAPER))
    ap.add_argument("--top", type=int, default=0, help="show only the N largest")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return self_test()

    path = Path(args.paper)
    if not path.is_file():
        print(f"FAIL: missing {path}", file=sys.stderr)
        return 2
    raw = path.read_text(encoding="utf-8")
    raw = expand_local_inputs(raw, path.parent, {path.resolve()})
    rows = sections(raw)
    total = sum(r[3] for r in rows)

    if args.json:
        print(json.dumps({"total": total, "sections": rows}, indent=2))
        return 0

    print(f"total prose words (bibliography and floats excluded): {total}")
    print()
    ordered = sorted(rows, key=lambda r: -r[3])
    if args.top:
        ordered = ordered[: args.top]
    for kind, title, line, words in ordered:
        print(f"  {words:5d}  L{line:<5d} {kind:<20s} {title}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
