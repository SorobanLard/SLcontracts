# Clone audit: `Address` / `String` on hot paths

Tracks issue #168. Covers the three functions the issue names:
`register_course`, `batch_enroll` (and the enrollment path it calls) and
`transfer_course`.

## Summary

`Address` and `String` in `soroban-sdk` are handles to host objects, so `.clone()`
copies a handle rather than the underlying bytes. That makes these clones cheap.
This audit therefore looked for two things: clones that were avoidable, and
storage keys that were built more than once. Neither is expensive on its own.

Outcome:

- **Changed:** redundant key construction in `register_course` and
  `transfer_course`, one avoidable clone in `transfer_course`.
- **Kept:** every remaining clone is needed to satisfy an API that takes its
  argument by value (see below).

No behaviour change. Storage writes, events and the order of checks are the same.

## Changes

| Function | Before | After |
|---|---|---|
| `register_course` | `DataKey::Course(course_id.clone())` built for the write and again for the TTL extension | Built once as `course_key` |
| `register_course` | `DataKey::CourseInstructorRef(course_id.clone())` built twice | Built once as `instructor_ref_key` |
| `register_course` | `DataKey::InstructorCourseCount(instructor.clone())` rebuilt for the write although `course_count_key` already held it | Reuses `course_count_key` |
| `register_course` | Registry membership found by scanning the whole Vec on every registration | Direct lookup via `InstructorRegisteredKey` (#170) |
| `transfer_course` | `course.instructor.clone()` to keep the old owner, then `new_instructor.clone()` to set the new one | `core::mem::replace(&mut course.instructor, new_instructor.clone())` returns the old owner without a second clone |
| `transfer_course` | `DataKey::Course` and `DataKey::CourseInstructorRef` each built twice | Built once each |

## Remaining clones and why they stay

**`register_course`**

- `course_id.clone()` in `has(&DataKey::Course(...))` and the `ArchivedCourse`
  lookup. `DataKey` variants take their payload by value.
- `instructor.clone()` in the count and list keys, and in `Course { instructor }`.
  `instructor` is used again afterwards.
- `course_id.clone()` in `Course { id }`, `catalog.push_back`,
  `instructor_courses.push_back` and the event topic. `course_id` is returned at
  the end of the function, so one copy has to stay.
- `env.clone()` and `token.clone()` in `get_fee_config(...)`. The function takes
  both by value and `token` is moved into `Course` later.

**`batch_enroll`**

- `seen.contains_key(id.clone())`. `Map::contains_key` takes its key by value.
- `course_ids.get(i).unwrap()` returns an owned `String`. This is not a `.clone()`
  call, but the loop reads each element once per pass, so the handle is reused
  across the validation and enroll loops.

**`transfer_course`**

- `previous_instructor.clone()` in the two count keys. `previous_instructor` is
  moved into the event at the end.
- `new_instructor.clone()` in the new owner's keys. `new_instructor` is also
  moved into the event at the end.

## Not changed, for follow-up

- `get_course_internal` builds `DataKey::Course` and `DataKey::ArchivedCourse`
  for every read. Reads run in `batch_enroll` twice per course (see
  [batch-enroll-cost.md](batch-enroll-cost.md)). Removing that re-read would save
  more than any clone change here.
- `get_fee_config` takes its arguments by value because it is a public contract
  function. An internal borrowing helper would remove the `token.clone()`, but
  that is a small gain.
