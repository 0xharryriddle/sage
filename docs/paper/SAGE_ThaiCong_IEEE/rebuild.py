#!/usr/bin/env python3
"""Rebuild the paper and six vector figures from retained, explicitly scoped inputs.

This regenerates presentation data, not historical trials or full-protocol evidence.
Only --publish changes canonical figure/PDF outputs, after a successful staged build.
"""
from __future__ import annotations

import argparse
import collections
import csv
from decimal import Decimal, ROUND_HALF_UP
import hashlib
import io
import json
import math
import os
from pathlib import Path
import random
import shutil
import statistics
import subprocess
import sys
import tempfile
import time

PAPER = Path(__file__).resolve().parent
ROOT = PAPER.parents[2]
EPOCH = "1789948800"
INPUTS = {
    "rq1": ("results/raw/paper/SAGE_ThaiCong_IEEE/rq1_migration_cost.csv", "e7e8397df3016e6c5ea816e16d267c32c3eed2c749ccc5ece4f4190ab017777b"),
    "scaling": ("results/raw/paper/SAGE_ThaiCong_IEEE/scaling.csv", "c913352d62e5c58ed16d665f328e0eddd2b512dfaf0f8c7bb8ef53253f331069"),
    "delay": ("results/raw/paper/SAGE_ThaiCong_IEEE/sensitivity.csv", "683d48313566bd5581ec27fac3dedacbc5b77fd348f91157829fd54fff8b3ca5"),
    "threshold": ("results/raw/m10_gate_isolation.csv", "a3b9ba73e754b92e3e8024c28f14f33469c5e1be824d717ea89d51ab61ec21d8"),
    "authorization": ("results/raw/m14b_native_gate_authorization.csv", "72d093d1c40731db7ae6cba3956d7f01620b9607eec7dbf5f2a9d04321081b8c"),
    "overhead": ("results/raw/dual_run_overhead.csv", "536cb71ceac06c10a1e98924b1416422b0cabe5cc2d0236e518855901bd5b3d3"),
    "wan": ("results/raw/multihost_wan_tps_3arm.csv", "1736533b4a0191887f5657aa49ed997128ea29dc6f20e7c5c7784a6e324c0530"),
    "certificate": ("results/raw/local_refresh/scientific_refresh_20260714T170701Z/attempts/remaining_release/cert_timing.csv", "b33b8f1ed49f9bb1bc13e53c64b853e780eea65858952b49cf6b27f38cf644e6"),
}
FIGURES = ("fig_arch", "fig_lts", "fig_quorum", "fig_results", "fig_seq", "fig_threat")
INPUT_NOTES = {
    "threshold": ("results/raw/m10_gate_isolation.README.md", "94f402174a66acada3c353316b255002cf83af2d80597e474af1177ddae07418"),
    "certificate": ("results/raw/cert_timing.README.md", "653b1a66e00b6aec6b62a39dce2ea17a1774c00cbc69bed54affeff6b2fd2344"),
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_inputs(root=ROOT):
    data = {}
    for key, (relative, expected) in INPUTS.items():
        path = root / relative
        require(path.is_file(), f"Missing retained input: {relative}")
        require(digest(path) == expected, f"Retained input drift: {relative}; do not silently repin")
        with path.open(newline="", encoding="utf-8") as handle:
            data[key] = list(csv.DictReader(handle))
    for relative, expected in INPUT_NOTES.values():
        path = root / relative
        require(path.is_file() and digest(path) == expected, f"Retained note drift: {relative}")
    return data


def wilson(k, n):
    require(isinstance(k, int) and isinstance(n, int) and n > 0 and 0 <= k <= n, "Invalid k/n")
    z = statistics.NormalDist().inv_cdf(0.975)
    p = k / n
    denominator = 1 + z * z / n
    centre = (p + z * z / (2 * n)) / denominator
    radius = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / denominator
    return max(0, centre - radius), min(1, centre + radius)


def seed_grid(rows, keys, seeds):
    expected = {(key, seed) for key in keys for seed in seeds}
    observed = [(row[0], int(row[1])) for row in rows]
    require(len(observed) == len(set(observed)), "Duplicate trial identity")
    require(set(observed) == expected, "Missing or unexpected trial identity")


def csv_text(fields, rows):
    stream = io.StringIO(newline="")
    writer = csv.writer(stream, lineterminator="\n")
    writer.writerow(fields)
    writer.writerows(rows)
    return stream.getvalue()


def bootstrap_mean(values, seed=20260922, draws=10000):
    rng = random.Random(seed)
    means = sorted(statistics.fmean(rng.choices(values, k=len(values))) for _ in range(draws))
    # Inverse empirical CDF: a deterministic percentile-bootstrap summary of retained seeds.
    return means[math.ceil(0.025 * draws) - 1], means[math.ceil(0.975 * draws) - 1]


def derive(data):
    outputs = {}
    # Analytic enumeration; these rows are mathematics, never empirical observations.
    strips = []
    for row, (n, f) in enumerate(((20, 6), (10, 2), (7, 2), (6, 1), (4, 1)), 1):
        for q in range(1, n + 1):
            cls = 2 if q > n - f else (1 if 2 * q > n + f else 0)
            strips.append((n, f, row, q, cls, int(q == 2 * f + 1), int(q == n - f)))
    outputs["quorum_strips.csv"] = csv_text(("n", "f", "row", "q", "cls", "twof1", "nf"), strips)
    lattice = [(q1, qf, 2 if qf > 5 else (1 if q1 + qf > 9 else 0))
               for q1 in range(1, 8) for qf in range(1, 8)]
    outputs["fence_lattice.csv"] = csv_text(("q1", "qF", "cls"), lattice)

    arms = ("Sage", "StopTheWorld", "HardFork", "ReconfigOnly", "CoxStyle")
    rows = data["rq1"]
    seed_grid([(r["strategy"], r["seed"]) for r in rows], arms, range(60))
    require(all(r["run_status"] == "completed" and r["safety_violation"] == "false"
                and int(r["max_finalized_height"]) == 40 for r in rows), "RQ1 incomplete execution")
    latencies = sorted(int(r["cutover_latency_micros"]) for r in rows if r["strategy"] == "Sage")
    require(all(v >= 0 for v in latencies), "Negative latency")
    outputs["cutover_ecdf.csv"] = csv_text(("x", "F"), [(v, (i + 1) / len(latencies)) for i, v in enumerate(latencies)])
    summaries = []
    for arm in arms:
        values = [int(r["downtime_micros"]) / 1000 for r in sorted(rows, key=lambda x: int(x["seed"])) if r["strategy"] == arm]
        require(all(v >= 0 for v in values), "Negative finalization gap")
        low, high = bootstrap_mean(values)
        summaries.append((arm, len(values), statistics.fmean(values), low, high))
    outputs["rq1_mean_ci.csv"] = csv_text(("arm", "seeds", "mean_ms", "bootstrap_low_ms", "bootstrap_high_ms"), summaries)

    for key, source_key, expected, x_scale, y_scale, filename, fields in (
        ("scaling", "n", (4, 7, 10, 13, 16, 20, 25, 31, 50, 100), 1, 1, "scaling.csv", ("n", "lat_us", "min_us", "max_us")),
        ("delay", "mean_delay_micros", (20000, 40000, 80000, 160000, 320000, 640000), 1000, 1000, "delay.csv", ("delay_ms", "lat_ms", "min_ms", "max_ms")),
    ):
        rows = data[key]
        seed_grid([(int(r[source_key]), r["seed"]) for r in rows], expected, range(20))
        require(all(r["migration_success"] == "true" and r["safety_violation"] == "false"
                    and r["run_status"] == "ok" for r in rows), f"Incomplete {key} run")
        groups = collections.defaultdict(list)
        for r in rows:
            value = int(r["cutover_latency_micros"]) / y_scale
            require(value >= 0, "Negative latency")
            groups[int(r[source_key])].append(value)
        outputs[filename] = csv_text(fields, [(x / x_scale, statistics.fmean(groups[x]), min(groups[x]), max(groups[x])) for x in expected])

    rows = data["threshold"]
    seed_grid([(r["arm"], r["seed"]) for r in rows], ("A_sage", "B_blind_unsafe", "C_blind_safe"), range(42, 47))
    require(all((r["n"], r["f"], r["partition_split"], r["h_c"]) == ("6", "1", "3", "4")
                and r["observed_fork"] in ("true", "false") for r in rows), "Threshold geometry/outcome mismatch")
    threshold_policies = {
        "A_sage": ("sage", "safe_bft", "n_minus_f"),
        "C_blind_safe": ("sageblind", "safe_bft", "none"),
        "B_blind_unsafe": ("sageblind", "unsafe_2f_plus_1", "none"),
    }
    require(all(tuple(r[k] for k in ("strategy", "commit_quorum", "cutover_gate"))
                == threshold_policies[r["arm"]] for r in rows), "Threshold policy mismatch")
    forest = []
    for y, arm in ((6, "A_sage"), (5, "C_blind_safe"), (4, "B_blind_unsafe")):
        group = [r for r in rows if r["arm"] == arm]
        k = sum(r["observed_fork"] == "true" for r in group)
        lo, hi = wilson(k, len(group))
        forest.append((y, k, len(group), k / len(group), lo, hi, "retained_trial_rows"))
    rows = data["authorization"]
    require(len(rows) == 3 and {r["arm"] for r in rows} == {"A_sage", "C_blind_gen", "B_blind_unsafe"}, "Authorization arm mismatch")
    authorization_policies = {
        "A_sage": ("sage", "generalized_bft", "4", "n_minus_f", "5"),
        "C_blind_gen": ("sageblind", "generalized_bft", "4", "none", "5"),
        "B_blind_unsafe": ("sageblind", "unsafe_2f_plus_1", "3", "none", "5"),
    }
    require(all(tuple(r[k] for k in ("strategy", "commit_quorum", "commit_q_value", "cutover_gate", "gate_q_value"))
                == authorization_policies[r["arm"]] for r in rows), "Authorization policy mismatch")
    for y, arm in ((3, "A_sage"), (2, "C_blind_gen"), (1, "B_blind_unsafe")):
        row = next(r for r in rows if r["arm"] == arm)
        require((row["n"], row["f"], row["h_c"], row["partition_split"]) == ("6", "1", "4", "4"), "Authorization geometry mismatch")
        require(row["inconclusive"] == "0" and row["vacuous"] == "false", "Authorization excluded observations")
        n, k = int(row["trials_observed"]), int(row["unauthorized_migrations"])
        require(n == 10 and abs(float(row["unauthorized_rate"]) - k / n) < 1e-12, "Authorization denominator/rate mismatch")
        lo, hi = wilson(k, n)
        forest.append((y, k, n, k / n, lo, hi, "retained_aggregate_only"))
    outputs["rq2_controls.csv"] = csv_text(("y", "k", "n", "rate", "lo", "hi", "evidence"), forest)

    # Paired seed resampling quantifies only the retained microbenchmark's
    # variability. It cannot establish equivalence or remove shared-host bias.
    rows = data["overhead"]
    modes = ("baseline_poa", "sage_dual_run", "cox_style")
    loads = (10, 25, 50, 100, 250, 500)
    seed_grid([((r["mode"], int(r["txs_per_block"])), r["seed"]) for r in rows],
              [(mode, load) for mode in modes for load in loads], range(10))
    require(all(r["run_status"] == "ok" and r["safety_violation"] == "false"
                and (r["n"], r["f"]) == ("20", "6") for r in rows), "Incomplete overhead run")
    overhead = []
    for load in loads:
        arms = {mode: [float(r["committed_tps_wall"]) for r in sorted(rows, key=lambda r: int(r["seed"]))
                       if r["mode"] == mode and int(r["txs_per_block"]) == load] for mode in modes}
        require(all(math.isfinite(v) and v > 0 for values in arms.values() for v in values),
                "Invalid overhead rate")
        base, sage, control = (arms[mode] for mode in modes)
        rng = random.Random(20260922)
        ratios = []
        for _ in range(10000):
            indices = rng.choices(range(10), k=10)
            ratios.append(100 * (statistics.fmean(sage[i] for i in indices)
                                 / statistics.fmean(control[i] for i in indices) - 1))
        ratios.sort()
        overhead.append((load, 100 * (statistics.fmean(sage) / statistics.fmean(base) - 1),
                         100 * (statistics.fmean(control) / statistics.fmean(base) - 1),
                         100 * (statistics.fmean(sage) / statistics.fmean(control) - 1),
                         ratios[249], ratios[9749]))
    outputs["overhead_ci.csv"] = csv_text(("txs_per_block", "sage_vs_base_pct", "switch_vs_base_pct",
                                          "sage_vs_switch_pct", "bootstrap_low_pct", "bootstrap_high_pct"), overhead)

    rows = data["wan"]
    require(len(rows) == 3 and {r["arm"] for r in rows} == {"sage", "hardfork", "cox_faithful"},
            "WAN arm mismatch")
    wan = []
    # Exact inverse Student-t CDF at 0.975 for df=2. These are conditional
    # descriptive intervals from rounded trial means, not deployment inference.
    tcrit = math.sqrt(2 * 0.95 ** 2 / (1 - 0.95 ** 2))
    for row in rows:
        require((row["trials"], row["n"], row["rtt_ms_approx"], row["migrated_all"], row["observed_fork"])
                == ("3", "6", "104", "true", "false"), "WAN geometry/outcome mismatch")
        values = [float(row[f"trial{i}_tps"]) for i in range(1, 4)]
        require(all(math.isfinite(v) and v > 0 for v in values), "Invalid WAN rate")
        mean = statistics.fmean(values)
        require(abs(mean - float(row["mean_committed_tps"])) <= 0.051, "WAN stored mean mismatch")
        half = tcrit * statistics.stdev(values) / math.sqrt(3)
        wan.append((row["arm"], 3, mean, mean - half, mean + half, "rounded_trial_means"))
    outputs["wan_descriptive_ci.csv"] = csv_text(("arm", "trials", "mean_tps", "t_low_tps", "t_high_tps", "evidence"), wan)
    rows = data["certificate"]
    expected = {31: 10, 64: 21, 100: 33, 200: 66}
    require(len(rows) == len(expected) and {int(r["n"]) for r in rows} == set(expected), "Certificate grid mismatch")
    certificate = []
    for r in sorted(rows, key=lambda row: int(row["n"])):
        n, f, signers, sign, p50, p95, size = (int(r[key]) for key in
            ("n", "f", "signers", "sign_total_us", "verify_all_p50_us", "verify_all_p95_us", "cert_bytes"))
        require(r["experiment"] == "cert_timing" and r["measurement_source"] == "ed25519_real_signatures"
                and f == expected[n] and signers == n - f and size == 72 * signers
                and sign > 0 and 0 < p50 <= p95, "Certificate geometry/measurement mismatch")
        certificate.append((n, f, signers, sign, p50, p95, size, "retained_aggregate_only"))
    outputs["certificate_cost.csv"] = csv_text(("n", "f", "signers", "sign_us", "verify_p50_us",
                                                "verify_p95_us", "bytes", "evidence"), certificate)
    return outputs


def decimal_cell(value, places, signed=False):
    """Round decimal display values explicitly, not at a binary-float tie."""
    number = Decimal(str(value)).quantize(Decimal(1).scaleb(-places), rounding=ROUND_HALF_UP)
    return format(number, f'{"+" if signed else ""}.{places}f')


def table_products(data, derived):
    """Render retained summaries; never invent swap outcomes or per-trial samples."""
    outputs, tables = {}, {}

    def rows(name):
        return list(csv.DictReader(io.StringIO(derived[name])))

    def emit(label, filename, cells, inputs, method, **metadata):
        path = f"tables/{filename}_rows.tex"
        outputs[path] = "% Generated by rebuild.py from retained inputs; do not hand-edit.\n" + "".join(
            " & ".join(row) + r" \\" + "\n" for row in cells)
        # End the alignment in the input too: LaTeX's input-return bookkeeping
        # can otherwise open a phantom first cell after the rule under rowcolors.
        outputs[path] += "\\bottomrule\n\\end{tabular}\n"
        tables[label] = {"output": path,
                         "inputs": {key: {"path": INPUTS[key][0], "sha256": INPUTS[key][1]} for key in inputs},
                         "method": method, **metadata}

    names = {"Sage": r"\sage{}", "StopTheWorld": "Stop-the-world", "HardFork": "Hard fork",
             "ReconfigOnly": "Reconfiguration only", "CoxStyle": r"Cox-style\tnote{b}"}
    rq1 = rows("rq1_mean_ci.csv")
    emit("tab:main", "rq1",
         [[names[r["arm"]], decimal_cell(r["mean_ms"], 3),
           f'$[{decimal_cell(r["bootstrap_low_ms"], 4)},{decimal_cell(r["bootstrap_high_ms"], 4)}]$'] for r in rq1],
         ["rq1"], "Arithmetic arm means of retained maximum gaps (microseconds / 1000); inverse empirical CDF bootstrap, 10000 resamples, seed 20260922. Decimal half-up display rounding: mean 3 decimals, interval 4 decimals.",
         evidence="retained_trial_rows", observations_per_arm=60,
         omitted_columns={"swap": "No observed protocol_swap_success field in retained input; not inferred from strategy."})

    def endpoint(value):
        number = float(value)
        return decimal_cell(value, 3 if 0 < abs(number) < 0.01 else 2, signed=True)

    overhead = []
    for r in rows("overhead_ci.csv"):
        overhead.append([r["txs_per_block"],
                         *[f'${decimal_cell(r[k], 2, signed=True)}' + r"\%$" for k in
                           ("sage_vs_base_pct", "switch_vs_base_pct", "sage_vs_switch_pct")],
                         f'$[{endpoint(r["bootstrap_low_pct"])},{endpoint(r["bootstrap_high_pct"])}]' + r"\%$"])
    emit("tab:overhead", "overhead", overhead, ["overhead"],
         "100*(ratio of arm means - 1); 10000 paired seed-index resamples, seed 20260922. Two decimals except nonzero CI endpoints below 0.01 use three decimals.",
         evidence="retained_trial_rows", observations_per_arm_per_load=10)

    forest = {int(r["y"]): r for r in rows("rq2_controls.csv")}
    control_cells, control_provenance = [], []
    for key, axis, order in (
        ("threshold", "Threshold, $3/3$", ((6, "A_sage"), (5, "C_blind_safe"), (4, "B_blind_unsafe"))),
        ("authorization", "Authorization, $4/2$", ((3, "A_sage"), (2, "C_blind_gen"), (1, "B_blind_unsafe"))),
    ):
        for y, arm in order:
            r = next(r for r in data[key] if r["arm"] == arm)
            result = forest[y]
            n, f = int(r["n"]), int(r["f"])
            if key == "threshold":
                # M10's provenance note defines its historical safe_bft label as
                # HotStuffNative (n-f), NOT the generalized minimum-safe formula.
                # This is a documented setting, not numeric runtime telemetry.
                q = n - f if r["commit_quorum"] == "safe_bft" else 2 * f + 1
                gate = str(n - f) if r["cutover_gate"] == "n_minus_f" else "--"
                outcome = "conflict"
            else:
                q = int(r["commit_q_value"])
                gate = r["gate_q_value"] if r["cutover_gate"] == "n_minus_f" else "--"
                outcome = "unauthorized"
            observed = f'{outcome} ${result["k"]}/{result["n"]}$'
            if result["evidence"] == "retained_aggregate_only":
                observed += r"\tnote{a}"
            control_cells.append([axis, r"\sage{}" if r["strategy"] == "sage" else "ungated", str(q), gate, observed])
            control_provenance.append({"input": key, "arm": arm, "events": int(result["k"]),
                                       "observations": int(result["n"]), "evidence": result["evidence"],
                                       "commit_q": q, "gate": gate})
    emit("tab:rq2controls", "rq2", control_cells, ["threshold", "authorization"],
         "Threshold: count observed_fork booleans in admitted trial rows. Authorization: retain aggregate unauthorized_migrations/trials_observed, never expand into synthetic trials. Quorums describe checked recorded policies, not new resolved-policy telemetry.",
         rows=control_provenance)

    emit("tab:certtiming", "certificate",
         [[r["n"], r["signers"], r["sign_us"], f'{r["verify_p50_us"]} / {r["verify_p95_us"]}', r["bytes"]]
          for r in rows("certificate_cost.csv")], ["certificate"],
         "Transcribe sealed campaign aggregates in integer microseconds/bytes; validate signer count, share-list size and p50 <= p95. No percentile recomputation without retained individual samples.",
         evidence="retained_aggregate_only", individual_samples_retained=False)
    outputs["TABLE_PROVENANCE.json"] = json.dumps({
        "schema_version": 1, "status": "PRESENTATION_REGENERATION_ONLY", "historical_trials_regenerated": False,
        "tables": tables,
        "input_notes": {key: {"path": rel, "sha256": sha} for key, (rel, sha) in INPUT_NOTES.items()},
        "remaining": ["Per-trial ledgers absent for aggregate-only campaigns.",
                      "Campaign environment records incomplete; no new experiment execution.",
                      "Clean public-checkout availability and full-protocol performance remain unestablished."]}, indent=2) + "\n"
    return outputs


def source_inventory():
    paths = [PAPER / "main.tex", PAPER / "references.bib", PAPER / "rebuild.py"]
    paths += sorted((PAPER / "sections").glob("*.tex"))
    paths += sorted((PAPER / "figures").glob("*.tex"))
    paths += sorted((PAPER / "figures").glob("*.sty"))
    return {str(p.relative_to(PAPER)): digest(p) for p in paths}


def compare_builds(first, second):
    """Require identical scientific/presentation inputs and every generated product."""
    require(first["source_sha256"] == second["source_sha256"], "Source changed between builds")
    require(first["inputs"] == second["inputs"], "Data changed between builds")
    require(first["outputs"] == second["outputs"], "Generated outputs are not byte-identical")


def build(stage, before):
    env = dict(os.environ, SOURCE_DATE_EPOCH=EPOCH, FORCE_SOURCE_DATE="1")
    logs = stage / "logs"
    logs.mkdir()
    started = time.monotonic()
    for name in FIGURES:
        with (logs / f"{name}.txt").open("w") as log:
            subprocess.run(["pdflatex", "-interaction=nonstopmode", "-halt-on-error", f"{name}.tex"],
                           cwd=stage / "figures", env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
    with (logs / "main.txt").open("w") as log:
        subprocess.run(["latexmk", "-pdf", "-interaction=nonstopmode", "-halt-on-error", "main.tex"],
                       cwd=stage, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
    require(source_inventory() == before, "Paper inputs changed during build")
    load_inputs()  # post-use raw-input recheck
    return time.monotonic() - started


def run_checks(stage):
    """Check the exact staged manuscript, including generated table inputs."""
    for script, extra in (
        ("verify_caption_style.py", [str(stage / "main.tex")]),
        ("verify_limitation_disclosures.py", [str(stage / "main.tex")]),
        ("verify_paper_claim_scope.py", ["--paper", str(stage / "main.tex")]),
        ("verify_document_order.py", [str(stage / "main.pdf")]),
    ):
        with (stage / "logs" / f"{script}.txt").open("w") as log:
            subprocess.run([sys.executable, "-B", str(ROOT / "scripts" / script), *extra],
                           cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check-data", action="store_true", help="derive and validate retained input closure; no files written")
    parser.add_argument("--publish", action="store_true", help="publish staged data and PDF outputs after a successful build")
    parser.add_argument("--verify-rebuild", action="store_true", help="build twice in independent scratch directories; compare every product")
    parser.add_argument("--work-root", type=Path, default=Path(os.environ.get("TMPDIR", str(Path.home() / ".hermes/cache/scratch"))))
    args = parser.parse_args()
    before = source_inventory()
    data = load_inputs()
    outputs = derive(data)
    generated = {f"figures/data/{name}": text for name, text in outputs.items()}
    generated.update(table_products(data, outputs))
    if args.check_data:
        print(json.dumps({"status": "PASS", "retained_inputs": len(INPUTS), "derived_files": sorted(generated),
                          "scope": "presentation regeneration only; no new trials"}, indent=2))
        return
    args.work_root.mkdir(parents=True, exist_ok=True)
    stage = Path(tempfile.mkdtemp(prefix="sage-paper-build-", dir=args.work_root))
    for relative in before:
        if relative == "rebuild.py":
            continue
        target = stage / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(PAPER / relative, target)
    for name, text in generated.items():
        target = stage / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")
    print(f"Staged build and logs retained at {stage}", flush=True)
    seconds = build(stage, before)
    products = ["main.pdf"] + [f"figures/{name}.pdf" for name in FIGURES] + list(generated)
    manifest = {
        "schema_version": 1, "status": "PRESENTATION_BUILD_ONLY", "submission_ready": False,
        "historical_trials_regenerated": False, "full_protocol_measured": False,
        "source_date_epoch": EPOCH, "source_sha256": before,
        "inputs": {key: {"path": value[0], "sha256": value[1]} for key, value in INPUTS.items()},
        "outputs": {name: {"sha256": digest(stage / name), "bytes": (stage / name).stat().st_size} for name in products},
        "scope": "All six vector figures and four numerical tables regenerated. Authorization counts and certificate timings remain aggregate-only; per-trial ledgers and historical environments are not invented.",
    }
    (stage / "BUILD_INPUTS.json").write_text(json.dumps(manifest, indent=2) + "\n")
    run_checks(stage)
    if args.verify_rebuild:
        repeat = subprocess.run([sys.executable, "-B", str(PAPER / "rebuild.py"),
                                 "--work-root", str(args.work_root)], cwd=ROOT,
                                text=True, capture_output=True, check=True)
        (stage / "logs/repeat-build.txt").write_text(repeat.stdout + repeat.stderr)
        result = json.loads(repeat.stdout[repeat.stdout.index("{"):])
        repeat_stage = Path(result["stage"])
        repeated = json.loads((repeat_stage / "BUILD_INPUTS.json").read_text())
        compare_builds(manifest, repeated)
        (stage / "REPRODUCIBILITY.json").write_text(json.dumps({
            "status": "PASS", "first": str(stage), "second": str(repeat_stage),
            "compared_products": len(products), "byte_identical": True,
            "scope": "presentation products only; no experiment rerun"}, indent=2) + "\n")
        print(f"PAPER BUILD REPRODUCIBLE: {len(products)} products byte-identical", flush=True)
    require(source_inventory() == before, "Paper inputs changed before publication")
    load_inputs()
    if args.publish:
        for name in products + ["BUILD_INPUTS.json"]:
            destination = PAPER / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            with tempfile.NamedTemporaryFile(dir=destination.parent, prefix=".rebuild-", delete=False) as handle:
                temporary = Path(handle.name)
                handle.write((stage / name).read_bytes())
                handle.flush()
                os.fsync(handle.fileno())
            os.replace(temporary, destination)
        for name in products:
            require(digest(PAPER / name) == manifest["outputs"][name]["sha256"], f"Published output mismatch: {name}")
    print(json.dumps({"stage": str(stage), "build_seconds": seconds, "published": args.publish,
                      "pdf_sha256": digest(stage / "main.pdf"), "scope": manifest["status"]}, indent=2))


if __name__ == "__main__":
    main()
