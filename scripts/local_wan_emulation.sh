#!/bin/bash
# P0-A (local tier): max-fidelity SINGLE-BOX WAN emulation for the SAGE testbed.
#
# HONESTY CONTRACT:
#   This runs N real OS processes over real loopback TCP with software netem-style
#   impairment (per-send loss + randomized delay/jitter, seeded, no sudo). It is a
#   SINGLE-MACHINE emulation, NOT a geo-distributed deployment. It abstracts away
#   independent host scheduler contention, clock drift, and disk I/O. It exists to
#   exercise the cutover under WAN-like delay/loss locally and to confirm zero forks
#   under impairment. For real geo numbers use scripts/deploy_multihost.sh with hosts.
#
# It sweeps the four WAN profiles from the paper and records, per profile, whether
# SAGE crossed the cutover with zero observed forks under impairment.
#
# Usage: bash scripts/local_wan_emulation.sh [N] [RUNS]
set -u
cd "$(dirname "$0")/.."

N="${1:-6}"
RUNS="${2:-3}"
HC="${HC:-4}"
MAXH="${MAXH:-8}"
MAXSECS="${MAXSECS:-30}"
OUT="results/raw/local_wan_emulation.csv"
BIN=./target/debug/spawn_testbed

if [ ! -x "$BIN" ]; then
  echo "[build] cargo build -p sage-node --bin spawn_testbed --bin validator_proc"
  cargo build -p sage-node --bin spawn_testbed --bin validator_proc || exit 1
fi

mkdir -p results/raw
echo "experiment,profile,delay_ms,jitter_ms,loss_pct,n,run,observed_fork,status" > "$OUT"

# profile: name delay_ms jitter_ms loss_pct
run_profile() {
  local name="$1" delay="$2" jitter="$3" loss="$4"
  echo "=== profile $name (delay=${delay}ms jitter=${jitter}ms loss=${loss}%) ==="
  for i in $(seq 1 "$RUNS"); do
    json=$("$BIN" --n "$N" --strategy sage --h-c "$HC" \
      --max-height "$MAXH" --max-secs "$MAXSECS" \
      --delay-ms "$delay" --jitter-ms "$jitter" --loss-pct "$loss" \
      --out "/tmp/localwan_${name}_${i}.csv" 2>/dev/null)
    fork=$(printf '%s\n' "$json" | grep -o '"observed_fork":[a-z]*' | head -1)
    case "$fork" in
      *true*) echo "  run $i: FORK"; ofork=true; status=forked ;;
      *false*) echo "  run $i: no fork"; ofork=false; status=ok ;;
      *) echo "  run $i: NO RESULT"; ofork=false; status=no_result ;;
    esac
    echo "local_wan,$name,$delay,$jitter,$loss,$N,$i,$ofork,$status" >> "$OUT"
  done
}

# one-way delay / jitter / loss buckets (paper Sec VI.A)
run_profile metro 10 2 0.0
run_profile continental 50 10 0.1
run_profile intercontinental 120 25 0.5
run_profile adverse 200 50 1.0

echo ""
echo "[done] wrote $OUT"
echo "NOTE: single-box emulation; not a geo-distributed deployment claim."
