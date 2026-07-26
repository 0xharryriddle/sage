#!/usr/bin/env python3
"""Fail-closed validation for cryptographically finalized client JSONL."""

import argparse
import hashlib
import json
import pathlib
import struct
import sys
from typing import Any, NoReturn

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

REQUIRED = {
    "schema_version",
    "campaign_id",
    "trial_id",
    "client_id",
    "sequence",
    "request_id",
    "payload_bytes",
    "payload_sha256",
    "request_wire_bytes",
    "first_send_monotonic_ns",
    "response_monotonic_ns",
    "attempt_count",
    "receipt_observations",
    "validator_ids",
    "receipt_quorum",
    "outcome",
    "latency_micros",
    "finalized_height",
    "block_hash",
    "state_root",
    "valid_finality_proof",
    "finality_certificate",
}

ENGINE_KINDS = {"Poa": 0, "HotStuff": 1, "Raft": 2, "DagBft": 3}


def fail(message: str) -> NoReturn:
    raise ValueError(message)


def hash32(value: Any, field: str) -> None:
    if not isinstance(value, str) or len(value) != 64:
        fail(f"{field} must be 64 lowercase hexadecimal characters")
    if value.lower() != value or any(char not in "0123456789abcdef" for char in value):
        fail(f"{field} must be lowercase hexadecimal")


def positive_int(value: Any, field: str, allow_zero: bool = False) -> None:
    minimum = 0 if allow_zero else 1
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        fail(f"{field} must be an integer >= {minimum}")


def integer(value: Any, field: str, minimum: int = 0) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        fail(f"{field} must be an integer >= {minimum}")
    return value


def bytes32(value: Any, field: str) -> bytes:
    if not isinstance(value, list) or len(value) != 32 or any(
        isinstance(byte, bool) or not isinstance(byte, int) or byte < 0 or byte > 255
        for byte in value
    ):
        fail(f"{field} must be an array of 32 bytes")
    return bytes(value)


def encode_bytes(value: bytes) -> bytes:
    return struct.pack(">Q", len(value)) + value


def domain_hash(tag: bytes, payload: bytes) -> bytes:
    return hashlib.sha256(
        b"SAGE"
        + b"\x00"
        + struct.pack(">Q", len(tag))
        + tag
        + struct.pack(">Q", len(payload))
        + payload
    ).digest()


# AuthorityScope discriminants, mirroring
# crates/sage-manifest/src/authority.rs::AuthorityScope::code().
AUTHORITY_SCOPE_CLIENT_FINALITY = 2


def authority_domain_bytes(chain_id: str, epoch: int, config_id: int, scope: int) -> bytes:
    """Mirror AuthorityDomain::encode (authority.rs:76)."""
    return (
        encode_bytes(chain_id.encode("utf-8"))
        + struct.pack(">Q", epoch)
        + struct.pack(">Q", config_id)
        + struct.pack(">I", scope)
    )


def derive_testbed_signing_seed(domain: bytes, signer: int, key_seed: int) -> bytes:
    """Mirror authority.rs::derive_testbed_key key material."""
    return domain_hash(
        b"ed25519-key-v1",
        domain + struct.pack(">I", signer) + struct.pack(">Q", key_seed),
    )


def testbed_registry_hash(domain: bytes, validator_count: int, key_seed: int) -> bytes:
    """Mirror authority.rs::registry_hash over the deterministic testbed committee."""
    body = domain + struct.pack(">Q", validator_count)
    for signer in range(validator_count):
        seed = derive_testbed_signing_seed(domain, signer, key_seed)
        public_key = Ed25519PrivateKey.from_private_bytes(seed).public_key()
        raw = public_key.public_bytes(
            encoding=serialization.Encoding.Raw,
            format=serialization.PublicFormat.Raw,
        )
        body += struct.pack(">I", signer) + encode_bytes(raw)
    return domain_hash(b"validator-registry-v1", body)


def authority_signing_digest(domain: bytes, registry_hash: bytes, payload_hash: bytes) -> bytes:
    """Mirror ValidatorAuthority::signing_digest (authority.rs:276).

    The node does NOT sign the bare finality payload hash: it signs a digest that
    also binds the authority domain (chain/epoch/config/scope) and the exact
    committee registry hash. Verifying the bare payload hash would accept a share
    replayed across scopes or committees, so this binding is load-bearing.
    """
    return domain_hash(
        b"authority-signature-v1",
        domain + encode_bytes(registry_hash) + encode_bytes(payload_hash),
    )


def finality_payload_bytes(
    payload: dict[str, Any],
    *,
    line_number: int,
    chain_id: str,
    epoch: int,
    config_id: int,
) -> bytes:
    prefix = f"line {line_number} finality payload"
    if set(payload) != {
        "chain_id",
        "epoch",
        "config_id",
        "request_id",
        "height",
        "root",
        "block_hash",
        "engine_id",
    }:
        fail(f"{prefix}: unexpected fields")
    if (
        payload["chain_id"] != chain_id
        or payload["epoch"] != epoch
        or payload["config_id"] != config_id
    ):
        fail(f"{prefix}: chain domain differs from immutable verifier context")
    engine = payload["engine_id"]
    if not isinstance(engine, dict) or set(engine) != {"kind", "generation"}:
        fail(f"{prefix}: malformed engine_id")
    kind = ENGINE_KINDS.get(engine["kind"])
    if kind is None:
        fail(f"{prefix}: unknown engine kind")
    generation = integer(engine["generation"], f"{prefix} engine generation")
    height = integer(payload["height"], f"{prefix} height", 1)
    return b"".join(
        (
            encode_bytes(chain_id.encode()),
            struct.pack(">Q", epoch),
            struct.pack(">Q", config_id),
            encode_bytes(bytes32(payload["request_id"], f"{prefix} request_id")),
            struct.pack(">Q", height),
            encode_bytes(bytes32(payload["root"], f"{prefix} root")),
            encode_bytes(bytes32(payload["block_hash"], f"{prefix} block_hash")),
            struct.pack(">I", kind),
            struct.pack(">Q", generation),
        )
    )


def verify_finality_certificate(
    row: dict[str, Any],
    line_number: int,
    *,
    validator_count: int,
    receipt_quorum: int,
    chain_id: str,
    epoch: int,
    config_id: int,
    key_seed: int,
) -> None:
    certificate = row["finality_certificate"]
    if not isinstance(certificate, dict) or set(certificate) != {"payload", "shares"}:
        fail(f"line {line_number}: malformed finality_certificate")
    payload = certificate["payload"]
    shares = certificate["shares"]
    if not isinstance(payload, dict) or not isinstance(shares, dict):
        fail(f"line {line_number}: malformed finality payload or shares")
    canonical = finality_payload_bytes(
        payload,
        line_number=line_number,
        chain_id=chain_id,
        epoch=epoch,
        config_id=config_id,
    )
    payload_hash = domain_hash(b"finality-certificate-v1", canonical)
    # The node signs an authority digest that binds the domain and the exact
    # committee registry, not the bare payload hash (authority.rs:276).
    authority_domain = authority_domain_bytes(
        chain_id, epoch, config_id, AUTHORITY_SCOPE_CLIENT_FINALITY
    )
    registry_hash = testbed_registry_hash(authority_domain, validator_count, key_seed)
    signing_digest = authority_signing_digest(authority_domain, registry_hash, payload_hash)
    if (
        bytes32(payload["request_id"], "finality request_id")
        != bytes.fromhex(row["request_id"])
        or payload["height"] != row["finalized_height"]
        or bytes32(payload["block_hash"], "finality block_hash")
        != bytes.fromhex(row["block_hash"])
        or bytes32(payload["root"], "finality root") != bytes.fromhex(row["state_root"])
    ):
        fail(f"line {line_number}: finality payload differs from request receipt")

    signer_ids: list[int] = []
    for raw_signer, envelope in shares.items():
        try:
            signer = int(raw_signer)
        except (TypeError, ValueError):
            fail(f"line {line_number}: finality share key is not a validator id")
        if str(signer) != raw_signer or signer < 0 or signer >= validator_count:
            fail(f"line {line_number}: finality signer outside immutable committee")
        if not isinstance(envelope, dict) or set(envelope) != {"Ed25519"}:
            fail(f"line {line_number}: finality share is not Ed25519")
        share = envelope["Ed25519"]
        if not isinstance(share, dict) or set(share) != {
            "signer",
            "payload_hash",
            "signature_bytes",
        }:
            fail(f"line {line_number}: malformed Ed25519 share")
        if (
            share["signer"] != signer
            or bytes32(share["payload_hash"], "share payload_hash") != payload_hash
        ):
            fail(f"line {line_number}: finality share signer/hash binding mismatch")
        signature = share["signature_bytes"]
        if not isinstance(signature, list) or len(signature) != 64 or any(
            isinstance(byte, bool) or not isinstance(byte, int) or byte < 0 or byte > 255
            for byte in signature
        ):
            fail(f"line {line_number}: malformed Ed25519 signature bytes")
        private_seed = derive_testbed_signing_seed(authority_domain, signer, key_seed)
        public_key = Ed25519PrivateKey.from_private_bytes(private_seed).public_key()
        try:
            public_key.verify(bytes(signature), signing_digest)
        except InvalidSignature:
            fail(f"line {line_number}: invalid Ed25519 finality share from validator {signer}")
        signer_ids.append(signer)

    if signer_ids != sorted(set(signer_ids)) or len(signer_ids) < receipt_quorum:
        fail(f"line {line_number}: finality signer quorum is invalid")
    if signer_ids != row["validator_ids"]:
        fail(f"line {line_number}: certificate signers differ from validator_ids")


def verify(
    path: pathlib.Path,
    require_finalized: bool,
    *,
    validator_count: int,
    receipt_quorum: int,
    chain_id: str,
    epoch: int,
    config_id: int,
    key_seed: int,
) -> int:
    identities: set[tuple[str, str, int, int]] = set()
    request_ids: set[str] = set()
    count = 0
    with path.open(encoding="utf-8") as handle:
        for line_number, raw in enumerate(handle, 1):
            if not raw.strip():
                fail(f"line {line_number}: blank lines are not allowed")
            try:
                parsed: Any = json.loads(raw)
            except json.JSONDecodeError as error:
                fail(f"line {line_number}: invalid JSON: {error}")
            if not isinstance(parsed, dict):
                fail(f"line {line_number}: row must be a JSON object")
            row: dict[str, Any] = parsed
            if set(row) != REQUIRED:
                fail(
                    f"line {line_number}: fields differ from schema: "
                    f"{sorted(set(row) ^ REQUIRED)}"
                )
            if row["schema_version"] != 3:
                fail(f"line {line_number}: unsupported schema_version")
            if not isinstance(row["campaign_id"], str) or not row["campaign_id"]:
                fail(f"line {line_number}: campaign_id must be non-empty")
            if not isinstance(row["trial_id"], str) or not row["trial_id"]:
                fail(f"line {line_number}: trial_id must be non-empty")
            positive_int(row["client_id"], "client_id", allow_zero=True)
            positive_int(row["sequence"], "sequence", allow_zero=True)
            positive_int(row["attempt_count"], "attempt_count")
            positive_int(row["payload_bytes"], "payload_bytes", allow_zero=True)
            positive_int(row["request_wire_bytes"], "request_wire_bytes")
            positive_int(
                row["first_send_monotonic_ns"],
                "first_send_monotonic_ns",
                allow_zero=True,
            )
            positive_int(
                row["response_monotonic_ns"],
                "response_monotonic_ns",
                allow_zero=True,
            )
            if row["response_monotonic_ns"] < row["first_send_monotonic_ns"]:
                fail(f"line {line_number}: response precedes first send")
            positive_int(row["receipt_quorum"], "receipt_quorum")
            positive_int(
                row["receipt_observations"], "receipt_observations", allow_zero=True
            )
            if not isinstance(row["validator_ids"], list) or any(
                isinstance(value, bool) or not isinstance(value, int) or value < 0
                for value in row["validator_ids"]
            ):
                fail(
                    f"line {line_number}: validator_ids must be non-negative integers"
                )
            if row["validator_ids"] != sorted(set(row["validator_ids"])):
                fail(f"line {line_number}: validator_ids must be sorted and unique")
            if len(row["validator_ids"]) != row["receipt_observations"]:
                fail(
                    f"line {line_number}: validator_ids count differs from observations"
                )
            if row["receipt_quorum"] != receipt_quorum:
                fail(
                    f"line {line_number}: receipt_quorum differs from verifier context"
                )
            if any(value >= validator_count for value in row["validator_ids"]):
                fail(f"line {line_number}: validator id outside immutable committee")
            hash32(row["request_id"], "request_id")
            hash32(row["payload_sha256"], "payload_sha256")
            identity = (
                row["campaign_id"],
                row["trial_id"],
                row["client_id"],
                row["sequence"],
            )
            if identity in identities:
                fail(f"line {line_number}: duplicate request identity {identity}")
            if row["request_id"] in request_ids:
                fail(f"line {line_number}: duplicate request_id")
            identities.add(identity)
            request_ids.add(row["request_id"])

            if not isinstance(row["valid_finality_proof"], bool):
                fail(
                    f"line {line_number}: valid_finality_proof must be boolean"
                )
            if row["outcome"] == "finalized":
                if row["valid_finality_proof"] is not True:
                    fail(f"line {line_number}: finalized row lacks a valid proof claim")
                if row["receipt_observations"] < row["receipt_quorum"]:
                    fail(f"line {line_number}: receipt observation quorum not reached")
                positive_int(row["latency_micros"], "latency_micros", allow_zero=True)
                expected_latency = (
                    row["response_monotonic_ns"]
                    - row["first_send_monotonic_ns"]
                ) // 1_000
                if row["latency_micros"] != expected_latency:
                    fail(
                        f"line {line_number}: latency does not match monotonic timestamps"
                    )
                positive_int(row["finalized_height"], "finalized_height")
                hash32(row["block_hash"], "block_hash")
                hash32(row["state_root"], "state_root")
                verify_finality_certificate(
                    row,
                    line_number,
                    validator_count=validator_count,
                    receipt_quorum=receipt_quorum,
                    chain_id=chain_id,
                    epoch=epoch,
                    config_id=config_id,
                    key_seed=key_seed,
                )
            else:
                if row["valid_finality_proof"]:
                    fail(
                        f"line {line_number}: non-finalized outcome cannot claim a valid proof"
                    )
                if row["receipt_observations"] != 0:
                    fail(
                        f"line {line_number}: non-finalized outcome cannot claim receipt observations"
                    )
                if require_finalized:
                    fail(
                        f"line {line_number}: non-finalized outcome {row['outcome']!r}"
                    )
                if not (
                    row["outcome"] in {"timeout", "transport_failure", "conflict"}
                    or row["outcome"].startswith("rejected:")
                ):
                    fail(f"line {line_number}: unknown outcome {row['outcome']!r}")
                for field in (
                    "latency_micros",
                    "finalized_height",
                    "block_hash",
                    "state_root",
                ):
                    if row[field] is not None:
                        fail(
                            f"line {line_number}: {field} must be null for "
                            f"{row['outcome']!r}"
                        )
                if row["finality_certificate"] is not None:
                    fail(
                        f"line {line_number}: non-finalized outcome carries a finality certificate"
                    )
            count += 1
    if count == 0:
        fail("input contains no request rows")
    return count


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("path", type=pathlib.Path)
    parser.add_argument("--require-finalized", action="store_true")
    parser.add_argument("--validator-count", type=int, required=True)
    parser.add_argument("--receipt-quorum", type=int, required=True)
    parser.add_argument("--chain-id", default="sage-local")
    parser.add_argument("--epoch", type=int, default=1)
    parser.add_argument("--config-id", type=int, default=1)
    parser.add_argument("--key-seed", type=int, default=42)
    args = parser.parse_args()
    try:
        if args.validator_count <= 0 or not (
            0 < args.receipt_quorum <= args.validator_count
        ):
            fail("invalid validator count or receipt quorum")
        count = verify(
            args.path,
            args.require_finalized,
            validator_count=args.validator_count,
            receipt_quorum=args.receipt_quorum,
            chain_id=args.chain_id,
            epoch=args.epoch,
            config_id=args.config_id,
            key_seed=args.key_seed,
        )
    except (OSError, ValueError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    print(f"PASS: validated {count} client request rows")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
