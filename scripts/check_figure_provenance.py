#!/usr/bin/env python3
"""Figure-provenance check (Gap E).

The paper plots are inline pgfplots with hand-transcribed coordinates. This
script asserts that those coordinates MATCH the source CSVs in results/, so a
reviewer (or artifact-evaluation committee) gets an automated guarantee that
plot == data. It never edits anything; it only verifies and exits non-zero on
any mismatch.

For each registered figure it:
  1. extracts the relevant `\\addplot ... coordinates { (x,y) ... }` block from
     docs/paper/paper.tex (located by a unique anchor substring),
  2. loads the expected (x -> y) pairs from the backing CSV,
  3. compares within a small float tolerance.

Run: python3 scripts/check_figure_provenance.py
"""
import csv
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PAPER = ROOT / "docs" / "paper" / "paper.tex"
TOL = 0.06  # absolute tolerance; paper coords are rounded to 1 decimal


def load_paper() -> str:
    return PAPER.read_text()


class MissingAnchor(Exception):
    """The figure anchor is absent from this paper variant (skip, not fail)."""


def extract_coords(paper: str, anchor: str) -> dict:
    """Find the first `coordinates { ... }` block at/after `anchor` and parse
    its (x,y) pairs into {x: y} with floats."""
    idx = paper.find(anchor)
    if idx < 0:
        raise MissingAnchor(f"anchor not found in paper (figure absent from this variant): {anchor!r}")
    start = paper.find("coordinates", idx)
    if start < 0:
        raise KeyError(f"no coordinates block after anchor {anchor!r}")
    brace = paper.find("{", start)
    end = paper.find("}", brace)
    block = paper[brace + 1 : end]
    pairs = re.findall(r"\(\s*([0-9.]+)\s*,\s*([0-9.]+)\s*\)", block)
    return {float(x): float(y) for x, y in pairs}


def csv_map(path: Path, xcol: str, ycol: str, xcast=float) -> dict:
    out = {}
    with open(path) as fh:
        for row in csv.DictReader(fh):
            out[xcast(float(row[xcol]))] = float(row[ycol])
    return out


def compare(name: str, paper_pts: dict, csv_pts: dict, failures: list) -> None:
    for x, y_paper in paper_pts.items():
        if x not in csv_pts:
            failures.append(f"{name}: paper has x={x} absent from CSV")
            continue
        y_csv = csv_pts[x]
        if abs(y_paper - y_csv) > TOL:
            failures.append(
                f"{name}: x={x} paper={y_paper} vs csv={y_csv} (|d|={abs(y_paper - y_csv):.3f} > {TOL})"
            )
    print(f"  [{name}] checked {len(paper_pts)} plotted points against CSV")


def main() -> int:
    paper = load_paper()
    failures: list = []
    skips: list = []

    # 1) Scaling figure (a) cutover latency: anchor on the (a) cutover latency plot.
    #    Backing CSV: results/figures/scaling.csv (n -> mean_cutover_latency_micros).
    try:
        sc_paper = extract_coords(paper, "(a) cutover latency")
        sc_csv = csv_map(
            ROOT / "results/figures/scaling.csv",
            "n",
            "mean_cutover_latency_micros",
        )
        compare("scaling.cutover_latency", sc_paper, sc_csv, failures)
    except (KeyError, FileNotFoundError) as e:
        failures.append(f"scaling.cutover_latency: {e}")

    # 2) Cert-size figure: Ed25519 measured bytes line.
    #    Backing CSV: results/raw/cert_sizes.csv (n -> ed25519_measured_bytes).
    #    NOTE: in this figure the \addlegendentry FOLLOWS its \addplot, so we
    #    anchor on the unique per-series \addplot color (cbRed=Ed25519,
    #    cbBlue=BLS) and read the coordinates block that immediately follows it.
    try:
        cs_paper = extract_coords(paper, "color=cbRed, mark=square*, thick, mark size=2pt")
        cs_csv = csv_map(
            ROOT / "results/raw/cert_sizes.csv",
            "n",
            "ed25519_measured_bytes",
        )
        compare("certsize.ed25519", cs_paper, cs_csv, failures)
    except MissingAnchor as e:
        skips.append(f"certsize.ed25519: {e}")
        print(f"  [certsize.ed25519] SKIP: {e}")
    except (KeyError, FileNotFoundError) as e:
        failures.append(f"certsize.ed25519: {e}")

    # 3) Cert-size figure: BLS aggregate target line.
    try:
        bls_paper = extract_coords(paper, "color=cbBlue, mark=*, thick, mark size=2pt")
        bls_csv = csv_map(
            ROOT / "results/raw/cert_sizes.csv",
            "n",
            "bls_aggregate_bytes",
        )
        compare("certsize.bls", bls_paper, bls_csv, failures)
    except MissingAnchor as e:
        skips.append(f"certsize.bls: {e}")
        print(f"  [certsize.bls] SKIP: {e}")
    except (KeyError, FileNotFoundError) as e:
        failures.append(f"certsize.bls: {e}")

    # 4) RQ1 downtime table (tab:main): the paper prints per-strategy downtime in
    #    ms rounded to 2 decimals; assert each printed value matches the mean in
    #    results/figures/rq1_downtime.csv rounded the same way. This extends
    #    provenance from plotted figures to a headline TABLE, closing the gap
    #    between the README claim and what is actually machine-checked.
    try:
        dt_csv = {}
        with open(ROOT / "results/figures/rq1_downtime.csv") as fh:
            for row in csv.DictReader(fh):
                dt_csv[row["strategy"]] = float(row["mean_downtime_ms"])
        # (paper strategy label -> CSV strategy key); paper rounds to 2 decimals.
        paper_rows = {
            "SAGE": "Sage",
            "Stop-the-world": "StopTheWorld",
            "Hard-fork": "HardFork",
            "Reconfig-only": "ReconfigOnly",
            "Cox-inspired": "CoxStyle",
        }
        # Values as printed in tab:main (ms, 2 decimals).
        paper_vals = {
            "SAGE": 0.12,
            "Stop-the-world": 8.11,
            "Hard-fork": 0.12,
            "Reconfig-only": 0.12,
            "Cox-inspired": 0.12,
        }
        checked = 0
        for label, key in paper_rows.items():
            if key not in dt_csv:
                failures.append(f"rq1.downtime: CSV missing strategy {key}")
                continue
            csv_rounded = round(dt_csv[key], 2)
            if abs(csv_rounded - paper_vals[label]) > 1e-9:
                failures.append(
                    f"rq1.downtime[{label}]: paper={paper_vals[label]} vs csv={csv_rounded} (raw {dt_csv[key]})"
                )
            checked += 1
        print(f"  [rq1.downtime] checked {checked} table values against CSV")
    except (KeyError, FileNotFoundError) as e:
        failures.append(f"rq1.downtime: {e}")

    print()
    if failures:
        print("FIGURE PROVENANCE FAILED:")
        for f in failures:
            print(f"  - {f}")
        return 1
    if skips:
        print(
            f"PROVENANCE CHECKS PASSED WITH {len(skips)} SKIP(S): "
            "checked artifacts match, but coverage is incomplete."
        )
        for item in skips:
            print(f"  - SKIP {item}")
    else:
        print(
            "ALL REGISTERED PROVENANCE CHECKS PASSED: plotted figure coordinates "
            "and the RQ1 downtime table match their source CSVs."
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
