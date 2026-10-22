# Contributing

## Storage keys

Before adding or changing any persistent storage key, read
[`docs/storage-keys.md`](docs/storage-keys.md).

In short:

- `DataKey` is **full** (50 / 50 variants, 0 headroom). Do not add variants to
  it.
- New **composite** keys must be standalone `#[contracttype]` structs stored
  directly (for example `ArbitrationCaseKey`), not tuple variants.
- New **singleton** keys go in `DataKey2` while it has headroom.
- Keep the documented variant counts, the enum comments, and the
  `test_data_key_variant_budgets_are_exact_and_documented` test in sync.

A compile-time assertion fails the build if either key enum exceeds the
50-variant XDR limit.

## Pull requests

- Branch from `main` and keep each PR focused.
- Run `cargo fmt --check` and `cargo test` before opening a PR.
- Add or update tests for any behaviour change, especially new events, guards,
  and storage keys.
- Update `README.md` when a public contract function or emitted event changes.
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
