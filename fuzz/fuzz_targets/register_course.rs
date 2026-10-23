#![no_main]

use libfuzzer_sys::fuzz_target;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Vec};
use sorobanlard::{SorobanLardContract, SorobanLardContractClient};

// Mirrors SorobanLardContract::MAX_COURSE_ID_LEN, which is private to the crate.
const MAX_COURSE_ID_LEN: usize = 256;

// Course IDs must be 1..=256 bytes of printable ASCII (0x20..=0x7E).
fn is_valid_course_id(bytes: &[u8]) -> bool {
    !bytes.is_empty()
        && bytes.len() <= MAX_COURSE_ID_LEN
        && bytes.iter().all(|b| (b' '..=b'~').contains(b))
}

fuzz_target!(|data: &[u8]| {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SorobanLardContract, ());
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();

    let admin = Address::generate(&env);
    let sec_admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let client = SorobanLardContractClient::new(&env, &contract_id);
    client.init(&admin, &sec_admin, &treasury, &20u32, &50u32, &1000u32, &17_280u32);
    client.add_approved_token(&admin, &token_id);

    // Every other register_course precondition is met, so the only thing
    // that can reject this call is the course ID itself.
    let instructor = Address::generate(&env);
    let course_id = String::from_bytes(&env, data);
    let result = client.try_register_course(
        &instructor,
        &course_id,
        &0i128,
        &token_id,
        &0u32,
        &None,
        &BytesN::from_array(&env, &[1u8; 32]),
        &Vec::new(&env),
    );

    assert_eq!(result.is_ok(), is_valid_course_id(data), "input: {data:?}");
});
