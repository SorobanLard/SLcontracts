// ============================================================
// INIT TREASURY, ROLE BOUNDARIES AND UPGRADE GUARDS (#374–#377)
// ============================================================
//
// #374 — `init` stores the treasury it was given, and the fee split proves it.
// #375 — a student cannot mark their own enrollment complete.
// #376 — an instructor cannot approve their own course.
// #377 — `upgrade_contract` refuses to run without a pending proposal.

use super::*;

// ── #374: the treasury init was given ───────────────────────────────────────

/// There is no `get_treasury`, so the stored address is read where it matters:
/// the platform's share of an enrollment is paid to it.
#[test]
fn test_374_init_stores_the_treasury_that_receives_the_platform_share() {
    let (env, contract_id, token_id, admin, _sec_admin, treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);
    let token_client = token::Client::new(&env, &token_id);

    let student = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_id).mint(&student, &100_000_000_000);

    let price: i128 = 1_000_000_000;
    register_and_approve_course(
        &env,
        &client,
        &token_id,
        &admin,
        &instructor,
        "COURSE-374-A",
        price,
    );

    assert_eq!(token_client.balance(&treasury), 0, "nothing collected yet");
    assert_eq!(
        client.get_platform_fee(&admin),
        20,
        "the fee percentage passed to init is the one in force"
    );
    client.enroll(&student, &String::from_str(&env, "COURSE-374-A"));

    assert_eq!(
        token_client.balance(&treasury),
        price * 20 / 100,
        "the platform share goes to the address passed to init, and nowhere else"
    );
}

/// The treasury has to be a third party: an admin that is also the treasury
/// would take the platform's cut as well as holding its keys.
#[test]
#[should_panic(expected = "admin and treasury must be distinct addresses")]
fn test_374_admin_cannot_also_be_the_treasury() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SorobanLardContract, ());
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let sec_admin = Address::generate(&env);

    client.init(&admin, &sec_admin, &admin, &20u32, &50u32, &1000u32, &17_280u32);
}

/// The secondary admin cannot be the treasury either.
#[test]
#[should_panic(expected = "secondary_admin and treasury must be distinct addresses")]
fn test_374_secondary_admin_cannot_also_be_the_treasury() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SorobanLardContract, ());
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let sec_admin = Address::generate(&env);

    client.init(
        &admin,
        &sec_admin,
        &sec_admin,
        &20u32,
        &50u32,
        &1000u32,
        &17_280u32,
    );
}

/// A contract-owned treasury would trap the platform fee for good.
#[test]
#[should_panic(expected = "treasury cannot be the contract address")]
fn test_374_treasury_cannot_be_the_contract_itself() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SorobanLardContract, ());
    let client = SorobanLardContractClient::new(&env, &contract_id);

    client.init(
        &Address::generate(&env),
        &Address::generate(&env),
        &contract_id,
        &20u32,
        &50u32,
        &1000u32,
        &17_280u32,
    );
}

/// Init happens once; a second call cannot quietly repoint the treasury.
#[test]
#[should_panic(expected = "contract already initialized")]
fn test_374_treasury_cannot_be_repointed_by_a_second_init() {
    let (env, contract_id, _token_id, admin, sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    client.init(
        &admin,
        &sec_admin,
        &Address::generate(&env),
        &20u32,
        &50u32,
        &1000u32,
        &17_280u32,
    );
}

// ── #375: completion is not the student's to declare ────────────────────────

/// A student marking their own enrollment complete would hand themselves a
/// certificate and close the refund window on their own course.
#[test]
#[should_panic(expected = "unauthorized: mark_completed - caller is not admin")]
fn test_375_student_cannot_mark_their_own_enrollment_completed() {
    let (env, contract_id, token_id, admin, _sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let student = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_id).mint(&student, &100_000_000_000);

    let course_id = String::from_str(&env, "COURSE-375-A");
    register_and_approve_course(
        &env,
        &client,
        &token_id,
        &admin,
        &instructor,
        "COURSE-375-A",
        1_000_000_000,
    );
    client.enroll(&student, &course_id);

    // The student is the `admin` argument here — their own authorisation is
    // real, their authority is not.
    client.mark_completed(&student, &student, &course_id, &None);
}

/// Nor can the course's instructor, who has an interest in the outcome.
#[test]
#[should_panic(expected = "unauthorized: mark_completed - caller is not admin")]
fn test_375_instructor_cannot_mark_an_enrollment_completed() {
    let (env, contract_id, token_id, admin, _sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let student = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_id).mint(&student, &100_000_000_000);

    let course_id = String::from_str(&env, "COURSE-375-B");
    register_and_approve_course(
        &env,
        &client,
        &token_id,
        &admin,
        &instructor,
        "COURSE-375-B",
        1_000_000_000,
    );
    client.enroll(&student, &course_id);

    client.mark_completed(&instructor, &student, &course_id, &None);
}

/// The admin can, and the enrollment is what changes — the control is on who
/// calls it, not on whether the call works.
#[test]
fn test_375_admin_can_mark_the_same_enrollment_completed() {
    let (env, contract_id, token_id, admin, _sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let student = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_id).mint(&student, &100_000_000_000);

    let course_id = String::from_str(&env, "COURSE-375-C");
    register_and_approve_course(
        &env,
        &client,
        &token_id,
        &admin,
        &instructor,
        "COURSE-375-C",
        1_000_000_000,
    );
    client.enroll(&student, &course_id);
    client.mark_completed(&admin, &student, &course_id, &None);

    assert_eq!(client.has_completed(&student, &course_id), Some(true));
}

// ── #376: approval is not the instructor's to grant ─────────────────────────

/// An instructor who could approve their own course would be the whole review
/// step by themselves.
#[test]
#[should_panic(expected = "unauthorized: approve_course - caller is not admin or approver")]
fn test_376_instructor_cannot_approve_their_own_course() {
    let (env, contract_id, token_id, _admin, _sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let course_id = String::from_str(&env, "COURSE-376-A");
    client.register_course(
        &instructor,
        &course_id,
        &1_000_000_000,
        &token_id,
        &0u32,
        &None,
        &BytesN::from_array(&env, &[1u8; 32]),
        &soroban_sdk::Vec::new(&env),
    );
    env.ledger().with_mut(|l| l.sequence_number += 1);

    client.approve_course(&instructor, &course_id);
}

/// Nor can an unrelated address that happens to hold a key.
#[test]
#[should_panic(expected = "unauthorized: approve_course - caller is not admin or approver")]
fn test_376_a_stranger_cannot_approve_a_course() {
    let (env, contract_id, token_id, _admin, _sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let course_id = String::from_str(&env, "COURSE-376-B");
    client.register_course(
        &instructor,
        &course_id,
        &1_000_000_000,
        &token_id,
        &0u32,
        &None,
        &BytesN::from_array(&env, &[1u8; 32]),
        &soroban_sdk::Vec::new(&env),
    );
    env.ledger().with_mut(|l| l.sequence_number += 1);

    client.approve_course(&Address::generate(&env), &course_id);
}

/// A course the instructor could not approve is approved by an admin, and by an
/// approver the admin has added — the two roles the gate does admit.
#[test]
fn test_376_admin_and_approver_can_approve_someone_elses_course() {
    let (env, contract_id, token_id, admin, _sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let approver = Address::generate(&env);
    client.add_approver(&admin, &approver);
    assert!(client.is_approver_address(&approver));

    for (course_id, caller) in [("COURSE-376-C", &admin), ("COURSE-376-D", &approver)] {
        let id = String::from_str(&env, course_id);
        client.register_course(
            &instructor,
            &id,
            &1_000_000_000,
            &token_id,
            &0u32,
            &None,
            &BytesN::from_array(&env, &[1u8; 32]),
            &soroban_sdk::Vec::new(&env),
        );
        env.ledger().with_mut(|l| l.sequence_number += 1);
        client.approve_course(caller, &id);

        let course = client.get_course(&id).expect("course exists");
        assert_eq!(course.status, CourseStatus::Active);
    }
}

// ── #377: an upgrade needs a proposal ───────────────────────────────────────

/// With nothing proposed there is no Wasm hash to install, and the call says so
/// rather than reaching the deployer.
#[test]
#[should_panic(expected = "no pending upgrade")]
fn test_377_upgrade_without_a_proposal_panics() {
    let (env, contract_id, _token_id, admin, sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    assert!(client.get_pending_upgrade().is_none());
    client.upgrade_contract(&admin, &sec_admin);
}

/// A cancelled proposal is gone: cancelling and then upgrading is the same as
/// never having proposed.
#[test]
#[should_panic(expected = "no pending upgrade")]
fn test_377_upgrade_after_cancel_panics() {
    let (env, contract_id, _token_id, admin, sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let hash = BytesN::from_array(&env, &[7u8; 32]);
    client.propose_upgrade(&admin, &sec_admin, &hash);
    assert!(client.get_pending_upgrade().is_some());

    client.cancel_upgrade(&admin, &sec_admin);
    assert!(client.get_pending_upgrade().is_none());

    client.upgrade_contract(&admin, &sec_admin);
}

/// A proposal that exists is still not enough while its time-lock stands — a
/// different refusal, and worth telling apart from the missing-proposal one.
#[test]
#[should_panic(expected = "upgrade time-lock has not elapsed")]
fn test_377_upgrade_before_the_timelock_elapses_panics() {
    let (env, contract_id, _token_id, admin, sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    client.set_upgrade_timelock(&admin, &sec_admin, &1_000u32);
    client.propose_upgrade(
        &admin,
        &sec_admin,
        &BytesN::from_array(&env, &[7u8; 32]),
    );

    client.upgrade_contract(&admin, &sec_admin);
}

/// Proposing records the hash and the ledger it becomes installable at, so an
/// operator can see what is pending before anything is run.
#[test]
fn test_377_a_proposal_records_the_hash_and_its_effective_ledger() {
    let (env, contract_id, _token_id, admin, sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let hash = BytesN::from_array(&env, &[9u8; 32]);
    let timelock = client.get_upgrade_timelock();
    let proposed_at = env.ledger().sequence();

    client.propose_upgrade(&admin, &sec_admin, &hash);

    let pending = client.get_pending_upgrade().expect("a proposal is pending");
    assert_eq!(pending.new_wasm_hash, hash);
    assert_eq!(pending.effective_ledger, proposed_at + timelock);
}
