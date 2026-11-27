#!/usr/bin/env bash
# End-to-end test of the COMPILED contract (#347).
#
# The unit suite runs against the Soroban test host, which is a mock: it never
# builds the Wasm, never deploys it, and never sees the XDR a real invocation
# produces. This script starts from source and drives the full lifecycle
# through `stellar contract invoke` against the deployed artifact:
#
#   build -> deploy -> init -> register -> approve -> enroll ->
#   mark_completed -> issue_certificate -> verify_certificate
#
# The lifecycle itself is not duplicated here: once the contract is deployed
# and initialised this hands over to scripts/smoke-test-testnet.sh, so there is
# one description of what a healthy run looks like.
#
# Usage:
#   scripts/e2e-wasm.sh
#
# Optional environment:
#   NETWORK     stellar CLI network name (default: local)
#   WASM        path to a prebuilt artifact; skips the build step
#   KEY_PREFIX  prefix for the generated CLI keys (default: e2e)
#
# For NETWORK=local, start the network first:
#   stellar container start local
#
# Exit codes:
#   0  the whole lifecycle passed against the compiled Wasm
#   1  a prerequisite is missing (toolchain, CLI, network)
#   2  a build, deploy or contract call failed

set -euo pipefail

NETWORK="${NETWORK:-local}"
KEY_PREFIX="${KEY_PREFIX:-e2e}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

step() { printf '\n== %s\n' "$1"; }
die()  { echo "error: $1" >&2; exit "${2:-1}"; }

command -v stellar >/dev/null 2>&1 || die "stellar CLI not found on PATH"
command -v cargo   >/dev/null 2>&1 || die "cargo not found on PATH"

# ---------------------------------------------------------------------------
# 1. Build the artifact the rest of this script runs against
# ---------------------------------------------------------------------------
if [ -n "${WASM:-}" ]; then
  [ -f "$WASM" ] || die "WASM is set but $WASM does not exist"
  step "using prebuilt artifact $WASM"
else
  step "building sorobanlard.wasm"
  (cd "$ROOT" && stellar contract build) || die "contract build failed" 2
  WASM="$(find "$ROOT/target" -name 'sorobanlard.wasm' -print -quit)"
  [ -n "$WASM" ] || die "build succeeded but sorobanlard.wasm was not found under target/" 2
  echo "built $WASM"
fi

# ---------------------------------------------------------------------------
# 2. Accounts
# ---------------------------------------------------------------------------
step "preparing accounts on $NETWORK"
RUN_ID="$(date +%s)"
ADMIN_ACCOUNT="${KEY_PREFIX}-admin-${RUN_ID}"
SEC_ADMIN_ACCOUNT="${KEY_PREFIX}-sec-${RUN_ID}"
TREASURY_ACCOUNT="${KEY_PREFIX}-treasury-${RUN_ID}"
INSTRUCTOR_ACCOUNT="${KEY_PREFIX}-instructor-${RUN_ID}"
STUDENT_ACCOUNT="${KEY_PREFIX}-student-${RUN_ID}"

for key in "$ADMIN_ACCOUNT" "$SEC_ADMIN_ACCOUNT" "$TREASURY_ACCOUNT" "$INSTRUCTOR_ACCOUNT" "$STUDENT_ACCOUNT"; do
  stellar keys generate --network "$NETWORK" --fund "$key" >/dev/null 2>&1 \
    || die "could not generate and fund $key — is the '$NETWORK' network running and configured?" 1
done

ADMIN_ADDRESS="$(stellar keys address "$ADMIN_ACCOUNT")"
SEC_ADMIN_ADDRESS="$(stellar keys address "$SEC_ADMIN_ACCOUNT")"
TREASURY_ADDRESS="$(stellar keys address "$TREASURY_ACCOUNT")"

# ---------------------------------------------------------------------------
# 3. Deploy the compiled contract and a token for the course to be priced in
# ---------------------------------------------------------------------------
step "deploying the compiled contract"
CONTRACT_ID="$(stellar contract deploy \
  --wasm "$WASM" \
  --source "$ADMIN_ACCOUNT" \
  --network "$NETWORK" 2>/dev/null)" || die "contract deploy failed" 2
echo "deployed $CONTRACT_ID"

step "deploying the native asset contract for the course token"
TOKEN_ADDRESS="$(stellar contract asset deploy \
  --asset native \
  --source "$ADMIN_ACCOUNT" \
  --network "$NETWORK" 2>/dev/null)" \
  || TOKEN_ADDRESS="$(stellar contract id asset --asset native --network "$NETWORK")"
[ -n "$TOKEN_ADDRESS" ] || die "could not resolve a token address" 2
echo "token $TOKEN_ADDRESS"

# ---------------------------------------------------------------------------
# 4. init — admin, secondary admin and treasury must all be distinct
# ---------------------------------------------------------------------------
step "initialising"
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$ADMIN_ACCOUNT" \
  --network "$NETWORK" \
  -- init \
  --admin "$ADMIN_ADDRESS" \
  --secondary_admin "$SEC_ADMIN_ADDRESS" \
  --treasury "$TREASURY_ADDRESS" \
  --default_fee_pct 20 \
  --max_courses_per_instructor 50 \
  --refund_window_ledgers 1000 \
  --revocation_challenge_period 17280 >/dev/null 2>&1 || die "init failed" 2

# ---------------------------------------------------------------------------
# 5. The lifecycle itself, against the deployment made above
# ---------------------------------------------------------------------------
step "running the lifecycle against the deployed Wasm"
CONTRACT_ID="$CONTRACT_ID" \
ADMIN_ACCOUNT="$ADMIN_ACCOUNT" \
INSTRUCTOR_ACCOUNT="$INSTRUCTOR_ACCOUNT" \
STUDENT_ACCOUNT="$STUDENT_ACCOUNT" \
NETWORK="$NETWORK" \
TOKEN_ADDRESS="$TOKEN_ADDRESS" \
  "$ROOT/scripts/smoke-test-testnet.sh"

echo
echo "e2e passed against the compiled artifact: $WASM"
echo "contract $CONTRACT_ID on $NETWORK"
