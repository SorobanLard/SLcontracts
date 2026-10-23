// ============================================================
// PROPERTY-BASED TESTS — ACCOUNTING ARITHMETIC (#132)
// ============================================================
//
// Randomised checks for the arithmetic-heavy accounting paths: the
// payment split in `deduct_fee()` / `enroll()`, `withdraw_earnings()`, and
// `process_refund()`. Each case asserts invariants that must hold for every
// input rather than for a single hand-picked example:
//
//   - the split is exact: `platform + instructor == amount`
//   - neither side of the split is ever negative or exceeds the amount
//   - no balance or earnings counter ever goes negative
//   - overflow panics loudly instead of wrapping silently
//   - an approved refund restores every party's balance exactly
//
// Each case spins up a fresh `Env`, so the case count is kept modest. Raise
// it locally with `PROPTEST_CASES=<n> cargo test proptests`.

use super::*;
use proptest::prelude::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Default number of cases per property when `PROPTEST_CASES` is unset.
const DEFAULT_CASES: u32 = 64;
const MIN_PRICE: i128 = 100_000;
const MAX_PRICE: i128 = 1_000_000_000_000;

/// Honours `PROPTEST_CASES` (used by CI to run more cases), falling back to
/// `DEFAULT_CASES` locally.
fn config() -> ProptestConfig {
    let cases = std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_CASES);
    ProptestConfig::with_cases(cases)
}

/// A course price accepted by `register_course`: free, or within the
/// configured stroop precision range.
fn course_price() -> impl Strategy<Value = i128> {
    prop_oneof![
        1 => Just(0i128),
        1 => Just(MIN_PRICE),
        1 => Just(MAX_PRICE),
        7 => MIN_PRICE..=MAX_PRICE,
    ]
}

/// A per-token fee in basis points, biased towards the 0% / 100% boundaries.
fn fee_bps() -> impl Strategy<Value = u32> {
    prop_oneof![
        1 => Just(0u32),
        1 => Just(10_000u32),
        6 => 0u32..=10_000,
    ]
}

/// Registers, approves and pays for one enrollment in a fresh environment,
/// returning everything later invariants need.
struct Enrolled {
    env: Env,
    contract_id: Address,
    token_id: Address,
    admin: Address,
    treasury: Address,
    instructor: Address,
    student: Address,
    course_id: String,
    student_start: i128,
}

fn enroll_with(price: i128, token_fee_bps: u32, course_fee_pct: u32) -> Enrolled {
    let (env, contract_id, token_id, admin, _sec_admin, treasury, instructor) = setup();
    let client = SorobanLardContractClient::new(&env, &contract_id);

    client.set_fee_config(&admin, &token_id, &token_fee_bps);

    let course_fee_pct = if course_fee_pct == 0 {
        0
    } else {
        course_fee_pct.max((token_fee_bps + 99) / 100)
    };
    let course_id = String::from_str(&env, "PROP-COURSE");
    client.register_course(
        &instructor,
        &course_id,
        &price,
        &token_id,
        &course_fee_pct,
        &None,
        &BytesN::from_array(&env, &[1u8; 32]),
        &soroban_sdk::Vec::new(&env),
    );
    env.ledger().with_mut(|l| {
        l.sequence_number += 1;
    });
    client.approve_course(&admin, &course_id);

    let student = Address::generate(&env);
    if price > 0 {
        token::StellarAssetClient::new(&env, &token_id).mint(&student, &price);
    }
    let student_start = token::Client::new(&env, &token_id).balance(&student);

    client.enroll(&student, &course_id);

    Enrolled {
        env,
        contract_id,
        token_id,
        admin,
        treasury,
        instructor,
        student,
        course_id,
        student_start,
    }
}

proptest! {
    #![proptest_config(config())]

    /// `deduct_fee()` splits any non-overflowing amount exactly, never
    /// produces a negative share, and honours the course fee override as a
    /// floor on the live per-token rate.
    #[test]
    fn prop_deduct_fee_split_is_exact(
        amount in prop_oneof![
            1 => Just(0i128),
            1 => Just(i128::MAX / 10_000),
            1 => i128::MIN..0i128,
            7 => 0i128..=(i128::MAX / 10_000),
        ],
        bps in fee_bps(),
        override_bps in proptest::option::of(0u32..=10_000),
    ) {
        let (env, contract_id, token_id, admin, _sec_admin, _treasury, _instructor) = setup();
        let client = SorobanLardContractClient::new(&env, &contract_id);
        client.set_fee_config(&admin, &token_id, &bps);

        let (net, fee) = env.as_contract(&contract_id, || {
            SorobanLardContract::deduct_fee(&env, &token_id, amount, override_bps, false, false)
        });

        if amount <= 0 {
            prop_assert_eq!((net, fee), (0, 0));
        } else {
            let effective_bps = override_bps.map_or(bps, |o| o.max(bps)) as i128;
            prop_assert_eq!(fee, amount * effective_bps / 10_000);
            prop_assert_eq!(net + fee, amount);
            prop_assert!(fee >= 0 && fee <= amount);
            prop_assert!(net >= 0 && net <= amount);
        }
    }

    /// When `amount * fee_bps` cannot be represented in an i128 the fee
    /// computation must panic rather than silently wrap.
    #[test]
    fn prop_deduct_fee_overflow_panics(
        (bps, amount) in (2u32..=10_000).prop_flat_map(|bps| {
            (Just(bps), (i128::MAX / bps as i128 + 1)..=i128::MAX)
        }),
    ) {
        let (env, contract_id, token_id, admin, _sec_admin, _treasury, _instructor) = setup();
        let client = SorobanLardContractClient::new(&env, &contract_id);
        client.set_fee_config(&admin, &token_id, &bps);

        let result = catch_unwind(AssertUnwindSafe(|| {
            env.as_contract(&contract_id, || {
                SorobanLardContract::deduct_fee(&env, &token_id, amount, None, false, false)
            })
        }));
        prop_assert!(result.is_err(), "fee overflow must panic, not wrap");
    }

    /// End-to-end `enroll()`: the amounts recorded on the enrollment, the
    /// treasury balance, the contract balance and the instructor earnings
    /// all agree and sum to the course price.
    #[test]
    fn prop_enroll_payment_split_conserves_value(
        price in course_price(),
        token_fee in fee_bps(),
        course_fee_pct in 0u32..=100,
    ) {
        let e = enroll_with(price, token_fee, course_fee_pct);
        let client = SorobanLardContractClient::new(&e.env, &e.contract_id);
        let token_client = token::Client::new(&e.env, &e.token_id);

        let enrollment = client
            .get_enrollment(&e.student, &e.student, &e.course_id)
            .unwrap();
        let platform = enrollment.platform_amount;
        let instructor = enrollment.instructor_amount;

        prop_assert_eq!(enrollment.amount_paid, price);
        prop_assert_eq!(platform + instructor, price);
        prop_assert!(platform >= 0 && instructor >= 0);

        prop_assert_eq!(token_client.balance(&e.student), e.student_start - price);
        prop_assert_eq!(token_client.balance(&e.treasury), platform);
        prop_assert_eq!(token_client.balance(&e.contract_id), instructor);
        prop_assert_eq!(
            client.get_instructor_earnings(&e.instructor, &e.token_id),
            instructor
        );
        prop_assert_eq!(client.get_instructor_total_earnings(&e.instructor), instructor);
    }

    /// `withdraw_earnings()` moves exactly the requested amount (or the full
    /// balance for `amount = 0`), never leaves a negative balance, and
    /// rejects over-withdrawals and negative amounts.
    #[test]
    fn prop_withdraw_earnings_conserves_value(
        price in course_price(),
        token_fee in fee_bps(),
        withdraw_permille in 0u32..=1_000,
        over_by in 1i128..=i128::MAX / 2,
        negative in i128::MIN..0i128,
    ) {
        let e = enroll_with(price, token_fee, 0);
        let client = SorobanLardContractClient::new(&e.env, &e.contract_id);
        let token_client = token::Client::new(&e.env, &e.token_id);

        let earned = client.get_instructor_earnings(&e.instructor, &e.token_id);
        prop_assert!(earned >= 0);

        let requested = earned * withdraw_permille as i128 / 1_000;
        client.withdraw_earnings(&e.instructor, &e.token_id, &requested);
        let withdrawn = if requested == 0 { earned } else { requested };

        let remaining = client.get_instructor_earnings(&e.instructor, &e.token_id);
        prop_assert!(remaining >= 0);
        prop_assert_eq!(remaining, earned - withdrawn);
        prop_assert_eq!(token_client.balance(&e.instructor), withdrawn);
        prop_assert_eq!(token_client.balance(&e.contract_id), remaining);

        // Withdrawing more than the remaining balance must fail.
        let over = remaining.checked_add(over_by).unwrap_or(i128::MAX);
        prop_assert!(client
            .try_withdraw_earnings(&e.instructor, &e.token_id, &over)
            .is_err());

        // Negative withdrawals are always rejected.
        prop_assert!(client
            .try_withdraw_earnings(&e.instructor, &e.token_id, &negative)
            .is_err());

        prop_assert_eq!(
            client.get_instructor_earnings(&e.instructor, &e.token_id),
            remaining
        );
    }

    /// An approved refund returns exactly what was paid: the student is made
    /// whole, the treasury and contract return to zero, and no instructor
    /// earnings remain.
    #[test]
    fn prop_refund_restores_all_balances(
        price in course_price(),
        token_fee in fee_bps(),
        course_fee_pct in 0u32..=100,
    ) {
        let e = enroll_with(price, token_fee, course_fee_pct);
        let client = SorobanLardContractClient::new(&e.env, &e.contract_id);
        let token_client = token::Client::new(&e.env, &e.token_id);

        client.request_refund(&e.student, &e.course_id);
        e.env.mock_all_auths_allowing_non_root_auth();
        client.process_refund(&e.admin, &e.student, &e.course_id, &true);

        prop_assert_eq!(token_client.balance(&e.student), e.student_start);
        prop_assert_eq!(token_client.balance(&e.treasury), 0);
        prop_assert_eq!(token_client.balance(&e.contract_id), 0);
        prop_assert_eq!(client.get_instructor_earnings(&e.instructor, &e.token_id), 0);
        prop_assert_eq!(client.get_instructor_total_earnings(&e.instructor), 0);
        prop_assert!(client
            .get_enrollment(&e.student, &e.student, &e.course_id)
            .is_none());
    }
}
