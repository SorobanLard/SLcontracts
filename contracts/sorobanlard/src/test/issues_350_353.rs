// ============================================================
// ADMIN TRANSFER, GOVERNANCE WINDOW AND ARCHIVAL (#350–#353)
// ============================================================
//
// #350 — `accept_admin` may only be completed by the nominated pair.
// #351 — `update_min_review_delay` changes the gate for approvals in flight.
// #352 — a course only archives once nothing is enrolled in it.
// #353 — `students_to_refund` pays each student back exactly what they paid.

use super::*;

// ── #350: the two-step admin transfer ────────────────────────────────────────

/// The nominated pair, and only the nominated pair, completes the transfer.
#[test]
fn test_350_nominated_admins_can_accept() {
    let (env, contract_id, _token_id, admin, sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let new_admin = Address::generate(&env);
    let new_sec = Address::generate(&env);

    client.transfer_admin(&admin, &sec_admin, &new_admin, &new_sec);
    client.accept_admin(&new_admin, &new_sec);

    // The new admin can perform an admin-only write; the old one cannot.
    client.update_default_fee(&new_admin, &30u32);
    assert_eq!(client.get_platform_fee(&new_admin), 30);
}

/// A third party cannot step into a transfer nominated for someone else, even
/// with its own authorisation in hand.
#[test]
#[should_panic(expected = "callers are not the pending admins")]
fn test_350_non_nominated_address_cannot_accept() {
    let (env, contract_id, _token_id, admin, sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let new_admin = Address::generate(&env);
    let new_sec = Address::generate(&env);
    let stranger = Address::generate(&env);

    client.transfer_admin(&admin, &sec_admin, &new_admin, &new_sec);
    client.accept_admin(&stranger, &new_sec);
}

/// Half of the nominated pair is not enough: both addresses must match.
#[test]
#[should_panic(expected = "callers are not the pending admins")]
fn test_350_secondary_admin_must_match_too() {
    let (env, contract_id, _token_id, admin, sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let new_admin = Address::generate(&env);
    let new_sec = Address::generate(&env);
    let stranger = Address::generate(&env);

    client.transfer_admin(&admin, &sec_admin, &new_admin, &new_sec);
    client.accept_admin(&new_admin, &stranger);
}

/// There is nothing to accept before a transfer has been proposed.
#[test]
#[should_panic(expected = "no pending admin")]
fn test_350_accept_before_transfer_panics() {
    let (env, contract_id, _token_id, _admin, _sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    client.accept_admin(&Address::generate(&env), &Address::generate(&env));
}

/// Accepting twice is refused: the nomination is cleared by the first accept,
/// so a replayed call finds no pending transfer.
#[test]
#[should_panic(expected = "no pending admin")]
fn test_350_accept_cannot_be_replayed() {
    let (env, contract_id, _token_id, admin, sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let new_admin = Address::generate(&env);
    let new_sec = Address::generate(&env);

    client.transfer_admin(&admin, &sec_admin, &new_admin, &new_sec);
    client.accept_admin(&new_admin, &new_sec);
    client.accept_admin(&new_admin, &new_sec);
}

// ── #351: the review delay, changed mid-flight ───────────────────────────────

/// Registers a course without approving it, far enough past init that the
/// governance window (which is measured from `InitLedger`) is not what blocks
/// the approval — the course's own review period is.
fn register_only(
    env: &Env,
    client: &SorobanLardContractClient,
    token_id: &Address,
    instructor: &Address,
    course_id: &str,
    price: i128,
) {
    client.register_course(
        instructor,
        &String::from_str(env, course_id),
        &price,
        token_id,
        &0u32,
        &None,
        &BytesN::from_array(env, &[1u8; 32]),
        &soroban_sdk::Vec::new(env),
    );
}

/// Raising the delay blocks a course that was approvable a moment earlier.
#[test]
#[should_panic(expected = "course review period has not elapsed")]
fn test_351_raising_the_delay_blocks_an_approvable_course() {
    let (env, contract_id, token_id, admin, _sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    // Put the governance window (measured from init) well behind us.
    env.ledger().with_mut(|l| l.sequence_number += 1_000);

    register_only(&env, &client, &token_id, &instructor, "COURSE-351-A", 1_000_000_000);
    env.ledger().with_mut(|l| l.sequence_number += 5);

    // With no delay this course would approve now. Raise the bar instead.
    client.update_min_review_delay(&admin, &50u32);
    client.approve_course(&admin, &String::from_str(&env, "COURSE-351-A"));
}

/// Lowering the delay lets a blocked approval through without waiting.
#[test]
fn test_351_lowering_the_delay_releases_a_blocked_course() {
    let (env, contract_id, token_id, admin, _sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    env.ledger().with_mut(|l| l.sequence_number += 1_000);
    client.update_min_review_delay(&admin, &50u32);
    assert_eq!(client.get_min_review_delay(), 50);

    register_only(&env, &client, &token_id, &instructor, "COURSE-351-B", 1_000_000_000);
    env.ledger().with_mut(|l| l.sequence_number += 5);

    // Blocked at a delay of 50, five ledgers after registration.
    assert!(client
        .try_approve_course(&admin, &String::from_str(&env, "COURSE-351-B"))
        .is_err());

    // The same course, same ledger, once the delay is lowered.
    client.update_min_review_delay(&admin, &0u32);
    client.approve_course(&admin, &String::from_str(&env, "COURSE-351-B"));

    let course = client
        .get_course(&String::from_str(&env, "COURSE-351-B"))
        .expect("course exists");
    assert_eq!(course.status, CourseStatus::Active);
}

/// Waiting out the delay works as well as lowering it — the gate is elapsed
/// ledgers, not a flag set at registration.
#[test]
fn test_351_waiting_out_the_delay_also_releases_the_course() {
    let (env, contract_id, token_id, admin, _sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    env.ledger().with_mut(|l| l.sequence_number += 1_000);
    client.update_min_review_delay(&admin, &50u32);

    register_only(&env, &client, &token_id, &instructor, "COURSE-351-C", 1_000_000_000);
    env.ledger().with_mut(|l| l.sequence_number += 50);

    client.approve_course(&admin, &String::from_str(&env, "COURSE-351-C"));
    let course = client
        .get_course(&String::from_str(&env, "COURSE-351-C"))
        .expect("course exists");
    assert_eq!(course.status, CourseStatus::Active);
}

// ── #352: archival requires an empty course ──────────────────────────────────

/// A course with a student still enrolled does not archive.
///
/// Note on the issue's wording: `archive_course` does not panic here. It
/// persists the course with its (unchanged) enrollment count and returns, so a
/// follow-up refund batch can finish the job. The observable guarantee is that
/// the course is still `Paused` and has not moved to `Archived`.
#[test]
fn test_352_active_enrollment_blocks_archival() {
    let (env, contract_id, token_id, admin, sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let student = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_id).mint(&student, &100_000_000_000);

    let course_id = String::from_str(&env, "COURSE-352-A");
    register_and_approve_course(
        &env,
        &client,
        &token_id,
        &admin,
        &instructor,
        "COURSE-352-A",
        1_000_000_000,
    );
    client.enroll(&student, &course_id);
    client.pause_course(&admin, &course_id);

    client.archive_course(&admin, &sec_admin, &course_id, &None);

    let course = client.get_course(&course_id).expect("course still exists");
    assert_eq!(
        course.status,
        CourseStatus::Paused,
        "a course with an active enrollment must not archive"
    );
    assert_eq!(course.active_enrollments, 1);
}

/// Refunding the last enrollment in the same call clears the list and archives
/// the course atomically.
#[test]
fn test_352_refunding_the_last_enrollment_archives_the_course() {
    let (env, contract_id, token_id, admin, sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let student = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_id).mint(&student, &100_000_000_000);

    let course_id = String::from_str(&env, "COURSE-352-B");
    register_and_approve_course(
        &env,
        &client,
        &token_id,
        &admin,
        &instructor,
        "COURSE-352-B",
        1_000_000_000,
    );
    client.enroll(&student, &course_id);
    client.pause_course(&admin, &course_id);

    let students = soroban_sdk::Vec::from_array(&env, [student.clone()]);
    client.archive_course(&admin, &sec_admin, &course_id, &Some(students));

    let course = client.get_course(&course_id).expect("archived record exists");
    assert_eq!(course.status, CourseStatus::Archived);
    assert_eq!(course.active_enrollments, 0);
    assert!(course.archived_at_ledger.is_some());
    assert!(
        !client.is_enrolled(&student, &course_id),
        "the enrollment record is removed, not just marked"
    );
    assert_eq!(client.get_total_active_enrollments(), 0);
}

/// A course that was never paused cannot be archived, whatever its enrollments.
#[test]
#[should_panic(expected = "course must be paused before archiving")]
fn test_352_an_active_course_cannot_be_archived() {
    let (env, contract_id, token_id, admin, sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    register_and_approve_course(
        &env,
        &client,
        &token_id,
        &admin,
        &instructor,
        "COURSE-352-C",
        1_000_000_000,
    );
    client.archive_course(
        &admin,
        &sec_admin,
        &String::from_str(&env, "COURSE-352-C"),
        &None,
    );
}

// ── #353: what each refunded student receives ────────────────────────────────

/// Every student in `students_to_refund` gets back exactly `amount_paid`, made
/// up of the platform's share from the treasury and the instructor's share from
/// the contract.
#[test]
fn test_353_each_student_is_refunded_exactly_what_they_paid() {
    let (env, contract_id, token_id, admin, sec_admin, treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);
    let token_client = token::Client::new(&env, &token_id);

    let price: i128 = 1_000_000_000;
    let platform_share = price * 20 / 100;

    let one = Address::generate(&env);
    let two = Address::generate(&env);
    for student in [&one, &two] {
        token::StellarAssetClient::new(&env, &token_id).mint(student, &100_000_000_000);
    }

    let course_id = String::from_str(&env, "COURSE-353-A");
    register_and_approve_course(
        &env,
        &client,
        &token_id,
        &admin,
        &instructor,
        "COURSE-353-A",
        price,
    );

    client.enroll(&one, &course_id);
    client.enroll(&two, &course_id);

    let one_before = token_client.balance(&one);
    let two_before = token_client.balance(&two);
    assert_eq!(token_client.balance(&treasury), platform_share * 2);

    client.pause_course(&admin, &course_id);
    let students = soroban_sdk::Vec::from_array(&env, [one.clone(), two.clone()]);
    client.archive_course(&admin, &sec_admin, &course_id, &Some(students));

    assert_eq!(
        token_client.balance(&one) - one_before,
        price,
        "the first student must be made whole"
    );
    assert_eq!(
        token_client.balance(&two) - two_before,
        price,
        "the second student must be made whole"
    );
    assert_eq!(
        token_client.balance(&treasury),
        0,
        "the treasury gives back exactly the platform share it took"
    );
}

/// The refund takes the instructor's share back out of their earnings, so an
/// archival cannot leave them paid for an enrollment that was reversed.
#[test]
fn test_353_refund_debits_instructor_earnings() {
    let (env, contract_id, token_id, admin, sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let price: i128 = 1_000_000_000;
    let instructor_share = price - price * 20 / 100;

    let student = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_id).mint(&student, &100_000_000_000);

    let course_id = String::from_str(&env, "COURSE-353-B");
    register_and_approve_course(
        &env,
        &client,
        &token_id,
        &admin,
        &instructor,
        "COURSE-353-B",
        price,
    );
    client.enroll(&student, &course_id);
    assert_eq!(
        client.get_instructor_earnings(&instructor, &token_id),
        instructor_share
    );

    client.pause_course(&admin, &course_id);
    let students = soroban_sdk::Vec::from_array(&env, [student.clone()]);
    client.archive_course(&admin, &sec_admin, &course_id, &Some(students));

    assert_eq!(
        client.get_instructor_earnings(&instructor, &token_id),
        0,
        "the refunded share must come back out of the instructor's earnings"
    );
}

/// A student who has already completed the course is not refunded by an
/// archival, and their enrollment is not cleared by it.
#[test]
fn test_353_completed_enrollment_is_not_refunded_by_archival() {
    let (env, contract_id, token_id, admin, sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);
    let token_client = token::Client::new(&env, &token_id);

    let price: i128 = 1_000_000_000;
    let student = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_id).mint(&student, &100_000_000_000);

    let course_id = String::from_str(&env, "COURSE-353-C");
    register_and_approve_course(
        &env,
        &client,
        &token_id,
        &admin,
        &instructor,
        "COURSE-353-C",
        price,
    );
    client.enroll(&student, &course_id);
    client.mark_completed(&admin, &student, &course_id, &None);

    let before = token_client.balance(&student);
    client.pause_course(&admin, &course_id);
    let students = soroban_sdk::Vec::from_array(&env, [student.clone()]);
    client.archive_course(&admin, &sec_admin, &course_id, &Some(students));

    assert_eq!(
        token_client.balance(&student),
        before,
        "a completed enrollment is not a refundable one"
    );
}

/// The refund list is bounded, so one call can never be asked to make an
/// unbounded number of transfers.
#[test]
#[should_panic(expected = "exceeds maximum allowed")]
fn test_353_refund_list_is_capped() {
    let (env, contract_id, token_id, admin, sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let course_id = String::from_str(&env, "COURSE-353-D");
    register_and_approve_course(
        &env,
        &client,
        &token_id,
        &admin,
        &instructor,
        "COURSE-353-D",
        1_000_000_000,
    );
    client.pause_course(&admin, &course_id);

    let mut students = soroban_sdk::Vec::new(&env);
    for _ in 0..(SorobanLardContract::MAX_STUDENTS_TO_REFUND + 1) {
        students.push_back(Address::generate(&env));
    }
    client.archive_course(&admin, &sec_admin, &course_id, &Some(students));
}
