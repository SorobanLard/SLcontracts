#!/usr/bin/env bash
# Post-deployment smoke test for a SorobanLard contract on testnet.
#
# Runs register -> approve -> enroll -> mark_completed -> issue_certificate ->
# verify_certificate against a free course (price 0), so no token funds move.
# The contract must already be deployed and initialised with `init`.
#
# Required environment:
#   CONTRACT_ID         contract address (C...)
#   ADMIN_ACCOUNT       stellar CLI key name of the admin
#   INSTRUCTOR_ACCOUNT  stellar CLI key name of the instructor
#   STUDENT_ACCOUNT     stellar CLI key name of the student
#
# Optional environment:
#   NETWORK             stellar CLI network name (default: testnet)
#   TOKEN_ADDRESS       approved SAC token used for the course (default: testnet USDC)
#
# Exit codes:
#   0  every step succeeded and verify_certificate returned true
#   1  missing environment variable or stellar CLI
#   2  a contract call failed (the failing step is printed)

set -euo pipefail

NETWORK="${NETWORK:-testnet}"
TOKEN_ADDRESS="${TOKEN_ADDRESS:-CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA}"

for var in CONTRACT_ID ADMIN_ACCOUNT INSTRUCTOR_ACCOUNT STUDENT_ACCOUNT; do
  if [ -z "${!var:-}" ]; then
    echo "error: $var is not set" >&2
    exit 1
  fi
done

if ! command -v stellar >/dev/null 2>&1; then
  echo "error: stellar CLI not found on PATH" >&2
  exit 1
fi

RUN_ID="$(date +%s)"
COURSE_ID="SMOKE-${RUN_ID}"
CERTIFICATE_ID="CERT-SMOKE-${RUN_ID}"
CONTENT_HASH="$(openssl rand -hex 32)"

ADMIN_ADDRESS="$(stellar keys address "$ADMIN_ACCOUNT")"
INSTRUCTOR_ADDRESS="$(stellar keys address "$INSTRUCTOR_ACCOUNT")"
STUDENT_ADDRESS="$(stellar keys address "$STUDENT_ACCOUNT")"

# invoke <step-name> <source-account> -- <contract fn> [args...]
invoke() {
  local step="$1"
  local source="$2"
  shift 2

  local output
  if ! output="$(stellar contract invoke \
      --id "$CONTRACT_ID" \
      --source "$source" \
      --network "$NETWORK" \
      -- "$@" 2>&1)"; then
    echo "FAIL: $step" >&2
    echo "$output" >&2
    exit 2
  fi

  echo "OK:   $step"
  printf '%s\n' "$output"
}

# add_approved_token is idempotent, so it is safe on an already-configured deployment.
invoke "approve token" "$ADMIN_ACCOUNT" \
  add_approved_token --admin "$ADMIN_ADDRESS" --token "$TOKEN_ADDRESS"

invoke "register course" "$INSTRUCTOR_ACCOUNT" \
  register_course \
  --instructor "$INSTRUCTOR_ADDRESS" \
  --course_id "$COURSE_ID" \
  --price 0 \
  --token "$TOKEN_ADDRESS" \
  --platform_fee_pct 0 \
  --content_hash "$CONTENT_HASH" \
  --categories '[]'

invoke "approve course" "$ADMIN_ACCOUNT" \
  approve_course --admin "$ADMIN_ADDRESS" --course_id "$COURSE_ID"

invoke "enroll student" "$STUDENT_ACCOUNT" \
  enroll --student "$STUDENT_ADDRESS" --course_id "$COURSE_ID"

# Supplying evidence_hash waives the student co-signature, so the smoke test
# does not need the student to sign a second time.
invoke "mark completed" "$ADMIN_ACCOUNT" \
  mark_completed \
  --admin "$ADMIN_ADDRESS" \
  --student "$STUDENT_ADDRESS" \
  --course_id "$COURSE_ID" \
  --evidence_hash "smoke-test-${RUN_ID}"

invoke "issue certificate" "$ADMIN_ACCOUNT" \
  issue_certificate \
  --admin "$ADMIN_ADDRESS" \
  --student "$STUDENT_ADDRESS" \
  --course_id "$COURSE_ID" \
  --certificate_id "$CERTIFICATE_ID" \
  --course_title "Smoke Test Course" \
  --enrollment_reference "smoke-${RUN_ID}"

VERIFY_OUTPUT="$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$ADMIN_ACCOUNT" \
  --network "$NETWORK" \
  -- verify_certificate --certificate_id "$CERTIFICATE_ID" 2>&1)" || {
  echo "FAIL: verify certificate" >&2
  echo "$VERIFY_OUTPUT" >&2
  exit 2
}

if [ "$(printf '%s' "$VERIFY_OUTPUT" | tr -d '[:space:]"')" != "true" ]; then
  echo "FAIL: verify_certificate returned: $VERIFY_OUTPUT" >&2
  exit 2
fi

echo "OK:   verify certificate (true)"
echo "Smoke test passed: course $COURSE_ID, certificate $CERTIFICATE_ID"
