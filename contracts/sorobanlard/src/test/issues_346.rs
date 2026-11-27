// ============================================================
// ARBITRATION CASE KEY (#346)
// ============================================================
//
// `ArbitrationCaseKey` is a direct `(caller, course_id)` key kept as a storage
// compatibility exception, so the property worth pinning down is that it
// addresses exactly one case and collides with nothing.
//
// The tests go through the public API because `DataKey` and `DataKey2` are
// private to the contract module. If a variant collided, one of these reads
// would return the wrong record.

use super::*;

const ARB_FEE: i128 = 5_000_000;

/// A case stored under `(caller, course_id)` comes back with every field it
/// went in with.
#[test]
fn test_346_arbitration_case_key_round_trips() {
    let (env, contract_id, token_id, admin, _sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let student = setup_arbitration(&env, &client, &token_id, &admin, &instructor, "COURSE-346-A", ARB_FEE);
    let course_id = String::from_str(&env, "COURSE-346-A");

    client.escalate_to_arbitration(&student, &course_id);

    let case = client
        .get_arbitration_case(&student, &course_id)
        .expect("the key that stored the case must find it again");
    assert_eq!(case.caller, student);
    assert_eq!(case.course_id, course_id);
    assert_eq!(case.instructor, instructor);
    assert_eq!(case.token, token_id);
    assert_eq!(case.fee_paid, ARB_FEE);
    assert_eq!(case.status, DisputeStatus::Open);
}

/// Both halves of the key matter. Another caller on the same course, and the
/// same caller on another course, each address a different case — which is the
/// property a composite key has to have to be safe as a direct key.
#[test]
fn test_346_both_halves_of_the_key_select_the_case() {
    let (env, contract_id, token_id, admin, _sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let one = setup_arbitration(&env, &client, &token_id, &admin, &instructor, "COURSE-346-B", ARB_FEE);
    let b = String::from_str(&env, "COURSE-346-B");

    // A second student on the same course, and the first student on a second course.
    let two = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_id).mint(&two, &100_000_000_000);
    client.enroll(&two, &b);

    register_and_approve_course(&env, &client, &token_id, &admin, &instructor, "COURSE-346-C", 1_000_000_000);
    let c = String::from_str(&env, "COURSE-346-C");
    client.enroll(&one, &c);

    client.escalate_to_arbitration(&one, &b);

    assert!(client.get_arbitration_case(&one, &b).is_some(), "the case that was opened");
    assert!(
        client.get_arbitration_case(&two, &b).is_none(),
        "a different caller on the same course must not read the first caller's case"
    );
    assert!(
        client.get_arbitration_case(&one, &c).is_none(),
        "the same caller on a different course must not read it either"
    );
}

/// Two cases live side by side without either overwriting the other.
#[test]
fn test_346_two_cases_coexist_under_their_own_keys() {
    let (env, contract_id, token_id, admin, _sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let one = setup_arbitration(&env, &client, &token_id, &admin, &instructor, "COURSE-346-D", ARB_FEE);
    let d = String::from_str(&env, "COURSE-346-D");

    let two = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_id).mint(&two, &100_000_000_000);
    client.enroll(&two, &d);

    client.escalate_to_arbitration(&one, &d);
    client.escalate_to_arbitration(&two, &d);

    let first = client.get_arbitration_case(&one, &d).expect("first case");
    let second = client.get_arbitration_case(&two, &d).expect("second case");
    assert_eq!(first.caller, one);
    assert_eq!(second.caller, two);
    assert_ne!(first.caller, second.caller);

    // And the course-level index sees both, so the direct key and the list key
    // describe the same world.
    assert_eq!(client.get_arbitration_cases_by_course(&d).len(), 2);
}

/// The key does not answer for a course that has no case, so an absent record
/// reads as absent rather than as some other variant's value.
#[test]
fn test_346_an_unopened_case_reads_as_none() {
    let (env, contract_id, token_id, admin, _sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let student = setup_arbitration(&env, &client, &token_id, &admin, &instructor, "COURSE-346-E", ARB_FEE);
    let course_id = String::from_str(&env, "COURSE-346-E");

    assert!(client.get_arbitration_case(&student, &course_id).is_none());
    assert_eq!(client.get_arbitration_cases_by_course(&course_id).len(), 0);
    assert_eq!(client.get_arbitration_cases_by_student(&student).len(), 0);
}
