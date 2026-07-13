#!/bin/bash
# Real-host INJECTED-RTT 3-arm throughput differential for SAGE.
#
# HONESTY CONTRACT:
#   Runs the three live-switch arms (SAGE / hardfork / Cox-style) on SIX REAL,
#   INDEPENDENT cloud hosts over REAL TCP, with a REAL kernel `tc netem` egress
#   delay injected on every host so the inter-VM RTT is a realistic wide-area
#   value rather than the sub-millisecond same-region default. This is stronger
#   than the single-box modeled WAN sweep (paper tab:wantps): it is real
#   machines + real network + real kernel-level latency. It is still SAME-REGION
#   hosts with SYNTHETIC added delay, NOT physically geo-distributed hosts in
#   different regions -- that distinction is stated honestly in the paper.
#
# Each arm runs under an IDENTICAL config (same n, f, workload, schedule, RTT),
# so the only variable is the migration strategy -- the "same 100m" fairness
# guarantee, now at real-host + injected-WAN-RTT.
#
# Usage: bash scripts/multihost_wan_tps_3arm.sh [DELAY_MS_PER_SIDE] [MAX_HEIGHT] [WORKLOAD_TXS]
#   DELAY_MS_PER_SIDE default 50 (=> ~104ms real inter-VM RTT, matching the
#   already-cited fork-differential WAN run).
set -u
cd "$(dirname "$0")/.."

HOSTS_FILE="${HOSTS_FILE:-config/hosts.txt}"
REMOTE_BIN="${REMOTE_BIN:-/tmp/sage_validator_proc}"
DELAY_MS="${1:-50}"
MAX_HEIGHT="${2:-30}"
WORKLOAD_TXS="${3:-200}"
IFACE="${IFACE:-enp0s5}"
OUT="${OUT:-results/raw/multihost_wan_tps_3arm.csv}"
MAX_SECS="${MAX_SECS:-90}"
# WAN pacemaker tuning (same as deploy_multihost.sh real-network defaults).
export VIEW_TIMEOUT_MS="${VIEW_TIMEOUT_MS:-3000}"
export PACEMAKER_TICK_MS="${PACEMAKER_TICK_MS:-200}"
export PROPOSAL_INTERVAL_MS="${PROPOSAL_INTERVAL_MS:-500}"

mkdir -p "$(dirname "$OUT")"

hosts_only() { awk '!/^#/ && NF>=4 {print $2}' "$HOSTS_FILE"; }

apply_netem() {
  local d=$1
  echo "[netem] applying ${d}ms egress delay on $IFACE across all hosts"
  for h in $(hosts_only); do
    ssh -n "$h" "sudo tc qdisc replace dev $IFACE root netem delay ${d}ms 2>/dev/null || \
                 sudo tc qdisc add dev $IFACE root netem delay ${d}ms" \
      && echo "  [ok] $h" || echo "  [WARN] $h netem apply failed"
  done
}

clear_netem() {
  echo "[netem] clearing impairment on all hosts"
  for h in $(hosts_only); do
    ssh -n "$h" "sudo tc qdisc del dev $IFACE root 2>/dev/null || true" >/dev/null 2>&1
  done
}

# Always clear netem on exit so we never leave the VMs impaired.
trap clear_netem EXIT

# Kill any stale validators + free the port before each arm.
cleanup_hosts() {
  for h in $(hosts_only); do
    ssh -n "$h" "pkill -f sage_validator_proc 2>/dev/null || true" >/dev/null 2>&1
  done
  sleep 2
}

echo "arm,delay_ms_per_side,rtt_ms_approx,n,mean_committed_tps,min_tps,max_tps,migrated_all,observed_fork" > "$OUT"

apply_netem "$DELAY_MS"
RTT=$((DELAY_MS * 2 + 4))  # ~2x one-way + host noise, matches cited ~104ms at 50ms/side

for arm in sage hardfork cox_faithful; do
  echo "=== ARM: $arm (RTT ~${RTT}ms, workload=${WORKLOAD_TXS} txs/blk) ==="
  cleanup_hosts
  # Map arm -> validator_proc strategy flag.
  case "$arm" in
    sage)         STRAT=sage ;;
    hardfork)     STRAT=hardfork ;;
    cox_faithful) STRAT=coxfaithful ;;
  esac
  arm_tmp="results/raw/_wan_tps_${arm}.csv"
  STRATEGY="$STRAT" MAX_HEIGHT="$MAX_HEIGHT" WORKLOAD_TXS="$WORKLOAD_TXS" \
    MAX_SECS="$MAX_SECS" OUT="$arm_tmp" \
    bash scripts/deploy_multihost.sh run "$STRAT" "$MAX_HEIGHT" || true

  # Reduce per-validator rows -> per-arm summary (mean/min/max TPS, migration, fork).
  python3 - "$arm_tmp" "$arm" "$DELAY_MS" "$RTT" >> "$OUT" <<'PY'
import csv, sys
path, arm, dms, rtt = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
tps=[]; migrated=[]; forked=False; n=0
try:
    with open(path) as f:
        for row in csv.DictReader(f):
            n = row.get("n", n)
            try: tps.append(float(row.get("committed_tps", 0)))
            except ValueError: pass
            mh = row.get("migration_success") or row.get("migrated") or ""
            migrated.append(str(mh).lower() in ("true","1"))
            if str(row.get("observed_fork","")).lower() in ("true","1"): forked=True
except FileNotFoundError:
    pass
tps=[t for t in tps if t>0]
mean = sum(tps)/len(tps) if tps else 0.0
lo = min(tps) if tps else 0.0
hi = max(tps) if tps else 0.0
mall = "true" if migrated and all(migrated) else "false"
print(f"{arm},{dms},{rtt},{n},{mean:.1f},{lo:.1f},{hi:.1f},{mall},{str(forked).lower()}")
PY
  rm -f "$arm_tmp"
done

echo ""
echo "=== real-host injected-RTT 3-arm TPS summary ($OUT) ==="
cat "$OUT"
