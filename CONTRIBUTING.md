# Contributing to SorobanLard

Thanks for your interest in contributing! This guide covers how the contract is
organized, the conventions we follow, and how to get your change merged.

## Repository layout

The core contract lives in a single large file:

```
contracts/sorobanlard/
├── src/
│   ├── lib.rs      # the entire contract (6000+ lines)
│   └── test.rs     # the test suite (291 tests)
├── Makefile        # build / optimize targets
└── Cargo.toml
```

### How `lib.rs` is organized

`lib.rs` is intentionally kept as one file, but it follows a fixed top-to-bottom
layout. When adding code, place it in the matching section rather than at the end
of the file:

1. **Fee / risk types** — enums and structs describing fees, risk parameters,
   and related configuration.
2. **Data types** — the domain structs and enums used by the contract's public
   API (positions, pools, etc.).
3. **Storage keys** — the `DataKey` / `DataKey2` enums used to address persistent
   storage.
4. **The `impl` block** — one large `impl` block containing the contract's
   functions, grouped by feature area.

Keep new public functions inside the existing `impl` block and near the related
feature code, and add a doc comment (`///`) describing behavior and any panics.

### `DataKey` vs `DataKey2`

Soroban's XDR representation limits an enum to **50 variants**. Because the
contract's storage keys exceed that limit, they are split across two enums:

- `DataKey` — the original set of storage keys.
- `DataKey2` — overflow keys added once `DataKey` reached the 50-variant cap.

When adding a new storage key:

- Add it to `DataKey` if it still has room (fewer than 50 variants).
- Otherwise add it to `DataKey2`.
- Never reorder or remove existing variants — their discriminants are part of
  the on-chain storage layout.

## Building and testing

Run these from `contracts/sorobanlard/`:

```sh
# Run the full test suite
cargo test

# Lint (must be clean)
cargo clippy --all-targets -- -D warnings

# Build the wasm artifact
make build

# Build an optimized wasm artifact
make optimize
```

### Adding a test

All tests live in `src/test.rs`. To add a test:

1. Add a `#[test]` function to `test.rs` following the naming and setup patterns
   of the surrounding tests.
2. Reuse the existing test helpers/fixtures rather than duplicating setup.
3. Cover both the success path and relevant failure/panic paths.
4. Run `cargo test` and confirm the new test passes alongside the existing suite.

## Pull request checklist

Before opening a PR, please confirm:

- [ ] The change is covered by tests (new behavior needs new tests).
- [ ] `cargo test` passes.
- [ ] `cargo clippy --all-targets -- -D warnings` is clean.
- [ ] New public functions have doc comments.
- [ ] New storage keys follow the `DataKey` / `DataKey2` rules above.
- [ ] The change stays focused on the linked issue.

## Error and panic messages

Before adding or changing any panic message or error condition, read
[`ERRORS.md`](ERRORS.md). It is the canonical mapping from contract failures
to user-facing API error responses in the backend.

### Why `DataKey` and `DataKey2` are split

`#[contracttype]` enums are limited to **50 variants** in the generated XDR.
`lib.rs` therefore splits storage keys across two enums so the contract can
keep growing without hitting that ceiling:

- `DataKey` — the original key enum, now **full** (50 / 50 variants, 0
  headroom). Do not add variants to it.
- `DataKey2` — the overflow enum for new keys. Check its current variant count
  and remaining headroom in the enum comment in `lib.rs` before adding a key.

### Where new keys go

1. **Composite** keys must be standalone `#[contracttype]` structs stored
   directly (for example `ArbitrationCaseKey`), not tuple variants.
2. **Singleton** keys go in `DataKey2` while it has headroom.
3. Once `DataKey2` approaches the 50-variant limit, add a new overflow enum
   (`DataKey3`, then `DataKey4`, ...) following the same pattern: a fresh
   `#[contracttype]` enum with its own documented variant count, and route new
   singleton keys there. Never reopen a full enum.

Keep the documented variant counts, the enum comments in `lib.rs`, and the
`test_data_key_variant_budgets_are_exact_and_documented` test in sync whenever
you add a key.

### Guardrails

- A compile-time assertion fails the build if either key enum exceeds the
  50-variant XDR limit.
- A CI check warns when either enum approaches the limit so the next overflow
  enum can be planned before the build breaks.

- Every panic message/condition must have a row in `ERRORS.md` describing its
  intended end-user meaning and the API response it maps to.
- Keep `code` values stable; only add new codes, never rename existing ones.
- Unknown panic messages must map to a generic `500` and be logged
  server-side, never leaked to end users.

## Panic messages

Panic message strings in `lib.rs` follow a short style guide so the eventual
error-enum migration stays mechanical:

- **Lowercase** the first word (e.g. `course not found`, not `Course not found`).
- **No trailing period** or other end punctuation.
- Use consistent **`X cannot Y`** phrasing for precondition failures
  (e.g. `instructor cannot grade frozen course`).
- Use one term per concept — prefer `unauthorized` for caller/permission
  failures rather than mixing role-specific or state-specific wording.
- Dynamic values are fine, but keep the surrounding text in the same style
  (e.g. `token decimal precision must be {} (found {})`).

A CI grep check enforces the basics (no trailing period, no leading capital) on
panic strings; run it locally with the command documented in the workflow.

## Pull requests

- Branch from `main` and keep each PR focused.
- Run `cargo fmt --check` and `cargo test` before opening a PR.
- Add or update tests for any behaviour change, especially new events, guards,
  and storage keys.
- Update `README.md` when a public contract function or emitted event changes.
- Update `ERRORS.md` whenever a new panic condition or error message is added,
  and keep the API error mapping in sync.

## Continuous integration

Every push and pull request runs the CI workflow defined in
[`.github/workflows/ci.yml`](.github/workflows/ci.yml), which executes
`cargo test --workspace` and caches the cargo registry and `target`
directories to keep runs fast.

Maintainers should configure branch protection on `main` so the CI check is
**required for merge**. This is applied in the repository settings (Settings →
Branches → branch protection rules) and is not part of the code in this repo.

CI runs `python3 scripts/check_readme_api.py`, which fails if a documented
`name(...)` call has a different number of arguments than the matching
`pub fn` in `lib.rs`. It only warns about public functions the README does
not mention yet.

## Dependency advisories

CI runs `cargo audit` against `Cargo.lock` on every push and pull request.

- A vulnerability with a fix available must be resolved by upgrading the
  dependency. Do not ignore it.
- An advisory with **no available fix** may be accepted by adding its ID to
  `ignore` in [`.cargo/audit.toml`](.cargo/audit.toml). Each entry must have a
  comment giving the advisory's reason for acceptance, the affected crate,
  and the date it should next be reviewed.
- Ignored advisories are reviewed when the dependency is upgraded and at least
  once per release. Remove the entry as soon as a fix is available.
