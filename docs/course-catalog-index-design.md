# Design: indexed course catalog

Tracks issue #169. This is a design proposal only. No storage layout changes
are made by this document. The issue asks for a design doc before any change,
because the change needs a storage migration.

## The problem

Two catalogs are stored as one persistent `Vec<String>` each:

- `DataKey::CourseList`: every registered course, used by `list_courses`.
- `DataKey::InstructorCourseList(Address)`: one per instructor, used by
  `get_courses_by_instructor` and read in full by `freeze_instructor`.

`register_course` reads the whole `CourseList`, pushes one ID and writes the
whole Vec back. The same happens for the instructor's list. So the cost of
registering course N grows with N.

Two separate costs follow from that:

1. **Metered cost.** Loading and storing the Vec costs host work proportional to
   its encoded size. Every registration pays for every earlier registration.
2. **Hard size limit.** A single ledger entry can only grow to the network's
   maximum entry size. Once `CourseList` reaches that size, `register_course`
   cannot write it at all, and the platform stops accepting new courses. This
   is a correctness problem, not only a performance one. Check the current value
   in the network configuration; it is not in this repository.

`list_courses` has the same shape: it loads the whole catalog to return one page
(see the comment on `MAX_PAGE_SIZE` in `lib.rs`).

## Why a Map does not fix it

A `Map<String, u32>` stored under one key is still one ledger entry. It keeps the
same size ceiling and the same load/store cost on every write. The benefit of an
index comes from splitting the data across entries, so each write touches only
the entry it needs.

## Proposal: one entry per index position

Replace each Vec with a counter plus one entry per position.

**Global catalog**

| Key | Tier | Holds |
|---|---|---|
| `DataKey2::CourseCount` | Instance | Number of courses ever registered. Singleton, so it goes in `DataKey2`, which has headroom. |
| `CourseIndexKey { index }` | Persistent | The course ID at that position. Standalone struct, following the `ArbitrationCaseKey` pattern, because composite keys do not go in `DataKey`. |

Registration: read the count, write `CourseIndexKey { index: count }`, bump the
count. Cost is constant and does not depend on catalog size.

`list_courses(offset, limit)`: read `CourseCount`, then read
`CourseIndexKey` for each index in `offset..min(offset + limit, count)`. Cost
depends on `limit`, which `MAX_PAGE_SIZE` already caps.

**Per instructor**

| Key | Tier | Holds |
|---|---|---|
| `InstructorCourseLen(Address)` | Instance or persistent | Length of that instructor's index. |
| `InstructorCourseIndexKey { instructor, index }` | Persistent | The course ID at that position. |

Do not use `InstructorCourseCount` as the length. It is decremented by
`transfer_course`, so it is the instructor's *current* count and does not match
the append-only list. `InstructorCourseList` is append-only, and
`transfer_course` appends the transferred course to the new owner's list. A
separate length key is needed.

**Ordering and stability.** Entries are append-only and never removed, so an
index stays valid for any caller that paginates by offset. This matches the
current behaviour, where archived courses stay in `CourseList`.

## Trade-offs

- **Read cost for large pages.** Reading 50 entries costs 50 separate ledger
  entry reads, each with its own footprint. For a small catalog a single Vec
  load can be cheaper. The break-even point needs measuring.
- **Rent and TTL.** Each entry needs its own TTL extension, so writes that touch
  many entries cost more in rent than one Vec. Reads that should keep an entry
  alive need the same extension.
- **Code churn.** `register_course`, `list_courses`, `get_courses_by_instructor`,
  `transfer_course` and `freeze_instructor` all change. `freeze_instructor`
  iterates the whole instructor list and would need a bounded or paginated
  version. That is a related concern in #238.

## Migration

Existing deployments have the Vecs populated, so the new layout needs a copy
step:

1. Pause the platform (`pause_platform`) so no registrations change the catalog
   during the copy.
2. Run an admin-only, chunked migration: each call copies up to N entries from
   the old Vec into the indexed keys and records a cursor. A single call cannot
   copy a large catalog within one transaction's budget.
3. Set a `CatalogMigrated` marker in `DataKey2` once the cursor reaches the end.
   Only then switch the readers to the new keys.
4. Remove the old Vecs after the marker is set.

Dual-reading the old and new layout would avoid the pause but doubles the read
paths. It is not recommended.

Because this changes stored data, it needs an upgrade path that is tested on a
copy of a real deployment, not only in the unit tests.

## Measurement plan

Before implementing, measure the current cost and the proposed cost at 10, 1,000
and 10,000 entries:

- Seed the catalog directly with `env.as_contract` so the test does not register
  10,000 courses through the public API.
- Use the same budget harness as `bench_batch_enroll_at_max_batch_size` (see
  [batch-enroll-cost.md](batch-enroll-cost.md)).
- Record `register_course` and `list_courses(0, 50)` for each size, both layouts.

These numbers are not yet measured. The local build must be run to produce them.

## Recommendation

- Accept the direction: one entry per index position with a counter.
- Do not change storage yet. Open a follow-up implementation issue that depends
  on the measurement above and includes the migration.
- Treat the entry size ceiling as the main reason to act. The performance gain
  matters less than the fact that the current layout eventually stops working.

## Related

- #167 (batch cost), #168 (clone audit), #170 (registry lookup, which removes the
  scan of `InstructorRegistry` but not its Vec load).
- #238: unbounded `InstructorCourseList` and `InstructorRegistry` growth.
- `docs/storage-keys.md`: tier rules for any new key.
