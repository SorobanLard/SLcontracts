"""Tests for scripts/check_invariants.sh (#339).

Run with: python3 -m unittest discover -s scripts/tests
"""

import os
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "check_invariants.sh"

INVARIANTS_MD = textwrap.dedent("""\
    # Invariants

    ### INV-01 — Payments are accounted for

    ### INV-02 — One active enrollment per student
    """)

TEST_RS_BOTH = textwrap.dedent("""\
    #[test]
    fn test_invariant_inv01_payment_accounted() {}

    #[test]
    fn test_invariant_inv02_single_enrollment() {}
    """)


class CheckInvariantsTest(unittest.TestCase):
    def run_script(self, invariants_md, test_rs):
        with tempfile.TemporaryDirectory() as tmp:
            doc = Path(tmp) / "INVARIANTS.md"
            tests = Path(tmp) / "test.rs"
            doc.write_text(invariants_md, encoding="utf-8")
            tests.write_text(test_rs, encoding="utf-8")
            env = dict(os.environ, INVARIANTS_DOC=str(doc), INVARIANTS_TESTS=str(tests))
            env.pop("GITHUB_STEP_SUMMARY", None)
            return subprocess.run(
                ["bash", str(SCRIPT)], capture_output=True, text=True, env=env,
            )

    def test_fails_when_invariant_has_no_matching_test(self):
        test_rs = TEST_RS_BOTH.split("#[test]\nfn test_invariant_inv02")[0]
        result = self.run_script(INVARIANTS_MD, test_rs)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("INV-02", result.stdout)

    def test_passes_when_all_invariants_have_matching_tests(self):
        result = self.run_script(INVARIANTS_MD, TEST_RS_BOTH)
        self.assertEqual(result.returncode, 0, result.stdout)


if __name__ == "__main__":
    unittest.main()
