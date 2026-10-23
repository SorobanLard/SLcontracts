# `batch_enroll` cost at `MAX_BATCH_SIZE`

Tracks issue #167: whether `MAX_BATCH_SIZE = 50` still leaves a safe margin
under the per-transaction instruction limit now that enrollment does more work
(risk fees, prerequisites, waitlist checks, token transfers).

## Status

- The benchmark is in `contracts/sorobanlard/src/test.rs` as
  `bench_batch_enroll_at_max_batch_size`. It is `#[ignore]`d so the normal suite
  stays fast.
- **No numbers are recorded yet.** The table below must be filled in from a
  local run before `MAX_BATCH_SIZE` is changed.
- `MAX_BATCH_SIZE` is unchanged.

## How to measure

```sh
cargo test bench_batch_enroll_at_max_batch_size -- --ignored --nocapture
```

The test registers and approves 50 courses, enrolls one student in all of them
in a single `batch_enroll` call, and prints:

```
batch_enroll(50): cpu_instructions=<N> memory_bytes=<M>
```

Notes on reading the output:

- The numbers come from the host's metering in the Rust test environment. The
  `soroban-sdk` docs say CPU is likely underestimated compared with the same code
  running as WASM, so treat them as a lower bound.
- The test calls `reset_unlimited()` first so a large batch cannot abort before
  its cost is read. A run that aborts with a budget error is itself a finding and
  should be reported.
- Use the same `soroban-sdk` version as `Cargo.lock` (currently 22.0.11) and
  record it with the numbers.

## Results

| Date | soroban-sdk | Batch size | CPU instructions | Memory bytes | Notes |
|---|---|---|---|---|---|
| _pending_ | 22.0.11 | 50 | _pending_ | _pending_ | Run locally and fill in. |

## Decision rule

Compare the CPU figure against the **current** per-transaction instruction limit
in the network configuration. Confirm the value there, since it is not in this
repository. Then:

- If the 50-course figure uses most of the limit, lower `MAX_BATCH_SIZE`.
- If it uses a small fraction, keeping 50 is fine. Note the margin in
  `MAX_BATCH_SIZE`'s doc comment.

Either way, once numbers are recorded, add a pointer to this file in the
`MAX_BATCH_SIZE` doc comment in `lib.rs`.

## Where the per-course work goes

These counts come from reading `batch_enroll`, `validate_enrollment` and
`enroll_internal_validated`. They are not measured.

For each course in the batch:

1. **Validation pass** (`validate_enrollment`):
   - reads the course (`get_course_internal`), which tries `Course` and then
     `ArchivedCourse`;
   - reads `CourseInstructorRef`, the student blocklist, the instructor freeze
     flag, the admin check and the `Enrollment` existence check;
   - calls `token.decimals()`, a cross-contract call;
   - loops over prerequisite courses, reading each `Enrollment`.
2. **Enroll pass** (`enroll_internal_validated`):
   - reads the same course again;
   - computes the fee split through `deduct_fee`, which reads the fee configs;
   - reads and may update `Treasury` and `PendingTreasury`;
   - for paid courses, makes token calls: `balance`, `transfer` from the
     student, `balance` again to verify the amount received, and a `transfer`
     to the treasury when the platform share is non-zero;
   - writes `Course`, `Enrollment`, `InstructorEarnings`, `TotalActiveEnrollments`
     and the events.

So a paid 50-course batch makes up to 250 cross-contract token calls (one
`decimals` plus up to four transfer-path calls per course), and it reads each
course twice. The double read is a candidate for a follow-up change.
Token decimals are also re-read per course even though they are the same token
across the batch (see #236).

## Related

- #167 (this issue), #168 (clone audit: [hot-path-clone-audit.md](hot-path-clone-audit.md)).
- #236: caching `token.decimals()`.
- `docs/storage-keys.md` for which keys each read touches.
