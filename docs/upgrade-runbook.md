# Upgrade Runbook: Testnet Rehearsal Before Mainnet

Every mainnet upgrade of the SorobanLard contract must first be exercised on
testnet with the **same Wasm hash** that will go to mainnet. Only after the
rehearsal passes may the upgrade be proposed on mainnet.

This runbook covers the two-step, time-locked upgrade flow implemented in
`contracts/sorobanlard/src/lib.rs`:

| Function | Who signs | Purpose |
|---|---|---|
| `set_upgrade_timelock(admin1, admin2, ledgers)` | both admins | Sets the review window used by future proposals |
| `propose_upgrade(admin1, admin2, new_wasm_hash)` | both admins | Records the pending Wasm hash and starts the timelock |
| `cancel_upgrade(admin1, admin2)` | both admins | Withdraws the pending proposal |
| `upgrade_contract(admin1, admin2)` | both admins | Swaps the contract Wasm once the timelock has elapsed |
| `get_pending_upgrade()` | none (read) | Returns `new_wasm_hash`, `proposed_at_ledger`, `effective_ledger` |
| `get_upgrade_timelock()` | none (read) | Returns the configured window in ledgers (default `17_280`, about 1 day at 5 s/ledger) |

Storage is preserved across an upgrade. **The contract has no migration
function.** If a `#[contracttype]` struct or a `DataKey` variant changes shape,
existing stored values may fail to decode after the upgrade. The rehearsal
below is the only check for that, so it has to run against state that looks
like mainnet, not an empty contract.

## Prerequisites

- Stellar CLI installed, with two funded testnet keys that are the admin and
  secondary admin of the rehearsal contract.
- The admin and secondary admin for mainnet are different addresses from the
  rehearsal keys. The rehearsal uses testnet-only keys.
- Build tooling from the [README](../README.md#building). The Makefile lives in
  `contracts/sorobanlard/`, so run every `make` command from there.

Set these once in your shell. The values are examples:

```bash
export NET=testnet
export SRC=rehearsal-admin          # stellar key alias, signs as admin1
export ADMIN2=rehearsal-secondary   # stellar key alias, signs as admin2
export CONTRACT_ID=<REHEARSAL_CONTRACT_ID>
```

## Step 1: Build the candidate Wasm

```bash
cd contracts/sorobanlard
make optimize
```

The output is `target/wasm32v1-none/release/sorobanlard.optimized.wasm`.
Record this path. Every later step uses this exact file.

## Step 2: Install the Wasm and record its hash

Upload the Wasm to the network. The upload returns its hash. That hash is what
`propose_upgrade` takes.

```bash
stellar contract install \
  --wasm target/wasm32v1-none/release/sorobanlard.optimized.wasm \
  --source-account "$SRC" \
  --network "$NET"
# prints the 32-byte Wasm hash, hex encoded
export NEW_WASM_HASH=<HASH_PRINTED_ABOVE>
```

Run this on testnet first. The mainnet upload must use the same file. Confirm
the hash is unchanged before proposing:

```bash
shasum -a 256 target/wasm32v1-none/release/sorobanlard.optimized.wasm
# the hex digest must equal NEW_WASM_HASH
```

`propose_upgrade` requires the Wasm to already be on-chain. Proposing a hash
that was never uploaded leaves the contract unable to execute the upgrade.

## Step 3: Verify the hash of a deployed contract

Use this to confirm which Wasm a contract is running, both before the
rehearsal (to match mainnet) and after the upgrade (to confirm the swap):

```bash
stellar contract fetch \
  --id "$CONTRACT_ID" \
  --network "$NET" \
  --out-file deployed.wasm
shasum -a 256 deployed.wasm
```

For a contract that is already live, the digest should equal the Wasm hash the
team recorded at its last deployment.

> The commands in Steps 2 and 3 were written against the Stellar CLI. Check
> `stellar contract --help` for your CLI version before running them. Flag
> names have changed between releases.

## Step 4: Build the rehearsal state

Rehearse on a testnet contract that runs the **current mainnet Wasm**, so the
upgrade is a real upgrade and not a fresh deploy.

1. Deploy the current mainnet Wasm to testnet. Get the exact file that matches
   the mainnet hash recorded in Step 3: rebuild the commit that was deployed to
   mainnet and confirm its `shasum -a 256` matches before you use it. Then
   install it on testnet and deploy by hash:

   ```bash
   stellar contract install \
     --wasm <PATH_TO_CURRENT_MAINNET_WASM> \
     --source-account "$SRC" \
     --network "$NET"

   stellar contract deploy \
     --wasm-hash <CURRENT_MAINNET_WASM_HASH> \
     --source-account "$SRC" \
     --network "$NET"
   ```

2. Initialise it with the same parameters as mainnet, using different testnet
   addresses for `admin`, `secondary_admin`, and `treasury`. The `init`
   function takes seven arguments. Check the current signature in
   `contracts/sorobanlard/src/lib.rs` before you run it.

3. Seed the contract with data that exercises the storage the upgrade touches:
   at least one approved token, a registered and approved course, an
   enrollment, a completed course with an issued certificate, a pending refund
   request, and an open arbitration case if the feature is in use on mainnet.

4. Shorten the timelock for the rehearsal only, so you do not wait a full day.
   Never do this on mainnet:

   ```bash
   stellar contract invoke --id "$CONTRACT_ID" --source-account "$SRC" --network "$NET" \
     -- set_upgrade_timelock --admin1 <ADMIN1> --admin2 <ADMIN2> --ledgers 20
   ```

5. Snapshot the state you will compare after the upgrade: `get_course`,
   `get_enrollment`, `get_certificate`, `get_instructor_earnings`, and the
   admin and config getters. Save the output in your scratchpad.

## Step 5: Propose the upgrade on testnet

```bash
stellar contract invoke --id "$CONTRACT_ID" --source-account "$SRC" --network "$NET" \
  -- propose_upgrade \
  --admin1 <ADMIN1> --admin2 <ADMIN2> \
  --new_wasm_hash "$NEW_WASM_HASH"
```

Both admins must sign. `propose_upgrade` emits `upgrade_proposed`. Read back
the pending state and check it matches the hash you intend to deploy:

```bash
stellar contract invoke --id "$CONTRACT_ID" --network "$NET" -- get_pending_upgrade
# new_wasm_hash must equal NEW_WASM_HASH
# effective_ledger = proposed_at_ledger + timelock
```

Proposing again replaces the pending upgrade and resets the timelock. Do not
propose twice by accident, and record the final `effective_ledger`.

## Step 6: Wait for the timelock

`upgrade_contract` panics with `upgrade time-lock has not elapsed` until the
current ledger reaches `effective_ledger`. Check the latest ledger through
Soroban RPC:

```bash
curl -s -X POST "$RPC_URL" -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getLatestLedger"}'
```

Compare `sequence` with `effective_ledger`. Do not attempt the upgrade earlier.

## Step 7: Execute the upgrade on testnet

```bash
stellar contract invoke --id "$CONTRACT_ID" --source-account "$SRC" --network "$NET" \
  -- upgrade_contract --admin1 <ADMIN1> --admin2 <ADMIN2>
```

Then verify:

1. The running Wasm hash matches the candidate (repeat Step 3). It must equal
   `NEW_WASM_HASH`.
2. `get_pending_upgrade` returns nothing.
3. The state snapshot from Step 4.5 still reads back the same values. Decoding
   errors here are the main storage-compatibility failure this rehearsal exists
   to catch.

## Step 8: Run smoke tests

Run the contract's read and write paths against the upgraded testnet contract:

- Read: `get_course`, `list_courses`, `get_enrollment`, `verify_certificate`
  on pre-upgrade records.
- Write: `register_course`, `approve_course`, `enroll` with a funded student,
  `mark_completed`, `issue_certificate`, and `withdraw_earnings`.
- Admin: `update_default_fee` or another admin-only setter, to confirm the
  admin keys still authorize.

Record the transaction hashes. Attach them to the mainnet proposal.

## Step 9: Propose on mainnet

Only after Steps 1 to 8 pass:

1. Confirm the mainnet contract is running the rehearsal's baseline hash
   (Step 3, run against mainnet).
2. Confirm `NEW_WASM_HASH` on mainnet matches the rehearsed hash. Do not rebuild
   for mainnet. Use the same `.optimized.wasm`.
3. Set the mainnet timelock to the production window the maintainers agreed
   on. The contract default is `17_280` ledgers (about 1 day). Set it with
   `set_upgrade_timelock` before proposing, because a change does not apply to
   a pending proposal. See [MAINNET_CHECKLIST.md](../MAINNET_CHECKLIST.md).
4. Run `propose_upgrade` with the mainnet admin pair, then verify with
   `get_pending_upgrade`.
5. Publish the proposal (hash, `effective_ledger`, and link to the rehearsal
   transactions) for community review.
6. After the timelock, run `upgrade_contract`, then repeat the hash check from
   Step 3 and the smoke tests from Step 8 against mainnet.

## Rollback

There is no on-chain rollback. To undo an upgrade, run a second proposal and
upgrade with the previous Wasm hash. Re-install the previous `.optimized.wasm`
first if its entry is no longer on-chain. Keep that file and its hash until the
new version has been stable on mainnet for a full timelock window.

To stop a proposal before it executes, run `cancel_upgrade` with both admins.

## Checklist

- [ ] Candidate built once; the same file used on testnet and mainnet
- [ ] Hash recorded with `stellar contract install` and matched with `shasum -a 256`
- [ ] Rehearsal contract runs the current mainnet Wasm hash
- [ ] State snapshot taken before the upgrade and compared after
- [ ] Testnet proposal, timelock, and upgrade completed without error
- [ ] Smoke tests passed; transaction hashes recorded
- [ ] Mainnet hash matches the rehearsed hash
- [ ] Mainnet timelock set to the agreed value before proposing
- [ ] Mainnet proposal announced with its `effective_ledger`
