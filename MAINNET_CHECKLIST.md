# Mainnet Pre-Launch Checklist

Complete every item before the first mainnet deployment, and again before any
configuration change that the checklist covers. The checklist lists each
admin-configurable parameter in `contracts/sorobanlard/src/lib.rs`, the setter
that changes it, what the contract uses if the setter is never called, and the
value to set for production.

Read the table columns as follows:

- **Setter** is the function to call. `[single]` needs the `admin` signature.
  `[pair]` needs both `admin` and `secondary_admin` (see
  [docs/admin-key-management.md](docs/admin-key-management.md)).
- **Unset default** is the value the contract uses if no setter has ever run.
- **Production value** is a recommendation. Where the contract enforces a
  constraint, the row says so. Where the value is a business or policy choice,
  it is marked **team decision**: set the value the maintainers agree on and
  record it here.

Getter functions let you confirm each value on-chain after you set it. Check
every one before launch.

> `get_platform_fee` requires the admin signature, so it cannot be read
> anonymously. Use the other getters for public verification.

## 1. Fees and treasury

| # | Parameter | Setter | Unset default | Production value | Rationale |
|---|---|---|---|---|---|
| 1.1 | Platform default fee (percent) | `init(default_fee_pct)` at deploy; `update_default_fee(admin, new_fee_pct)` `[single]` | Set at `init`. `get_platform_fee` falls back to 20 only if the key is missing | **Team decision.** Set explicitly and record it | Courses registered with fee `0` follow the live default at enrollment. A course registered with an explicit percentage keeps it, so a default change reaches only courses that follow the default |
| 1.2 | Per-token fee (basis points) | `set_fee_config(admin, token, fee_bps)` `[single]` | Token follows the default fee (default × 100) | Set explicitly for every approved token, for example `500` for 5% or `2000` for 20% | A per-token rate takes precedence at enrollment, and registration rejects an explicit course fee below it. Without an explicit rate, the token's fee moves whenever the default fee changes. `fee_bps` is capped at 10000 |
| 1.3 | Treasury address | `init(treasury)` at deploy; `update_treasury(admin1, admin2, new_treasury)` `[pair]` | Set at `init` | A cold or multisig account, distinct from both admins and from the contract | `init` rejects a treasury equal to the contract address, to `admin`, or to `secondary_admin`. A treasury change takes effect 100 ledgers after proposal |
| 1.4 | Arbitration fee per case (stroops) | `set_arbitration_fee_config(admin, fee_per_case)` `[single]` | `0`. **Escalation is disabled** | A non-zero amount in the settlement token's stroops. **Team decision** on the amount | `escalate_to_arbitration` panics with `arbitration fee not configured` while the value is `0`. A zero fee leaves students and instructors with no way to dispute. Negative values are rejected |
| 1.5 | Risk surcharge switch | `set_risk_config_enabled(admin, enabled)` `[single]` | `false` (disabled) | **Team decision.** Enable only with the surcharge values in 1.6 set and tested | Surcharges apply only while enabled |
| 1.6 | Risk surcharge values | `set_risk_fee_config(admin, large_payment_surcharge_bps, large_payment_threshold, new_customer_surcharge_bps, btc_eth_surcharge_bps)` `[single]` | All surcharges `0`. Threshold `1_000_000_000_000` stroops (100,000 units at 7 decimals) | **Team decision.** Set every field explicitly, even if a value is `0`. `large_payment_threshold` must be `>= 0` | The unset default is safe only because the switch in 1.5 is off. Setting values explicitly makes the intent auditable |
| 1.7 | BTC/ETH token classification | `set_token_btc_eth(admin, token, is_btc_eth)` `[single]` | `false` for every token | `true` for each approved token that is BTC- or ETH-denominated | `btc_eth_surcharge_bps` applies only to tokens marked here |

## 2. Token whitelist

| # | Parameter | Setter | Unset default | Production value | Rationale |
|---|---|---|---|---|---|
| 2.1 | Approved payment tokens | `add_approved_token(admin, token)` / `remove_approved_token(admin, token)` `[single]` | None approved. Enrollment in any token fails | Only standard, fee-free, non-rebasing SAC-style tokens. Verify each mainnet contract address independently before adding it | The contract cannot check token behaviour on-chain. Fee-on-transfer and rebasing tokens break the earnings accounting. See [INVARIANTS.md](INVARIANTS.md) and the doc comment on `add_approved_token` |
| 2.2 | Course price bounds | Constants `MIN_COURSE_PRICE_STROOPS` and `MAX_COURSE_PRICE_STROOPS` | `100_000` and `1_000_000_000_000` stroops | No change. Not configurable without a Wasm upgrade | `register_course` rejects prices outside these bounds, which catches whole-unit typing mistakes |

## 3. Upgrade governance

| # | Parameter | Setter | Unset default | Production value | Rationale |
|---|---|---|---|---|---|
| 3.1 | Upgrade time-lock (ledgers) | `set_upgrade_timelock(admin1, admin2, ledgers)` `[pair]` | `17_280` ledgers (about 1 day at 5 s/ledger) | **Team decision.** Set it explicitly before the first proposal. A longer window gives the community more time to review. About 120,960 ledgers is one week | The time-lock is read when a proposal is made. Changing it does not affect a pending upgrade. See [docs/upgrade-runbook.md](docs/upgrade-runbook.md) |

## 4. Admin roles and access

| # | Parameter | Setter | Unset default | Production value | Rationale |
|---|---|---|---|---|---|
| 4.1 | `admin` and `secondary_admin` | `init(admin, secondary_admin)`; `transfer_admin(admin1, admin2, new_admin, new_secondary_admin)` `[pair]` then `accept_admin(new_admin, new_secondary_admin)` | Set at `init`. Both must be distinct | Each a smart-account multisig, with disjoint signer sets | Pair operations need both roles. See [docs/admin-key-management.md](docs/admin-key-management.md). Transfers are limited to one per `17_280` ledgers |
| 4.2 | Admin role expiry | `set_admin_expiry(admin1, admin2, expires_at)` `[pair]` | Unset (no expiry) | Unset, or a dated rotation plan recorded before the expiry ledger | Once the ledger reaches the expiry, every admin call panics with `admin role has expired` |
| 4.3 | Approvers | `add_approver(admin, approver)` / `remove_approver(admin, approver)` `[single]` | None | Keep the list as short as possible. Review each approver's key | Approvers can approve and reject courses, but cannot change configuration |
| 4.4 | High-value course threshold (stroops) | `set_high_value_course_threshold(admin, threshold)` `[single]` | Unset. No course needs co-approval | Set it. **Team decision** on the value | Courses priced at or above the threshold need approval from two different admins (issue #167) |
| 4.5 | Platform pause state | `pause_platform(admin)` / `unpause_platform(admin)` `[single]` | Unpaused at `init` | Unpaused at launch, after all other items are verified | The contract has no public getter for the pause flag. Confirm it with a test enrollment on testnet and in the launch runbook |

## 5. Enrollment and capacity

| # | Parameter | Setter | Unset default | Production value | Rationale |
|---|---|---|---|---|---|
| 5.1 | Platform enrollment cap | `set_platform_enrollment_cap(admin, cap)` `[single]`. Pass `None` to remove the cap | No cap (unlimited) | **Team decision.** A launch cap limits the damage from an unexpected problem while the platform is monitored | Enrollments stop at the cap. Raise or remove it later with the same setter |
| 5.2 | Max courses per instructor | `init(max_courses_per_instructor)`; `update_max_courses_limit(admin, new_max)` `[single]` | Set at `init`. The getter falls back to 50 | **Team decision.** Set it explicitly | Limits how many courses one instructor can register |
| 5.3 | Minimum review delay (ledgers) | `update_min_review_delay(admin, delay)` `[single]` | `0`. **No delay** | **Team decision.** A non-zero value, for example `17_280` (about 1 day), is a reasonable starting point | This value does two things. `approve_course` refuses to run until `delay` ledgers have passed since `init`, and it refuses to approve a course until `delay` ledgers have passed since that course was registered. At `0`, approvals are allowed immediately after deployment |
| 5.4 | Minimum completion duration (ledgers) | `update_min_completion_ledgers(admin, ledgers)` `[single]` | `0` | **Team decision.** Set it before the first course is registered | The value is copied into each course at registration. Changing it later affects only courses registered afterward. It sets the minimum time between `enroll` and `mark_completed` |

## 6. Refunds and certificates

| # | Parameter | Setter | Unset default | Production value | Rationale |
|---|---|---|---|---|---|
| 6.1 | Refund window (ledgers) | `init(refund_window_ledgers)`; `update_refund_window(admin, window)` `[single]` | Set at `init`. The getter falls back to 1000 | **Team decision.** `17_280` (about 1 day) is the example used in the `init` doc comment | Refund requests after the window are rejected |
| 6.2 | Revocation challenge period (ledgers) | `init(revocation_challenge_period)`; `set_revocation_challenge_period(admin, period)` `[single]` | Set at `init`. The fallback is `17_280` | **Team decision.** `17_280` is the default | Students can challenge a certificate revocation during this window |

## 7. Fixed constants (require a Wasm upgrade to change)

These values are compiled into the contract. Confirm each one before launch,
because changing it needs the upgrade process in
[docs/upgrade-runbook.md](docs/upgrade-runbook.md).

| Constant | Value | Meaning |
|---|---|---|
| `ARBITRATION_TIMEOUT_LEDGERS` | `17_280` | Ledgers an arbitration case can stay open before the fee is refunded automatically |
| `ARCHIVE_COOLDOWN_LEDGERS` | `172_800` | Ledgers before an archived course ID can be registered again |
| `MIN_ADMIN_TRANSFER_COOLDOWN` | `17_280` | Minimum ledgers between two `transfer_admin` calls |
| `CIRCUIT_BREAKER_THRESHOLD` | `5` | Consecutive token-transfer failures before a course is paused automatically |
| `MAX_BATCH_SIZE` | `50` | Maximum course IDs per `batch_enroll` call |
| `MAX_PAGE_SIZE` | `50` | Maximum course IDs per `list_courses` call |
| `MAX_STUDENTS_TO_REFUND` | `100` | Maximum students refunded per `archive_course` call |
| `MAX_CATEGORIES` / `MAX_CATEGORY_LEN` | `10` / `64` | Category list limits per course |

## 8. Verification before launch

- [ ] Every row in sections 1 to 6 has been set on mainnet, or deliberately left at its unset default, and the choice is recorded here
- [ ] Each mainnet setter call was reviewed by both admin keyholder groups before it was signed
- [ ] The value returned by each getter matches the value recorded in this checklist
- [ ] Each approved token's contract address was verified independently
- [ ] Arbitration is enabled: `get_arbitration_fee_config` returns a non-zero `fee_per_case`
- [ ] `get_upgrade_timelock` returns the agreed production value
- [ ] `get_admin_expiry` is unset, or a rotation plan exists
- [ ] The platform is unpaused only after all of the above are complete
