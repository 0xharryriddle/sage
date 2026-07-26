#!/usr/bin/env python3
"""Run a fail-closed, cryptographically finalized local client campaign.

This harness is development evidence, never publishable topology evidence. It
spawns real validator/client processes over loopback, preserves raw streams and
provenance, and independently verifies request-bound finality certificates,
client/schedule reconciliation, and telemetry before sealing a trial.
"""

import argparse
import hashlib
import json
import os
import pathlib
import shutil
import signal
import socket
import subprocess
import sys
import time
from typing import Any, NoReturn

ROOT = pathlib.Path(__file__).resolve().parents[1]


def fail(message: str) -> NoReturn:
    raise RuntimeError(message)


def free_addresses(count: int) -> list[str]:
    sockets: list[socket.socket] = []
    try:
        for _ in range(count):
            sock = socket.socket()
            sock.bind(("127.0.0.1", 0))
            sockets.append(sock)
        return [f"127.0.0.1:{sock.getsockname()[1]}" for sock in sockets]
    finally:
        for sock in sockets:
            sock.close()


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json_atomic(path: pathlib.Path, value: dict[str, Any]) -> None:
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")
    temporary.replace(path)


def run_checked(command: list[str], stdout: pathlib.Path, stderr: pathlib.Path) -> None:
    with stdout.open("wb") as out, stderr.open("wb") as err:
        result = subprocess.run(command, cwd=ROOT, stdout=out, stderr=err, check=False)
    if result.returncode != 0:
        fail(f"command failed ({result.returncode}): {' '.join(command)}; see {stderr}")


def start_process(command: list[str], stdout: pathlib.Path, stderr: pathlib.Path) -> subprocess.Popen[bytes]:
    out = stdout.open("wb")
    err = stderr.open("wb")
    try:
        process = subprocess.Popen(
            command,
            cwd=ROOT,
            stdout=out,
            stderr=err,
            start_new_session=True,
        )
    finally:
        out.close()
        err.close()
    return process


def stop_all(processes: list[subprocess.Popen[bytes]]) -> None:
    for process in processes:
        if process.poll() is None:
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
    deadline = time.monotonic() + 3
    for process in processes:
        remaining = max(0.0, deadline - time.monotonic())
        try:
            process.wait(timeout=remaining)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()


def wait_all(processes: list[subprocess.Popen[bytes]], timeout: float, label: str) -> None:
    deadline = time.monotonic() + timeout
    for process in processes:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            fail(f"{label} exceeded campaign timeout")
        try:
            code = process.wait(timeout=remaining)
        except subprocess.TimeoutExpired as error:
            raise RuntimeError(f"{label} exceeded campaign timeout") from error
        if code != 0:
            fail(f"{label} process {process.pid} exited with {code}")


def json_result(path: pathlib.Path) -> dict[str, Any]:
    objects: list[dict[str, Any]] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as error:
            fail(f"{path}:{line_number}: invalid validator JSON: {error}")
        if isinstance(value, dict):
            objects.append(value)
    if len(objects) != 1:
        fail(f"{path}: expected exactly one ProcessResult, found {len(objects)}")
    return objects[0]


def concatenate(inputs: list[pathlib.Path], output: pathlib.Path) -> None:
    with output.open("wb") as destination:
        for path in inputs:
            destination.write(path.read_bytes())


def jsonl_objects(path: pathlib.Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for line_number, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not raw.strip():
            fail(f"{path}:{line_number}: blank JSONL row")
        try:
            value = json.loads(raw)
        except json.JSONDecodeError as error:
            fail(f"{path}:{line_number}: invalid JSON: {error}")
        if not isinstance(value, dict):
            fail(f"{path}:{line_number}: expected JSON object")
        rows.append(value)
    return rows


def measurement_summary(schedule: pathlib.Path) -> dict[str, Any]:
    rows = [row for row in jsonl_objects(schedule) if row.get("phase") == "measurement"]
    if not rows:
        fail("measurement window contains no scheduled requests")
    finalized = sum(row.get("outcome") == "observed_finalized" for row in rows)
    scheduled = len(rows)
    nonfinalized = scheduled - finalized
    outcomes: dict[str, int] = {}
    for row in rows:
        outcome = row.get("outcome")
        if not isinstance(outcome, str):
            fail("measurement schedule contains a malformed outcome")
        outcome_name: str = outcome
        outcomes[outcome_name] = outcomes.get(outcome_name, 0) + 1
    return {
        "scheduled_requests": scheduled,
        "finalized_requests": finalized,
        "nonfinalized_requests": nonfinalized,
        "success_fraction": finalized / scheduled,
        "outcomes": outcomes,
    }


def observed_fault_events(results: list[dict[str, Any]]) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    for result in results:
        validator = result.get("validator")
        for event in result.get("phase_events", []):
            if isinstance(event, dict) and event.get("name") == "partition_engaged":
                events.append(
                    {
                        "name": "partition_engaged",
                        "validator": validator,
                        "elapsed_micros": event.get("elapsed_micros"),
                        "height": event.get("height"),
                    }
                )
    return sorted(events, key=lambda event: int(event.get("validator", -1)))


def observed_safety_violation(results: list[dict[str, Any]]) -> bool:
    roots_by_height: dict[int, set[str]] = {}
    for result in results:
        for detail in result.get("committed_detail", []):
            if not isinstance(detail, list) or len(detail) < 2:
                fail("validator result contains malformed committed_detail")
            height, root = detail[0], detail[1]
            if not isinstance(height, int) or not isinstance(root, str):
                fail("validator result contains malformed committed identity")
            roots_by_height.setdefault(height, set()).add(root)
    return any(len(roots) > 1 for roots in roots_by_height.values())


def partition_keep(validator_id: int, validator_count: int) -> str:
    if validator_count % 2 != 0:
        fail("balanced partition diagnostic requires an even validator count")
    split = validator_count // 2
    side = range(0, split) if validator_id < split else range(split, validator_count)
    return ",".join(str(member) for member in side)


def provenance(binary_validator: pathlib.Path, binary_client: pathlib.Path, contract: pathlib.Path) -> dict[str, Any]:
    commit = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True, capture_output=True, check=True
    ).stdout.strip()
    diff = subprocess.run(
        ["git", "diff", "--binary", "HEAD"], cwd=ROOT, capture_output=True, check=True
    ).stdout
    return {
        "git_commit": commit,
        "git_diff_sha256": hashlib.sha256(diff).hexdigest(),
        "cargo_lock_sha256": sha256(ROOT / "Cargo.lock"),
        "validator_binary_sha256": sha256(binary_validator),
        "client_binary_sha256": sha256(binary_client),
        "contract_sha256": sha256(contract),
    }


def host_load() -> dict[str, Any]:
    """Capture host contention at measurement time.

    Latency/throughput numbers from a co-tenanted box are not a property of the
    system under test: an identical (rate, seed, contract) trial can move by 2x
    when other processes compete for cores. Code provenance alone cannot detect
    that after the fact, so every trial records the load it was measured under.
    """
    try:
        one, five, fifteen = os.getloadavg()
    except OSError:
        return {"available": False}
    cores = os.cpu_count() or 1
    return {
        "available": True,
        "loadavg_1m": round(one, 2),
        "loadavg_5m": round(five, 2),
        "loadavg_15m": round(fifteen, 2),
        "cpu_count": cores,
        "load_per_core_1m": round(one / cores, 3),
    }


def run_trial(
    args: argparse.Namespace, rate: int, trial_root: pathlib.Path
) -> dict[str, Any]:
    trial_root.mkdir(parents=True)
    load = host_load()
    if (
        args.max_load_per_core > 0.0
        and load.get("available")
        and load["load_per_core_1m"] > args.max_load_per_core
    ):
        fail(
            f"host too contended to measure: load_per_core_1m="
            f"{load['load_per_core_1m']} exceeds --max-load-per-core "
            f"{args.max_load_per_core} (loadavg_1m={load['loadavg_1m']}, "
            f"cpu_count={load['cpu_count']}); latency on a co-tenanted host is "
            f"not a property of the system under test"
        )
    peer_addresses = free_addresses(args.validators)
    client_addresses = free_addresses(args.validators)
    peers = ",".join(peer_addresses)
    clients = ",".join(client_addresses)
    start_ms = int(time.time() * 1000) + args.barrier_secs * 1000
    total_secs = args.warmup_secs + args.measurement_secs + args.cooldown_secs
    validator_runtime = total_secs + args.validator_grace_secs
    processes: list[subprocess.Popen[bytes]] = []
    validator_outputs: list[pathlib.Path] = []
    request_parts: list[pathlib.Path] = []
    schedule_parts: list[pathlib.Path] = []
    try:
        for validator_id in range(args.validators):
            stdout = trial_root / f"validator_{validator_id}.stdout.jsonl"
            stderr = trial_root / f"validator_{validator_id}.stderr.log"
            validator_outputs.append(stdout)
            command = [
                str(args.validator_binary),
                "--id", str(validator_id),
                "--n", str(args.validators),
                "--f", str(args.fault_bound),
                "--peers", peers,
                "--strategy", args.strategy,
                "--max-height", str(1_000_000_000),
                "--h-d", "2", "--h-c", "4", "--h-r", "8",
                "--max-secs", str(validator_runtime),
                "--seed", str(args.seed),
                "--start-at-unix-ms", str(start_ms),
                "--client-listen", client_addresses[validator_id],
                "--client-capacity", str(args.client_capacity),
                "--client-batch-limit", str(args.client_batch_limit),
                "--client-ingress-workers", str(args.ingress_workers),
                "--client-ingress-queue", str(args.ingress_queue),
                "--client-ready-delay-ms", "0",
                "--proposal-interval-ms", str(args.proposal_interval_ms),
                "--store-path", str(trial_root / f"validator_{validator_id}.store.json"),
            ]
            if args.strategy == "sage":
                command.append("--cutover-quorum")
            if args.scenario == "balanced_partition_safety":
                command.extend(
                    [
                        "--partition-keep",
                        partition_keep(validator_id, args.validators),
                        "--partition-at",
                        str(args.partition_at_height),
                    ]
                )
            processes.append(start_process(command, stdout, stderr))

        for client_id in range(args.clients):
            requests = trial_root / f"client_{client_id}.requests.jsonl"
            schedule = trial_root / f"client_{client_id}.schedule.jsonl"
            stdout = trial_root / f"client_{client_id}.stdout.log"
            stderr = trial_root / f"client_{client_id}.stderr.log"
            request_parts.append(requests)
            schedule_parts.append(schedule)
            command = [
                str(args.client_binary),
                "--validators", clients,
                "--campaign-id", args.campaign_id,
                "--trial-id", trial_root.name,
                "--client-id", str(client_id),
                "--client-count", str(args.clients),
                "--offered-load-tps", str(rate),
                "--warmup-secs", str(args.warmup_secs),
                "--measurement-secs", str(args.measurement_secs),
                "--cooldown-secs", str(args.cooldown_secs),
                "--payload-bytes", str(args.payload_bytes),
                "--receipt-quorum", str(args.receipt_quorum),
                "--workers", str(args.client_workers),
                "--queue-capacity", str(args.client_queue),
                "--poll-ms", str(args.poll_ms),
                "--chain-id", args.chain_id,
                "--epoch", str(args.epoch),
                "--config-id", str(args.config_id),
                "--key-seed", str(args.seed),
                "--start-at-unix-ms", str(start_ms),
                "--request-output", str(requests),
                "--schedule-output", str(schedule),
            ]
            processes.append(start_process(command, stdout, stderr))

        client_processes = processes[args.validators:]
        validator_processes = processes[:args.validators]
        wait_all(client_processes, total_secs + args.barrier_secs + 15, "client")
        wait_all(validator_processes, validator_runtime + args.barrier_secs + 20, "validator")

        validator_results = [json_result(path) for path in validator_outputs]
        validator_ids = {result.get("validator") for result in validator_results}
        if validator_ids != set(range(args.validators)):
            fail(f"validator result set mismatch: {validator_ids!r}")
        if args.scenario == "steady_state" and any(
            result.get("migration_success") is not True for result in validator_results
        ):
            fail("one or more validators did not complete migration")
        requests = trial_root / "requests.jsonl"
        schedule = trial_root / "schedule.jsonl"
        concatenate(request_parts, requests)
        concatenate(schedule_parts, schedule)
        if not requests.stat().st_size:
            fail("no request reached a socket write; trial is invalid")

        verifier_log = trial_root / "verification.log"
        verifier_error = trial_root / "verification.stderr.log"
        telemetry_command = [
            "python3",
            "scripts/verify_process_telemetry.py",
            *map(str, validator_outputs),
            "--expected-validators",
            str(args.validators),
        ]
        if args.scenario == "balanced_partition_safety":
            telemetry_command.extend(
                ["--allow-incomplete-phases", "--require-phase", "partition_engaged"]
            )
        commands = [
            ["python3", "scripts/verify_open_loop_schedule.py", str(schedule),
             "--offered-load-tps", str(rate), "--warmup-secs", str(args.warmup_secs),
             "--measurement-secs", str(args.measurement_secs), "--expected-clients", str(args.clients)],
            ["python3", "scripts/verify_client_results.py", str(requests),
             "--validator-count", str(args.validators),
             "--receipt-quorum", str(args.receipt_quorum),
             "--chain-id", args.chain_id, "--epoch", str(args.epoch),
             "--config-id", str(args.config_id), "--key-seed", str(args.seed)],
            ["python3", "scripts/verify_client_campaign.py", "--schedule", str(schedule),
             "--requests", str(requests), "--campaign-id", args.campaign_id,
             "--trial-id", trial_root.name, "--client-count", str(args.clients),
             "--validator-count", str(args.validators), "--receipt-quorum", str(args.receipt_quorum)],
            telemetry_command,
        ]
        with verifier_log.open("wb") as out, verifier_error.open("wb") as err:
            for command in commands:
                result = subprocess.run(command, cwd=ROOT, stdout=out, stderr=err, check=False)
                if result.returncode != 0:
                    fail(f"verification failed: {' '.join(command)}")

        measurement = measurement_summary(schedule)
        if args.scenario == "steady_state" and measurement["finalized_requests"] == 0:
            fail("steady-state measurement contains no cryptographically finalized requests")
        overloaded = (
            measurement["nonfinalized_requests"] > 0
            and measurement["success_fraction"]
            < args.overload_success_fraction_threshold
        )
        fault_events = observed_fault_events(validator_results)
        safety_violation = observed_safety_violation(validator_results)
        if args.scenario == "steady_state" and fault_events:
            fail("steady-state trial unexpectedly observed fault events")
        if args.scenario == "balanced_partition_safety" and len(fault_events) != args.validators:
            fail("partition trial did not observe one partition event per validator")
        if safety_violation:
            fail("cross-validator committed state roots reveal a safety violation")
        result = {
            "schema_version": 1,
            "campaign_id": args.campaign_id,
            "trial_id": trial_root.name,
            "rate_tps": rate,
            "seed": args.seed,
            "strategy": args.strategy,
            "scenario": args.scenario,
            "status": "verified_cryptographic_finality",
            "publishable": False,
            "cryptographic_finality_available": True,
            "validator_count": args.validators,
            "client_count": args.clients,
            "receipt_quorum": args.receipt_quorum,
            "request_rows": sum(1 for _ in requests.open(encoding="utf-8")),
            "schedule_rows": sum(1 for _ in schedule.open(encoding="utf-8")),
            "measurement": measurement,
            "overloaded": overloaded,
            "fault_events": fault_events,
            "safety_violation": safety_violation,
            "host_load": load,
        }
        (trial_root / "trial_result.json").write_text(json.dumps(result, indent=2) + "\n")
        return {
            "rate_tps": rate,
            "seed": args.seed,
            "strategy": args.strategy,
            "scenario": args.scenario,
            "status": "verified",
            "artifact_dir": trial_root.name,
            "overloaded": overloaded,
        }
    finally:
        stop_all(processes)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--campaign-id", required=True)
    parser.add_argument("--output-root", type=pathlib.Path, required=True)
    parser.add_argument("--rates", type=int, nargs="+", required=True)
    parser.add_argument("--warmup-secs", type=int, default=2)
    parser.add_argument("--measurement-secs", type=int, default=5)
    parser.add_argument("--cooldown-secs", type=int, default=2)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--chain-id", default="sage-local")
    parser.add_argument("--epoch", type=int, default=1)
    parser.add_argument("--config-id", type=int, default=1)
    parser.add_argument("--validators", type=int, default=4)
    parser.add_argument("--fault-bound", type=int, default=1)
    parser.add_argument("--clients", type=int, default=2)
    parser.add_argument("--receipt-quorum", type=int, default=3)
    parser.add_argument("--strategy", default="sage")
    parser.add_argument(
        "--scenario",
        choices=("steady_state", "balanced_partition_safety"),
        default="steady_state",
    )
    parser.add_argument("--partition-at-height", type=int, default=4)
    parser.add_argument("--required-overload-points", type=int, default=2)
    parser.add_argument(
        "--overload-success-fraction-threshold", type=float, default=0.95
    )
    parser.add_argument("--payload-bytes", type=int, default=512)
    # Default 0.0 = record load but do not gate, preserving the documented
    # REPRODUCE.md invocation. Publishable latency sweeps should pass a
    # threshold (0.5 is a reasonable quiet-host bar) so a contended run fails
    # closed instead of producing numbers that move 2x between identical trials.
    parser.add_argument("--max-load-per-core", type=float, default=0.0)
    parser.add_argument("--client-workers", type=int, default=64)
    parser.add_argument("--client-queue", type=int, default=4096)
    parser.add_argument("--client-capacity", type=int, default=100000)
    parser.add_argument("--client-batch-limit", type=int, default=400)
    parser.add_argument("--ingress-workers", type=int, default=16)
    parser.add_argument("--ingress-queue", type=int, default=4096)
    parser.add_argument("--poll-ms", type=int, default=10)
    parser.add_argument("--proposal-interval-ms", type=int, default=100)
    parser.add_argument("--barrier-secs", type=int, default=3)
    parser.add_argument("--validator-grace-secs", type=int, default=3)
    parser.add_argument("--validator-binary", type=pathlib.Path,
                        default=ROOT / "target/release/validator_proc")
    parser.add_argument("--client-binary", type=pathlib.Path,
                        default=ROOT / "target/release/client_proc")
    parser.add_argument("--contract", type=pathlib.Path,
                        default=ROOT / "results/manifests/sota_client_benchmark_contract_v1.json")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    cleanup_metadata: dict[str, Any] | None = None
    cleanup_manifest_path: pathlib.Path | None = None
    try:
        if args.validators != 4 or args.clients != 2 or args.fault_bound != 1:
            fail("local contract topology requires 4 validators, 2 clients, f=1")
        if args.receipt_quorum != 3:
            fail("local finality quorum must be 3")
        if (
            not args.rates
            or args.rates != sorted(set(args.rates))
            or min(args.rates) <= 0
            or min(args.warmup_secs, args.measurement_secs, args.cooldown_secs) < 0
        ):
            fail("rates must be positive and durations non-negative")
        if args.scenario == "steady_state":
            if args.required_overload_points < 0:
                fail("required overload points must be non-negative")
        elif args.required_overload_points != 0:
            fail("partition diagnostics require --required-overload-points 0")
        if not 0 < args.overload_success_fraction_threshold < 1:
            fail("overload success-fraction threshold must be between zero and one")
        if args.partition_at_height <= 0:
            fail("partition trigger height must be positive")
        for path in (args.validator_binary, args.client_binary, args.contract):
            if not path.is_file():
                fail(f"required artifact missing: {path}")
        campaign_root = args.output_root / args.campaign_id
        campaign_root.mkdir(parents=True, exist_ok=False)
        contract_copy = campaign_root / "contract.json"
        shutil.copyfile(args.contract, contract_copy)
        provenance_data = provenance(
            args.validator_binary, args.client_binary, contract_copy
        )
        metadata = {
            "schema_version": 2,
            "campaign_id": args.campaign_id,
            "status": "running",
            "publishable": False,
            "cryptographic_finality_available": True,
            "topology_limitation": "loopback local development evidence is not publishable topology evidence",
            "strategy": args.strategy,
            "scenario": args.scenario,
            "seed": args.seed,
            "validator_count": args.validators,
            "client_count": args.clients,
            "receipt_quorum": args.receipt_quorum,
            "rates": args.rates,
            "required_overload_points": args.required_overload_points,
            "observed_overload_points": 0,
            "overload_success_fraction_threshold": args.overload_success_fraction_threshold,
            "contract_path": contract_copy.name,
            "timing": {
                "warmup_secs": args.warmup_secs,
                "measurement_secs": args.measurement_secs,
                "cooldown_secs": args.cooldown_secs,
            },
            "provenance": provenance_data,
            "trials": [],
        }
        manifest_path = campaign_root / "campaign_manifest.json"
        cleanup_metadata = metadata
        cleanup_manifest_path = manifest_path
        write_json_atomic(manifest_path, metadata)
        for rate in args.rates:
            trial = run_trial(
                args,
                rate,
                campaign_root / f"rate_{rate}_seed_{args.seed}_{args.scenario}",
            )
            metadata["trials"].append(trial)
            write_json_atomic(manifest_path, metadata)
        metadata["observed_overload_points"] = sum(
            bool(trial["overloaded"]) for trial in metadata["trials"]
        )
        metadata["status"] = "complete"
        write_json_atomic(manifest_path, metadata)
        seal = subprocess.run(
            ["python3", "scripts/verify_local_client_campaign.py", str(campaign_root)],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        (campaign_root / "campaign_verification.log").write_text(
            seal.stdout, encoding="utf-8"
        )
        (campaign_root / "campaign_verification.stderr.log").write_text(
            seal.stderr, encoding="utf-8"
        )
        if seal.returncode != 0:
            metadata["status"] = "invalid"
            write_json_atomic(manifest_path, metadata)
            fail("local campaign failed independent sealing verification")
        print(f"PASS: local diagnostic campaign sealed at {campaign_root}")
        return 0
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        if cleanup_metadata is not None and cleanup_manifest_path is not None:
            cleanup_metadata["status"] = "invalid"
            cleanup_metadata["failure_reason"] = str(error)[:1_000]
            try:
                write_json_atomic(cleanup_manifest_path, cleanup_metadata)
            except OSError as manifest_error:
                print(
                    f"FAIL: could not persist invalid campaign status: {manifest_error}",
                    file=sys.stderr,
                )
        print(f"FAIL: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
