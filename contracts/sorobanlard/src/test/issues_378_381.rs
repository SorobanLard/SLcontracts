// ============================================================
// RISK FEE CONFIG, INDEXES AND TREASURY ROTATION (#378–#381)
// ============================================================
//
// #378 — `set_risk_fee_config` stores what it was given.
// #379 — `CourseCertificates` lists every certificate issued for a course.
// #380 — `EnrollmentHistory` lists every archived enrollment for a pair.
// #381 — a treasury change reaches the fee split once it is effective.

use super::*;

// ── #378: the risk fee configuration ────────────────────────────────────────

/// Every field is stored and read back as given, not merged with the defaults.
#[test]
fn test_378_set_risk_fee_config_stores_every_field() {
    let (env, contract_id, _token_id, admin, _sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    client.set_risk_fee_config(&admin, &150u32, &5_000_000_000i128, &75u32, &200u32);

    let config = client.get_risk_fee_config();
    assert_eq!(config.large_payment_surcharge_bps, 150);
    assert_eq!(config.large_payment_threshold, 5_000_000_000);
    assert_eq!(config.new_customer_surcharge_bps, 75);
    assert_eq!(config.btc_eth_surcharge_bps, 200);
}

/// Before anything is set, the getter answers with the documented defaults
/// rather than panicking on a missing key.
#[test]
fn test_378_unset_config_reads_as_the_defaults() {
    let (env, contract_id, _token_id, _admin, _sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let config = client.get_risk_fee_config();
    assert_eq!(config.large_payment_surcharge_bps, 0);
    assert_eq!(config.new_customer_surcharge_bps, 0);
    assert_eq!(config.btc_eth_surcharge_bps, 0);
    assert_eq!(config.large_payment_threshold, 1_000_000_000_000);
}

/// A later call replaces the whole configuration.
#[test]
fn test_378_a_second_call_replaces_the_configuration() {
    let (env, contract_id, _token_id, admin, _sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    client.set_risk_fee_config(&admin, &150u32, &5_000_000_000i128, &75u32, &200u32);
    client.set_risk_fee_config(&admin, &0u32, &1i128, &0u32, &0u32);

    let config = client.get_risk_fee_config();
    assert_eq!(config.large_payment_surcharge_bps, 0);
    assert_eq!(config.large_payment_threshold, 1);
    assert_eq!(config.new_customer_surcharge_bps, 0);
    assert_eq!(config.btc_eth_surcharge_bps, 0);
}

/// A negative threshold would make "large payment" meaningless, since every
/// amount is above it.
#[test]
#[should_panic(expected = "large_payment_threshold cannot be negative")]
fn test_378_negative_threshold_is_refused() {
    let (env, contract_id, _token_id, admin, _sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    client.set_risk_fee_config(&admin, &150u32, &-1i128, &75u32, &200u32);
}

/// Fee policy is the admin's; a stranger cannot set it.
#[test]
#[should_panic(expected = "unauthorized: set_risk_fee_config - caller is not admin")]
fn test_378_only_an_admin_may_set_the_config() {
    let (env, contract_id, _token_id, _admin, _sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    client.set_risk_fee_config(
        &Address::generate(&env),
        &150u32,
        &5_000_000_000i128,
        &75u32,
        &200u32,
    );
}

/// Sets up one course and one fresh student, and returns what the platform
/// takes from a single enrollment.
fn platform_take_on_first_enrollment(
    env: &Env,
    client: &SorobanLardContractClient,
    token_id: &Address,
    admin: &Address,
    instructor: &Address,
    treasury: &Address,
    course_id: &str,
) -> i128 {
    let token_client = token::Client::new(env, token_id);
    let student = Address::generate(env);
    token::StellarAssetClient::new(env, token_id).mint(&student, &100_000_000_000);

    register_and_approve_course(env, client, token_id, admin, instructor, course_id, 1_000_000_000);

    let before = token_client.balance(treasury);
    client.enroll(&student, &String::from_str(env, course_id));
    token_client.balance(treasury) - before
}

/// With risk pricing switched on, a new customer's enrollment hands the
/// platform more than the base fee.
#[test]
fn test_378_risk_enabled_applies_the_surcharge_to_the_split() {
    let (env, contract_id, token_id, admin, _sec_admin, treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let base = 1_000_000_000i128 * 20 / 100;

    client.set_risk_fee_config(&admin, &0u32, &1_000_000_000_000i128, &500u32, &0u32);
    client.set_risk_config_enabled(&admin, &true);
    assert!(client.is_risk_config_enabled());

    let taken = platform_take_on_first_enrollment(
        &env,
        &client,
        &token_id,
        &admin,
        &instructor,
        &treasury,
        "COURSE-378-A",
    );
    assert!(
        taken > base,
        "a new-customer surcharge must raise the platform share: took {} against a base of {}",
        taken,
        base
    );
}

/// With risk pricing switched off the same configuration changes nothing.
#[test]
fn test_378_risk_disabled_leaves_the_split_alone() {
    let (env, contract_id, token_id, admin, _sec_admin, treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let base = 1_000_000_000i128 * 20 / 100;

    client.set_risk_fee_config(&admin, &0u32, &1_000_000_000_000i128, &500u32, &0u32);
    client.set_risk_config_enabled(&admin, &false);
    assert!(!client.is_risk_config_enabled());

    let taken = platform_take_on_first_enrollment(
        &env,
        &client,
        &token_id,
        &admin,
        &instructor,
        &treasury,
        "COURSE-378-B",
    );
    assert_eq!(
        taken, base,
        "a configured surcharge must not be charged while risk pricing is off"
    );
}

// ── #379: the per-course certificate index ──────────────────────────────────

/// Issues a certificate to `student` for a course they have completed.
fn complete_and_certify(
    env: &Env,
    client: &SorobanLardContractClient,
    admin: &Address,
    student: &Address,
    course_id: &str,
    certificate_id: &str,
) {
    let id = String::from_str(env, course_id);
    client.mark_completed(admin, student, &id, &None);
    let enrollment_ref = get_enrollment_ref(env, client, student, course_id);
    client.issue_certificate(
        admin,
        student,
        &id,
        &String::from_str(env, certificate_id),
        &String::from_str(env, "Course Title"),
        &enrollment_ref,
        &None,
        &None,
    );
}

/// Every certificate issued for a course is listed, once each, for all of its
/// students — not just the first.
#[test]
fn test_379_course_certificates_lists_every_issued_certificate() {
    let (env, contract_id, token_id, admin, _sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let one = Address::generate(&env);
    let two = Address::generate(&env);
    for student in [&one, &two] {
        token::StellarAssetClient::new(&env, &token_id).mint(student, &100_000_000_000);
    }

    let course_id = String::from_str(&env, "COURSE-379-A");
    register_and_approve_course(
        &env,
        &client,
        &token_id,
        &admin,
        &instructor,
        "COURSE-379-A",
        1_000_000_000,
    );
    client.enroll(&one, &course_id);
    client.enroll(&two, &course_id);

    assert_eq!(client.get_course_certificates(&course_id).len(), 0);

    complete_and_certify(&env, &client, &admin, &one, "COURSE-379-A", "CERT-379-1");
    complete_and_certify(&env, &client, &admin, &two, "COURSE-379-A", "CERT-379-2");

    let certificates = client.get_course_certificates(&course_id);
    assert_eq!(certificates.len(), 2, "both certificates are indexed");
    assert!(certificates.contains(String::from_str(&env, "CERT-379-1")));
    assert!(certificates.contains(String::from_str(&env, "CERT-379-2")));
}

/// The index is per course: one course's certificates never appear under
/// another's.
#[test]
fn test_379_the_index_is_scoped_to_one_course() {
    let (env, contract_id, token_id, admin, _sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let student = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_id).mint(&student, &100_000_000_000);

    for course in ["COURSE-379-B", "COURSE-379-C"] {
        register_and_approve_course(
            &env,
            &client,
            &token_id,
            &admin,
            &instructor,
            course,
            1_000_000_000,
        );
        client.enroll(&student, &String::from_str(&env, course));
    }

    complete_and_certify(&env, &client, &admin, &student, "COURSE-379-B", "CERT-379-B");

    let b = client.get_course_certificates(&String::from_str(&env, "COURSE-379-B"));
    let c = client.get_course_certificates(&String::from_str(&env, "COURSE-379-C"));
    assert_eq!(b.len(), 1);
    assert!(b.contains(String::from_str(&env, "CERT-379-B")));
    assert_eq!(c.len(), 0, "the other course has issued nothing");
}

/// A course nobody has been certified on reads as an empty list, not an error.
#[test]
fn test_379_an_unknown_course_reads_as_empty() {
    let (env, contract_id, _token_id, _admin, _sec_admin, _treasury, _instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let certificates = client.get_course_certificates(&String::from_str(&env, "NO-SUCH-COURSE"));
    assert_eq!(certificates.len(), 0);
}

// ── #380: the per-pair enrollment history ───────────────────────────────────

/// Each completed enrollment is archived before the next one overwrites it, so
/// a student who re-enrolls twice has two entries, oldest first.
#[test]
fn test_380_history_holds_every_archived_enrollment_for_the_pair() {
    let (env, contract_id, token_id, admin, _sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let student = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_id).mint(&student, &100_000_000_000);

    let course_id = String::from_str(&env, "COURSE-380-A");
    register_and_approve_course(
        &env,
        &client,
        &token_id,
        &admin,
        &instructor,
        "COURSE-380-A",
        1_000_000_000,
    );

    client.enroll(&student, &course_id);
    assert_eq!(
        client
            .get_enrollment_history(&student, &student, &course_id)
            .len(),
        0,
        "the live enrollment is not history yet"
    );

    client.mark_completed(&admin, &student, &course_id, &None);
    env.ledger().with_mut(|l| l.sequence_number += 1);
    client.re_enroll(&student, &course_id);

    let history = client.get_enrollment_history(&student, &student, &course_id);
    assert_eq!(history.len(), 1, "the completed enrollment was archived");
    assert!(history.get_unchecked(0).completed);

    client.mark_completed(&admin, &student, &course_id, &None);
    env.ledger().with_mut(|l| l.sequence_number += 1);
    client.re_enroll(&student, &course_id);

    let history = client.get_enrollment_history(&student, &student, &course_id);
    assert_eq!(history.len(), 2, "both earlier enrollments are kept");
    assert!(
        history.get_unchecked(0).enrolled_at_ledger <= history.get_unchecked(1).enrolled_at_ledger,
        "entries are appended, oldest first"
    );
}

/// History is keyed by the pair: another student's entries on the same course,
/// and the same student's entries on another course, stay out of it.
#[test]
fn test_380_history_is_keyed_by_student_and_course() {
    let (env, contract_id, token_id, admin, _sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let one = Address::generate(&env);
    let two = Address::generate(&env);
    for student in [&one, &two] {
        token::StellarAssetClient::new(&env, &token_id).mint(student, &100_000_000_000);
    }

    for course in ["COURSE-380-B", "COURSE-380-C"] {
        register_and_approve_course(
            &env,
            &client,
            &token_id,
            &admin,
            &instructor,
            course,
            1_000_000_000,
        );
    }
    let b = String::from_str(&env, "COURSE-380-B");
    let c = String::from_str(&env, "COURSE-380-C");

    // Only `one` on course B builds any history.
    client.enroll(&one, &b);
    client.enroll(&two, &b);
    client.enroll(&one, &c);
    client.mark_completed(&admin, &one, &b, &None);
    env.ledger().with_mut(|l| l.sequence_number += 1);
    client.re_enroll(&one, &b);

    assert_eq!(client.get_enrollment_history(&one, &one, &b).len(), 1);
    assert_eq!(
        client.get_enrollment_history(&two, &two, &b).len(),
        0,
        "another student on the same course has their own history"
    );
    assert_eq!(
        client.get_enrollment_history(&one, &one, &c).len(),
        0,
        "the same student on another course has their own history"
    );
}

/// A pair with nothing archived reads as an empty list.
#[test]
fn test_380_a_pair_with_no_history_reads_as_empty() {
    let (env, contract_id, token_id, admin, _sec_admin, _treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    let student = Address::generate(&env);
    register_and_approve_course(
        &env,
        &client,
        &token_id,
        &admin,
        &instructor,
        "COURSE-380-D",
        1_000_000_000,
    );

    let history = client.get_enrollment_history(
        &student,
        &student,
        &String::from_str(&env, "COURSE-380-D"),
    );
    assert_eq!(history.len(), 0);
}

// ── #381: a treasury change reaching the split ──────────────────────────────

/// The rotation is announced immediately but only takes effect 100 ledgers
/// later, so an enrollment in between still pays the old treasury and the
/// first one after it pays the new.
#[test]
fn test_381_treasury_change_reaches_the_split_once_effective() {
    let (env, contract_id, token_id, admin, sec_admin, treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);
    let token_client = token::Client::new(&env, &token_id);

    let price: i128 = 1_000_000_000;
    let platform_share = price * 20 / 100;

    let before_rotation = Address::generate(&env);
    let after_rotation = Address::generate(&env);
    for student in [&before_rotation, &after_rotation] {
        token::StellarAssetClient::new(&env, &token_id).mint(student, &100_000_000_000);
    }

    let course_id = String::from_str(&env, "COURSE-381-A");
    register_and_approve_course(
        &env,
        &client,
        &token_id,
        &admin,
        &instructor,
        "COURSE-381-A",
        price,
    );

    let new_treasury = Address::generate(&env);
    client.update_treasury(&admin, &sec_admin, &new_treasury);

    // Still inside the 100-ledger delay: the old treasury is paid.
    client.enroll(&before_rotation, &course_id);
    assert_eq!(token_client.balance(&treasury), platform_share);
    assert_eq!(
        token_client.balance(&new_treasury),
        0,
        "the rotation is not effective yet"
    );

    env.ledger().with_mut(|l| l.sequence_number += 100);

    // The next enrollment applies the pending change and pays the new treasury.
    client.enroll(&after_rotation, &course_id);
    assert_eq!(
        token_client.balance(&new_treasury),
        platform_share,
        "the next split goes to the new treasury"
    );
    assert_eq!(
        token_client.balance(&treasury),
        platform_share,
        "the old treasury keeps what it already had and takes no more"
    );
}
