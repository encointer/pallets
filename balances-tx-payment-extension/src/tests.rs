use encointer_primitives::CommunityIdentifier;

#[test]
fn name_symbol_and_decimals_work() {
	new_test_ext().execute_with(|| {
		let cid = CommunityIdentifier::default();
		assert_eq!(EncointerBalances::name(&cid), "Encointer".as_bytes().to_vec());
		assert_eq!(EncointerBalances::symbol(&cid), "ETR".as_bytes().to_vec());
		assert_eq!(EncointerBalances::decimals(&cid), 18);
	})
}

fn almost_eq(a: u128, b: u128, delta: u128) -> bool {
	let diff = if a > b { a - b } else { b - a };
	return diff < delta
}
#[test]
fn balance_type_to_fungible_balance_works() {
	new_test_ext().execute_with(|| {
		let cid = CommunityIdentifier::default();
		// a delta of 10000 corresponds to 10000 * 10 ^ -18 = 10 ^ -14
		assert!(almost_eq(
			EncointerBalances::balance_type_to_fungible_balance(cid, BalanceType::from_num(1f64)),
			1_000_000_000_000_000_000u128,
			10000
		));

		assert!(almost_eq(
			EncointerBalances::balance_type_to_fungible_balance(cid, BalanceType::from_num(0.1f64)),
			0_100_000_000_000_000_000u128,
			10000
		));

		assert!(almost_eq(
			EncointerBalances::balance_type_to_fungible_balance(
				cid,
				BalanceType::from_num(123.456f64)
			),
			123_456_000_000_000_000_000u128,
			10000
		));
	})
}

#[test]
fn fungible_balance_to_balance_type_works() {
	new_test_ext().execute_with(|| {
		let cid = CommunityIdentifier::default();

		assert_eq!(
			EncointerBalances::fungible_balance_to_balance_type(cid, 0_000_000_100_000_000_000u128),
			BalanceType::from_num(0.0000001f64)
		);

		assert_eq!(
			EncointerBalances::fungible_balance_to_balance_type(cid, 1_000_000_000_000_000_000u128),
			BalanceType::from_num(1f64)
		);

		assert_eq!(
			EncointerBalances::fungible_balance_to_balance_type(cid, 0_100_000_000_000_000_000u128),
			BalanceType::from_num(0.1f64)
		);
		let balance: f64 = EncointerBalances::fungible_balance_to_balance_type(
			cid,
			123_456_000_000_000_000_000u128,
		)
		.lossy_into();
		assert_relative_eq!(balance, 123.456f64, epsilon = 1.0e-14);
	})
}

#[test]
fn total_issuance_and_balance_works() {
	new_test_ext().execute_with(|| {
		let cid = CommunityIdentifier::default();
		let alice = AccountKeyring::Alice.to_account_id();
		assert_ok!(EncointerBalances::issue(cid, &alice, BalanceType::from_num(50.1)));
		assert!(almost_eq(
			<EncointerBalances as Inspect<<TestRuntime as frame_system::Config>::AccountId>>::balance(
				cid, &alice
			),
			50_100_000_000_000_000_000u128,
			10000
		));

		assert!(almost_eq(
			<EncointerBalances as Inspect<<TestRuntime as frame_system::Config>::AccountId>>::reducible_balance(
				cid, &alice, false
			),
			50_100_000_000_000_000_000u128,
			10000
		));

		assert!(almost_eq(
			<EncointerBalances as Inspect<<TestRuntime as frame_system::Config>::AccountId>>::total_issuance(
				cid
			),
			50_100_000_000_000_000_000u128,
			10000
		));
	})
}

#[test]
fn minimum_balance_works() {
	new_test_ext().execute_with(|| {
		let cid = CommunityIdentifier::default();
		assert_eq!(EncointerBalances::minimum_balance(cid), 0);
	})
}

#[test]
fn can_deposit_works() {
	new_test_ext().execute_with(|| {
		let cid = CommunityIdentifier::default();
		let wrong_cid = CommunityIdentifier::from_str("aaaaaaaaaa").unwrap();
		let alice = AccountKeyring::Alice.to_account_id();
		let bob = AccountKeyring::Bob.to_account_id();
		let ferdie = AccountKeyring::Ferdie.to_account_id();
		assert_ok!(EncointerBalances::issue(cid, &alice, BalanceType::from_num(50)));

		assert!(
			EncointerBalances::can_deposit(wrong_cid, &alice, 10) ==
				DepositConsequence::UnknownAsset
		);

		assert_ok!(EncointerBalances::issue(
			cid,
			&alice,
			BalanceType::from_num(4.5 * 10f64.powf(18f64))
		));
		assert_ok!(EncointerBalances::issue(
			cid,
			&bob,
			BalanceType::from_num(4.5 * 10f64.powf(18f64))
		));

		assert!(
			EncointerBalances::can_deposit(
				cid,
				&ferdie,
				EncointerBalances::balance_type_to_fungible_balance(
					cid,
					BalanceType::from_num(4.5 * 10f64.powf(18f64))
				)
			) == DepositConsequence::Overflow
		);

		// in the very weird case where some some balances are negative we need to test for overflow of
		// and account balance, because now an account can overflow but the total issuance does not.
		assert_ok!(EncointerBalances::burn(
			cid,
			&bob,
			BalanceType::from_num(4.5 * 10f64.powf(18f64))
		));

		assert_ok!(EncointerBalances::issue(
			cid,
			&bob,
			BalanceType::from_num(-4.5 * 10f64.powf(18f64))
		));

		assert_ok!(EncointerBalances::issue(
			cid,
			&alice,
			BalanceType::from_num(4.5 * 10f64.powf(18f64))
		));

		assert!(
			EncointerBalances::can_deposit(
				cid,
				&alice,
				EncointerBalances::balance_type_to_fungible_balance(
					cid,
					BalanceType::from_num(4.5 * 10f64.powf(18f64))
				)
			) == DepositConsequence::Overflow
		);

		assert!(
			EncointerBalances::can_deposit(
				cid,
				&alice,
				EncointerBalances::balance_type_to_fungible_balance(cid, BalanceType::from_num(1))
			) == DepositConsequence::Success
		);
	})
}

#[test]
fn can_withdraw_works() {
	new_test_ext().execute_with(|| {
		let cid = CommunityIdentifier::default();
		let wrong_cid = CommunityIdentifier::from_str("aaaaaaaaaa").unwrap();
		let alice = AccountKeyring::Alice.to_account_id();
		let bob = AccountKeyring::Bob.to_account_id();
		let ferdie = AccountKeyring::Ferdie.to_account_id();
		assert_ok!(EncointerBalances::issue(cid, &alice, BalanceType::from_num(10)));
		assert_ok!(EncointerBalances::issue(cid, &bob, BalanceType::from_num(1)));

		assert!(
			EncointerBalances::can_withdraw(wrong_cid, &alice, 10) ==
				WithdrawConsequence::UnknownAsset
		);

		assert!(
			EncointerBalances::can_withdraw(
				cid,
				&bob,
				EncointerBalances::balance_type_to_fungible_balance(cid, BalanceType::from_num(12))
			) == WithdrawConsequence::Underflow
		);

		assert!(
			EncointerBalances::can_withdraw(
				cid,
				&bob,
				EncointerBalances::balance_type_to_fungible_balance(cid, BalanceType::from_num(0))
			) == WithdrawConsequence::Success
		);

		assert!(
			EncointerBalances::can_withdraw(
				cid,
				&bob,
				EncointerBalances::balance_type_to_fungible_balance(cid, BalanceType::from_num(2))
			) == WithdrawConsequence::NoFunds
		);

		assert!(
			EncointerBalances::can_withdraw(
				cid,
				&bob,
				EncointerBalances::balance_type_to_fungible_balance(cid, BalanceType::from_num(1))
			) == WithdrawConsequence::Success
		);
	})
}

#[test]
fn set_balance_and_set_total_issuance_works() {
	new_test_ext().execute_with(|| {
		let cid = CommunityIdentifier::default();
		let alice = AccountKeyring::Alice.to_account_id();
		assert_ok!(EncointerBalances::issue(cid, &alice, BalanceType::from_num(10)));

		assert!(almost_eq(
			<EncointerBalances as Inspect<<TestRuntime as frame_system::Config>::AccountId>>::balance(
				cid, &alice
			),
			10_000_000_000_000_000_000u128,
			10000
		));

		assert_ok!(EncointerBalances::set_balance(cid, &alice, 20_000_000_000_000_000_000u128));

		assert!(almost_eq(
			<EncointerBalances as Inspect<<TestRuntime as frame_system::Config>::AccountId>>::balance(
				cid, &alice
			),
			20_000_000_000_000_000_000u128,
			10000
		));

		assert!(almost_eq(
			<EncointerBalances as Inspect<<TestRuntime as frame_system::Config>::AccountId>>::total_issuance(
				cid
			),
			10_000_000_000_000_000_000u128,
			10000
		));

		EncointerBalances::set_total_issuance(cid, 30_000_000_000_000_000_000u128);

		assert!(almost_eq(
			<EncointerBalances as Inspect<<TestRuntime as frame_system::Config>::AccountId>>::total_issuance(
				cid
			),
			30_000_000_000_000_000_000u128,
			10000
		));
	})
}
