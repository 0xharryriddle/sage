#!/bin/bash
# TLA+ -> Rust CONFORMANCE state export (backs paper App. app:spec, answers R2-Q8).
#
# Exhaustively dumps TLC's reachable state space for the SAGE cutover model over
# n in {4,7,10} (faithful, QuorumGated=TRUE) and the blind broken control
# (QuorumGated=FALSE, n in {4,7}), then PROJECTS each state space to the set of
# distinct `committed` vectors (the decision OUTCOME the conformance test checks).
#
# The raw TLC dumps are enormous (n=10 alone is ~740MB of state text); we keep
# only the tiny projected committed-vector sets (a few KB each) under
# formal/states/committed/, which is what crates/sage-node/tests/formal_conformance.rs
# reads and compares against the Rust reference enumeration.
#
# The projection also self-checks the safety property (at most one non-zero side
# id per committed vector): faithful configs MUST yield ZERO unsafe vectors; the
# blind controls MUST yield >=1 unsafe vector (the falsifiability / "teeth" gate,
# mirroring the empirical M3 result where the blind HardFork forks 5/5).
#
# Usage: bash formal/dump_states.sh
set -u
cd "$(dirname "$0")"

JAR=tla2tools.jar
if [ ! -f "$JAR" ]; then
  echo "FATAL: $JAR not found in formal/" >&2
  exit 2
fi

mkdir -p states/committed
TMPCFG=$(mktemp -d)
trap 'rm -rf "$TMPCFG" states/*.dump' EXIT

# Emit a TLC config with NO invariants so exploration does not halt at the first
# counterexample (we want the FULL reachable state space, then project it).
gen_cfg() {
  local n=$1 f=$2 gated=$3 out=$4
  cat >"$out" <<CFG
SPECIFICATION Spec
CONSTANTS
    N = $n
    F = $f
    QuorumGated = $gated
CFG
}

# name n f gated
dump_one() {
  local name=$1 n=$2 f=$3 gated=$4
  local cfg="$TMPCFG/$name.cfg"
  gen_cfg "$n" "$f" "$gated" "$cfg"
  echo "=== dumping $name (n=$n f=$f QuorumGated=$gated) ==="
  # -dump appends .dump to the given path.
  java -cp "$JAR" tlc2.TLC -deadlock -workers 4 \
       -dump "states/$name" -config "$cfg" Sage.tla >/dev/null 2>&1
  if [ ! -f "states/$name.dump" ]; then
    echo "  [FAIL] $name: no dump produced" >&2
    return 1
  fi
}

for spec in "n4 4 1 TRUE" "n7 7 2 TRUE" "n10 10 3 TRUE" \
            "blind_n4 4 1 FALSE" "blind_n7 7 2 FALSE"; do
  # shellcheck disable=SC2086
  dump_one $spec || exit 1
done

# Project every dump to distinct committed vectors + safety self-check.
python3 - <<'PY'
import re, os, glob, sys
def committed_vectors(path):
    vecs=set()
    with open(path) as fh:
        for line in fh:
            m=re.search(r'committed = <<([^>]*)>>', line)
            if m:
                body=m.group(1).strip()
                vals=tuple(int(x) for x in body.split(',')) if body else ()
                vecs.add(vals)
    return vecs

def is_safe(vec):
    # Safety: no two validators committed a DIFFERENT boundary block (side id).
    return len({v for v in vec if v!=0}) <= 1

fail=0
for name in ['n4','n7','n10','blind_n4','blind_n7']:
    p=f'states/{name}.dump'
    if not os.path.exists(p):
        print(f'{name}: MISSING dump'); fail=1; continue
    vecs=committed_vectors(p)
    unsafe=[v for v in vecs if not is_safe(v)]
    outp=f'states/committed/committed_{name}.txt'
    with open(outp,'w') as o:
        for v in sorted(vecs):
            o.write(','.join(str(x) for x in v)+'\n')
    faithful = not name.startswith('blind')
    ok = (faithful and len(unsafe)==0) or ((not faithful) and len(unsafe)>0)
    tag = 'OK' if ok else 'UNEXPECTED'
    if not ok: fail=1
    print(f'  [{tag}] {name}: {len(vecs)} committed vectors, {len(unsafe)} unsafe (faithful={faithful})')
sys.exit(fail)
PY
rc=$?

echo ""
if [ "$rc" -eq 0 ]; then
  echo "STATE PROJECTIONS WRITTEN to formal/states/committed/ (5 configs)."
  echo "Faithful: 0 unsafe committed vectors. Blind controls: >=1 unsafe (gate has teeth)."
else
  echo "DUMP/PROJECTION FAILED -- see above." >&2
fi
exit $rc
