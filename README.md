# SorobanLard — Contract Repo

> **On-chain course enrollment, payments, and certificate issuance on Stellar Soroban**

SorobanLard is an online learning platform for practical vocational skills — tailoring, makeup artistry, baking, photography, hairstyling, nail technology, fashion design, and more. This Soroban smart contract handles the trustless financial and credential layer of the platform: course enrollment payments, automatic instructor revenue splits, and verifiable on-chain certificates of completion.

## Short version

SorobanLard Contracts is the Soroban smart contract layer for the SorobanLard platform. It manages course lifecycle state, enrollment payments, instructor revenue splits, certificate issuance, and revocation in a trustless and auditable way.

The contract provides the core rules for:
- registering and approving courses
- pausing and unpausing courses
- archiving courses permanently
- enrolling students and splitting payments automatically
- marking enrollments complete
- issuing and revoking certificates
- emitting events for off-chain indexing and backend synchronization

### Main contract concepts
- Courses move through a lifecycle from Pending to Active, Paused, and Archived.
- Enrollments trigger automatic platform and instructor payment splits.
- Certificates can be issued after completion and later revoked with metadata captured on-chain.

### Key features
- multi-admin authorization for sensitive operations
- course status enforcement and lifecycle rules
- instructor earnings accounting
- certificate issuance and revocation tracking
- Soroban event emission for off-chain systems and indexers

The detailed documentation below expands on the architecture, data model, contract functions, and deployment flow.

This is **Repo 1 of 3** in the SorobanLard project:

| Repo | Description |
|------|-------------|
| `sorobanlard-contract` ← you are here | Soroban smart contract (Rust) |
| `sorobanlard-backend` | NestJS REST API + content management |
| `sorobanlard-frontend` | Next.js student and instructor portal |

---

## Table of Contents

- [What the Contract Does](#what-the-contract-does)
- [What Lives Off-Chain](#what-lives-off-chain)
- [Storage Tiers](#storage-tiers)
- [Architecture](#architecture)
- [Data Structures](#data-structures)
- [Contract Functions](#contract-functions)
- [Revenue Split Logic](#revenue-split-logic)
- [Certificate Verification](#certificate-verification)
- [Events](#events)
- [Project Structure](#project-structure)
- [Prerequisites](#prerequisites)
- [Setup & Installation](#setup--installation)
- [Running Tests](#running-tests)
- [Upgrades & Storage Migrations](#upgrades--storage-migrations)
- [Building](#building)
- [Deploying to Testnet](#deploying-to-testnet)
- [Security Considerations](#security-considerations)
- [Roadmap](#roadmap)

---

## What the Contract Does

The SorobanLard contract is the trust layer for the platform. It handles three responsibilities:

**1. Course Registry**
Instructors register courses on-chain with a price and fee structure. Each course starts as `Pending` — an admin must approve it before students can enroll. This gives the platform moderation control without being able to move anyone's funds.

**2. Enrollment Payments**
When a student enrolls, the contract automatically splits the payment in a single transaction:
- Platform fee → treasury address
- Instructor share → instructor address directly

No escrow needed — the payment splits and lands in both wallets immediately. The contract records the enrollment so no student can enroll twice.

**3. On-Chain Certificates**
After a student completes a course (verified off-chain by the backend), the admin issues an on-chain certificate. Each certificate is a permanent, verifiable record tied to the student's Stellar address and the course. Anyone can verify a certificate by querying `verify_certificate(certificate_id)` — no trusted middleman needed. Certificates can be revoked by the admin if issued in error.

---

## What Lives Off-Chain

The contract intentionally stores only what is necessary for payments and verification. Everything else lives in the backend database:

| Off-chain (backend) | On-chain (contract) |
|---|---|
| Video lessons | Course ID, price, instructor address |
| Course descriptions | Total enrollments, total earned |
| Assignments | Enrollment records |
| Student progress % | Completion status |
| Downloadable resources | Certificate with course title |
| User profiles | — |
| Instructor analytics | — |

This keeps the contract small, cheap to call, and focused on what blockchains are actually good at: verifiable ownership, trustless payments, and permanent records.

---

## Storage Tiers

The contract uses Soroban **instance** storage for singleton config, auth flags,
and global counters, and **persistent** storage for per-entity records and
append-only collections. The rule of thumb for new keys is:

> Singleton config / auth flag / global counter → instance.
> Per-course / per-student / per-instructor record or list → persistent.

Every current `DataKey` / `DataKey2` variant is classified in
[docs/storage-keys.md](docs/storage-keys.md), along with the rationale and the
race-freedom argument for `TotalActiveEnrollments`.

---

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                SorobanLard Contract (Soroban)                    │
│                                                              │
│  ┌──────────────┐   ┌──────────────────┐   ┌─────────────┐  │
│  │   Course     │   │   Enrollment     │   │ Certificate │  │
│  │   Registry   │   │   + Payment      │   │   Issuance  │  │
│  │  (Pending →  │   │   Split          │   │  (on-chain  │  │
│  │   Active)    │   │  (instant)       │   │   NFT-like) │  │
│  └──────────────┘   └──────────────────┘   └─────────────┘  │
└──────────────────────────────────────────────────────────────┘
         ↑                    ↑                    ↑
    Instructor           Student pays         Admin issues
    registers            at enroll           after backend
    course               (auto-split)        verifies completion
```

```
┌────────────┐   ┌──────────────┐   ┌───────────────────┐
│  Waitlist  │   │ Arbitration  │   │ Fee configuration │
│ promotion  │   │   disputes   │   │ platform + risk   │
└────────────┘   └──────────────┘   └───────────────────┘
```

### Roles

| Role | Address | Permissions |
|------|---------|-------------|
| **Admin** | Set at `init` | Approve/archive courses, mark completions, issue/revoke certificates, update fee |
| **Secondary Admin** | Set at `init` | Co-signs sensitive operations, including archival and upgrades |
| **Instructor** | Set per course | Register courses, pause/unpause their own courses |
| **Student** | Any address | Enroll in active courses |
| **Treasury** | Set at `init` | Receives platform fee share (passive recipient only) |

---

## Data Structures

### `Course`

```rust
pub struct Course {
    pub id: String,                    // matches backend DB record ID
    pub instructor: Address,
    pub price: i128,                   // enrollment price in USDC stroops
    pub platform_fee_percent: u32,     // 0-100; remainder goes to instructor
    pub token: Address,                // USDC Stellar Asset Contract
    pub total_enrollments: u32,
    pub total_earned: i128,
    pub status: CourseStatus,          // Pending | Active | Paused | Archived
    pub created_at_ledger: u32,
}
```

### `Enrollment`

```rust
pub struct Enrollment {
    pub student: Address,
    pub course_id: String,
    pub amount_paid: i128,
    pub enrolled_at_ledger: u32,
    pub completed: bool,
    pub certificate_issued: bool,
}
```

### `Certificate`

```rust
pub struct Certificate {
    pub id: String,                    // unique cert ID (UUID from backend)
    pub student: Address,
    pub course_id: String,
    pub course_title: String,          // stored on-chain for direct verification
    pub instructor: Address,
    pub issued_at_ledger: u32,
    pub revoked: bool,
}
```

### `CourseStatus` state machine

```
Pending ── approve_course()  ──→ Active
Pending ── reject_course()   ──→ Rejected (terminal)
Active  ── pause_course() / freeze_instructor() ──→ Paused
Paused  ── unpause_course()  ──→ Active
Paused  ── archive_course()  ──→ Archived (both admins, no active enrollments, terminal)
```

The full transition tables for `CourseStatus` and the `Enrollment` lifecycle
(active, completed, certificate issued, refunded, archived to history,
expired), with every function that triggers each transition, are in
[STATE_MACHINES.md](STATE_MACHINES.md). The accounting and state invariants
those transitions must preserve are listed in [INVARIANTS.md](INVARIANTS.md).

---

## Contract Functions

### `init(admin, secondary_admin, treasury, default_fee_pct, max_courses_per_instructor, refund_window_ledgers, revocation_challenge_period)`
Initialises the contract. Called once by the deployer, immediately after deployment. Every argument has operational consequences; see [Initialize after deployment](#initialize-after-deployment) for recommended values and the decisions to make first.

### Course management

| Function | Caller | Description |
|---|---|---|
| `register_course(instructor, course_id, price, token, platform_fee_pct, max_capacity, content_hash, categories)` | Instructor | Register a course — starts as Pending. `price` is in stroops (0 for free); `max_capacity` is optional; `content_hash` is 32 bytes. |
| `approve_course(admin, course_id)` | Admin | Move Pending → Active |
| `pause_course(caller, course_id)` | Admin or Instructor | Move Active → Paused |
| `unpause_course(caller, course_id)` | Admin or Instructor | Move Paused → Active |
| `archive_course(admin1, admin2, course_id, students_to_refund)` | Both admins | Permanently archive. `students_to_refund` is optional. |

### Enrollment

| Function | Caller | Description |
|---|---|---|
| `enroll(student, course_id)` | Student | Pay and enroll. Splits payment instantly. |

### Completion & Certificates

| Function | Caller | Description |
|---|---|---|
| `mark_completed(admin, student, course_id, evidence_hash)` | Admin | Called after backend verifies all lessons done. A non-empty `evidence_hash` replaces the student co-signature. |
| `issue_certificate(admin, student, course_id, cert_id, course_title, enrollment_reference, expires_at_ledger, instructor_signature)` | Admin | Issues on-chain certificate. Requires completion. `enrollment_reference` is a free-form backend ID (e.g. UUID). |
| `revoke_certificate(admin, certificate_id, reason)` | Admin | Flags certificate as revoked |
| `bulk_revoke_course_certificates(admin, course_id)` | Admin | Flags every certificate of a course as revoked in one transaction |

### Admin management

| Function | Caller | Description |
|---|---|---|
| `transfer_admin(admin1, admin2, new_admin, new_secondary_admin)` | Both admins | Transfer admin role |
| `update_treasury(admin1, admin2, new_treasury)` | Both admins | Update treasury address |
| `update_default_fee(admin, new_fee_pct)` | Admin | Update default platform fee |

### Read-only queries

| Function | Returns |
|---|---|---|
| `get_course(course_id)` | Full `Course` struct |
| `get_enrollment(caller, student, course_id)` | Full `Enrollment` struct (student or admin only) |
| `get_certificate(caller, certificate_id)` | Full `Certificate` struct (student, instructor, or admin only) |
| `is_enrolled(student, course_id)` | `bool` |
| `has_completed(student, course_id)` | `bool` |
| `verify_certificate(certificate_id)` | `bool` — true if exists and not revoked |
| `get_platform_fee(admin)` | `u32` — current default fee % (admin only) |

---

## Revenue Split Logic

When a student calls `enroll()`, the contract calculates the split and executes both transfers in the same transaction:

```
price = 100 USDC
platform_fee_percent = 20

platform_amount   = 100 × 20 / 100 = 20 USDC → treasury
instructor_amount = 100 - 20       = 80 USDC → instructor
```

Both transfers are atomic — if either fails, the whole transaction reverts. The student is never charged without the instructor and treasury being paid.

The platform fee percentage can be:
- **Global default** (set at `init`, updatable by admin)
- **Per-course override** (set by instructor at registration time, non-zero value)

---

## Certificate Verification

Any third party — an employer, a client, a government agency — can verify a SorobanLard certificate without trusting the platform:

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- verify_certificate \
  --certificate_id "CERT-12345-TAILORING"
```

Returns `true` if the certificate exists and has not been revoked. The `get_certificate()` function returns the full record including the student's Stellar address, course title, instructor, and issuance ledger — all permanently recorded and independently auditable.

---

## Events

| Event name | Payload | When |
|---|---|---|
| `course_registered` | `course_id` | Instructor registers a course |
| `course_approved` | `course_id` | Admin approves a course |
| `course_paused` | `course_id` | Course paused |
| `course_unpaused` | `course_id` | Course unpaused |
| `course_archived` | `course_id` | Course archived |
| `student_enrolled` | `(course_id, student, amount_paid)` | Student enrolls and pays |
| `course_completed` | `(course_id, student)` | Completion marked by admin |
| `certificate_issued` | `(certificate_id, student, course_id)` | Certificate issued |
| `certificate_revoked` | `certificate_id` | Certificate revoked |
| `course_certificates_revoked` | `(admin, course_id, revoked_count, ledger)` | All certificates of a course bulk-revoked |
| `admin_transferred` | `new_admin` | Admin role transferred |
| `upgrade_executed` | `(old_wasm_hash: Option<BytesN<32>>, new_wasm_hash, admin1, admin2, ledger_sequence)` | `upgrade_contract` swaps the contract Wasm (topic also carries `new_wasm_hash`). `old_wasm_hash` is `None` for a deployment's first upgrade |
| `storage_migrated` | `(from_version, to_version, admin1, admin2, ledger_sequence)` | `migrate()` transforms stored records after an upgrade |
| `dispute_escalated` | `(caller, course_id, fee)` | Arbitration case opened |
| `arbitration_resolved` | `(caller, course_id, for_student, admin)` | Admin resolved an open case |
| `arbitration_auto_refunded` | `(caller, course_id, fee, ledger)` | Case timed out; fee refunded to the escalating caller |

The backend listens to these events and updates the database, sends notifications, and triggers email confirmations. In particular, `arbitration_resolved` lets the backend notify both the student and the instructor and update the dispute status shown in the UI.

---

## Project Structure

```
sorobanlard-contract/
├── Cargo.toml                         ← Rust workspace config
├── Cargo.lock
├── .cargo/
│   └── audit.toml                     ← cargo-audit advisory policy
├── .gitignore
├── README.md
├── INVARIANTS.md                      ← Invariant test map and detailed audit model
├── AUDIT_CHECKLIST.md                 ← Pre-audit security self-review checklist
├── STATE_MACHINES.md                  ← Documented state transitions
├── scripts/
│   ├── check_invariants.sh            ← Invariant documentation / test coverage check
│   ├── check_readme_api.py            ← README vs lib.rs signature check
│   └── smoke-test-testnet.sh          ← Post-deployment testnet smoke test
└── contracts/
    └── sorobanlard/
        ├── Cargo.toml                 ← Contract package config
        ├── Makefile                   ← Build / deploy shortcuts
        └── src/
            ├── lib.rs                 ← Full contract logic
            └── test.rs                ← Contract tests and invariant suite
```

---

## Prerequisites

```bash
# Rust + wasm32 target
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32v1-none

# Stellar CLI
cargo install --locked stellar-cli --features opt

# Testnet account
stellar keys generate --global my-account --network testnet
stellar keys fund my-account --network testnet
```

---

## Setup & Installation

```bash
git clone https://github.com/your-org/sorobanlard-contract.git
cd sorobanlard-contract
cargo check
```

Deployment variables that the Makefile reads (such as `STELLAR_ACCOUNT`) are listed in [`contracts/sorobanlard/.env.example`](contracts/sorobanlard/.env.example). Copy it to `.env` in that directory and export it before running `make deploy-testnet`:

```bash
cd contracts/sorobanlard
cp .env.example .env
set -a; source .env; set +a
```

---

## Running Tests

```bash
cargo test
cargo test -- --nocapture          # with output
cargo test test_full_lifecycle     # single test
```

Expected:
```
running 14 tests
test test::test_approve_already_active_course ........... ok
test test::test_approve_course_success .................. ok
test test::test_approve_course_unauthorized ............. ok
test test::test_certificate_requires_completion ......... ok
test test::test_enroll_duplicate ........................ ok
test test::test_enroll_pending_course ................... ok
test test::test_enroll_success_with_payment_split ....... ok
test test::test_full_lifecycle_enroll_complete_certify .. ok
test test::test_init_success ............................ ok
test test::test_is_enrolled_check ....................... ok
test test::test_multiple_students_same_course ........... ok
test test::test_pause_and_unpause_course ................ ok
test test::test_register_course_custom_fee .............. ok
test test::test_register_course_success ................. ok
test test::test_register_duplicate_course ............... ok
test test::test_revoke_certificate ...................... ok
test test::test_update_platform_fee ...................... ok
```

---

## Upgrades & Storage Migrations

`upgrade_contract` replaces only the contract Wasm; every stored record keeps the encoding written by the old code. A `#[contracttype]` struct is stored as a map keyed by field name, and decoding requires that map to contain **exactly** the struct's fields. Adding, removing, or renaming a field — even an `Option<T>` one — makes every pre-upgrade record of that type undecodable by the new Wasm.

When a release changes a stored struct (`Course`, `Enrollment`, `Certificate`, ...):

1. Keep the old shape in the new code as a legacy type (e.g. `CourseV1`). Add new fields as `Option<T>` so a migration can leave them unset when there is no sensible backfill value.
2. Bump `STORAGE_SCHEMA_VERSION` and add a step to `run_migration_step` that reads each affected record as the legacy type and rewrites it in the new shape.
3. After `upgrade_contract` executes, both admins call `migrate(admin1, admin2)` once. It applies every pending step in order, records the new version (`get_storage_schema_version()`), and rejects repeat calls.

**Required regression tests:** the tests under "UPGRADE STORAGE COMPATIBILITY & MIGRATION" in `src/test.rs` (`test_records_deserialize_unchanged_after_simulated_upgrade` and the migration tests) simulate an upgrade and read back records written before it. They must pass for any change to a `#[contracttype]` struct; extend them with the legacy/new shapes whenever a migration step is added.

---

## Building

```bash
make build      # → target/wasm32v1-none/release/sorobanlard.wasm
make optimize   # → target/wasm32v1-none/release/sorobanlard.optimized.wasm
```

---

## Deploying to Testnet

```bash
export STELLAR_ACCOUNT=my-account
make deploy-testnet
# → CXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
```

### Initialize after deployment

`init` takes seven arguments. It can be called once; a second call panics with `contract already initialized`.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source my-account \
  --network testnet \
  -- init \
  --admin <ADMIN_ADDRESS> \
  --secondary_admin <SECONDARY_ADMIN_ADDRESS> \
  --treasury <TREASURY_ADDRESS> \
  --default_fee_pct 20 \
  --max_courses_per_instructor 50 \
  --refund_window_ledgers 17280 \
  --revocation_challenge_period 17280
```

| Argument | Recommended value | Rationale |
|---|---|---|
| `admin` | A dedicated admin key (ideally a multi-sig account) | Approves courses, issues certificates, and governs every admin function. |
| `secondary_admin` | A separate key held by a different person | Co-signs sensitive operations such as `transfer_admin`, `update_treasury`, and upgrades. Must differ from `admin` and `treasury`. |
| `treasury` | A cold or multi-sig account that receives the platform fee | Must differ from `admin`, `secondary_admin`, and the contract address. |
| `default_fee_pct` | `20` | Platform fee in percent (0–100), used when a course does not override it. Matches the testnet default. |
| `max_courses_per_instructor` | `50` | Caps how many courses one instructor can register. Can be raised later with `update_max_courses_limit`. |
| `refund_window_ledgers` | `17280` (about 1 day at 5 s per ledger) | Students can request a refund only within this many ledgers after enrolling. Use `120960` (about 7 days) for a more generous policy. |
| `revocation_challenge_period` | `17280` (about 1 day) | How long a student can challenge a certificate revocation before it becomes permanent. |

#### Decisions to make before calling `init`

- [ ] **Who holds `secondary_admin`'s key?** It should be a different person or device from `admin`, or the two-admin control is lost.
- [ ] **Is `admin` a multi-sig account?** A single key is a single point of failure for every admin operation.
- [ ] **Which treasury account receives fees?** Confirm it can receive every approved token before going live.
- [ ] **What refund window fits the course policy?** Pick a value and record it; it is set at init and changed later with `update_refund_window`.
- [ ] **What certificate challenge period is acceptable?** Shorter periods make revocations final sooner; longer periods give students more time to contest them.
- [ ] **Which tokens will be approved?** `init` does not approve any token. Call `add_approved_token` afterwards, and only for SAC tokens with 7 decimals.
- [ ] **Is the governance window set?** `approve_course` is blocked until `update_min_review_delay` ledgers have passed since init. The default is `0` (no window), so set it explicitly if you want one.
- [ ] **Are the addresses correct for this network?** Double-check every address against the target network before submitting; a wrong treasury address cannot be changed without an admin transaction.

### Smoke test after deployment

Run the smoke test once after `init` to confirm the deployment behaves end to end before handing the contract ID to the backend or frontend teams. It runs register → approve → enroll → mark complete → issue certificate → verify against a free course, so no token funds are moved.

```bash
export CONTRACT_ID=<CONTRACT_ID>
export ADMIN_ACCOUNT=my-account
export INSTRUCTOR_ACCOUNT=instructor-account
export STUDENT_ACCOUNT=student-account
./scripts/smoke-test-testnet.sh
```

Exit codes: `0` if every step succeeded and `verify_certificate` returned `true`; `1` on a missing environment variable or missing `stellar` CLI; `2` if any contract call failed (the failing step is printed).

### Register and approve a test course

```bash
# Register
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source instructor-account \
  --network testnet \
  -- register_course \
  --instructor <INSTRUCTOR_ADDRESS> \
  --course_id "COURSE-TAILORING-001" \
  --price 500000000 \
  --token <USDC_SAC_ADDRESS> \
  --platform_fee_pct 0

# Approve
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source my-account \
  --network testnet \
  -- approve_course \
  --admin <ADMIN_ADDRESS> \
  --course_id "COURSE-TAILORING-001"
```

> **USDC SAC on Testnet:** `CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA`

---

## Security Considerations

- **Authorization**: Every write function calls `require_auth()` on the relevant party. Instructors cannot approve their own courses. Students cannot issue their own certificates. Only the admin can revoke certificates.
- **No double enrollment**: The contract checks for an existing enrollment record before processing payment. A student cannot be charged twice for the same course.
- **Atomic payment split**: The student → contract payment, the contract → treasury platform fee and the instructor earnings credit all happen in the same invocation. If any step fails, including the treasury transfer after the student's payment has landed, the whole invocation reverts: no enrollment, earnings credit or `total_earned` change is kept. This is verified by `test_enroll_rolls_back_when_second_split_transfer_fails`.
- **Token assumptions**: Approved tokens must be standard, fee-free, non-rebasing SAC-style tokens. Instructor earnings are a plain stroops counter that only matches the contract's real balance under that assumption. Fee-on-transfer tokens are rejected at enrollment, but withdrawals and refunds trust the token to deliver the full amount. See `add_approved_token` and [INVARIANTS.md](INVARIANTS.md).
- **Certificate immutability**: Revoked certificates are flagged — not deleted. This preserves the audit trail and prevents a revocation from being hidden.
- **Admin is a single address**: For production, consider using a multi-sig wallet as the admin address to avoid a single point of failure. Privileged operations need both `admin` and `secondary_admin`, so each role should be a separate multisig. See [docs/admin-key-management.md](docs/admin-key-management.md) for the pattern and init-time rules.
- **Pre-launch and upgrades**: Review [MAINNET_CHECKLIST.md](MAINNET_CHECKLIST.md) before deploying to mainnet, and rehearse every upgrade with [docs/upgrade-runbook.md](docs/upgrade-runbook.md).
- **Fee validation**: Platform fee percentage is validated to be ≤ 100 at both `init` and `register_course` time.

---

## Roadmap

- [x] Course registry with approval workflow
- [x] Enrollment payment with automatic split
- [x] On-chain certificate issuance and verification
- [x] Course pause/unpause by instructor or admin
- [x] Certificate revocation
- [x] 14+ unit tests
- [ ] Discount codes / coupon system (Soroban-based)
- [ ] Subscription enrollment model (monthly access)
- [ ] Instructor revenue withdrawal tracking
- [ ] Course bundle enrollments (enroll in multiple courses at once)
- [ ] Contract upgrade mechanism
- [ ] Mainnet deployment

---

## License

MIT

## Handsoff notes

<!-- handsoff-issue-232 -->
- #232: Review panic-based error handling: client apps cannot distinguish failure reasons programmatically

<!-- handsoff-issue-233 -->
- #233: Add explicit test that blocked students cannot bypass via re_enroll after block

<!-- handsoff-issue-234 -->
- #234: Review re-propose-upgrade pattern for timelock reset abuse

<!-- handsoff-issue-228 -->
- #228: Review LastAdminTransferLedger cooldown for bypass via accept_admin timing

<!-- handsoff-issue-229 -->
- #229: Add negative test: approver (non-admin) cannot perform admin-only operations

<!-- handsoff-issue-230 -->
- #230: Confirm withdraw_tokens cannot drain funds owed to instructors/students

<!-- handsoff-issue-231 -->
- #231: Review integer overflow surface in revenue split and risk surcharge math

<!-- handsoff-issue-208 -->
- #208: Expand Security Considerations section to cover arbitration, risk fees, and upgrade timelocks

<!-- handsoff-issue-204 -->
- #204: Introduce a typed error enum for certificate issuance/revocation panics

<!-- handsoff-issue-205 -->
- #205: Introduce a typed error enum for enrollment/payment panics

<!-- handsoff-issue-206 -->
- #206: Introduce a typed #[contracterror] enum for course-lifecycle panics

<!-- handsoff-issue-207 -->
- #207: Add a state-machine diagram for DisputeStatus / ArbitrationCase lifecycle

<!-- handsoff-issue-213 -->
- #213: Add rustdoc examples to the public contract interface for docs.rs-style generation

<!-- handsoff-issue-235 -->
- #235: Add a cross-contract call failure/panic differentiation test for enroll_internal

<!-- handsoff-issue-200 -->
- #200: Add defensive checked arithmetic audit pass beyond the existing checked_sub calls

<!-- handsoff-issue-202 -->
- #202: Replace unwrap_or_else(|| panic!(...)) pattern with a shared helper macro

<!-- handsoff-issue-203 -->
- #203: Introduce a typed error enum for arbitration/dispute panics

<!-- handsoff-issue-218 -->
- #218: Events table in README is missing ~10 events emitted by lib.rs

<!-- handsoff-issue-219 -->
- #219: Data Structures section in README omits most Course/Enrollment/Certificate fields

<!-- handsoff-issue-198 -->
- #198: Add explicit error for calling get_risk_fee_config or related admin getters before they're configured

<!-- handsoff-issue-199 -->
- #199: Standardize "not found" vs "does not exist" vs implicit None-returns across read-only queries

<!-- handsoff-issue-224 -->
- #224: Confirm max_certificates enforcement cannot be bypassed via bulk_revoke + reissue cycling

<!-- handsoff-issue-225 -->
- #225: Review set_fee_config retroactivity vs. Enrollment.platform_amount snapshot

<!-- handsoff-issue-226 -->
- #226: Add a check that ApprovedToken removal does not retroactively break existing courses

<!-- handsoff-issue-227 -->
- #227: Review set_risk_config_enabled toggling mid-flight for fee consistency

<!-- handsoff-issue-222 -->
- #222: Add fuzz-style test for register_course course_id character validation

<!-- handsoff-issue-223 -->
- #223: Security review: single compromised admin's blast radius with SecondaryAdmin absent from read-paths

<!-- handsoff-issue-133 -->
- #133: Add property/invariant test coverage for course, fee, refund, and arbitration lifecycles

<!-- handsoff-issue-134 -->
- #134: Expand admin audit checklist and timelock floor coverage

<!-- handsoff-issue-143 -->
- #143: Add tests for unauthorized cross-role access and lifecycle bypasses

<!-- handsoff-issue-146 -->
- #146: Add invariant tests for accounting and multi-sig safety
