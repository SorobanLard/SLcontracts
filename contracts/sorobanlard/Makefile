# SorobanLard Contract — Makefile
#
# Deployment runbook: after running `make deploy-testnet`, record the resulting
# contract ID, deployed Wasm hash, and deployment date in DEPLOYMENTS.md at the
# repository root. Keep DEPLOYMENTS.md in sync with every deployment.

# Toolchain assumptions:
#   - Rust with the wasm32v1-none target installed:
#       rustup target add wasm32v1-none
#   - Stellar CLI (stellar-cli) providing the `stellar` binary, with the
#     `contract build`, `contract optimize`, and `contract deploy` subcommands
#     (recent stellar-cli releases). Verify with `stellar --version`.
#   - `cargo` / `rustup` on PATH for the test/fmt/check/clean targets.
#
# Environment variables:
#   STELLAR_ACCOUNT  (required for deploy-testnet) — the source identity used by
#                    `stellar contract deploy --source`. This is a named identity
#                    configured in stellar-cli (e.g. via `stellar keys generate`)
#                    or a secret key, not a filesystem path.
#   NETWORK          (optional) — overrides the target network for deploy-testnet.
#                    Defaults to `testnet` below.
#   CONTRACT_NAME    (optional) — overrides the contract/wasm base name.
#                    Defaults to `sorobanlard` below.

NETWORK       = testnet
CONTRACT_NAME = sorobanlard
WASM_PATH     = target/wasm32v1-none/release/$(CONTRACT_NAME).wasm
WASM_OPT      = target/wasm32v1-none/release/$(CONTRACT_NAME).optimized.wasm

.PHONY: test build optimize deploy-testnet clean fmt check

# Run the contract's Rust test suite via `cargo test`.
test:
	cargo test

# Compile the contract to wasm using `stellar contract build`,
# producing $(WASM_PATH).
build:
	stellar contract build

# Build, then shrink the wasm with `stellar contract optimize`,
# producing $(WASM_OPT).
optimize: build
	stellar contract optimize --wasm $(WASM_PATH)
	@echo "Optimized: $(WASM_OPT)"

# Deploy to testnet. After a successful deploy, update DEPLOYMENTS.md with the
# new contract ID, Wasm hash, and deployment date (see repo root DEPLOYMENTS.md).

# Optimize, then deploy the optimized wasm to $(NETWORK) using the identity
# in $(STELLAR_ACCOUNT) via `stellar contract deploy`.
deploy-testnet: optimize
	stellar contract deploy \
		--wasm $(WASM_OPT) \
		--source $(STELLAR_ACCOUNT) \
		--network $(NETWORK)
	@echo "Deployed. Remember to record the contract ID, Wasm hash, and date in DEPLOYMENTS.md."

# Format the Rust sources with `cargo fmt`.
fmt:
	cargo fmt

# Type-check the crate with `cargo check` (no codegen).
check:
	cargo check

# Remove build artifacts with `cargo clean`.
clean:
	cargo clean
