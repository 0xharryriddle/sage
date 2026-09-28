"""Manuscript scope regressions, not a proof checker or runtime conformance test."""
from pathlib import Path
import re
import unittest

PAPER = Path(__file__).resolve().parent


def normalized(text):
    return " ".join(text.split())


class FinalParentScopeTests(unittest.TestCase):
    def alternative(self):
        text = (PAPER / "sections/analysis.tex").read_text()
        start = r"\paragraph{Alternative source-adapter contract}"
        self.assertEqual(text.count(start), 1, "Missing or duplicated conditional adapter analysis")
        return normalized(text.split(start, 1)[1].split(r"\subsection", 1)[0])

    def test_alternative_keeps_native_safety_and_all_signer_premises(self):
        text = self.alternative()
        for required in ("unique parent-linked finalized prefix", "$q_1>f$ alone does not establish source safety",
                         "every correct validator", "before creating any externally releasable source authorization",
                         "exact finalized parent", "atomically and durably installs", "before signing services",
                         "proposal, vote, catch-up and buffered-release", "Restart reloads and validates"):
            with self.subTest(premise=required):
                self.assertIn(required, text)

    def test_proof_handles_uninformed_past_and_nonboundary_signers(self):
        text = self.alternative()
        for required in (r"\label{prop:final-parent-retirement}", "contains a correct signer", "earlier prepared or buffered",
                         "different finalized parent", "has not adopted", "need not belong to the Boundary signer set"):
            with self.subTest(argument=required):
                self.assertIn(required, text)

    def test_no_protocol_or_measurement_promotion(self):
        text = self.alternative()
        for required in (r"does not replace \cref{rule:lock}", "does not require an activation certificate",
                         "target-authorization", "not a necessity result for this stronger adapter",
                         "not a liveness result", "not established for the evaluated implementation",
                         "neither the reported timings nor the existing bounded models"):
            with self.subTest(boundary=required):
                self.assertIn(required, text)

    def test_discussion_excludes_speculative_adapters_and_old_measurements(self):
        text = normalized((PAPER / "sections/discussion.tex").read_text())
        for required in (r"\cref{prop:final-parent-retirement}", "conditional alternative", "speculative or pipelined source engines",
                         "source serialization cost and progress", "not measured by the existing campaigns"):
            with self.subTest(boundary=required):
                self.assertIn(required, text)

    def test_existing_handoff_proof_does_not_consume_alternative(self):
        text = (PAPER / "sections/analysis.tex").read_text()
        match = re.search(r"\\label\{th:safety\}(.*?)\\end\{proof\}", text, re.S)
        self.assertIsNotNone(match)
        handoff = normalized(match.group(1))
        self.assertIn(r"durable \cref{rule:lock} with $q_1>2f$", handoff)
        self.assertNotIn("prop:final-parent-retirement", handoff)


if __name__ == "__main__":
    unittest.main(verbosity=2)
