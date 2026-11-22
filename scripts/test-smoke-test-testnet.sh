#!/usr/bin/env bash
# Tests for scripts/smoke-test-testnet.sh exit codes and step reporting.
#
# A stub `stellar` CLI is placed first on PATH so the smoke test can run
# without a network. The stub's behaviour is controlled by:
#   STUB_FAIL_FN        contract function name that should fail (optional)
#   STUB_VERIFY_RESULT  value printed by verify_certificate (default: true)
#
# Usage: scripts/test-smoke-test-testnet.sh
# Exits 0 when every case passes, 1 otherwise.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SMOKE_TEST="$SCRIPT_DIR/smoke-test-testnet.sh"

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

STUB_BIN="$WORK_DIR/bin"
mkdir -p "$STUB_BIN"

cat > "$STUB_BIN/stellar" <<'STUB'
#!/usr/bin/env bash
if [ "${1:-}" = "keys" ] && [ "${2:-}" = "address" ]; then
  echo "G${3:-UNKNOWN}"
  exit 0
fi

if [ "${1:-}" = "contract" ] && [ "${2:-}" = "invoke" ]; then
  fn=""
  while [ $# -gt 0 ]; do
    if [ "$1" = "--" ]; then
      fn="${2:-}"
      break
    fi
    shift
  done

  if [ -n "${STUB_FAIL_FN:-}" ] && [ "$fn" = "$STUB_FAIL_FN" ]; then
    echo "error: simulated failure in $fn"
    exit 1
  fi

  if [ "$fn" = "verify_certificate" ]; then
    echo "${STUB_VERIFY_RESULT:-true}"
  fi
  exit 0
fi

echo "stub stellar: unsupported command: $*" >&2
exit 99
STUB
chmod +x "$STUB_BIN/stellar"

PASSED=0
FAILED=0

# run_case <name> <expected-exit> <expected-stderr-substring> [VAR=value...]
run_case() {
  local name="$1"
  local expected_exit="$2"
  local expected_stderr="$3"
  shift 3

  local stdout_file="$WORK_DIR/stdout"
  local stderr_file="$WORK_DIR/stderr"

  env \
    PATH="$STUB_BIN:$PATH" \
    CONTRACT_ID="CTESTCONTRACT" \
    ADMIN_ACCOUNT="admin" \
    INSTRUCTOR_ACCOUNT="instructor" \
    STUDENT_ACCOUNT="student" \
    "$@" \
    bash "$SMOKE_TEST" >"$stdout_file" 2>"$stderr_file"
  local actual_exit=$?

  local ok=1
  if [ "$actual_exit" -ne "$expected_exit" ]; then
    echo "  expected exit $expected_exit, got $actual_exit"
    ok=0
  fi
  if [ -n "$expected_stderr" ] && ! grep -qF -- "$expected_stderr" "$stderr_file"; then
    echo "  expected stderr to contain: $expected_stderr"
    ok=0
  fi

  if [ "$ok" -eq 1 ]; then
    echo "PASS: $name"
    PASSED=$((PASSED + 1))
  else
    echo "FAIL: $name"
    echo "  --- stdout ---"; sed 's/^/  /' "$stdout_file"
    echo "  --- stderr ---"; sed 's/^/  /' "$stderr_file"
    FAILED=$((FAILED + 1))
  fi
}

# Happy path: every step succeeds and verify_certificate returns true.
run_case "exits 0 when verify_certificate returns true" 0 ""

# Each contract step failing must exit 2 and name the failing step.
run_case "exits 2 when approve token fails"     2 "FAIL: approve token"     STUB_FAIL_FN=add_approved_token
run_case "exits 2 when register course fails"   2 "FAIL: register course"   STUB_FAIL_FN=register_course
run_case "exits 2 when approve course fails"    2 "FAIL: approve course"    STUB_FAIL_FN=approve_course
run_case "exits 2 when enroll student fails"    2 "FAIL: enroll student"    STUB_FAIL_FN=enroll
run_case "exits 2 when mark completed fails"    2 "FAIL: mark completed"    STUB_FAIL_FN=mark_completed
run_case "exits 2 when issue certificate fails" 2 "FAIL: issue certificate" STUB_FAIL_FN=issue_certificate
run_case "exits 2 when verify certificate fails" 2 "FAIL: verify certificate" STUB_FAIL_FN=verify_certificate

# verify_certificate succeeding but returning false is also a failure.
run_case "exits 2 when verify_certificate returns false" 2 "FAIL: verify_certificate returned" STUB_VERIFY_RESULT=false

echo
echo "$PASSED passed, $FAILED failed"
[ "$FAILED" -eq 0 ]
