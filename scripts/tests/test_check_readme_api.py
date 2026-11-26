"""Tests for scripts/check_readme_api.py (#338).

Run with: python3 -m unittest discover -s scripts/tests
"""

import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "check_readme_api.py"

LIB_RS = textwrap.dedent("""\
    #[contractimpl]
    impl Contract {
        pub fn enroll(env: Env, student: Address, course_id: u32) {
        }

        pub fn get_course(env: Env, course_id: u32) -> Course {
        }
    }
    """)

README_MATCHING = textwrap.dedent("""\
    # Contract

    - `enroll(student, course_id)` enrolls a student.
    - `get_course(course_id)` returns a course.
    """)


class CheckReadmeApiTest(unittest.TestCase):
    def run_script(self, lib_rs, readme):
        with tempfile.TemporaryDirectory() as tmp:
            lib_path = Path(tmp) / "lib.rs"
            readme_path = Path(tmp) / "README.md"
            lib_path.write_text(lib_rs, encoding="utf-8")
            readme_path.write_text(readme, encoding="utf-8")
            return subprocess.run(
                [sys.executable, str(SCRIPT), "--strict",
                 "--lib-rs", str(lib_path), "--readme", str(readme_path)],
                capture_output=True, text=True,
            )

    def test_fails_when_lib_rs_function_missing_from_readme(self):
        lib_rs = LIB_RS.replace(
            "    }\n}\n",
            "    }\n\n    pub fn withdraw(env: Env, amount: i128) {\n    }\n}\n",
        )
        result = self.run_script(lib_rs, README_MATCHING)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("`withdraw`", result.stdout)

    def test_fails_when_readme_function_missing_from_lib_rs(self):
        readme = README_MATCHING + "- `refund(course_id)` refunds a student.\n"
        result = self.run_script(LIB_RS, readme)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("`refund`", result.stdout)

    def test_passes_when_readme_and_lib_rs_match(self):
        result = self.run_script(LIB_RS, README_MATCHING)
        self.assertEqual(result.returncode, 0, result.stdout)


if __name__ == "__main__":
    unittest.main()
