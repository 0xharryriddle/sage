#!/usr/bin/env python3
"""Fail-closed check that the appendix bodies follow the reference list.

IEEEtran uses `\\appendices` and elsarticle uses `\\appendix`; either appears
where declared in the source, so moving the
bibliography is a silent, easy-to-regress edit. Checking the SOURCE order is not
enough either: what matters is the rendered document.

Detecting this from rendered text is subtle. The body cites appendices inline
("see Appendix A"), so searching for the word "Appendix" finds body pages long
before the appendix itself and yields a false "wrong order" verdict. This gate
therefore locates the appendix by its SECTION CONTENT (the distinctive title of
each appendix section) and the bibliography by its ENTRY LIST ("[1] Author"),
never by a bare heading word.

Usage:
    verify_document_order.py PAPER.pdf
    verify_document_order.py --self-test
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tempfile
from pathlib import Path

# Distinctive appendix section titles. Matched on squashed text so IEEEtran's
# letter-spaced small-caps headings ("D EFERRED P ROOFS") still match.
APPENDIX_TITLES = [
    "DEFERREDPROOFS",
    "FORMALSPECIFICATIONOFTHEMIGRATIONCONTROLLER",
    "FORMALSPECIFICATIONANDBOUNDEDCHECKS",
    "WORKEDMIGRATIONEXAMPLE",
    "FULLNUMERICALRESULTS",
]

# A bibliography entry list looks like "[1] A. Author, ...". Two consecutive
# numbered entries on one page is a reliable signal, and cannot be confused with
# an inline citation such as "[12]" in running prose.
BIB_ENTRY = re.compile(r"\[\s*(\d{1,2})\s*\]\s+[A-Z]")


def squash(text: str) -> str:
    return re.sub(r"[^A-Za-z]", "", text).upper()


def page_texts(pdf: Path) -> list[str]:
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "p.txt"
        subprocess.run(
            ["pdftotext", "-layout", str(pdf), str(out)],
            check=True,
            capture_output=True,
        )
        raw = out.read_text(encoding="utf-8", errors="replace")
    pages = raw.split("\f")
    # pdftotext appends a trailing empty element after the final form feed.
    while pages and not pages[-1].strip():
        pages.pop()
    return pages


def find_bibliography_page(pages: list[str]) -> int | None:
    for idx, text in enumerate(pages, start=1):
        entries = {int(m.group(1)) for m in BIB_ENTRY.finditer(text)}
        if len({e for e in entries if e <= 5}) >= 2:
            return idx
    return None


def find_appendix_pages(pages: list[str]) -> list[int]:
    hits = []
    for idx, text in enumerate(pages, start=1):
        squashed = squash(text)
        if any(title in squashed for title in APPENDIX_TITLES):
            hits.append(idx)
    return hits


def gate(pdf: Path) -> list[str]:
    failures: list[str] = []
    pages = page_texts(pdf)
    if not pages:
        return ["no pages extracted from the PDF"]

    bib = find_bibliography_page(pages)
    app = find_appendix_pages(pages)

    if bib is None:
        failures.append("could not locate the bibliography entry list")
    if not app:
        failures.append("could not locate any appendix section body")
    if bib is not None and app:
        # In a two-column IEEE layout the reference list and the first appendix
        # legitimately share a page: the bibliography ends mid-column and the
        # appendix continues in the next column. Only a STRICT precedence is a
        # real ordering defect, so same-page overlap is allowed here and the
        # authoritative ordering check is the source-order test below.
        if min(app) < bib:
            failures.append(
                f"appendix body starts on page {min(app)} but the reference list "
                f"starts on page {bib}: the appendix must FOLLOW the references"
            )
    return failures


def gate_source(tex: Path) -> list[str]:
    """Check an IEEE/Elsevier appendix declaration follows the bibliography.

    Page geometry cannot decide this on its own because two-column flow lets the
    reference list and the first appendix share a page.
    """
    body = tex.read_text(encoding="utf-8")
    # Ignore actual TeX comments, but retain escaped percent signs. An even
    # number of backslashes leaves the percent unescaped.
    body = re.sub(r"(?<!\\)((?:\\\\)*)%[^\n]*", r"\1", body)
    bib = re.search(r"\\end\s*\{thebibliography\}", body)
    if bib is None:
        # BibTeX manuscripts place the bibliography with a command rather than
        # an inline thebibliography environment. Its source position is the
        # relevant ordering boundary; the rendered-page probe above proves that
        # the entry list exists.
        bib = re.search(r"\\bibliography\s*\{", body)
    app = re.search(r"\\(?:appendices|appendix)(?![A-Za-z@])", body)
    if app is None:
        return ["no \\appendices or \\appendix declaration found in the source"]
    if bib is None:
        return ["no inline or BibTeX bibliography found in the source"]
    if app.start() < bib.start():
        return [
            "appendix declaration appears BEFORE the bibliography in the source: "
            "move the appendix block after the bibliography"
        ]
    return []


def report(pdf: Path) -> int:
    pages = page_texts(pdf)
    bib = find_bibliography_page(pages)
    app = find_appendix_pages(pages)
    print(f"  pages                 : {len(pages)}")
    print(f"  reference list starts : page {bib}")
    print(f"  appendix body pages   : {app}")
    failures = gate(pdf)
    # The source-order test is authoritative; page geometry alone cannot decide
    # ordering because two-column flow lets the bibliography and the first
    # appendix share a page.
    tex = pdf.with_suffix(".tex")
    if tex.is_file():
        src = gate_source(tex)
        print(f"  source order          : {'OK' if not src else 'WRONG'}")
        failures.extend(src)
    if failures:
        print("\nDOCUMENT ORDER GATE: FAIL")
        for f in failures:
            print(f"  !! {f}")
        return 1
    print("\nDOCUMENT ORDER GATE: PASS -- appendix follows the references")
    return 0


def self_test() -> int:
    """A gate that cannot fail proves nothing."""
    ok = True

    # Case 1: an inline "see Appendix A" in body prose must NOT be mistaken for
    # the appendix itself. This is the exact false positive that made three
    # earlier hand-written checks report the wrong verdict.
    body_citing = "Consensus proceeds; see Appendix A for the deferred proof."
    if squash(body_citing) and any(t in squash(body_citing) for t in APPENDIX_TITLES):
        print("  [MISS] an inline 'Appendix A' citation was treated as the appendix")
        ok = False
    else:
        print("  [OK] inline 'see Appendix A' is not mistaken for the appendix")

    # Case 2: an inline citation "[12]" must not be read as a reference list.
    prose = "as shown previously [12] and confirmed in [13] under partition."
    entries = {int(m.group(1)) for m in BIB_ENTRY.finditer(prose)}
    if len({e for e in entries if e <= 5}) >= 2:
        print("  [MISS] inline citations were treated as a bibliography page")
        ok = False
    else:
        print("  [OK] inline citations are not mistaken for the reference list")

    # Case 3: a real entry list IS detected.
    biblist = "[1] M. Yin, HotStuff, 2019.\n[2] G. Danezis, Narwhal, 2022."
    entries = {int(m.group(1)) for m in BIB_ENTRY.finditer(biblist)}
    if len({e for e in entries if e <= 5}) >= 2:
        print("  [OK] a real reference list is detected")
    else:
        print("  [MISS] a real reference list was not detected")
        ok = False

    # Case 4: a real appendix heading IS detected through letter-spacing.
    spaced = "A PPENDIX A\nD EFERRED P ROOFS"
    if "DEFERREDPROOFS" in squash(spaced):
        print("  [OK] a letter-spaced appendix heading is detected")
    else:
        print("  [MISS] letter-spaced appendix heading was not detected")
        ok = False

    # Exercise both document classes and deliberately invalid source orders.
    source_cases = [
        ("Elsevier BibTeX", "\\bibliography{refs}\n\\appendix\n", True),
        ("IEEE BibTeX", "\\bibliography{refs}\n\\appendices\n", True),
        ("inline bibliography", "\\end{thebibliography}\n\\appendix\n", True),
        ("reversed Elsevier", "\\appendix\n\\bibliography{refs}\n", False),
        ("reversed IEEE", "\\appendices\n\\bibliography{refs}\n", False),
        ("comment-only appendix", "\\bibliography{refs}\n% \\appendix\n", False),
        ("comment-only bibliography", "% \\bibliography{refs}\n\\appendix\n", False),
        ("command prefix", "\\bibliography{refs}\n\\appendixname\n", False),
        ("escaped percent", "\\% \\bibliography{refs}\n\\appendix\n", True),
    ]
    with tempfile.TemporaryDirectory(prefix="sage-document-order-") as tmp:
        source = Path(tmp) / "probe.tex"
        for name, body, expected in source_cases:
            source.write_text(body, encoding="utf-8")
            actual = not gate_source(source)
            match = actual == expected
            ok = ok and match
            print(f"  [{'OK' if match else 'MISS'}] {name}: {'PASS' if actual else 'FAIL'} as expected={expected}")

    print()
    print("SELF-TEST: PASS" if ok else "SELF-TEST: FAIL")
    return 0 if ok else 1


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("pdf", nargs="?")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return self_test()
    if not args.pdf:
        print("FAIL: provide a PDF path or --self-test", file=sys.stderr)
        return 2
    path = Path(args.pdf)
    if not path.is_file():
        print(f"FAIL: missing {path}", file=sys.stderr)
        return 2
    return report(path)


if __name__ == "__main__":
    raise SystemExit(main())
