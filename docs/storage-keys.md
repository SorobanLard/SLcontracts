# Storage Key Classification

This document classifies every `DataKey` and `DataKey2` variant in
`contracts/sorobanlard/src/lib.rs` by the Soroban storage tier it uses. Keep it
in sync whenever a key is added, renamed, or moved.

`DataKey2` exists only because `#[contracttype]` enums have a variant-count
limit; it is otherwise an ordinary key namespace and the same tier rules apply.

## Why the tier matters

Soroban has two durable storage tiers with different rent and lifetime
semantics:

- **Instance storage** is a single entry attached to the contract instance.
  It shares one TTL/rent budget, is extended by most state-changing methods,
  and is best for small, hot, singleton state. It is *not* suitable for
  per-entity records or collections that grow with usage.
- **Persistent storage** gives every key its own entry and TTL. Entries can be
  extended individually and archived independently, which is required for
  per-entity records and append-only lists that may grow without bound.

Using the wrong tier is not merely a cost issue: `instance()` and
`persistent()` are separate key spaces. Reading a key through a different tier
than it was written to makes the value look missing.

## Rule of thumb

> **Singleton config, auth flag, or global counter → instance storage.
> Per-course / per-student / per-instructor record or append-only list →
> persistent storage.**

Small per-token/per-address flags are kept in instance storage because they are
read on hot paths (enrollment validation, authorization) and are tightly bounded
by the token whitelist or admin-controlled lists. Anything whose size is driven
by users, courses, enrollments, or certificates must be persistent.

## Full classification

| Key | Tier | Reason |
|---|---|---|
| `DataKey::Course(String)` | Persistent | Per-course record; one entry per course. |
| `DataKey::CourseInstructorRef(String)` | Persistent | Per-course immutable lookup; one entry per course. |
| `DataKey::Enrollment(Address, String)` | Persistent | Per-student/per-course record. |
| `DataKey::Certificate(String)` | Persistent | Per-certificate record. |
| `DataKey::Admin` | Instance | Singleton authorization config. |
| `DataKey::SecondaryAdmin` | Instance | Singleton multi-sig authorization config. |
| `DataKey::AdminExpiresAt` | Instance | Singleton admin-expiry config. |
| `DataKey::Treasury` | Instance | Singleton payout address. |
| `DataKey::DefaultFee` | Instance | Singleton global fee config. |
| `DataKey::PendingTreasury` | Instance | Singleton pending treasury update. |
| `DataKey::ApprovedToken(Address)` | Instance | Small, bounded whitelist flag read on enrollment; one flag per approved token. |
| `DataKey::PendingAdmin` | Instance | Singleton pending admin transfer. |
| `DataKey::PendingSecondaryAdmin` | Instance | Singleton pending admin transfer. |
| `DataKey::PlatformPaused` | Instance | Singleton global pause flag. |
| `DataKey::InstructorEarnings(Address, Address)` | Persistent | Per-instructor/per-token balance that grows with activity. |
| `DataKey::InstructorTotalEarnings(Address)` | Persistent | Per-instructor accumulator. |
| `DataKey::InstructorCourseCount(Address)` | Instance | Small per-instructor counter, bounded by `MaxCoursesPerInstructor`. |
| `DataKey::InstructorPendingCourseCount(Address)` | Instance | Small per-instructor counter, bounded by `MaxCoursesPerInstructor`. |
| `DataKey::InstructorCourseList(Address)` | Persistent | Per-instructor append-only list; grows with courses. |
| `DataKey::MaxCoursesPerInstructor` | Instance | Singleton global config. |
| `DataKey::MinReviewDelay` | Instance | Singleton global config. |
| `DataKey::DefaultMinCompletionLedgers` | Instance | Singleton global config. |
| `DataKey::RefundWindow` | Instance | Singleton global config. |
| `DataKey::RefundRequest(Address, String)` | Persistent | Per-student/per-course record. |
| `DataKey::InstructorBlocked(Address)` | Instance | Small, bounded authorization flag read on hot paths. |
| `DataKey::CourseList` | Persistent | Global append-only catalog; grows with courses. |
| `DataKey::EnrollmentHistory(Address, String)` | Persistent | Per-student/per-course append-only history. |
| `DataKey::InstructorStats(Address)` | Persistent | Per-instructor record that grows with activity. |
| `DataKey::StudentBlocked(Address)` | Instance | Small, bounded authorization flag read on hot paths. |
| `DataKey::FeeConfig(Address)` | Instance | Small per-token config, bounded by the token whitelist. |
| `DataKey::ArbitrationFeeConfig` | Instance | Singleton fee config. |
| `DataKey::RiskFeeConfig` | Instance | Singleton fee config. |
| `DataKey::RiskConfigEnabled` | Instance | Singleton feature flag. |
| `DataKey::TotalActiveEnrollments` | Instance | Single global counter (see race-freedom note below). |
| `DataKey::PlatformEnrollmentCap` | Instance | Singleton global config. |
| `DataKey::PendingUpgrade` | Instance | Singleton pending governance action. |
| `DataKey::UpgradeTimelock` | Instance | Singleton governance config. |
| `DataKey::CourseWaitlist(String)` | Persistent | Per-course append-only list; grows with students. |
| `DataKey::InstructorRegistry` | Persistent | Global append-only list; grows with instructors. |
| `DataKey::ArchivedCourse(String)` | Persistent | Per-course archived record. |
| `DataKey::RevocationChallengePeriod` | Instance | Singleton global config. |
| `DataKey::CourseCertificates(String)` | Persistent | Per-course append-only certificate list. |
| `DataKey::EnrollmentByRef(String)` | Persistent | Per-enrollment lookup record. |
| `DataKey::EnrollmentRefCounter` | Instance | Single global counter. |
| `DataKey::Approver(Address)` | Instance | Small, bounded authorization flag read on hot paths. |
| `DataKey::InitLedger` | Instance | Singleton initialization marker. |
| `DataKey::LastAdminTransferLedger` | Instance | Singleton cooldown marker. |
| `DataKey::CourseFailureCount(String)` | Persistent | Per-course counter; one record per course. |
| `DataKey::BtcEthToken(Address)` | Instance | Small per-token classification flag, bounded by the token whitelist. |
| `DataKey::StudentEnrollmentCount(Address)` | Persistent | Per-student lifetime counter. |
| `InstructorRegisteredKey { instructor }` | Persistent | Standalone key; one flag per instructor recording registry membership, so `register_course` avoids scanning `InstructorRegistry`. |
| `DataKey2::HighValueCourseThreshold` | Instance | Singleton global config. |
| `DataKey2::HighValueCourseApproval(String)` | Instance | Short-lived per-course co-signature state (transient governance step). |

## Platform counter race-freedom

`TotalActiveEnrollments` is an instance-storage counter, not a persistent
per-entity record, because it is a single global value read on every enrollment
cap check. It does not need a compare-and-swap or version field:

- Soroban executes a contract invocation sequentially. A single invocation's
  read-modify-write of the counter is never interleaved with another
  invocation.
- Concurrent-looking transactions are ordered by consensus into separate
  ledgers; each one observes the state committed by its predecessor.
- A panic rolls back the entire invocation, including the counter write, so
  failed enrollments cannot leave a partial increment behind.

`test_stress_total_active_enrollments_sequential` exercises many sequential
enrollments and completions across many courses and asserts the counter against
an independently computed expected value.

## Adding a new key

1. Decide the tier using the rule of thumb above.
2. Access the key through exactly one tier everywhere (`instance()` *or*
   `persistent()`), never both.
3. Extend the TTL on persistent writes and reads, as existing helpers do.
4. Add the variant to this table.
