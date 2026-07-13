#!/usr/bin/env python3
"""Aggregate per-host ProcessResult JSON from deploy_multihost.sh (P0-A).

Reads one result_<id>.json per validator, detects cross-host forks at the STATE
level (two correct validators committing distinct state_roots at the same
height), and emits CSV rows with real wall-clock metrics. Real-network evidence
only; never invents data.

Fork semantics match the project fork detector: agreement is on the DECIDED
VALUE (finalized state_root), not the engine-tagged block hash. At the
PoA->HotStuff boundary the same logical block encodes an engine tag, so the
block hash differs while the state_root is identical -- that is NOT a fork.
The real ProcessResult schema (crates/sage-node) is:
  max_finalized_height, total_duration_secs, migration_success, cutover_height,
  cutover_cert, committed_detail: [[height, state_root, engine], ...]
"""
import json
import sys
from pathlib import Path


def main() -> int:
    if len(sys.argv) < 5:
        print("usage: aggregate_multihost.py <tmpdir> <hosts_file> <strategy> <n>", file=sys.stderr)
        return 2
    tmpdir, hosts_file, strategy, n = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4])

    regions = {}
    with open(hosts_file) as fh:
        for line in fh:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split()
            if len(parts) >= 3:
                regions[parts[0]] = parts[2]

    # height -> set of distinct state_roots committed by any validator
    per_height_roots = {}
    rows = []
    for jf in sorted(Path(tmpdir).glob("result_*.json")):
        vid = jf.stem.split("_")[-1]
        try:
            data = json.loads(jf.read_text() or "{}")
        except json.JSONDecodeError:
            data = {}
        height = data.get("max_finalized_height", 0)
        wall_ms = int(round(float(data.get("total_duration_secs", 0.0)) * 1000))
        cutover = data.get("cutover_height")
        migrated = bool(data.get("migration_success", False))
        has_cert = data.get("cutover_cert") is not None
        # Real-host throughput / finality-latency metrics (E5 instrumentation).
        total_txs = int(data.get("total_txs", 0))
        committed_tps = float(data.get("committed_tps", 0.0))
        p50 = int(data.get("inter_commit_p50_micros", 0))
        p95 = int(data.get("inter_commit_p95_micros", 0))
        p99 = int(data.get("inter_commit_p99_micros", 0))
        gmax = int(data.get("inter_commit_max_micros", 0))
        cut_gap = int(data.get("cutover_gap_micros", 0))
        # committed_detail entries are [height, state_root, engine]
        for entry in (data.get("committed_detail") or []):
            if len(entry) >= 2:
                h, root = entry[0], entry[1]
                per_height_roots.setdefault(str(h), set()).add(root)
        rows.append((vid, regions.get(vid, "unknown"), height, wall_ms,
                     cutover, migrated, has_cert,
                     total_txs, committed_tps, p50, p95, p99, gmax, cut_gap))

    observed_fork = any(len(s) > 1 for s in per_height_roots.values())

    print("experiment,n,strategy,validator_id,region,committed_height,wall_ms,"
          "cutover_height,migration_success,has_cutover_cert,observed_fork,"
          "total_txs,committed_tps,inter_commit_p50_micros,inter_commit_p95_micros,"
          "inter_commit_p99_micros,inter_commit_max_micros,cutover_gap_micros")
    for (vid, region, height, wall, cutover, migrated, has_cert,
         total_txs, committed_tps, p50, p95, p99, gmax, cut_gap) in rows:
        print(f"multihost,{n},{strategy},{vid},{region},{height},{wall},"
              f"{cutover if cutover is not None else ''},"
              f"{str(migrated).lower()},{str(has_cert).lower()},"
              f"{str(observed_fork).lower()},"
              f"{total_txs},{committed_tps:.3f},{p50},{p95},{p99},{gmax},{cut_gap}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
