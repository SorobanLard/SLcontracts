// ============================================================
// UPGRADE SIGNATURES (#349)
// ============================================================
//
// `upgrade_contract`, `propose_upgrade` and `cancel_upgrade` all go through
// `require_multi_admin`, which refuses two identical callers.
//
// The successful upgrade itself is not here: it ends in
// `update_current_contract_wasm`, which needs a real uploaded Wasm hash, so the
// test host cannot reach it. `scripts/e2e-wasm.sh` is where that belongs.

use super::*;

/// One admin passing themselves twice is the cheapest way to try to satisfy a
/// two-signature check, and it is refused.
#[test]
#[should_panic(expected = "unauthorized: requires both admin signatures")]
fn test_349_upgrade_with_a_single_admin_is_refused() {
    let (env, contract_id, _token_id, admin, sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    client.propose_upgrade(&admin, &sec_admin, &BytesN::from_array(&env, &[7u8; 32]));
    client.upgrade_contract(&admin, &admin);
}

/// Nor does the secondary admin alone carry it.
#[test]
#[should_panic(expected = "unauthorized: requires both admin signatures")]
fn test_349_upgrade_with_the_secondary_admin_alone_is_refused() {
    let (env, contract_id, _token_id, admin, sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    client.propose_upgrade(&admin, &sec_admin, &BytesN::from_array(&env, &[7u8; 32]));
    client.upgrade_contract(&sec_admin, &sec_admin);
}

/// An admin plus a stranger is not two admins.
#[test]
#[should_panic(expected = "unauthorized: requires both admin signatures")]
fn test_349_an_admin_and_a_stranger_cannot_upgrade() {
    let (env, contract_id, _token_id, admin, sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    client.propose_upgrade(&admin, &sec_admin, &BytesN::from_array(&env, &[7u8; 32]));
    client.upgrade_contract(&admin, &Address::generate(&env));
}

/// Proposing an upgrade needs both signatures too — otherwise one admin could
/// queue the code that the other is only asked to rubber-stamp.
#[test]
#[should_panic(expected = "unauthorized: requires both admin signatures")]
fn test_349_proposing_with_a_single_admin_is_refused() {
    let (env, contract_id, _token_id, admin, _sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    client.propose_upgrade(&admin, &admin, &BytesN::from_array(&env, &[7u8; 32]));
}

/// And so does cancelling one.
#[test]
#[should_panic(expected = "unauthorized: requires both admin signatures")]
fn test_349_cancelling_with_a_single_admin_is_refused() {
    let (env, contract_id, _token_id, admin, sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    client.propose_upgrade(&admin, &sec_admin, &BytesN::from_array(&env, &[7u8; 32]));
    client.cancel_upgrade(&admin, &admin);
}

/// With both signatures the call passes the authorisation gate and is stopped
/// only by the time-lock — which is how you tell the two refusals apart.
#[test]
#[should_panic(expected = "upgrade time-lock has not elapsed")]
fn test_349_both_admins_pass_the_signature_gate_and_meet_the_timelock() {
    let (env, contract_id, _token_id, admin, sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    client.set_upgrade_timelock(&admin, &sec_admin, &1_000u32);
    client.propose_upgrade(&admin, &sec_admin, &BytesN::from_array(&env, &[7u8; 32]));

    client.upgrade_contract(&admin, &sec_admin);
}

/// Either order of the two admins satisfies the check: it is a set of two
/// signatures, not a sequence.
#[test]
#[should_panic(expected = "upgrade time-lock has not elapsed")]
fn test_349_the_order_of_the_two_admins_does_not_matter() {
    let (env, contract_id, _token_id, admin, sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    client.set_upgrade_timelock(&admin, &sec_admin, &1_000u32);
    client.propose_upgrade(&sec_admin, &admin, &BytesN::from_array(&env, &[7u8; 32]));

    client.upgrade_contract(&sec_admin, &admin);
}

/// The proposal itself records what both admins agreed to.
#[test]
fn test_349_a_two_signature_proposal_is_recorded() {
    let (env, contract_id, _token_id, admin, sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let hash = BytesN::from_array(&env, &[9u8; 32]);
    let proposed_at = env.ledger().sequence();
    let timelock = client.get_upgrade_timelock();

    client.propose_upgrade(&admin, &sec_admin, &hash);

    let pending = client.get_pending_upgrade().expect("a proposal is pending");
    assert_eq!(pending.new_wasm_hash, hash);
    assert_eq!(pending.effective_ledger, proposed_at + timelock);
}
