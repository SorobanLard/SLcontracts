# Contract Invariants

This is the list of financial and state-machine invariants the SorobanLard
contract relies on. Each one is stated once, linked to the code that
enforces it, and cross-referenced to the tests that verify it. Its purpose
is to give an external audit, and future property-based tests, one list to
check against.

- Code: [contracts/sorobanlard/src/lib.rs](contracts/sorobanlard/src/lib.rs)
- Tests: [contracts/sorobanlard/src/test.rs](contracts/sorobanlard/src/test.rs)
- Lifecycles: [STATE_MACHINES.md](STATE_MACHINES.md)

**Status legend**

| Status | Meaning |
|--------|---------|
| ✅ Verified | At least one test asserts the invariant directly. |
| 🟡 Partial | Tests cover some paths or a single case, but not the general statement. |
| ❌ Unverified | No test asserts it. Listed under [Follow-ups](#follow-ups). |
| ⚠️ Known gap | The invariant does **not** fully hold in the current code. Listed under [Follow-ups](#follow-ups). |

---

## Token assumptions (preconditions)

Every financial invariant below assumes these hold for each token approved via
`add_approved_token`. The contract cannot check them on-chain. They are an
**admin responsibility**, written down in the `add_approved_token` doc comment
and in the crate-level *Token Assumptions* section of `lib.rs`.

| ID | Assumption | Consequence if broken | Tests |
|----|------------|-----------------------|-------|
| T1 | `transfer(from, to, amount)` moves exactly `amount`: no transfer fee, no burn. | Inbound payments in `enroll` / `re_enroll` are rejected by the balance-delta check. Outbound transfers (`withdraw_earnings`, refunds) are **not** checked, so recipients get less than the amount debited. | ✅ `test_enroll_rejects_fee_on_transfer_token`, `test_withdraw_earnings_with_fee_on_transfer_token_underpays_instructor` |
| T2 | Balances never change without a `transfer` (no rebasing, no interest, no admin clawback of contract funds). | The contract's real balance drifts away from `sum(InstructorEarnings)` (F4). After a negative rebase, the last instructors to withdraw cannot. | 🟡 `test_reconciliation_instructor_earnings_match_contract_balance` (standard SAC only; no rebasing mock) |
| T3 | The token reports 7 decimals. | Enforced: `register_course` and `validate_enrollment` panic otherwise. | 🟡 `test_register_course_invalid_token` covers a non-contract address only. No test uses a contract that reports the wrong number of decimals. |

---

## Financial invariants

| ID | Invariant | Enforced by | Tests | Status |
|----|-----------|-------------|-------|--------|
| F1 | **Split is exact.** For every enrollment, `platform_amount + instructor_amount == amount_paid`, with both `>= 0`. | `deduct_fee` returns `(amount - fee, fee)`; free courses give `(0, 0)`. | `test_enroll_success_with_payment_split`, `test_fee_preview_matches_actual_enroll_deduction`, `test_enroll_fee_honors_registered_override_as_floor`, `test_enroll_zero_price_free_course` | ✅ |
| F2 | **Course revenue accounting.** For every course, `Course.total_earned == Σ (platform_amount + instructor_amount)` over every enrollment ever created for it: the active record plus every `EnrollmentHistory` entry, refunded ones included. `total_earned` is gross revenue, so refunds do not decrement it. | `enroll_internal`, `enroll_internal_validated` and `re_enroll` add `course.price` alongside an enrollment whose split sums to `course.price`; no other function writes `total_earned`. | `test_enroll_success_with_payment_split`, `test_multiple_students_same_course`, `test_enroll_total_earned_overflow`, `test_enroll_rolls_back_when_second_split_transfer_fails` | 🟡 No test sums over history after refunds and re-enrollments. |
| F3 | **No enrollment without confirmed payment.** An `Enrollment` with `amount_paid > 0` exists only if the contract received exactly `course.price` from the student in the same invocation. | Balance-delta check in `enroll_internal` (`"token transfer amount mismatch"`). Soroban atomicity reverts the enrollment if any transfer panics. | `test_enroll_insufficient_funds_rollback`, `test_enroll_rejects_fee_on_transfer_token`, `test_enroll_rolls_back_when_second_split_transfer_fails` | 🟡 `re_enroll` and `enroll_internal_validated` (batch) have the same check but no dedicated test. |
| F4 | **Earnings are backed by funds.** For every approved token, `token.balance(contract) >= Σ_instructors InstructorEarnings(instructor, token)`. Equality holds when no arbitration fees are held, no unsolicited transfers were received, and `withdraw_tokens` was not used. | Credit only after the student → contract transfer; debit before the contract → recipient transfer in `withdraw_earnings`, `process_refund` and `archive_course`. | `test_reconciliation_instructor_earnings_match_contract_balance` | ⚠️ `withdraw_tokens` can sweep earnings-backed funds (FU-1). |
| F5 | **Earnings ledger.** `InstructorEarnings(i, t) == Σ instructor_amount credited − Σ refund debits − Σ withdrawn`, and it is never negative. | `credit_instructor_earnings`; `debit_instructor_earnings` panics on underflow; `withdraw_earnings` panics if `amount > balance` or `amount < 0`. | `test_instructor_earnings_accumulate_and_withdraw`, `test_multiple_enrollments_aggregate_earnings`, `test_double_withdrawal_prevented`, `test_zero_balance_withdrawal_is_safe`, `test_unauthorized_instructor_withdraw_fails` | ✅ |
| F6 | **Lifetime earnings accumulator.** `InstructorTotalEarnings(i) == Σ_t (credits − refund debits)` across all tokens. Withdrawals do **not** reduce it. | Updated in `credit_instructor_earnings` / `debit_instructor_earnings` only. | `test_instructor_total_earnings_across_courses` | 🟡 Not checked after a refund or withdrawal (FU-2). |
| F7 | **Refunds return exactly what was collected.** An approved refund pays the student `platform_amount` from the treasury and `instructor_amount` from the contract, the values **persisted on the enrollment**, even if fees changed since. | `process_refund` and `archive_course` read `enrollment.platform_amount` / `instructor_amount`. | `test_refund_lifecycle`, `test_archive_course_with_refunds`, `test_archive_course_incremental_refunds` | 🟡 No test changes the fee between enrollment and refund (FU-3). |
| F8 | **Refunds debit the instructor that was credited.** | `process_refund` / `archive_course` debit `course.instructor`. | — | ⚠️ After `transfer_course` the debit hits the **new** instructor, although the credit went to the old one (FU-4). |
| F9 | **A failed split leaves no trace.** If any transfer inside `enroll` fails, including the treasury transfer **after** the student's payment landed, no Enrollment, `InstructorEarnings` credit, `total_earned`, `total_enrollments`, `active_enrollments`, `TotalActiveEnrollments`, `InstructorStats` or student enrollment count update persists, and every token balance is restored. | Soroban runs each invocation atomically. | `test_enroll_rolls_back_when_second_split_transfer_fails`, `test_enroll_insufficient_funds_rollback` | ✅ |
| F10 | **Arbitration fees are conserved.** Each `fee_paid` held by the contract is paid out exactly once, to the student or the instructor, when the case leaves `Open`. | `resolve_arbitration` panics unless the case is `Open`. | `test_resolve_arbitration_refund_to_student`, `test_resolve_arbitration_payout_to_instructor`, `test_duplicate_escalation_rejected` | ✅ |

---

## Enrollment invariants

| ID | Invariant | Enforced by | Tests | Status |
|----|-----------|-------------|-------|--------|
| E1 | **At most one active enrollment per (student, course).** | `validate_enrollment` (`"already enrolled in this course"`); `batch_enroll` rejects duplicates; `re_enroll` overwrites only a completed record. | `test_enroll_duplicate`, `test_batch_enroll_rejects_duplicates`, `test_batch_enroll_rejects_duplicate_in_large_batch` | ✅ |
| E2 | **`completed` is one-way** on a given record. Only `re_enroll` replaces the record, after archiving it. | `mark_completed` is the only writer (`"already marked as completed"`). | `test_mark_completed_twice_rejected`, `test_mark_completed_twice_idempotency_check`, `test_re_enroll_archives_original_completion_and_certificate` | ✅ |
| E3 | **`certificate_issued` is one-way** and implies `completed`. A revoked certificate is never re-issued on the same enrollment. | `issue_certificate` requires `completed && !certificate_issued`; revocation never clears it. | `test_certificate_requires_completion`, `test_issue_certificate_after_revoke_is_rejected`, `test_reissue_certificate_allowed_after_re_enroll` | ✅ |
| E4 | **Refunded enrollments are never completed or certified**, and refunds only apply to non-completed enrollments. | `mark_completed` / `issue_certificate` reject `is_refunded`; `request_refund` and `archive_course` skip completed ones. | `test_refund_lifecycle`, `test_archive_course_with_refunds` | ⚠️ `process_refund` does not re-check `completed`, so a completion between request and approval is still refunded (FU-5). |
| E5 | **Refunds are archived, not deleted.** Every refund appends the record to `EnrollmentHistory` with `is_refunded = true` before removing the active key. | `process_refund`, `archive_course`. | `test_process_refund_archives_enrollment_to_history`, `test_archive_course_with_refunds` | ✅ |
| E6 | **Re-enrollment preserves history.** `re_enroll` appends the previous record unchanged (including `evidence_hash` and `certificate_id`) before writing the new one. | `re_enroll`. | `test_re_enroll_archives_original_completion_and_certificate`, `test_re_enroll_multiple_times_accumulates_history`, `test_re_enroll_before_completion_fails`, `test_re_enroll_without_prior_enrollment_fails` | ✅ |
| E7 | **Expired enrollments cannot be completed.** | `mark_completed` expiry check; `has_completed` returns `Some(false)`. | `test_mark_completed_panics_when_enrollment_expired`, `test_has_completed_returns_false_when_enrollment_expired`, `test_has_completed_returns_true_when_completed_before_expiry` | ✅ |
| E8 | **Active counters match records.** `course.active_enrollments` == number of that course's active records with `!completed`, and `TotalActiveEnrollments == Σ_courses active_enrollments`. | Incremented in enroll paths; decremented in `mark_completed`, `process_refund`, `archive_course`. | `test_platform_counter_decrements_on_completion`, `test_platform_counter_decrements_on_refund`, `test_enrollment_blocked_at_platform_cap` | 🟡 Expired enrollments stay counted by design (see STATE_MACHINES.md note 3). Not checked as a general sum (FU-6). |
| E9 | **Capacity.** `total_enrollments <= max_capacity` when set. | `validate_enrollment`, `re_enroll`. | `test_enroll_at_capacity_succeeds`, `test_enroll_beyond_capacity_rejected`, `test_batch_enroll_respects_capacity` | ✅ |

---

## Course state-machine invariants

| ID | Invariant | Enforced by | Tests | Status |
|----|-----------|-------------|-------|--------|
| C1 | **Only `Active` courses accept payment.** | Status checks in `validate_enrollment`, `enroll_internal` (re-check), `enroll_internal_validated`, `re_enroll`, `join_waitlist`. | `test_enroll_pending_course`, `test_enroll_rejected_for_just_paused_course`, `test_enrollment_rejected_event_emitted_for_pending_course`, `test_enrollment_rejected_event_emitted_for_paused_course`, `test_enrollment_rejected_event_emitted_for_archived_course` | ✅ |
| C2 | **Every transition matches the table in STATE_MACHINES.md.** No function moves a course along an edge that is not listed there. | Per-function status guards. | `test_approve_course_success`, `test_approve_already_active_course`, `test_pause_and_unpause_course`, `test_pause_course_double_pause_rejected`, `test_archive_active_course_rejected`, `test_archive_pending_course_rejected`, `test_archive_paused_course_succeeds`, `test_freeze_instructor_lifecycle`, `test_frozen_instructor_cannot_unpause_course` | 🟡 `reject_course` → `Rejected` and "Rejected is terminal" are not asserted (FU-7). |
| C3 | **Archiving needs zero active enrollments.** | `archive_course` saves and returns early while `active_enrollments > 0`. | `test_archive_course_blocked_by_active_enrollment`, `test_archive_course_incremental_refunds` | ✅ |
| C4 | **Archived course IDs are not reused within the cooldown.** | `register_course` checks `ArchivedCourse(id).archived_at_ledger + ARCHIVE_COOLDOWN_LEDGERS`. | `test_archive_then_reregister_fails` | 🟡 Re-registration after the cooldown is untested, and is blocked anyway if `DataKey::Course(id)` was recreated (STATE_MACHINES.md note 6, FU-8). |
| C5 | **High-value courses need two distinct approvers.** | `approve_course` `HighValueCourseApproval` handling. | — | ❌ (FU-9) |

---

## Certificate invariants

| ID | Invariant | Enforced by | Tests | Status |
|----|-----------|-------------|-------|--------|
| R1 | **Certificate IDs are unique.** | `issue_certificate` (`"certificate ID already exists"`). | `test_certificate_id_collision_across_courses`, `test_course_certificate_id_collision_verification` | ✅ |
| R2 | **Revocation is permanent once the challenge deadline passes.** `Certificate.revoked` may only go from `true` back to `false` through `challenge_revocation`, by the holder, while `ledger < revocation_deadline`. After that it never flips back. | `apply_revocation` is the only writer of `true`; `challenge_revocation` is the only writer of `false` and enforces the deadline. | `test_issue_146_revocation_becomes_permanent_after_deadline`, `test_issue_146_late_challenge_fails`, `test_issue_146_challenge_boundary_condition`, `test_issue_146_unauthorized_challenge_fails`, `test_revoke_certificate_id_cannot_be_reused_to_unrevoke`, `test_revoke_certificate_double_revocation_rejected` | ✅ |
| R3 | **Revoked certificates are never deleted**, and revocation metadata (`revoked_by`, `revocation_ledger`, `revocation_reason`) is set together with `revoked`. | `apply_revocation`. | `test_revoke_certificate_metadata_persisted`, `test_revoke_certificate_metadata_unchanged_after_rejected_double_revoke`, `test_certificate_issued_by_persists_after_revocation` | ✅ |
| R4 | **`certificates_issued <= max_certificates`** when set, and `course.certificates_issued == len(CourseCertificates(course_id))`. | `issue_certificate`. | — | ❌ (FU-10) |

---

## Follow-ups

These are the invariants that are unverified or known not to hold. Each one
should become its own GitHub issue.

| ID | Invariant | Suggested issue |
|----|-----------|-----------------|
| FU-1 | F4 | `withdraw_tokens` can sweep tokens that back `InstructorEarnings`. Cap the sweep at `balance − Σ InstructorEarnings − open arbitration fees`, or document it as an emergency-only break-glass. |
| FU-2 | F6 | Add a test showing `InstructorTotalEarnings` drops on refund and stays put on withdrawal, and document that it is a lifetime figure. |
| FU-3 | F7 | Add a test that changes `FeeConfig(token)` / `DefaultFee` between enrollment and refund and asserts the refund uses the persisted split. |
| FU-4 | F8 | Refunds after `transfer_course` debit the new instructor's earnings for revenue credited to the old one. Decide the intended owner and add a test. |
| FU-5 | E4 | `process_refund` should reject an enrollment completed after the refund request, and get a test for it. |
| FU-6 | E8, F2 | Property test: after a random sequence of enroll / complete / refund / re-enroll, check `active_enrollments`, `TotalActiveEnrollments` and `total_earned == Σ splits over active + history`. |
| FU-7 | C2 | Test `reject_course` sets `Rejected`, that `approve_course` / `pause_course` / `unpause_course` then panic, and that `InstructorPendingCourseCount` is handled (it is currently not decremented). |
| FU-8 | C4 | Test re-registration after `ARCHIVE_COOLDOWN_LEDGERS`, and stop `issue_certificate` / `process_refund` from writing an archived course back to `DataKey::Course(id)`. |
| FU-9 | C5 | Test the high-value two-approver flow: the first call leaves `Pending`, the same approver twice panics, and a second distinct approver activates the course. |
| FU-10 | R4 | Test the `max_certificates` limit and that the `CourseCertificates` index matches `certificates_issued`. |
| FU-11 | T2 | Add a rebasing-token mock to show the drift between contract balance and `Σ InstructorEarnings`, so the T2 assumption is covered by a test. |
