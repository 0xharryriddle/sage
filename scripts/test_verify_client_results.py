#!/usr/bin/env python3
"""Positive and tamper-negative tests for cryptographic client results."""

import copy
import hashlib
import json
import pathlib
import struct
import subprocess
import tempfile
import unittest

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

ROOT = pathlib.Path(__file__).resolve().parents[1]
VERIFIER = ROOT / "scripts" / "verify_client_results.py"
VALIDATOR_COUNT = 4
RECEIPT_QUORUM = 3
CHAIN_ID = "sage-local"
EPOCH = 1
CONFIG_ID = 1
KEY_SEED = 42


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


# These helpers re-derive the node's authority signing scheme INDEPENDENTLY of
# verify_client_results.py (deliberately not imported): a fixture generator that
# reused the verifier's own helpers would agree with the verifier by construction
# and could not catch a scheme drift like the raw-payload_hash regression.
# Mirrors crates/sage-manifest/src/authority.rs.
AUTHORITY_SCOPE_CLIENT_FINALITY = 2


def authority_domain_bytes() -> bytes:
    return (
        encode_bytes(CHAIN_ID.encode())
        + struct.pack(">Q", EPOCH)
        + struct.pack(">Q", CONFIG_ID)
        + struct.pack(">I", AUTHORITY_SCOPE_CLIENT_FINALITY)
    )


def testbed_signing_key(signer: int) -> Ed25519PrivateKey:
    seed = domain_hash(
        b"ed25519-key-v1",
        authority_domain_bytes() + struct.pack(">I", signer) + struct.pack(">Q", KEY_SEED),
    )
    return Ed25519PrivateKey.from_private_bytes(seed)


def testbed_registry_hash() -> bytes:
    domain = authority_domain_bytes()
    body = domain + struct.pack(">Q", VALIDATOR_COUNT)
    for signer in range(VALIDATOR_COUNT):
        raw = testbed_signing_key(signer).public_key().public_bytes(
            encoding=serialization.Encoding.Raw,
            format=serialization.PublicFormat.Raw,
        )
        body += struct.pack(">I", signer) + encode_bytes(raw)
    return domain_hash(b"validator-registry-v1", body)


def authority_signing_digest(payload_hash: bytes) -> bytes:
    return domain_hash(
        b"authority-signature-v1",
        authority_domain_bytes()
        + encode_bytes(testbed_registry_hash())
        + encode_bytes(payload_hash),
    )


def signed_certificate(request_id: str) -> dict:
    payload = {
        "chain_id": CHAIN_ID,
        "epoch": EPOCH,
        "config_id": CONFIG_ID,
        "request_id": list(bytes.fromhex(request_id)),
        "height": 8,
        "root": [4] * 32,
        "block_hash": [3] * 32,
        "engine_id": {"kind": "HotStuff", "generation": 1},
    }
    canonical = b"".join(
        (
            encode_bytes(CHAIN_ID.encode()),
            struct.pack(">Q", EPOCH),
            struct.pack(">Q", CONFIG_ID),
            encode_bytes(bytes(payload["request_id"])),
            struct.pack(">Q", payload["height"]),
            encode_bytes(bytes(payload["root"])),
            encode_bytes(bytes(payload["block_hash"])),
            struct.pack(">I", 1),
            struct.pack(">Q", 1),
        )
    )
    payload_hash = domain_hash(b"finality-certificate-v1", canonical)
    signing_digest = authority_signing_digest(payload_hash)
    shares = {}
    for signer in range(RECEIPT_QUORUM):
        signature = testbed_signing_key(signer).sign(signing_digest)
        shares[str(signer)] = {
            "Ed25519": {
                "signer": signer,
                "payload_hash": list(payload_hash),
                "signature_bytes": list(signature),
            }
        }
    return {"payload": payload, "shares": shares}


def base_row() -> dict:
    request_id = "01" * 32
    return {
        "schema_version": 3,
        "campaign_id": "campaign-1",
        "trial_id": "trial-42",
        "client_id": 0,
        "sequence": 0,
        "request_id": request_id,
        "payload_bytes": 512,
        "payload_sha256": "02" * 32,
        "request_wire_bytes": 1322,
        "first_send_monotonic_ns": 1_000_000,
        "response_monotonic_ns": 2_234_567,
        "attempt_count": 4,
        "receipt_observations": 3,
        "validator_ids": [0, 1, 2],
        "receipt_quorum": RECEIPT_QUORUM,
        "outcome": "finalized",
        "latency_micros": 1234,
        "finalized_height": 8,
        "block_hash": "03" * 32,
        "state_root": "04" * 32,
        "valid_finality_proof": True,
        "finality_certificate": signed_certificate(request_id),
    }


class VerifyClientResultsTests(unittest.TestCase):
    def verify(self, rows, *extra):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "results.jsonl"
            path.write_text(
                "".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8"
            )
            return subprocess.run(
                [
                    "python3",
                    str(VERIFIER),
                    str(path),
                    "--validator-count",
                    str(VALIDATOR_COUNT),
                    "--receipt-quorum",
                    str(RECEIPT_QUORUM),
                    "--chain-id",
                    CHAIN_ID,
                    "--epoch",
                    str(EPOCH),
                    "--config-id",
                    str(CONFIG_ID),
                    "--key-seed",
                    str(KEY_SEED),
                    *extra,
                ],
                check=False,
                capture_output=True,
                text=True,
            )

    def assert_rejected(self, row, message):
        result = self.verify([row])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(message, result.stderr)

    def test_accepts_verified_finality_certificate(self):
        result = self.verify([base_row()], "--require-finalized")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_accepts_zero_microsecond_finalized_latency(self):
        row = base_row()
        row["response_monotonic_ns"] = row["first_send_monotonic_ns"] + 999
        row["latency_micros"] = 0
        result = self.verify([row])
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_true_boolean_without_certificate(self):
        row = base_row()
        row["finality_certificate"] = None
        self.assert_rejected(row, "malformed finality_certificate")

    def test_rejects_request_id_tampering(self):
        row = base_row()
        row["request_id"] = "09" * 32
        self.assert_rejected(row, "finality payload differs from request receipt")

    def test_rejects_payload_hash_tampering(self):
        row = base_row()
        row["finality_certificate"]["shares"]["0"]["Ed25519"]["payload_hash"][0] ^= 1
        self.assert_rejected(row, "signer/hash binding mismatch")

    def test_rejects_signature_tampering(self):
        row = base_row()
        row["finality_certificate"]["shares"]["0"]["Ed25519"]["signature_bytes"][0] ^= 1
        self.assert_rejected(row, "invalid Ed25519 finality share")

    def test_rejects_insufficient_signer_quorum(self):
        row = base_row()
        del row["finality_certificate"]["shares"]["2"]
        row["validator_ids"] = [0, 1]
        row["receipt_observations"] = 2
        self.assert_rejected(row, "receipt observation quorum not reached")

    def test_rejects_timestamp_latency_mismatch(self):
        row = base_row()
        row["latency_micros"] += 1
        self.assert_rejected(row, "latency does not match monotonic timestamps")

    def test_rejects_response_before_send(self):
        row = base_row()
        row["response_monotonic_ns"] = row["first_send_monotonic_ns"] - 1
        self.assert_rejected(row, "response precedes first send")

    def test_rejects_duplicate_request_id_across_trials(self):
        first = base_row()
        second = copy.deepcopy(first)
        second["trial_id"] = "trial-43"
        second["sequence"] = 1
        result = self.verify([first, second])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("duplicate request_id", result.stderr)

    def test_rejects_nonfinalized_receipt_claim(self):
        row = base_row()
        row.update(
            outcome="timeout",
            receipt_observations=1,
            validator_ids=[0],
            latency_micros=None,
            finalized_height=None,
            block_hash=None,
            state_root=None,
            valid_finality_proof=False,
            finality_certificate=None,
        )
        self.assert_rejected(row, "non-finalized outcome cannot claim receipt observations")

    def test_rejects_duplicate_validator_observation(self):
        row = base_row()
        row["validator_ids"] = [0, 0, 2]
        self.assert_rejected(row, "validator_ids must be sorted and unique")

    def test_rejects_unknown_field(self):
        row = base_row()
        row["unregistered"] = True
        self.assert_rejected(row, "fields differ from schema")

    # --- scheme-drift regressions -------------------------------------------
    # These pin the authority-digest binding. Before the fix, the verifier
    # checked signatures against the bare finality payload hash, so real node
    # artifacts (which sign an authority digest binding domain + committee
    # registry) were rejected while these weaker shares were accepted.

    def test_rejects_share_signed_over_raw_payload_hash(self):
        """A share signed over the bare payload hash must NOT verify.

        This is the exact regression that made every real client campaign
        artifact fail: the node binds domain + registry_hash into the signed
        digest, so a raw-payload_hash signature is a strictly weaker credential.
        """
        row = base_row()
        certificate = row["finality_certificate"]
        payload_hash = bytes(certificate["shares"]["0"]["Ed25519"]["payload_hash"])
        for signer in range(RECEIPT_QUORUM):
            weak = testbed_signing_key(signer).sign(payload_hash)
            certificate["shares"][str(signer)]["Ed25519"]["signature_bytes"] = list(weak)
        self.assert_rejected(row, "invalid Ed25519 finality share")

    def test_rejects_share_signed_under_wrong_authority_scope(self):
        """A share valid under another AuthorityScope must not cross scopes."""
        row = base_row()
        certificate = row["finality_certificate"]
        payload_hash = bytes(certificate["shares"]["0"]["Ed25519"]["payload_hash"])
        wrong_scope_domain = (
            encode_bytes(CHAIN_ID.encode())
            + struct.pack(">Q", EPOCH)
            + struct.pack(">Q", CONFIG_ID)
            + struct.pack(">I", 1)  # Cutover, not ClientFinality
        )
        digest = domain_hash(
            b"authority-signature-v1",
            wrong_scope_domain
            + encode_bytes(testbed_registry_hash())
            + encode_bytes(payload_hash),
        )
        for signer in range(RECEIPT_QUORUM):
            signature = testbed_signing_key(signer).sign(digest)
            certificate["shares"][str(signer)]["Ed25519"]["signature_bytes"] = list(signature)
        self.assert_rejected(row, "invalid Ed25519 finality share")

    def test_rejects_share_bound_to_a_different_committee(self):
        """A share bound to a different committee registry must not verify."""
        row = base_row()
        certificate = row["finality_certificate"]
        payload_hash = bytes(certificate["shares"]["0"]["Ed25519"]["payload_hash"])
        foreign_registry = domain_hash(b"validator-registry-v1", b"a-different-committee")
        digest = domain_hash(
            b"authority-signature-v1",
            authority_domain_bytes()
            + encode_bytes(foreign_registry)
            + encode_bytes(payload_hash),
        )
        for signer in range(RECEIPT_QUORUM):
            signature = testbed_signing_key(signer).sign(digest)
            certificate["shares"][str(signer)]["Ed25519"]["signature_bytes"] = list(signature)
        self.assert_rejected(row, "invalid Ed25519 finality share")


if __name__ == "__main__":
    unittest.main()
