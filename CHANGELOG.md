# Changelog

All notable changes to the SorobanLard contract are documented in this file.

This project adheres to [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
conventions and follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Recording Wasm Hashes

Because the contract supports live, in-place Wasm upgrades via
`propose_upgrade` / `upgrade_contract`, every released entry below records the
Wasm hash that shipped it. Going forward, **each release entry MUST include the
Wasm hash** of the deployed contract under a `Wasm hash:` line, so auditors and
integrators can answer "what changed between hash A and hash B" without diffing
the full `lib.rs` history.

Convention:

- Add a new versioned section (or extend `Unreleased`) for each change set.
- When a release is deployed, move the relevant entries out of `Unreleased` into
  a dated version section and add the deployed Wasm hash.
- The hash recorded is the one passed to `propose_upgrade` and later applied by
  `upgrade_contract`.

## [Unreleased]

### Added

- CHANGELOG tracking contract version history and upgrade hashes.

## [0.1.0] - Initial tracked release

Wasm hash: _not recorded (pre-changelog)_

### Added

- Core course lifecycle: instructor course registration, admin approval, and
  course archiving with student refunds.
- Enrollment flow with atomic payment split: platform fee forwarded to the
  treasury, instructor share credited to pull-based `InstructorEarnings`.
- Certificate issuance and revocation, including bulk revocation of a course's
  certificates.
- Platform pause/unpause and approved-token allowlist management.
- Admin transfer via two-step `transfer_admin` / `accept_admin` flow.
- Time-locked treasury updates (`update_treasury`, effective after 100 ledgers).
- Admin expiry via `set_admin_expiry`, blocking all admin operations once the
  expiry ledger is reached.
- Student blocking policy (`block_student` / `unblock_student`) freezing
  forward-looking enrollments, completions, certificates, and refunds.
- Time-locked, two-step contract upgrades: `propose_upgrade`,
  `upgrade_contract`, and `cancel_upgrade` with a review timelock.

### Changed

- Revenue split uses integer arithmetic (`platform_amount = price * pct / 100`),
  with truncation remainder retained by the instructor share.

### Fixed

- Enrollment is all-or-nothing: a failed split transfer reverts the entire
  invocation, leaving no enrollment, earnings credit, or `total_earned` change.

## [0.2.0] - Fee & risk system

Wasm hash: _to be recorded on deploy_

### Added

- Risk-based fee surcharges layered on top of the base platform fee.
- Configurable default fee and max-courses limit via `update_default_fee` and
  `update_max_courses_limit`.
- `get_platform_fee` admin-only retrieval of platform fee configuration.

## [0.3.0] - Arbitration module

Wasm hash: _to be recorded on deploy_

### Added

- Dispute arbitration: open disputes hold an arbitration fee, finalized by
  multi-sig `resolve_arbitration` which disburses the held fee to the student
  or instructor.

## [0.4.0] - Waitlist

Wasm hash: _to be recorded on deploy_

### Added

- Course waitlist support for students when a course is at capacity.

## [0.5.0] - Prerequisites

Wasm hash: _to be recorded on deploy_

### Added

- Course prerequisites gating enrollment on prior completion of required
  courses.

## [0.6.0] - Categories

Wasm hash: _to be recorded on deploy_

### Added

- Course categories for grouping and discovery.

[Unreleased]: https://github.com/SorobanLard/sorobanlard/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/SorobanLard/sorobanlard/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/SorobanLard/sorobanlard/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/SorobanLard/sorobanlard/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/SorobanLard/sorobanlard/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/SorobanLard/sorobanlard/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/SorobanLard/sorobanlard/releases/tag/v0.1.0
