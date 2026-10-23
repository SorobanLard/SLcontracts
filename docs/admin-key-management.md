# Admin Key Management and Multisig Setup

The contract stores two admin addresses, `admin` and `secondary_admin`, set once
in `init()`. Both are plain Stellar `Address` values. The contract only calls
`require_auth()` on them, so each one can be any account type Soroban accepts.
This guide explains how the two roles are enforced and how to back each one
with a multisig, so that no single key can change the contract.

## How the two admin roles are enforced

The contract has two kinds of admin operation. They use different checks.

| Kind | Examples | Who may call | Check in `lib.rs` |
|---|---|---|---|
| Single-admin | `pause_platform`, `add_approved_token`, `update_default_fee`, `freeze_instructor` | `admin` only | `require_admin()` |
| Admin or approver | `approve_course` | `admin` or an address added with `add_approver` | `require_admin_or_approver()` |
| Pair (multi-admin) | `propose_upgrade`, `upgrade_contract`, `cancel_upgrade`, `set_upgrade_timelock`, `transfer_admin`, `update_treasury`, `set_admin_expiry` | `admin` and `secondary_admin`, in either order | `require_multi_admin()` |

Consequences for operators:

- **`secondary_admin` cannot perform single-admin operations.** Its role is
  only to co-sign the pair operations above. Treat it as a second signature
  requirement, not as a backup owner.
- **Pair operations need both addresses to authorize the same invocation.**
  The two signatures must come from two different addresses. Passing the same
  address twice is rejected with `unauthorized: requires both admin signatures`.
- **Admin expiry blocks both kinds of operation.** If `set_admin_expiry` has
  set an expiry ledger, every admin call panics with `admin role has expired`
  once the current ledger reaches it. Do not set an expiry on a key set you
  cannot rotate in time.

## Init-time requirements

`init()` rejects the deployment unless all of these hold. Check them against
the addresses you plan to use before you deploy.

| Rule | Panic message |
|---|---|
| `admin` ≠ `treasury` | `admin and treasury must be distinct addresses` |
| `secondary_admin` ≠ `treasury` | `secondary_admin and treasury must be distinct addresses` |
| `admin` ≠ `secondary_admin` | `admin and secondary_admin must be distinct addresses` |
| `treasury` is not the contract's own address | `treasury cannot be the contract address` |

The distinctness rule means the two admin roles must be two separate
addresses. It does not stop them from sharing a signer, so the setup below
keeps the signer sets separate as well.

`init()` also takes `default_fee_pct` (at most 100), `max_courses_per_instructor`,
`refund_window_ledgers`, and `revocation_challenge_period`. These are set once
and changed later with their own admin functions, so they do not need to be
settled before the admin keys are.

## Recommended pattern: two Soroban smart-account multisigs

Use a **smart account contract** for each admin role. A smart account is a
contract address that implements `__check_auth`, so it decides for itself
whether a set of signatures is enough. A typical configuration is M-of-N
signers with a threshold, plus rules for adding and removing signers.

Use the following layout:

```
admin           -> smart account A   (signers: keyholders A1..A5, threshold 3-of-5)
secondary_admin -> smart account B   (signers: keyholders B1..B5, threshold 3-of-5)
treasury        -> a separate account, not either admin
```

Why this layout:

- **Two independent quorums.** Every pair operation needs `3-of-5` from group A
  and `3-of-5` from group B. Compromising one group is not enough.
- **No shared signers.** If a keyholder is in both groups, a single person can
  satisfy both quorums. Keep the two sets disjoint.
- **Treasury is not an admin.** The treasury receives fees. It should not be
  able to change admin config, so keep it separate from both roles.

Choose a library-supported smart-account implementation and read its own
documentation for deployment and signer management. This repository does not
ship one. The contract does not care which implementation you use, because it
only sees the address and the `require_auth()` result.

### Deploy and verify a smart account before using it as admin

1. Deploy the smart account contract to testnet with its signer set and
   threshold. Record the contract address.
2. Confirm it authorizes correctly. Run one admin-free test call that requires
   auth, with the threshold number of signers, and confirm it succeeds. Then run
   it with one signer fewer and confirm it fails.
3. Use it as the `admin` in a testnet `init()` and run one single-admin and one
   pair operation through it.
4. Repeat for the secondary account, then deploy the mainnet contract with the
   verified addresses.

### Classic Stellar multisig accounts

A classic account (a `G…` address with additional signers and thresholds) can
also be an `Address`. It is harder to rely on for this contract, because
Soroban authorization entries are checked differently from classic
transaction thresholds. The team has not verified the exact behaviour here.
Before you use one, run the same testnet check as above, and confirm that a
pair operation fails when one signer is missing. If you cannot confirm that,
use a smart account.

## Rotating an admin

Rotation is two steps and is subject to a cooldown. Read
`transfer_admin` and `accept_admin` in `contracts/sorobanlard/src/lib.rs`
before you start.

1. Set up the new pair of smart accounts (or the new keyholder sets) and verify
   them on testnet, as above.
2. Call `transfer_admin(admin1, admin2, new_admin, new_secondary_admin)` with
   the current pair. This records a pending transfer.
3. Call `accept_admin(new_admin, new_secondary_admin)` with the new pair. The
   call panics with `callers are not the pending admins` unless the arguments
   match the pending transfer.

`transfer_admin` enforces a cooldown of `17_280` ledgers (about one day at
5 seconds per ledger) between transfers. This makes rapid rotation after a
compromise harder. Plan a rotation with the cooldown in mind. You cannot
transfer again until it has passed.

`update_treasury` is also a pair operation, but it does not change the admin
addresses.

## Operator checklist

- [ ] Admin and secondary admin are different addresses, and neither is the treasury
- [ ] Each admin role is a smart account (or other verified multisig) with a threshold above one
- [ ] Signer sets for the two admin roles do not overlap
- [ ] Signer sets are not held by one person or one device
- [ ] Each smart account was tested on testnet for both a passing and a failing signature set
- [ ] `set_admin_expiry` is either unset, or there is a rotation plan before the expiry ledger
- [ ] The rotation runbook (above) has been rehearsed on testnet at least once
- [ ] Recovery contacts for each keyholder are written down outside the repository
