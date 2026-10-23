#!/usr/bin/env bash
# Compare the invariants documented in INVARIANTS.md against the
# `test_invariant_invNN_*` tests in the contract test suite.
#
# Prints a summary (also appended to $GITHUB_STEP_SUMMARY when run in
# GitHub Actions) and exits non-zero if any documented invariant has no test.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DOC="$ROOT/INVARIANTS.md"
TESTS="$ROOT/contracts/sorobanlard/src/test.rs"

documented=$( (grep -oE '^### INV-[0-9]+' "$DOC" || true) | grep -oE '[0-9]+' | sort -u || true)
tested=$( (grep -oE 'fn test_invariant_inv[0-9]+_' "$TESTS" || true) | grep -oE '[0-9]+' | sort -u || true)
test_count=$(grep -cE 'fn test_invariant_inv[0-9]+_' "$TESTS" || true)
doc_count=$(printf '%s\n' "$documented" | grep -c . || true)

missing=$(comm -23 <(printf '%s\n' "$documented") <(printf '%s\n' "$tested") | grep . || true)
undocumented=$(comm -13 <(printf '%s\n' "$documented") <(printf '%s\n' "$tested") | grep . || true)
covered=$((doc_count - $(printf '%s\n' "$missing" | grep -c . || true)))

summary="## Invariant coverage

| Metric | Count |
|---|---|
| Documented invariants (INVARIANTS.md) | $doc_count |
| Invariants with at least one test | $covered |
| Invariant tests (\`test_invariant_*\`) | $test_count |
"

if [ -n "$missing" ]; then
  summary+="
**Documented invariants without a test:** $(printf 'INV-%s ' $missing)
"
fi
if [ -n "$undocumented" ]; then
  summary+="
**Invariant tests referencing an undocumented ID:** $(printf 'INV-%s ' $undocumented)
"
fi

echo "$summary"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  echo "$summary" >> "$GITHUB_STEP_SUMMARY"
fi

if [ -n "$missing" ] || [ -n "$undocumented" ]; then
  exit 1
fi
