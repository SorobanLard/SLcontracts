# Dispute System Design Notes

This document captures the rationale behind the arbitration/dispute data model
(`ArbitrationCase`, `DisputeStatus`, and `DataKey`). It replaces the stray
root-level file `SorobanLard dispute struct and datakey`, which was an accidental
copy of `Cargo.toml` rather than actual design notes.

## `ArbitrationCase`

An `ArbitrationCase` is the on-chain record of a single dispute. It groups
together everything needed to resolve a disagreement between the parties of an
escrow:

- the escrow it belongs to (so the case can be tied back to the funds in play),
- the parties involved (buyer/seller or payer/payee),
- the current `DisputeStatus`,
- any evidence or resolution metadata required by the arbiter.

Keeping the case as a single struct means a dispute can be loaded, inspected,
and updated atomically, and that the storage layout stays predictable across
contract upgrades.

## `DisputeStatus`

`DisputeStatus` is the state machine for a case. It is intentionally small and
explicit so that transitions are easy to audit:

- an **open** state when a dispute is first raised,
- an **in-review** state while an arbiter is evaluating evidence,
- a **resolved** state once a decision has been recorded.

Modeling status as an enum (rather than free-form flags) prevents invalid
combinations and makes the allowed transitions obvious in code review.

## `DataKey`

`DataKey` is the storage key enum used to address contract state in Soroban's
persistent/temporary storage. Each variant maps to a distinct logical record
(e.g. a case by id, an index of open cases, or configuration values). Using a
typed enum instead of raw byte keys:

- avoids key collisions between unrelated records,
- keeps serialization stable and explicit,
- makes it straightforward to enumerate and reason about what the contract
  stores.

## Why this lives in `docs/`

Design notes belong under `docs/` with a `.md` extension so GitHub renders them
as documentation. The previous root-level file had no extension and contained
workspace configuration, which was confusing to anyone looking for dispute
design rationale.
