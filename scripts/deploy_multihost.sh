#!/bin/bash
# P0-A: Multi-HOST geo-distributed deployment harness for the SAGE testbed.
#
# HONESTY CONTRACT (read this before citing any numbers it produces):
#   This script runs the SAGE multi-process testbed across SEVERAL REAL HOSTS
#   (cloud VMs or LAN machines) over a REAL network, NOT loopback. It is the
#   harness the Q1 plan (docs/plans/2026-06-22-q1-elevation-plan.md, Phase P0-A)
#   requires to convert simulator microseconds into wall-clock geo-distributed
#   metrics. It is provided as reviewable, runnable code; it produces real
#   numbers ONLY when pointed at real hosts via config/hosts.txt. With no hosts
#   file it refuses to fabricate a geo result and exits with guidance.
#
# hosts file format (config/hosts.txt), one validator per line:
#   <validator_id> <ssh_user@host_or_ip> <region_label> <bind_ip:port>
# example:
#   0 ubuntu@52.1.2.3   us-east-1     10.0.0.10:7000
#   1 ubuntu@18.4.5.6   eu-west-1     10.0.1.10:7000
#   2 ubuntu@13.7.8.9   ap-southeast  10.0.2.10:7000
#
# Requirements on each host: ssh access, the prebuilt `validator_proc` binary
# distributed to the same path (this script can scp it), and TCP reachability
# on the chosen ports between all hosts.
#
# Usage:
#   bash scripts/deploy_multihost.sh build        # build the binary locally
#   bash scripts/deploy_multihost.sh distribute    # scp binary to every host
#   bash scripts/deploy_multihost.sh run [STRATEGY] [MAX_HEIGHT]
#
set -u
cd "$(dirname "$0")/.."

HOSTS_FILE="${HOSTS_FILE:-config/hosts.txt}"
REMOTE_BIN="${REMOTE_BIN:-/tmp/sage_validator_proc}"
STRATEGY="${2:-sage}"
MAX_HEIGHT="${3:-8}"
H_D="${H_D:-2}"
H_C="${H_C:-4}"
H_R="${H_R:-8}"
MAX_SECS="${MAX_SECS:-60}"
# Real-network pacemaker tuning. Single-box defaults (200ms view timeout) burn
# views before staggered ssh-launched validators form a mesh over a real WAN.
# Widen for real inter-host RTT + process start skew; override via env.
VIEW_TIMEOUT_MS="${VIEW_TIMEOUT_MS:-2000}"
PACEMAKER_TICK_MS="${PACEMAKER_TICK_MS:-200}"
PROPOSAL_INTERVAL_MS="${PROPOSAL_INTERVAL_MS:-500}"
# Synthetic throughput workload (height-derived txs per block). 0 = empty blocks
# (fork/partition experiments); >0 gives a real committed-tx count for the TPS
# and finality-latency metrics. Override via env.
WORKLOAD_TXS="${WORKLOAD_TXS:-0}"
OUT="${OUT:-results/raw/multihost_${STRATEGY}.csv}"

cmd="${1:-help}"

build() {
  echo "[build] cargo build --release -p sage-node --bin validator_proc --bin spawn_testbed"
  cargo build --release -p sage-node --bin validator_proc --bin spawn_testbed --features real-crypto
}

require_hosts() {
  if [ ! -f "$HOSTS_FILE" ]; then
    cat >&2 <<EOF
[refuse] No hosts file at $HOSTS_FILE.

This harness will NOT fabricate a geo-distributed result. To run a real
multi-host experiment, create $HOSTS_FILE with one validator per line:

  <validator_id> <ssh_user@host> <region_label> <bind_ip:port>

Then: bash scripts/deploy_multihost.sh build
      bash scripts/deploy_multihost.sh distribute
      bash scripts/deploy_multihost.sh run sage 8

For a single-box, honestly-labeled emulation instead, use:
  bash scripts/local_wan_emulation.sh
EOF
    exit 3
  fi
}

distribute() {
  require_hosts
  local localbin=target/release/validator_proc
  [ -x "$localbin" ] || { echo "[distribute] build first: bash $0 build" >&2; exit 1; }
  while read -r id host region bind; do
    [ -z "${id:-}" ] && continue
    case "$id" in \#*) continue ;; esac
    echo "[distribute] $host ($region) <- $localbin"
    scp -q "$localbin" "$host:$REMOTE_BIN" || { echo "scp to $host failed" >&2; exit 1; }
  done < "$HOSTS_FILE"
  echo "[distribute] done"
}

run() {
  require_hosts
  mkdir -p "$(dirname "$OUT")"

  # Build the peer map as a position-indexed addr list (validator_proc contract:
  # --peers is comma-separated, index = validator id). Sort by id to be safe.
  local peers
  peers=$(awk '!/^#/ && NF>=4 {print $1, $4}' "$HOSTS_FILE" | sort -n | awk '{print $2}' | paste -sd, -)
  echo "[run] peer map (index=id): $peers"
  echo "[run] strategy=$STRATEGY max_height=$MAX_HEIGHT h_c=$H_C"

  # Total validator count (for partition side computation) and the BFT fault
  # bound f = floor((n-1)/3). validator_proc defaults to n=4 if --n is omitted,
  # so we MUST pass both --n and --f explicitly or a 6-peer list is rejected.
  local total f
  total=$(awk '!/^#/ && NF>=4' "$HOSTS_FILE" | wc -l | tr -d ' ')
  f=$(( (total - 1) / 3 ))

  # Optional partition: PARTITION=A splits validators into side A={0..A-1} and
  # side B={A..total-1}, engaging at h_c. Each validator is told which peers it
  # can still reach (--partition-keep) and blocks the other side at the socket
  # layer. For a genuine fork window BOTH sides must independently reach BFT
  # quorum (2f+1), which requires total>=4f+2 (e.g. n=6,f=1,3/3). A side smaller
  # than quorum cannot fork — it stalls. When PARTITION is set and STRATEGY=sage,
  # the n-f cutover-quorum gate is enabled (the safety mechanism under test).
  local side_a="" side_b=""
  if [ -n "${PARTITION:-}" ]; then
    side_a=$(seq 0 $((PARTITION-1)) | paste -sd, -)
    side_b=$(seq "$PARTITION" $((total-1)) | paste -sd, -)
    echo "[run] PARTITION at h_c=$H_C: side A={$side_a} | side B={$side_b} (cross-side sockets blocked)"
  fi

  # Launch one remote validator_proc per host, backgrounded, capturing each
  # ProcessResult JSON line. Real network, real clocks, independent OS processes.
  local pids=() tmpdir
  tmpdir=$(mktemp -d)
  local n=0
  while read -r id host region bind; do
    [ -z "${id:-}" ] && continue
    case "$id" in \#*) continue ;; esac
    n=$((n+1))
    local bindport="${bind##*:}"
    local extra=""
    if [ -n "${PARTITION:-}" ]; then
      if [ "$id" -lt "$PARTITION" ]; then
        extra="--partition-keep $side_a --partition-at $H_C"
      else
        extra="--partition-keep $side_b --partition-at $H_C"
      fi
      # The n-f cutover-quorum gate is SAGE's safety mechanism; the hardfork
      # broken control deliberately omits it so the fork window is exercised.
      if [ "$STRATEGY" = "sage" ]; then
        extra="$extra --cutover-quorum"
      fi
    fi
    ssh "$host" "$REMOTE_BIN --id $id --n $total --f $f --peers '$peers' --bind 0.0.0.0:$bindport \
        --strategy $STRATEGY --max-height $MAX_HEIGHT \
        --h-d $H_D --h-c $H_C --h-r $H_R --max-secs $MAX_SECS \
        --view-timeout-ms $VIEW_TIMEOUT_MS --pacemaker-tick-ms $PACEMAKER_TICK_MS \
        --proposal-interval-ms $PROPOSAL_INTERVAL_MS --workload-txs $WORKLOAD_TXS $extra" \
        > "$tmpdir/result_$id.json" 2>"$tmpdir/err_$id.log" &
    pids+=("$!")
    echo "[run] launched validator $id on $host ($region)${extra:+ [$extra]}"
  done < "$HOSTS_FILE"

  echo "[run] waiting for $n validators..."
  for p in "${pids[@]}"; do wait "$p" || true; done

  # Aggregate results: detect cross-host forks + collect wall-clock metrics.
  # The aggregator emits its own header row (schema-accurate); do not pre-write one.
  python3 scripts/aggregate_multihost.py "$tmpdir" "$HOSTS_FILE" "$STRATEGY" "$n" > "$OUT" \
    && echo "[run] wrote $OUT" || echo "[run] aggregation failed; raw JSON in $tmpdir"
  echo "[run] raw per-host JSON kept in $tmpdir"
}

case "$cmd" in
  build) build ;;
  distribute) distribute ;;
  run) run ;;
  *)
    cat <<EOF
SAGE multi-host deployment harness (P0-A)
  build       compile validator_proc/spawn_testbed (release, real-crypto)
  distribute  scp the binary to every host in $HOSTS_FILE
  run [S] [H] launch one validator per host; S=strategy(default sage), H=max_height
Environment: HOSTS_FILE, REMOTE_BIN, H_D, H_C, H_R, MAX_SECS, OUT
This harness refuses to run without a real $HOSTS_FILE; it never fabricates geo data.
EOF
    ;;
esac
