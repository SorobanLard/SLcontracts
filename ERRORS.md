# Error & Panic Message Contract

This document is the canonical reference for how the SorobanLard contract's
failure conditions map to end-user-facing meaning and to the API error
responses returned by the NestJS backend. Whenever a new panic condition or
`panic!`/`panic_with_error!` message is added to the contract, add a row here
and update the backend mapping in the same PR.

## How to read this table

- **Contract condition** — the panic message / condition raised by the contract
  (see `contracts/sorobanlard/src/lib.rs`).
- **End-user meaning** — what the user should be told happened.
- **API response** — the HTTP status and error `code` the backend returns when
  it surfaces this contract failure.

## Contract failures → user-facing UX

| Contract condition | End-user meaning | API response |
|--------------------|------------------|--------------|
| `admin role has expired` | The platform's admin authority has lapsed; the action cannot be performed until admins are rotated. | `403` `ADMIN_EXPIRED` |
| Unauthorized caller (missing/invalid admin or instructor signature) | You are not allowed to perform this action. | `403` `FORBIDDEN` |
| `student is blocked` | Your account has been blocked by the platform; you cannot enroll, complete, or receive certificates. | `403` `STUDENT_BLOCKED` |
| Course not found / unknown course id | The requested course does not exist. | `404` `COURSE_NOT_FOUND` |
| Course not active (pending, paused, or archived) | This course is not currently open for enrollment. | `409` `COURSE_NOT_ACTIVE` |
| Platform paused | Enrollments are temporarily disabled platform-wide. | `503` `PLATFORM_PAUSED` |
| Token not approved | This payment token is not accepted. | `400` `TOKEN_NOT_APPROVED` |
| Insufficient balance / transfer failure during enrollment | Payment could not be completed; no enrollment was created. | `402` `PAYMENT_FAILED` |
| Enrollment already exists | You are already enrolled in this course. | `409` `ALREADY_ENROLLED` |
| Enrollment not found | No enrollment record exists for this student and course. | `404` `ENROLLMENT_NOT_FOUND` |
| Certificate already issued | A certificate has already been issued for this enrollment. | `409` `CERTIFICATE_EXISTS` |
| Certificate not found | The requested certificate does not exist. | `404` `CERTIFICATE_NOT_FOUND` |
| Certificate revoked | This certificate has been revoked and is no longer valid. | `410` `CERTIFICATE_REVOKED` |
| Insufficient instructor earnings for refund | The refund cannot be processed because instructor funds are unavailable. | `409` `REFUND_UNAVAILABLE` |
| Arbitration already resolved / not open | This dispute is not open for resolution. | `409` `ARBITRATION_NOT_OPEN` |
| Upgrade timelock not elapsed | The contract upgrade is still within its review window. | `409` `UPGRADE_TIMELOCK_ACTIVE` |
| Treasury update still pending | The new treasury address is not yet active. | `409` `TREASURY_UPDATE_PENDING` |
| Any other unexpected panic | Something went wrong processing your request. | `500` `CONTRACT_ERROR` |

## Backend mapping convention

The NestJS API wraps every contract invocation. When the simulated transaction
fails, the backend inspects the contract error/panic message and maps it to the
`code` above; unknown messages fall through to `500 CONTRACT_ERROR` with the raw
message logged (never returned to the client). Keep this table and the backend
mapping in sync.

## PR checklist

- [ ] If this PR adds or changes a contract panic condition, add/update the
      corresponding row in this file **and** the backend error mapping.
- [ ] If this PR changes an API error `code` or status, update the table above.
