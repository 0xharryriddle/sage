#!/usr/bin/env bash
# Standalone design-only bounded Rule 1 check. Deliberately outside run_tlc.sh
# and its packet witnesses. No change to the existing verdict matrix.
set -euo pipefail
cd "$(dirname "$0")"

if [[ ! -f tla2tools.jar ]]; then
    printf 'FAIL: formal/tla2tools.jar is missing\n' >&2
    exit 2
fi
if ! command -v java >/dev/null 2>&1; then
    printf 'FAIL: java is required for TLC\n' >&2
    exit 2
fi
if ! command -v timeout >/dev/null 2>&1; then
    printf 'FAIL: coreutils timeout is required\n' >&2
    exit 2
fi

# Isolate TLC metadata and output; never cleanup registered formal/states/.
meta=$(mktemp -d)
trap 'rm -rf -- "$meta"' EXIT

check() {
    local cfg=$1 expected=$2 label=$3 out rc=0 metadir
    metadir=$(mktemp -d "$meta/tlc.XXXXXX")
    out="$metadir/output"
    printf '=== %s: %s ===\n' "$label" "$cfg"
    timeout --signal=TERM --kill-after=10s "${TLC_TIMEOUT:-180s}" \
        java -Xmx2g -cp tla2tools.jar tlc2.TLC -workers 4 \
        -metadir "$metadir" -config "$cfg.cfg" SageReadinessLock.tla \
        >"$out" 2>&1 || rc=$?

    # TLC emits exact named invariant violations; a parser error, deadlock,
    # Java failure, timeout, or an unrelated invariant cannot masquerade as a
    # successful negative control. Exit status alone is not a TLC verdict.
    if [[ $rc == 124 || $rc == 137 ]]; then
        printf 'FAIL: %s timed out (exit %s)\n' "$cfg" "$rc" >&2
    elif grep -Eq 'Deadlock reached' "$out"; then
        printf 'FAIL: %s TLC deadlock\n' "$cfg" >&2
    elif grep -Eq 'Parse Error|Semantic errors|Exception|Error: Parsing' "$out"; then
        printf 'FAIL: %s TLC parser/evaluator error\n' "$cfg" >&2
    elif [[ $expected == HOLD && $rc == 0 ]] && \
         grep -Fq 'No error has been found' "$out" && \
         ! grep -Eq 'Error:|Exception|is violated|Deadlock reached' "$out"; then
        printf 'OK: %s all selected invariants HOLD\n' "$cfg"
        return 0
    elif [[ $expected != HOLD && $rc == 12 ]] && \
         grep -Fxq "Error: Invariant $expected is violated." "$out" && \
         [[ $(grep -Ec '^Error: Invariant ' "$out") == 1 ]] && \
         [[ $(grep -Ec '^Error:' "$out") == 2 ]] && \
         grep -Fxq 'Error: The behavior up to this point is:' "$out"; then
        printf 'OK: %s expected %s counterexample\n' "$cfg" "$expected"
        return 0
    else
        printf 'FAIL: %s wrong TLC verdict (exit %s, expected %s)\n' \
            "$cfg" "$rc" "$expected" >&2
    fi
    cat "$out" >&2
    return 1
}

# The negative configs are NOT safety passes: each must expose its own real
# source-quorum defect. Reachability is separate and uses a negated witness.
check SageReadinessLock HOLD 'FAITHFUL bounded safety'
check SageReadinessLockEarlyShare NoSourceQuorumAfterCutCert 'BROKEN early share'
check SageReadinessLockRestartBypass NoSourceQuorumAfterCutCert 'BROKEN restart bypass'
check SageReadinessLockReachable NoCutCertAfterLockedRestart 'POSITIVE CutCert across crash/reload'
check SageReadinessLockSourceReachable NoSourceVoteBeforeLock 'POSITIVE source vote before lock'
