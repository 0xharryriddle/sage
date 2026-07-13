# Graph Report - .  (2026-07-05)

## Corpus Check
- 151 files · ~129,080 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 796 nodes · 1875 edges · 30 communities detected
- Extraction: 63% EXTRACTED · 37% INFERRED · 0% AMBIGUOUS · INFERRED: 690 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Experiment CLI & CSV harness|Experiment CLI & CSV harness]]
- [[_COMMUNITY_Core blockstate data model|Core block/state data model]]
- [[_COMMUNITY_Node runtime & transport|Node runtime & transport]]
- [[_COMMUNITY_Adversary & partition model|Adversary & partition model]]
- [[_COMMUNITY_Statistics (CI  hypothesis)|Statistics (CI / hypothesis)]]
- [[_COMMUNITY_HotStuff engine & quorum|HotStuff engine & quorum]]
- [[_COMMUNITY_Fork detection & figures|Fork detection & figures]]
- [[_COMMUNITY_Engine trait  PoA  shadow|Engine trait / PoA / shadow]]
- [[_COMMUNITY_Node config|Node config]]
- [[_COMMUNITY_Simulator config|Simulator config]]
- [[_COMMUNITY_Store & multi-host aggregation|Store & multi-host aggregation]]
- [[_COMMUNITY_Schedule & rollback|Schedule & rollback]]
- [[_COMMUNITY_SAGE controller & readiness|SAGE controller & readiness]]
- [[_COMMUNITY_Safety invariants|Safety invariants]]
- [[_COMMUNITY_In-memory transport|In-memory transport]]
- [[_COMMUNITY_Community 15|Community 15]]
- [[_COMMUNITY_Community 16|Community 16]]
- [[_COMMUNITY_Community 17|Community 17]]
- [[_COMMUNITY_Community 18|Community 18]]
- [[_COMMUNITY_Community 19|Community 19]]
- [[_COMMUNITY_Community 20|Community 20]]
- [[_COMMUNITY_Community 22|Community 22]]
- [[_COMMUNITY_Community 23|Community 23]]
- [[_COMMUNITY_Community 24|Community 24]]
- [[_COMMUNITY_Community 25|Community 25]]
- [[_COMMUNITY_Community 26|Community 26]]
- [[_COMMUNITY_Community 27|Community 27]]
- [[_COMMUNITY_Community 28|Community 28]]
- [[_COMMUNITY_Community 29|Community 29]]
- [[_COMMUNITY_Community 30|Community 30]]

## God Nodes (most connected - your core abstractions)
1. `main()` - 19 edges
2. `TcpTransport` - 18 edges
3. `HotStuffEngine` - 17 edges
4. `NodeValidator` - 17 edges
5. `write_csv()` - 17 edges
6. `MemoryBackend` - 16 edges
7. `main()` - 15 edges
8. `main()` - 14 edges
9. `hash_domain()` - 14 edges
10. `Simulation` - 13 edges

## Surprising Connections (you probably didn't know these)
- `main()` --calls--> `check_all()`  [INFERRED]
  /home/harry-riddle/dev/github.com/0xharryriddle/sage/crates/sage-experiments/src/bin/verify.rs → /home/harry-riddle/dev/github.com/0xharryriddle/sage/crates/sage-controller/src/invariants.rs
- `compute_downtime_stats()` --calls--> `holm_bonferroni()`  [INFERRED]
  /home/harry-riddle/dev/github.com/0xharryriddle/sage/crates/sage-experiments/src/bin/run_rq1.rs → /home/harry-riddle/dev/github.com/0xharryriddle/sage/crates/sage-stats/src/correction.rs
- `compute_downtime_stats()` --calls--> `mann_whitney_u()`  [INFERRED]
  /home/harry-riddle/dev/github.com/0xharryriddle/sage/crates/sage-experiments/src/bin/run_rq1.rs → /home/harry-riddle/dev/github.com/0xharryriddle/sage/crates/sage-stats/src/hypothesis.rs
- `main()` --calls--> `wilson_interval()`  [INFERRED]
  /home/harry-riddle/dev/github.com/0xharryriddle/sage/crates/sage-experiments/src/bin/run_rq3.rs → /home/harry-riddle/dev/github.com/0xharryriddle/sage/crates/sage-stats/src/ci.rs
- `main()` --calls--> `wilson_interval()`  [INFERRED]
  /home/harry-riddle/dev/github.com/0xharryriddle/sage/crates/sage-experiments/src/bin/run_termination.rs → /home/harry-riddle/dev/github.com/0xharryriddle/sage/crates/sage-stats/src/ci.rs

## Communities

### Community 0 - "Experiment CLI & CSV harness"
Cohesion: 0.04
Nodes (66): command_version(), load_config(), run_single(), stable_config_hash(), write_csv(), write_metadata(), ExperimentError, block_at() (+58 more)

### Community 1 - "Core block/state data model"
Cohesion: 0.04
Nodes (47): Block, BlockHeader, FinalizedBlock, header(), rejects_wrong_parent_link(), Transaction, validates_parent_link(), Certificate (+39 more)

### Community 2 - "Node runtime & transport"
Cohesion: 0.07
Nodes (31): build_validator(), four_validator_poa(), four_validator_sage_finalizes_after_cutover(), four_validator_sage_migration_reaches_cutover(), genesis_state(), hex_encode(), LocalRuntime, multi_process_validators_agree_across_cutover() (+23 more)

### Community 3 - "Adversary & partition model"
Cohesion: 0.05
Nodes (27): AdversaryAction, AdversaryModel, partition_blocks_cross_group_messages(), PartitionState, MetricsRecorder, RunMetrics, drops_cross_partition_message(), NetworkModel (+19 more)

### Community 4 - "Statistics (CI / hypothesis)"
Cohesion: 0.06
Nodes (49): bootstrap_ci_for_constant_values_is_tight(), Interval, mean_ci_bootstrap(), norm_quantile(), wilson_ci_for_all_successes_is_bounded(), wilson_ci_for_zero_successes_is_bounded(), wilson_interval(), ExperimentCli (+41 more)

### Community 5 - "HotStuff engine & quorum"
Cohesion: 0.09
Nodes (15): HotStuffEngine, HotStuffMessage, leader_proposes_provisional_block(), no_timeout_advance_below_threshold(), QuorumCertificate, timeout_certificate_formation(), timeout_idempotent(), TimeoutCertificate (+7 more)

### Community 6 - "Fork detection & figures"
Cohesion: 0.09
Nodes (38): ForkReport, Cli, generate_if_exists(), M4Row, M5Row, M6Row, main(), mean() (+30 more)

### Community 7 - "Engine trait / PoA / shadow"
Cohesion: 0.08
Nodes (17): BootstrapAnchor, ConsensusEngine, FinalizationCertificate, ProposeContext, ShadowVerdict, detects_root_mismatch(), ShadowValidator, local_proposer_builds_block_with_expected_root() (+9 more)

### Community 8 - "Node config"
Cohesion: 0.08
Nodes (21): NodeConfig, NodeStrategy, AdjustedDecision, holm_bonferroni(), holm_correction_all_significant(), holm_correction_empty(), holm_correction_none_significant(), Event (+13 more)

### Community 9 - "Simulator config"
Cohesion: 0.07
Nodes (11): AdversaryConfig, SimConfig, SimulationConfig, ValidatorsConfig, WorkloadConfig, engine_id_equality_includes_generation(), EngineId, EngineKind (+3 more)

### Community 10 - "Store & multi-host aggregation"
Cohesion: 0.14
Nodes (12): main(), conflicting_vote_for_same_validator_engine_view_is_rejected(), dummy_block(), dummy_cert(), highest_block_tracks_blocks(), MemoryBackend, missing_block_errors(), put_and_get_block() (+4 more)

### Community 11 - "Schedule & rollback"
Cohesion: 0.14
Nodes (19): AbortTrigger, anchor(), certificate(), finalized_block(), refuses_rollback_at_deadline(), replay_block(), replay_context_root_commits_to_metadata(), replay_context_size_is_per_block_and_bounded() (+11 more)

### Community 12 - "SAGE controller & readiness"
Cohesion: 0.11
Nodes (9): emits_cutover_height_after_readiness(), SageController, emits_payload_after_kappa(), invalid_record_resets_streak(), ReadinessContext, ReadinessTracker, rec(), ShadowRecord (+1 more)

### Community 13 - "Safety invariants"
Cohesion: 0.13
Nodes (21): block(), check_all(), check_cutcert_unique(), check_no_absolute_reversion(), check_no_conflicting_commits(), check_shadow_never_finalizes(), check_single_finalizer(), CommittedBlockSummary (+13 more)

### Community 14 - "In-memory transport"
Cohesion: 0.31
Nodes (6): broadcast_reaches_all_peers(), dummy_envelope(), InMemoryTransport, partition_isolates_sides_and_heals(), recv_drains_queue(), unknown_peer_errors()

### Community 15 - "Community 15"
Cohesion: 0.31
Nodes (9): compare(), csv_map(), extract_coords(), load_paper(), main(), MissingAnchor, The figure anchor is absent from this paper variant (skip, not fail)., Find the first `coordinates { ... }` block at/after `anchor` and parse     its ( (+1 more)

### Community 16 - "Community 16"
Cohesion: 0.25
Nodes (7): BlockStore, CertificateKind, CertificateStore, ManifestStore, SafetyStore, StateStore, VoteRecord

### Community 17 - "Community 17"
Cohesion: 0.25
Nodes (1): MigrationPhase

### Community 18 - "Community 18"
Cohesion: 0.33
Nodes (5): ClassRow, default_model(), main(), SummaryRow, TxClass

### Community 19 - "Community 19"
Cohesion: 0.33
Nodes (3): SignatureEnvelope, SignatureScheme, SimulatedSignatureScheme

### Community 20 - "Community 20"
Cohesion: 0.5
Nodes (3): ConsensusMessage, MessageEnvelope, SageMessage

### Community 22 - "Community 22"
Cohesion: 0.67
Nodes (2): ManifestVerifier, VerificationContext

### Community 23 - "Community 23"
Cohesion: 1.0
Nodes (1): StatsError

### Community 24 - "Community 24"
Cohesion: 1.0
Nodes (1): SimError

### Community 25 - "Community 25"
Cohesion: 1.0
Nodes (1): NetworkError

### Community 26 - "Community 26"
Cohesion: 1.0
Nodes (1): ConsensusError

### Community 27 - "Community 27"
Cohesion: 1.0
Nodes (1): ManifestError

### Community 28 - "Community 28"
Cohesion: 1.0
Nodes (1): CoreError

### Community 29 - "Community 29"
Cohesion: 1.0
Nodes (1): StoreError

### Community 30 - "Community 30"
Cohesion: 1.0
Nodes (1): ControllerError

## Knowledge Gaps
- **107 isolated node(s):** `The figure anchor is absent from this paper variant (skip, not fail).`, `Find the first `coordinates { ... }` block at/after `anchor` and parse     its (`, `StatsError`, `AdjustedDecision`, `MannWhitneyResult` (+102 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **Thin community `Community 17`** (8 nodes): `phase.rs`, `activating_can_abort_to_rollback()`, `cutover_decision_does_not_transfer_authority_directly()`, `legacy_authoritative_through_activation()`, `MigrationPhase`, `.can_transition_to()`, `.legacy_is_authoritative()`, `rejects_direct_v1_to_v2_transition()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 22`** (3 nodes): `verify.rs`, `ManifestVerifier`, `VerificationContext`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 23`** (2 nodes): `error.rs`, `StatsError`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 24`** (2 nodes): `error.rs`, `SimError`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 25`** (2 nodes): `error.rs`, `NetworkError`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 26`** (2 nodes): `error.rs`, `ConsensusError`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 27`** (2 nodes): `error.rs`, `ManifestError`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 28`** (2 nodes): `error.rs`, `CoreError`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 29`** (2 nodes): `error.rs`, `StoreError`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 30`** (2 nodes): `error.rs`, `ControllerError`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `hash_domain()` connect `Core block/state data model` to `Experiment CLI & CSV harness`, `HotStuff engine & quorum`, `Fork detection & figures`, `Engine trait / PoA / shadow`, `Schedule & rollback`?**
  _High betweenness centrality (0.032) - this node is a cross-community bridge._
- **Why does `main()` connect `Node runtime & transport` to `Experiment CLI & CSV harness`, `Core block/state data model`, `Statistics (CI / hypothesis)`, `Fork detection & figures`, `Node config`, `Store & multi-host aggregation`?**
  _High betweenness centrality (0.023) - this node is a cross-community bridge._
- **Are the 15 inferred relationships involving `main()` (e.g. with `.new()` and `.get()`) actually correct?**
  _`main()` has 15 INFERRED edges - model-reasoned connections that need verification._
- **Are the 15 inferred relationships involving `write_csv()` (e.g. with `main()` and `main()`) actually correct?**
  _`write_csv()` has 15 INFERRED edges - model-reasoned connections that need verification._
- **What connects `The figure anchor is absent from this paper variant (skip, not fail).`, `Find the first `coordinates { ... }` block at/after `anchor` and parse     its (`, `StatsError` to the rest of the system?**
  _107 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Experiment CLI & CSV harness` be split into smaller, more focused modules?**
  _Cohesion score 0.04 - nodes in this community are weakly interconnected._
- **Should `Core block/state data model` be split into smaller, more focused modules?**
  _Cohesion score 0.04 - nodes in this community are weakly interconnected._