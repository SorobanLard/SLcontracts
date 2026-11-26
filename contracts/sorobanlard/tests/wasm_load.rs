//! Loads the compiled contract wasm into the soroban-sdk test host (#341).
//!
//! The wasm must be built first, so this test is ignored by a plain
//! `cargo test`. `scripts/test_wasm_build.sh` builds it with `make build` and
//! then runs this test with `--ignored`. Set `SOROBANLARD_WASM` to load a
//! different file (for example the `.optimized.wasm` variant).

use soroban_sdk::Env;
use std::path::PathBuf;

fn wasm_path() -> PathBuf {
    match std::env::var("SOROBANLARD_WASM") {
        Ok(path) => PathBuf::from(path),
        Err(_) => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/wasm32v1-none/release/sorobanlard.wasm"),
    }
}

#[test]
#[ignore = "requires the wasm from `make build`; run via scripts/test_wasm_build.sh"]
fn test_host_loads_compiled_wasm() {
    let path = wasm_path();
    let wasm = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    assert!(!wasm.is_empty(), "{} is empty", path.display());

    let env = Env::default();
    // Uploading and instantiating the wasm makes the host parse and validate it.
    env.register(wasm.as_slice(), ());
}
