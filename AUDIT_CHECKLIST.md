# SorobanLard Security Audit Checklist

This is a self-review checklist for maintainers. Work through it before any external audit. Generic Soroban/Rust checklists don't cover this contract's specific risks, so this one focuses on them. It is ordered by impact: **fund-moving operations first**, then **access control**, then **availability / DoS**.

All functions are in [`contracts/sorobanlard/src/lib.rs`](contracts/sorobanlard/src/lib.rs). Related issues are linked as `#NN`. Related invariants (see [INVARIANTS.md](INVARIANTS.md)) are shown as `INV-NN`.

**How to use it:** tick each item only after reading the code path, and link the commit you reviewed. When an item is accepted as a known risk rather than fixed, write down the decision and why next to it.

---

## 1. Fund-moving operations (critical)

### 1.1 Enrollment payment split — `enroll`, `batch_enroll`, `re_enroll`, `enroll_internal`, `deduct_fee`
- [ ] Treasury fee + instructor earnings credit == amount the student paid, in every path (`enroll`, `batch_enroll`, `re_enroll`, waitlist promotion). — INV-01
- [ ] When any transfer in the split fails, no `Enrollment`, `InstructorEarnings` credit or `total_earned` update is left behind. — #139
- [ ] Integer truncation in `price * bps / 10000` always favours the instructor share and never creates or loses a stroop.
- [ ] `checked_mul` / `overflow-checks = true` cannot abort a legitimate enrollment at `MAX_COURSE_PRICE_STROOPS` with maximum surcharge stacking. — #231
- [ ] The course's registration-time fee override acts as a floor (`deduct_fee`) and can never push the fee above 100%.
- [ ] A student cannot enroll twice for one payment, and cannot be charged twice for one enrollment. — INV-02

### 1.2 Risk-fee surcharge and fee griefing — `set_fee_config`, `set_risk_fee_config`, `set_risk_config_enabled`, `compute_fee_breakdown`, `risk_score_internal`
- [ ] The effective fee is capped at 10,000 bps and `platform_fee <= payment_amount`. — INV-05
- [ ] **Fee griefing:** a *single* admin can raise `fee_bps` or the surcharges to 100% at any time (no multi-sig, no delay). Students have no `max_fee` / slippage parameter on `enroll`, so a quote from `get_effective_fee_for_payment` can change before the enrollment transaction lands. Decide whether fee changes need a delay, a cap below 100%, or multi-sig.
- [ ] `set_risk_fee_config` does not bound the individual `*_surcharge_bps` values. Confirm that only the final clamp protects students, and that this is intended.
- [ ] `resolve_risk_flags` currently returns `(false, false)`. When real per-student or BTC/ETH flags are added, the preview (`calculate_risk_score`, `get_effective_fee_for_payment`, `do_complete_payment`) and the actual charge must still read the same source.
- [ ] Every surcharge that is applied emits `risk_fee_applied`, so off-chain systems can see fee changes.

### 1.3 Arbitration fee pool custody — `escalate_to_arbitration`, `resolve_arbitration`
- [ ] The contract holds exactly `fee_paid` per open case, and resolution releases exactly that amount once. — INV-07, #237
- [ ] There is no on-chain reconciliation (`sum(open fee_paid)` vs. the contract's token balance). Decide whether to add `get_arbitration_pool_balance(token)`. — #237
- [ ] Arbitration fees share the contract's token balance with instructor earnings. Confirm a payout from one pool can never consume funds owed to the other.
- [ ] A case resolved with `for_student = false` pays the instructor recorded *at escalation time*. Check this is correct after a `transfer_course`.
- [ ] `case_id` comes from `course_id` alone. Confirm nothing relies on it being globally unique across callers.
- [ ] `resolve_arbitration` is a single-admin action that moves funds. Decide whether that is acceptable.

### 1.4 Emergency sweep — `withdraw_tokens`
- [ ] Both admins together can sweep **any** amount of any token held by the contract, including unwithdrawn `InstructorEarnings` and open arbitration fees. Decide whether to subtract outstanding obligations before a sweep, or document it as an emergency-only power. — #230
- [ ] Zero or negative amounts are rejected before the token is called (#236). The same admin signing twice is rejected. — INV-08

### 1.5 Instructor withdrawals — `withdraw_earnings`, `credit_instructor_earnings`, `debit_instructor_earnings`
- [ ] Withdrawals never exceed the credited balance, and the balance drops by exactly the amount paid out. — INV-06
- [ ] Storage is updated before the outgoing token transfer (checks-effects-interactions).
- [ ] A frozen instructor's ability to withdraw is an explicit, documented decision.

### 1.6 Refunds — `request_refund`, `process_refund`, `archive_course`
- [ ] Refunds are debited from the instructor's earnings. Check behaviour when the instructor has already withdrawn (the documented "instructor insolvency" panic).
- [ ] `archive_course` refund lists are capped at `MAX_STUDENTS_TO_REFUND`, and the same student cannot be refunded twice across incremental calls.
- [ ] The refund window (`update_refund_window`) cannot be changed in a way that strands refunds that are already pending.
- [ ] A refunded enrollment is archived to history with `is_refunded = true` and can no longer produce a certificate (#254).

---

## 2. Access control

### 2.1 Dual-admin multi-sig gaps — `require_multi_admin`, `require_admin`
- [ ] Every function described as multi-sig calls `require_auth()` on **both** addresses *and* `require_multi_admin`. — INV-08
- [ ] List every single-admin operation by blast radius (irreversible > reversible, financial > non-financial): `issue_certificate`, `revoke_certificate`, `bulk_revoke_course_certificates`, `set_fee_config`, `set_risk_fee_config`, `resolve_arbitration`, `block_student`, `freeze_instructor`, `pause_platform`, `remove_approved_token`. Decide which ones should move to multi-sig. — #223
- [ ] Approvers (`add_approver`, `require_admin_or_approver`) can only reach the functions they are meant to.
- [ ] Admin expiry (`set_admin_expiry`) blocks single-admin and multi-admin paths alike, and `transfer_admin` / `accept_admin` is the only way out of it.
- [ ] `transfer_admin` cooldown (`MIN_ADMIN_TRANSFER_COOLDOWN`) and the two-step `accept_admin` handover cannot be bypassed, for example by proposing the same address as both new admins.
- [ ] The `update_treasury` delay (100 ledgers) cannot be skipped, and fees during the delay window go to the old treasury, as documented.

### 2.2 Time-locked upgrade mechanism — `propose_upgrade`, `upgrade_contract`, `cancel_upgrade`, `set_upgrade_timelock`
- [ ] `upgrade_contract` cannot run before `effective_ledger`. — INV-09
- [ ] `set_upgrade_timelock` enforces `MIN_UPGRADE_TIMELOCK_LEDGERS`, so a compromised admin pair cannot set it to 0. — INV-09, #143
- [ ] The mainnet time-lock is set to the value recommended in the README ("Upgrade Time-Lock Guidance") *before* any funds are deposited.
- [ ] A re-proposal replaces the pending upgrade and resets the time-lock. Confirm observers can detect it (`upgrade_proposed` event).
- [ ] `cancel_upgrade` needs both admins, so one honest admin cannot stop a malicious proposal alone. Decide whether a single-admin emergency cancel should exist. — #141
- [ ] After an upgrade, existing storage layouts (`DataKey`, `Course`, `Enrollment`, `Certificate`) still decode under the new code, and `CONTRACT_VERSION` was bumped.

### 2.3 Instructor and student controls — `freeze_instructor`, `transfer_course`, `block_student`
- [ ] A frozen instructor cannot get around the freeze through `transfer_course`, by registering new courses, or through ordering edge cases. — #240
- [ ] `enroll` checks the instructor's frozen status on its own, not only the course status. — #240
- [ ] Blocked students are rejected in every path listed in the Student Blocking Policy (enroll, batch, re-enroll, completion, certificate, refund).

### 2.4 Credentials — `issue_certificate`, `revoke_certificate`, `challenge_revocation`, `get_certificate`
- [ ] A certificate needs a completed enrollment and cannot be issued twice for one enrollment. — INV-03
- [ ] A finalised revocation is permanent, and `challenge_revocation` only works inside the challenge period. — INV-04
- [ ] `get_certificate` read-auth: only the student, instructor or admin can read the full record.

---

## 3. Availability / DoS

- [ ] **Unbounded vectors:** check every `Vec` stored in a single entry and walked in full (waitlists in `join_waitlist` / `leave_waitlist` / `promote_from_waitlist`, `get_course_certificates`, `get_all_instructors`, `bulk_revoke_course_certificates`). A large list must not push a call past the CPU or ledger-entry-size limits and brick the function.
- [ ] Batch limits are enforced: `MAX_BATCH_SIZE` (batch_enroll), `MAX_PAGE_SIZE` (list_courses), `MAX_STUDENTS_TO_REFUND` (archive_course).
- [ ] **Circuit breaker** (`CIRCUIT_BREAKER_THRESHOLD`): an outside party must not be able to trigger consecutive token-transfer failures that auto-pause a course another instructor depends on.
- [ ] Persistent and instance storage TTLs are extended on every read and write path that matters, so a course, enrollment, certificate or arbitration case cannot expire and lose funds or credentials.
- [ ] `pause_platform` is a single-admin action. Confirm a paused platform still allows withdrawals and refunds that are already owed.
- [ ] Panic-based errors give clients no structured failure reasons (#232). Confirm no guard depends on a specific panic string across versions.
- [ ] An approved token contract that is malicious or upgradeable (`add_approved_token`) cannot re-enter or block enrollments or withdrawals for other tokens.

---

## Before the external audit

- [ ] Every item above is ticked, or recorded as an accepted risk with the reason.
- [ ] `cargo test` passes, including the invariant suite (`cargo test invariants::`), and `scripts/check_invariants.sh` reports no gaps.
- [ ] The README "Security Considerations" section matches the decisions recorded here. — #208
- [ ] Admin keys are multi-sig wallets, and the key-management setup is documented. — #184
