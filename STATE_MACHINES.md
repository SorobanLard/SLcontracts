# State Machines

This document lists every state transition in the SorobanLard contract for
the two lifecycles that matter most: a course's `CourseStatus`, and a
student's `Enrollment`. Each row names the function that causes the
transition, who may call it, and the checks that must pass.

All function references are in [contracts/sorobanlard/src/lib.rs](contracts/sorobanlard/src/lib.rs).
The financial guarantees that ride on these transitions are listed in
[INVARIANTS.md](INVARIANTS.md).

---

## 1. `CourseStatus`

```
                 register_course()
                        │
                        ▼
                  ┌──────────┐  reject_course()   ┌──────────┐
                  │ Pending  │ ─────────────────▶ │ Rejected │  (terminal)
                  └──────────┘                    └──────────┘
                        │ approve_course()
                        ▼
                  ┌──────────┐  pause_course() / freeze_instructor()
                  │  Active  │ ──────────────────────────────────────┐
                  └──────────┘                                       ▼
                        ▲                                      ┌──────────┐
                        └────────────── unpause_course() ───── │  Paused  │
                                                               └──────────┘
                                                                     │ archive_course()
                                                                     │ (active_enrollments == 0)
                                                                     ▼
                                                               ┌──────────┐
                                                               │ Archived │  (terminal)
                                                               └──────────┘
```

### Transition table

| # | From | To | Function | Caller | Guards (panics otherwise) |
|---|------|----|----------|--------|---------------------------|
| C1 | *(no record)* | `Pending` | `register_course` | Instructor (signs) | Token reports 7 decimals; instructor not frozen; valid `course_id`; valid price/fee/content hash; no `DataKey::Course(id)` already exists; if `DataKey::ArchivedCourse(id)` exists, `ARCHIVE_COOLDOWN_LEDGERS` (172,800) must have passed since `archived_at_ledger`; per-instructor course and pending-course limits. |
| C2 | `Pending` | `Active` | `approve_course` | Admin or approver | Governance window since `init` has passed (`MinReviewDelay`); course review delay since `created_at_ledger` has passed; status is `Pending`. If `price >= HighValueCourseThreshold`, the **first** call only records the approver and returns (status stays `Pending`). A **second, different** admin/approver performs the transition. |
| C3 | `Pending` | `Rejected` | `reject_course` | Admin or approver | Status is `Pending`. |
| C4 | `Active` | `Paused` | `pause_course` | Admin or the course instructor | Status is `Active`. |
| C5 | `Active` | `Paused` | `freeze_instructor` | Admin | Applies to every `Active` course in the instructor's `InstructorCourseList`. Other statuses are skipped. |
| C6 | `Active` | `Paused` | `enroll` → `enroll_internal` (circuit breaker) | Any student (indirect) | Fires when `CourseFailureCount(id) >= CIRCUIT_BREAKER_THRESHOLD` (5). **Note:** the early-exit branch writes `Paused` and then panics, so Soroban reverts the write with the rest of the call. See [Notes](#notes-and-known-gaps). |
| C7 | `Paused` | `Active` | `unpause_course` | Admin, or the course instructor if not frozen | Status is `Paused`. A frozen instructor cannot undo a freeze-pause. Only an admin can. |
| C8 | `Paused` | `Archived` | `archive_course` | Admin **and** secondary admin (multi-sig) | Status is `Paused`. Optional `students_to_refund` (≤ `MAX_STUDENTS_TO_REFUND`) is refunded first. The transition happens **only if `active_enrollments == 0`** afterwards. Otherwise the decremented counts are saved and the course stays `Paused`, so refunds can continue in batches. On success the record moves from `DataKey::Course(id)` to `DataKey::ArchivedCourse(id)` and `archived_at_ledger` is set. |

### Non-transitions (status unchanged)

| Function | Effect on status |
|----------|------------------|
| `unfreeze_instructor` | None. Courses paused by `freeze_instructor` stay `Paused` until someone calls `unpause_course`. |
| `transfer_course` | None. Allowed from any status except `Archived`. |
| `update_course`, `set_enrollment_expiry`, `set_course_expiry`, `set_prerequisite_courses`, `update_content_hash`, `set_max_certificates`, `clear_course_max_capacity`, `set_enrollment_start_ledger` | None. These panic on `Archived` (via the `Course` key lookup or an explicit check). |
| `pause_platform` | None. It blocks enrollment platform-wide through `PlatformPaused` without touching any course's status. |

### Rejected and Archived are terminal

- **Rejected** has no outgoing transition. The record stays under
  `DataKey::Course(id)`, so `register_course` with the same ID panics with
  `"course already registered"` forever.
- **Archived** has no outgoing transition. After the cooldown, the same ID can
  be registered again, but that creates a **new** `Pending` course
  (counters reset to zero). It does not revive the archived record.

### What each status allows

| Status | `enroll` / `batch_enroll` / `re_enroll` / `join_waitlist` | `mark_completed` | `issue_certificate` | `process_refund` |
|--------|------|------|------|------|
| `Pending` | ✗ | n/a (no enrollment can exist) | n/a | n/a |
| `Active` | ✓ | ✓ | ✓ | ✓ |
| `Paused` | ✗ | ✓ | ✓ | ✓ |
| `Archived` | ✗ | ✗ (explicit check) | ✓ for completed enrollments (see [note 6](#notes-and-known-gaps)) | ✓ if a request is still pending (see [note 6](#notes-and-known-gaps)) |
| `Rejected` | ✗ | n/a (no enrollment can exist) | n/a | n/a |

`get_course_internal` falls back to `DataKey::ArchivedCourse(id)`, so
functions that only look up the course still find an archived one.

---

## 2. `Enrollment` lifecycle

An enrollment is stored at `DataKey::Enrollment(student, course_id)`. Its
state is not a single enum. It comes from the flags `completed`,
`certificate_issued` and `is_refunded`, from where the record is stored
(active key or `DataKey::EnrollmentHistory`), and, for expiry, from the
current ledger.

| State | How it is represented |
|-------|-----------------------|
| **None** | No `Enrollment(student, course_id)` key. |
| **Active** | Key exists, `completed == false`, `is_refunded == false`, not expired. |
| **Expired** *(derived)* | Active, but `course.enrollment_expiry_ledgers = Some(n)` and `ledger >= enrolled_at_ledger + n`. Nothing is written. It is computed on read. |
| **Completed** | Key exists, `completed == true`, `certificate_issued == false`. |
| **CertificateIssued** | Key exists, `completed == true`, `certificate_issued == true`, `certificate_id = Some(..)`. |
| **Refunded** | Active key **removed**. A copy with `is_refunded == true` is appended to `EnrollmentHistory(student, course_id)`. |
| **Archived-to-history** | The Completed/CertificateIssued record is appended unchanged to `EnrollmentHistory` and a new Active record overwrites the active key. |

```
                enroll() / batch_enroll() / waitlist promotion
     None ───────────────────────────────────────────────────────▶ Active
      ▲                                                             │  │  │
      │ (key removed)                                               │  │  │ ledger passes expiry
      │                                                             │  │  ▼
   Refunded ◀── process_refund(approved) / archive_course(refund) ──┘  │ Expired ──▶ (refund only)
   (history)                                                           │
                                                      mark_completed() │
                                                                       ▼
                       ┌───────────── re_enroll() ─────────────── Completed
                       │                                               │ issue_certificate()
                       │                                               ▼
                       ├───────────── re_enroll() ───────────  CertificateIssued
                       ▼
    old record → EnrollmentHistory,  new record → Active
```

### Transition table

| # | From | To | Function | Caller | Guards (panics otherwise) | Side effects |
|---|------|----|----------|--------|---------------------------|--------------|
| E1 | None | Active | `enroll` → `enroll_internal` | Student (signs) | Platform not paused; student not blocked, not admin, not the contract, not the instructor; instructor not frozen; course `Active`; past registration ledger and `enrollment_start_ledger`; **no existing enrollment**; under `max_capacity`, course expiry and platform cap; token approved with 7 decimals; prerequisites certified and not revoked. | Full price student → contract (balance-delta checked); platform share contract → treasury; instructor share credited to `InstructorEarnings`; `total_enrollments`, `active_enrollments`, `total_earned`, `TotalActiveEnrollments`, `InstructorStats.total_students`, student enrollment count incremented. |
| E2 | None | Active | `batch_enroll` → `enroll_internal_validated` | Student (signs) | Same as E1, checked up front for every course in the batch. No duplicates allowed within a batch. | Same as E1, per course. |
| E3 | None | Active | `promote_from_waitlist` → `enroll_internal` | Internal (runs after each `archive_course` refund) | Same as E1 for the waitlisted student. Because `archive_course` requires the course to be `Paused`, this check always fails (see [note 7](#notes-and-known-gaps)). | Same as E1. |
| E4 | Active | Completed | `mark_completed` | Admin, plus student co-signature unless a non-empty `evidence_hash` is given | Student not blocked; course exists and is not `Archived`; not refunded; not already completed; `min_completion_ledgers` have passed; **not expired**. | `completed = true`, `completed_at_ledger` set; `active_enrollments`, `TotalActiveEnrollments` decremented; `InstructorStats.total_completions` incremented. |
| E5 | Completed | CertificateIssued | `issue_certificate` | Admin | Student not blocked; enrollment exists, completed, not refunded, `certificate_issued == false`; certificate ID unused; under `max_certificates`. | `Certificate` stored; `certificate_issued = true`, `certificate_id` set; `course.certificates_issued`, `InstructorStats.total_certificates` incremented; ID appended to `CourseCertificates(course_id)`. |
| E6 | Active / Expired | *(RefundRequest Pending)* | `request_refund` | Student (signs) | Student not blocked; enrollment exists and is **not completed**; `ledger - enrolled_at_ledger <= RefundWindow`; no `RefundRequest(student, course_id)` already exists (any status). | Creates `RefundRequest { status: Pending }`. Enrollment unchanged. |
| E7 | Active / Expired (with Pending request) | Refunded | `process_refund(approved = true)` | Admin (treasury must also authorise its transfer) | Request is `Pending`; course and enrollment exist. | Platform share treasury → student; instructor share debited from `InstructorEarnings` and contract → student; enrollment moved to history with `is_refunded = true`; active key removed; `active_enrollments`, `TotalActiveEnrollments` decremented; request → `Approved`. `total_earned` is **not** decremented. |
| E8 | *(RefundRequest Pending)* | *(RefundRequest Rejected)* | `process_refund(approved = false)` | Admin | Request is `Pending`. | Enrollment unchanged. |
| E9 | Active / Expired | Refunded | `archive_course(students_to_refund)` | Admin + secondary admin | Course `Paused`; enrollment exists, `!completed && !is_refunded`. | Same token movements and history write as E7; `active_enrollments`, `TotalActiveEnrollments` decremented; then the next waitlisted student is promoted (E3). |
| E10 | Completed / CertificateIssued | Archived-to-history + new Active | `re_enroll` | Student (signs) | Platform not paused; student not blocked, not instructor; instructor not frozen; course `Active`; token approved; existing enrollment **is completed**; under capacity and course expiry. | Old record (with `evidence_hash`, `certificate_id`) appended to `EnrollmentHistory`; payment and split as in E1; new record with fresh `enrollment_ref` overwrites the active key; counters incremented as in E1. |
| E11 | Active | Expired | *(ledger time passes)* | — | `course.enrollment_expiry_ledgers` is `Some(n)` and `ledger >= enrolled_at_ledger + n`. | Nothing is written. `has_completed` returns `Some(false)`; `mark_completed` panics with `"enrollment has expired"`. |
| E12 | Refunded | Active | `enroll` (E1) | Student | The active key was removed, so the "no existing enrollment" check passes. | A new enrollment. The old `RefundRequest` (now `Approved`) stays, so this enrollment **cannot be refunded through `request_refund`**. |

### Certificate revocation does not change the enrollment

`revoke_certificate`, `bulk_revoke_course_certificates` and
`challenge_revocation` only change the `Certificate` record. The enrollment
keeps `certificate_issued == true`, so a revoked certificate cannot be
re-issued against the same enrollment. The student needs a new attempt via
`re_enroll` (E10).

```
Certificate:  valid ── revoke_certificate / bulk_revoke ──▶ pending revocation
                ▲                                              │       │
                └──── challenge_revocation (ledger < deadline) ┘       │ ledger >= revocation_deadline
                                                                       ▼
                                                             permanently revoked (terminal)
```

---

## Notes and known gaps

These behaviours came up while writing the tables. They are recorded here
rather than changed. Each is a candidate follow-up issue.

1. **Circuit-breaker auto-pause (C6) cannot persist.** Both the early-exit
   `Paused` write and the pre-increment of `CourseFailureCount` happen inside
   the same invocation that then panics (or whose transfer panics). Soroban
   reverts every write in a failed invocation, so the counter never builds up
   across failed calls and the early-exit pause is rolled back.
   `test_circuit_breaker_trips_after_threshold_failures` only checks the panic
   message, using a counter written directly into storage.
2. **`reject_course` does not decrement `InstructorPendingCourseCount`.**
   `approve_course` does. Rejected courses therefore keep using up the
   instructor's pending-registration quota.
3. **Expired enrollments still count as active.** `active_enrollments` and
   `TotalActiveEnrollments` are only decremented on completion or refund.
   An expired enrollment keeps its capacity slot, and blocks
   `archive_course`, until it is refunded.
4. **`process_refund` does not re-check `completed`.** `request_refund`
   rejects completed enrollments, but if `mark_completed` runs between the
   request and its approval, the completed enrollment is still refunded.
5. **One refund request per (student, course) ever.** E12: after a refund
   and a fresh `enroll`, the stale `Approved` `RefundRequest` blocks any new
   request.
6. **Writes to an archived course recreate `DataKey::Course(id)`.**
   `issue_certificate` and `process_refund` load the course through
   `get_course_internal` (which falls back to `ArchivedCourse`) and then save
   it to `DataKey::Course(id)`. Afterwards the ID shows up again as an
   `Archived` record under the live key, and `register_course` with that ID
   fails with `"course already registered"` even after the cooldown.
7. **A non-empty waitlist makes `archive_course` refunds revert.** Each
   refund calls `promote_from_waitlist`, which calls `enroll_internal` on a
   course that must be `Paused`. `validate_enrollment` then panics with
   `"course is not available for enrollment"`, which reverts the whole
   archive call.
