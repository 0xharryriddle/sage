#!/usr/bin/env python3
"""Fail-closed check that a Limitations rewrite loses no disclosed limitation.

Compressing a Limitations section is legitimate editing; silently DROPPING a
disclosed limitation is not. Prose similarity cannot tell the two apart, so this
gate pins each mandatory disclosure to a set of alternative surface forms and
requires at least one to survive somewhere in the manuscript.

The check is deliberately whole-document, not section-local: a limitation moved
from Limitations into Methodology is still disclosed, while a limitation deleted
from the paper is not.

Usage:
    verify_limitation_disclosures.py PAPER.tex            # gate the paper
    verify_limitation_disclosures.py --self-test          # prove it has teeth
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


INPUT_RE = re.compile(r"\\(?:input|include)\{([^}]+)\}")


def expand_source(path: Path, root: Path, seen: set[Path] | None = None) -> str:
    """Expand manuscript-local inputs before checking disclosure coverage."""
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

# Each disclosure: (id, human description, [regex alternatives]).
# A disclosure PASSES if ANY alternative matches. Alternatives exist because
# honest rewording is allowed; what is not allowed is the fact disappearing.
DISCLOSURES: list[tuple[str, str, list[str]]] = [
    (
        "D01",
        "physical geo-WAN / multi-region behaviour NOT established",
        [
            r"not\s+a\s+physical\s+multi-region",
            r"physical\s+geo-WAN[^.]{0,80}(not|remains|outside|future)",
            r"geo-WAN[^.]{0,60}(not established|remains future|outside)",
            r"physical\s+cross-region[^.]{0,60}(future|outside|not)",
            r"no[^.]{0,40}geo-distributed",
            r"geo-distributed[^.]{0,60}(remain|open|future|not)",
        ],
    ),
    (
        "D02",
        "configured endpoints do NOT attest substrate identity or placement",
        [
            r"configured[- ]endpoint[^.]{0,100}(do not|does not|unattested|not attest)[^.]{0,100}(identity|placement|isolation)",
            r"(machine|process)\s+identity[^.]{0,100}(unattested|not authenticated|do not authenticate)",
            r"do not authenticate[^.]{0,120}(machine|process|placement|physical isolation)",
            r"placement[^.]{0,100}(unattested|not attest|do not authenticate)",
        ],
    ),
    (
        "D03",
        "committee-wide fence deployment NOT implemented",
        [
            r"fence\s+deployment[^.]{0,80}(not implemented|unimplemented|remains|future|absent)",
            r"(committee-wide|live-host)\s+fence[^.]{0,80}(not|future|absent|unimplemented)",
            r"fence[^.]{0,40}(specified but|not deployed|deployment future)",
        ],
    ),
    (
        "D04",
        "terminal-certificate dissemination NOT implemented",
        [
            r"terminal[- ](certificate|share)\s+(dissemination|gossip)[^.]{0,80}"
            r"(not|absent|unimplemented|remains|future)",
            r"terminal\s+round[^.]{0,60}(not|absent|unmeasured|future)",
            r"dissemination[^.]{0,60}(not implemented|absent|remains future)",
        ],
    ),
    (
        "D05",
        "manifest gossip absent (local accept-rule only)",
        [
            r"manifest\s+gossip[^.]{0,60}(absent|not|unimplemented|future)",
            r"(not|does not)[^.]{0,40}gossip[^.]{0,40}manifest",
            r"manifest[^.]{0,60}dissemination[^.]{0,60}(not|absent|future)",
        ],
    ),
    (
        "D06",
        "partition result is GATE-CLOSURE evidence, not migration liveness",
        [
            r"gate-closure\s+evidence[^.]{0,60}not[^.]{0,40}liveness",
            r"not\s+migration\s+liveness",
            r"safety-preserving\s+stall",
            r"gate\s+stayed\s+closed[^.]{0,80}not[^.]{0,40}liveness",
        ],
    ),
    (
        "D07",
        "one OPEN pacemaker fault remains",
        [
            r"pacemaker\s+fault",
            r"future-view\s+proposal\s+requires\s+a\s+justify\s+QC",
            r"justify\s+QC[^.]{0,60}(fault|open|pre-existing)",
        ],
    ),
    (
        "D08",
        "cited comparators were NOT executed",
        [
            r"comparators?[^.]{0,60}(not executed|unexecuted)",
            r"not\s+executed\s+(in|as part of)\s+this\s+(work|study|paper)",
            r"unexecuted\s+(qualitative\s+)?comparator",
            r"no\s+cited\s+comparator",
            r"none\s+of\s+these\s+systems\s+was\s+executed",
        ],
    ),
    (
        "D09",
        "no competitive external-client benchmark",
        [
            r"external-client\s+benchmark[^.]{0,40}(not|absent|was executed|remains)",
            r"no\s+(competitive\s+)?external-client",
            r"competitive[^.]{0,40}benchmark[^.]{0,40}(not|absent|remains|open)",
            r"client-observed[^.]{0,60}(remains|future|not)",
        ],
    ),
    (
        "D10",
        "serialized simulator CANNOT realize a concurrent two-leader fork",
        [
            r"cannot\s+realize\s+a?\s*concurrent",
            r"serialized[^.]{0,80}cannot[^.]{0,40}fork",
            r"cannot\s+fork\s+by\s+construction",
            r"property\s+of\s+the\s+model",
        ],
    ),
    (
        "D11",
        "theorems scoped to ONE IDENTICAL committee",
        [
            r"identical\s+committee",
            r"V_M\s*=\s*V_1\s*=\s*V_2",
            r"one\s+fixed\s+identical\s+committee",
        ],
    ),
    (
        "D12",
        "monolithic-state scope; sharded migration out of scope",
        [
            r"monolithic[- ]state",
            r"shard(ed|ing)[^.]{0,60}(out of scope|not|future|excluded)",
        ],
    ),
    (
        "D13",
        "no full refinement proof (bounded conformance only)",
        [
            r"refinement\s+proof[^.]{0,60}(remains|not|future|absent)",
            r"no\s+(full\s+)?refinement\s+proof",
            r"bounded[^.]{0,60}conformance[^.]{0,80}(not|rather than)\s+.{0,30}refinement",
        ],
    ),
    (
        "D14",
        "small endpoint trial counts / fixed constructed schedules",
        [
            r"small\s+(real-host|endpoint)\s+trial",
            r"fixed[- ]schedule\s+conformance",
            r"constructed\s+schedule[^.]{0,80}(not|rather than)\s+.{0,40}"
            r"(distribution|Bernoulli|operational)",
            r"not\s+samples?\s+of\s+an\s+operational",
            r"trial\s+counts[^.]{0,40}(small|limited)",
        ],
    ),
    (
        "D15",
        "overhead rate is inferred, not observed client TPS",
        [
            r"inferred\s+validator\s+commit\s+rate[^.]{0,100}(not|rather than)[^.]{0,60}(client|throughput|TPS)",
            r"height[^.]{0,50}configured[- ]load\s+proxy[^.]{0,80}(not|rather than)",
            r"numerator\s+is\s+inferred[^.]{0,80}(not|rather than)[^.]{0,50}client",
        ],
    ),
    (
        "D16",
        "aggregate-only campaigns lack a retained per-trial ledger",
        [
            r"aggregate-only[^.]{0,120}(per-trial|trial ledger|independently)",
            r"per-trial\s+(records|ledger)[^.]{0,100}(not retained|were not retained|missing)",
            r"no\s+reconstructable\s+(attempted|trial|conclusive)[^.]{0,50}denominator",
        ],
    ),
    (
        "D17",
        "no comprehensive passing seal for the current manuscript generation",
        [
            r"no\s+single\s+comprehensive[^.]{0,100}(seal|gate|binding)",
            r"current[- ]generation[^.]{0,100}(not sealed|seal is unavailable|not bound)",
            r"generation-specific[^.]{0,120}(not comprehensive|separately|distinct)",
        ],
    ),
    (
        "D18",
        "client finality tier is attested but carries no recovery semantics",
        [
            # The tier IS now on the wire and inside the signed payload, so the
            # old "not exposed" disclosure would be a FALSE statement about the
            # code. The residual, still-true limitation is that knowing a receipt
            # is revocable does not tell an integrator how to compensate one.
            r"tier says whether a receipt is revocable, not what to undo",
            r"revocable[^.]{0,120}without supplying the compensation logic",
            r"receipt reconciliation[^.]{0,120}remain outside the live path",
        ],
    ),
]


def strip_comments(tex: str) -> str:
    out = []
    for line in tex.splitlines():
        idx = None
        for i, ch in enumerate(line):
            if ch == "%" and (i == 0 or line[i - 1] != "\\"):
                idx = i
                break
        out.append(line if idx is None else line[:idx])
    return "\n".join(out)


def normalize(tex: str) -> str:
    """Flatten to a single searchable string: drop comments, collapse space."""
    body = strip_comments(tex)
    body = body.replace("~", " ")
    body = re.sub(r"\\(emph|textbf|texttt|textit)\{([^{}]*)\}", r"\2", body)
    body = re.sub(r"\s+", " ", body)
    return body


def audit_normalized(body: str) -> list[tuple[str, str, bool, str]]:
    """Audit text that is ALREADY normalized (used by the self-test)."""
    results = []
    for did, desc, patterns in DISCLOSURES:
        hit = ""
        for pat in patterns:
            m = re.search(pat, body, re.IGNORECASE)
            if m:
                hit = m.group(0)[:70]
                break
        results.append((did, desc, bool(hit), hit))
    return results


def audit(tex: str) -> list[tuple[str, str, bool, str]]:
    body = normalize(tex)
    results = []
    for did, desc, patterns in DISCLOSURES:
        hit = ""
        for pat in patterns:
            m = re.search(pat, body, re.IGNORECASE)
            if m:
                hit = m.group(0)[:70]
                break
        results.append((did, desc, bool(hit), hit))
    return results


def report(results: list[tuple[str, str, bool, str]]) -> int:
    missing = [r for r in results if not r[2]]
    for did, desc, ok, hit in results:
        mark = "OK  " if ok else "LOST"
        print(f"  [{mark}] {did} {desc}")
        if ok:
            print(f"         matched: {hit!r}")
    print()
    if missing:
        print(f"DISCLOSURE GATE: FAIL -- {len(missing)} disclosure(s) lost")
        for did, desc, _, _ in missing:
            print(f"  !! {did} {desc}")
        return 1
    print(f"DISCLOSURE GATE: PASS -- all {len(results)} disclosures present")
    return 0


def self_test() -> int:
    """A gate that cannot fail proves nothing. Each case must be caught."""
    base = Path(__file__).resolve().parents[1] / "docs/paper/SAGE_ThaiCong_IEEE/main.tex"
    if not base.is_file():
        print(f"SELF-TEST SKIPPED: {base} not found", file=sys.stderr)
        return 2
    tex = expand_source(base, base.parent)
    body = normalize(tex)

    ok = True
    full = audit_normalized(body)
    if any(not r[2] for r in full):
        print("  [MISS] baseline paper should pass all disclosures")
        for did, desc, hit, _ in full:
            if not hit:
                print(f"         missing {did} {desc}")
        ok = False
    else:
        print("  [OK] baseline paper passes")

    # Negative controls: excise each disclosure's evidence and confirm detection.
    for did, desc, patterns in DISCLOSURES:
        mutated = body
        for pat in patterns:
            mutated = re.sub(pat, "REDACTED", mutated, flags=re.IGNORECASE)
        res = {r[0]: r[2] for r in audit_normalized(mutated)}
        if res.get(did, True):
            print(f"  [MISS] excising {did} was NOT detected ({desc})")
            ok = False
        else:
            print(f"  [OK] excising {did} is detected")

    print()
    print("SELF-TEST: PASS" if ok else "SELF-TEST: FAIL")
    return 0 if ok else 1


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("paper", nargs="?")
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
    return report(audit(expand_source(path, path.resolve().parent)))


if __name__ == "__main__":
    raise SystemExit(main())
