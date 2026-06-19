# BFT Consensus and Reconfiguration Research

This document summarizes consensus and reconfiguration literature that should guide SAGE's Rust implementation.

## 1. PBFT

Sources:

- Castro and Liskov, Practical Byzantine Fault Tolerance, OSDI 1999: https://www.usenix.org/conference/osdi-99/practical-byzantine-fault-tolerance
- PDF: https://css.csail.mit.edu/6.824/2014/papers/castro-practicalbft.pdf

### Implementation-Relevant Ideas

- Classic BFT threshold: `n >= 3f + 1`.
- Commit quorums rely on `2f + 1` matching messages.
- Normal path has pre-prepare, prepare, and commit phases.
- View change is the most complicated part because prepared certificates must be carried into the new view.
- Checkpoints and watermarks bound log growth and support recovery.
- Clients wait for `f + 1` matching replies to ensure at least one correct replica replied.

### SAGE Implications

- Implement certificates as first-class typed objects.
- Add stable checkpoints before state transfer or validator changes.
- Keep view-change and recovery metadata persistent.
- Use PBFT as a safety baseline, but avoid PBFT-style complex view change where HotStuff-style chained QCs are simpler.

## 2. HotStuff

Sources:

- HotStuff paper: https://arxiv.org/abs/1803.05069
- ACM DOI: https://dl.acm.org/doi/10.1145/3293611.3331591

### Implementation-Relevant Ideas

- Leader-based BFT with linear communication under threshold or aggregate signatures.
- Quorum certificate (QC) is the core object.
- Safety depends on locks and chained QCs.
- A common rule commits a block when it is the grandparent of a certified 3-chain.
- Pacemaker is separate from safety and should only affect liveness.
- Replicas must persist highest voted view, locked QC, highest QC, and committed block.

### SAGE Implications

- Use HotStuff-like QCs for the target engine.
- Separate block tree, vote aggregation, pacemaker, and safety rules in code.
- The target bootstrap must install a boundary QC or genesis anchor from the SAGE manifest.
- Crash recovery must not allow double voting after restart.

## 3. Tendermint

Sources:

- Tendermint paper: https://arxiv.org/abs/1807.04938
- Tendermint docs: https://docs.tendermint.com/master/introduction/what-is-tendermint.html

### Implementation-Relevant Ideas

- Height/round protocol with propose, prevote, precommit, and commit steps.
- A block commits with `2/3+` precommits at a given height/round.
- Locking and unlocking rules are critical to safety.
- Validator-set changes are usually applied at height boundaries.

### SAGE Implications

- If SAGE later supports a Tendermint target or source, locks must be persisted and epoch-tagged.
- Validator-set and engine-generation changes should activate at deterministic boundaries.
- Round-step state machines are easier to inspect than deeply asynchronous protocol code.

## 4. Flexible BFT

Sources:

- Flexible BFT paper: https://arxiv.org/abs/1904.10067
- ACM DOI: https://dl.acm.org/doi/10.1145/3319535.3354225

### Implementation-Relevant Ideas

- Safety depends on explicit quorum intersection properties, not one universal quorum constant.
- Different phases can use different quorums if required intersections hold.
- Quorum policy should be a first-class object.

### SAGE Implications

- Do not hardcode `2f + 1` everywhere.
- Model quorum rules as typed policies: majority, BFT, weighted BFT, migration cutover quorum, abort quorum.
- Validate quorum-intersection requirements at startup/config load.

## 5. SMR Reconfiguration and Dynamic BFT

Sources:

- Lamport, Reconfiguring a State Machine: https://lamport.azurewebsites.net/pubs/reconfiguration-tutorial.pdf
- Vertical Paxos: https://lamport.azurewebsites.net/pubs/vertical-paxos.pdf
- Foundations of Dynamic BFT: https://eprint.iacr.org/2022/597

### Implementation-Relevant Ideas

- Reconfiguration is an ordered safety problem, not just a metadata update.
- Decisions in old configurations must be discoverable or preserved by new configurations.
- Common approaches include ordered config changes, epoch barriers, joint consensus, and external masters.
- New replicas must install certified state before voting.
- Removed replicas' new votes must be rejected while their historical votes remain verifiable.

### SAGE Implications

- Every proposal, vote, QC, checkpoint, manifest, and client request must include `epoch` or `config_id`.
- SAGE should treat engine migration as an epoch transition with a certified boundary.
- New validators or target-engine voters should not vote until state sync and manifest verification complete.

## 6. BFT-SMaRt

Sources:

- BFT-SMaRt paper PDF: https://www.di.fc.ul.pt/~bessani/publications/dsn14-bftsmart.pdf

### Implementation-Relevant Ideas

- Practical BFT library with clean separation of total order multicast, state transfer, durable execution, and reconfiguration.
- Supports BFT and CFT modes.
- Reconfiguration uses signed administrative operations ordered through the protocol.
- Clients track current view and replicas reject stale-view requests.

### SAGE Implications

- Keep state transfer independently testable.
- Treat migration and reconfiguration as ordered administrative commands.
- Add client-side epoch/view tracking to avoid stale assumptions.
- Prefer hash-only votes after block/data dissemination to reduce bandwidth.

## 7. Sui Lutris

Sources:

- Sui Lutris paper: https://arxiv.org/abs/2310.18042
- Sui research papers: https://docs.sui.io/references/research-papers

### Implementation-Relevant Ideas

- Combines consensusless broadcast/certification with consensus ordering.
- Independent transactions can finalize without total-order consensus.
- Conflicting transactions are routed through consensus.
- Object-centric dependencies enable parallel execution and conflict detection.

### SAGE Implications

- Future SAGE engines could support fast paths for non-conflicting operations.
- Certificates should include object/resource versions if SAGE adopts an object model.
- Reconfiguration must preserve both consensus and consensusless safety.

## 8. DAG-BFT Systems

Sources:

- DAG-Rider: https://arxiv.org/abs/2102.08325
- Narwhal and Tusk PDF: https://aptoslabs.com/pdf/2105.11827.pdf
- Bullshark: https://arxiv.org/abs/2201.05677
- DAG Meets BFT overview: https://decentralizedthoughts.github.io/2022-06-28-DAG-meets-BFT/

### Implementation-Relevant Ideas

- DAG-BFT separates data dissemination from ordering.
- Narwhal provides an availability DAG; ordering protocols consume the DAG.
- Bullshark improves the synchronous common case.
- Hard parts include certificate verification, equivocation, garbage collection, deterministic ordering, and epoch transitions.

### SAGE Implications

- Keep SAGE's engine abstraction broad enough for DAG-BFT targets.
- Certificates should commit to author, round, parents, epoch, and batch digest.
- Reconfiguration should occur at certified round/epoch boundaries.

## 9. Concrete Requirements for SAGE

- Use `epoch` and `config_id` everywhere.
- Make quorum logic pluggable and verified.
- Persist safety-critical vote/lock/QC state before sending votes.
- Do not allow cross-epoch votes unless explicitly permitted.
- Model checkpoints and state transfer early, even in the simulator.
- Add adversarial tests for equivocation, stale epochs, lock violations, conflicting reconfigs, partial state transfer, and validator removal.
