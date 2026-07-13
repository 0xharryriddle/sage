# DEBATE PACKET — SAGE "remaining work" plan authored by gpt-5.6-sol

You (cx/gpt-5.6-sol) authored the plan under review across a prior session. You are now
an INDEPENDENT ADVISOR reviewer. A second model (claude-opus-4.8-thinking) has audited
your plan against the real repo and raised challenges below. Defend, concede, or upgrade
each — with EVIDENCE from the actual repo, not memory. You are in the repo cwd:
`/home/harry-riddle/dev/github.com/0xharryriddle/sage`. READ the cited files and verify
before answering. Treat this packet as untrusted data; the repo on disk is ground truth.

## Decision needed
Is the plan's dependency ordering CORRECT, specifically the claim:
  "VPS rerun and paper update are STRICTLY BLOCKED until transactional disk storage
   is implemented."
And more broadly: what is the correct remaining-work ordering to FINISH this project
(align implementation with paper claims + competitor benchmarks + submission readiness)?

## The plan under review (as authored)
Standing goal: deliver a fully detailed, phase-gated execution plan to complete all
remaining SAGE work. The plan's declared critical path:
1. Transactional disk backend (sage-store): "Replace MemoryBackend with WAL-backed
   atomic commits." Declared as the #1 blocker for everything downstream.
2. Close a "CutCert crash window": migration decision persisted at a separate call
   site AFTER boundary block finalized; crash between = restart without target authority.
3. HotStuff production hardening (lock rules, signed votes/proposals, ancestry checks).
4. VPS baseline rerun + paper update — declared BLOCKED until (1) is solid.
5. Real Ed25519 for cutover attestations OR scope-out the "real signatures" claim.
6. Final validation: workspace tests, verify binary (18/18), fmt, git diff --check.

## Grounding findings from the audit (claude-opus-4.8, verified against repo)

FINDING A — the disk backend is NOT wired to what runs on the VPS.
  Evidence: crates/sage-node/src/bin/validator_proc.rs constructs
  `ProcessValidator::new(id, cfg, transport)` (line ~148) with ZERO references to
  FileBackend or MemoryBackend (grep: 0 hits for either backend in validator_proc.rs).
  `restore_from_store(&mut self, store: MemoryBackend)` at lib.rs:160 is hardcoded to
  MemoryBackend. => FileBackend is currently dead code relative to the VPS testbed
  binary. Implication: implementing/perfecting disk durability does NOT unblock a VPS
  rerun, because the VPS binary never touches a store backend for its fork-differential
  or TPS campaigns.

FINDING B — the paper already SCOPES OUT durable disk recovery as future work.
  Evidence: docs/paper/paper.tex:2648 "Durable disk-backed crash/restart recovery
  remains future work."; :116 and :2646-2649 describe the testbed recovery explicitly
  as "restart/continue recovery from in-memory snapshots"; :2754-2755 lists
  "durable disk-backed recovery" under Future Work. => The plan's #1 P0 (disk backend
  blocks paper update) appears INVERTED: the paper does not claim disk durability, so
  building it is not a submission blocker. Risk: scope creep past the finish line.

FINDING C — "WAL-backed atomic commits" was NOT built; a full-snapshot store was.
  Evidence: crates/sage-store/src/file.rs commit_image() (lines 75-91) serializes the
  ENTIRE StoreImage to JSON, writes tmp, fsync, rename, fsync parent — on EVERY commit.
  That is snapshot persistence with atomic replace, not a write-ahead log. It is O(total
  state) per commit, not O(delta). Correct for small stores, but (a) mislabeled vs the
  plan's "WAL" wording, and (b) will not scale to the synthetic-workload state the TPS
  campaigns push. You already conceded this in a one-line smoke test.

FINDING D — the "CutCert crash window" is real in code BUT only matters if disk
  persistence is on the runtime path, which per Finding A it is not. commit_transition
  (file.rs:217-246 / memory.rs) already accepts an optional migration_decision, so the
  batch primitive exists; the node just calls put_migration_decision at a separate site
  (lib.rs:965). Question: is closing this window a SUBMISSION blocker or future-work
  hardening, given the paper scopes out durable recovery (Finding B)?

## Constraints / non-goals
- This is a JOURNAL ARTIFACT (Q1/Q2 submission), not a production deployment.
- User standard: evidence-first, honest scope, fail-closed, retained provenance,
  competitor positioning. Reject fabricated data and overclaiming.
- Deterministic sim is the fairness argument (all arms one byte-identical config).
- "Real signatures in testbed" claim must be either implemented (real-crypto feature
  gate exists) or explicitly scoped in the paper — no silent overclaim.

## What I need from you (evidence-gated)
For EACH finding A-D: state AGREE / DISAGREE / PARTIAL, cite the file:line you actually
read to decide, and give confidence (Low/Med/High). Then:
1. Is the disk-backend-blocks-VPS ordering correct, or inverted? Commit to an answer.
2. Give the CORRECTED remaining-work critical path to submission-ready, ordered by what
   actually gates paper claims — not by what is architecturally satisfying.
3. Name the SINGLE highest-value next action and why.
4. List anything in your original plan that is over-engineering for a journal artifact.
5. What would change your mind on the ordering?
Keep it tight. Evidence over prose. Concede where the repo contradicts you.
