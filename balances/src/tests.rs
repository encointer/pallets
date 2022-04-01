// Copyright (c) 2019 Alain Brenzikofer
// This file is part of Encointer
//
// Encointer is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Encointer is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Encointer.  If not, see <http://www.gnu.org/licenses/>.

//! Unit tests for the encointer_balances module.

use super::*;
use crate::mock::DefaultDemurrage;
use approx::{assert_abs_diff_eq, assert_relative_eq};
use encointer_primitives::{
	communities::CommunityIdentifier,
	fixed::{traits::LossyInto, transcendental::exp},
};
use frame_support::{
	assert_noop, assert_ok,
	traits::{
		tokens::{
			fungibles::{Inspect, InspectMetadata, Unbalanced},
			DepositConsequence, WithdrawConsequence,
		},
		OnInitialize,
	},
};
use mock::{master, new_test_ext, EncointerBalances, Origin, System, TestRuntime};
use sp_runtime::DispatchError;
use sp_std::str::FromStr;
use test_utils::{
	helpers::{assert_dispatch_err, last_event},
	AccountKeyring,
};

#[test]
fn issue_should_work() {
	new_test_ext().execute_with(|| {
		let cid = CommunityIdentifier::default();
		let alice = AccountKeyring::Alice.to_account_id();
		assert_ok!(EncointerBalances::issue(cid, &alice, BalanceType::from_num(50.1)));
		assert_eq!(EncointerBalances::balance(cid, &alice), BalanceType::from_num(50.1));
		assert_eq!(EncointerBalances::total_issuance(cid), BalanceType::from_num(50.1));
	});
}

#[test]
fn burn_should_work() {
	new_test_ext().execute_with(|| {
		let cid = CommunityIdentifier::default();
		let alice = AccountKeyring::Alice.to_account_id();
		assert_ok!(EncointerBalances::issue(cid, &alice, BalanceType::from_num(50)));
		assert_ok!(EncointerBalances::burn(cid, &alice, BalanceType::from_num(20)));
		assert_eq!(EncointerBalances::balance(cid, &alice), BalanceType::from_num(30));
		assert_eq!(EncointerBalances::total_issuance(cid), BalanceType::from_num(30));
		assert_noop!(
			EncointerBalances::burn(cid, &alice, BalanceType::from_num(31)),
			Error::<TestRuntime>::BalanceTooLow,
		);
	});
}

#[test]
fn transfer_should_work() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1);
		System::on_initialize(System::block_number());

		let alice = AccountKeyring::Alice.to_account_id();
		let bob = AccountKeyring::Bob.to_account_id();
		let cid = CommunityIdentifier::default();
		assert_ok!(EncointerBalances::issue(cid, &alice, BalanceType::from_num(50)));
		assert_ok!(EncointerBalances::transfer(
			Some(alice.clone()).into(),
			bob.clone(),
			cid,
			BalanceType::from_num(9.999)
		));

		let balance: f64 = EncointerBalances::balance(cid, &alice).lossy_into();
		assert_relative_eq!(balance, 40.001, epsilon = 1.0e-9);

		let balance: f64 = EncointerBalances::balance(cid, &bob).lossy_into();
		assert_relative_eq!(balance, 9.999, epsilon = 1.0e-9);

		let balance: f64 = EncointerBalances::total_issuance(cid).lossy_into();
		assert_relative_eq!(balance, 50.0, epsilon = 1.0e-9);

		assert_eq!(
			last_event::<TestRuntime>(),
			Some(
				Event::Transferred(cid, alice.clone(), bob.clone(), BalanceType::from_num(9.999))
					.into()
			)
		);

		assert_noop!(
			EncointerBalances::transfer(Some(alice).into(), bob, cid, BalanceType::from_num(60)),
			Error::<TestRuntime>::BalanceTooLow,
		);
	});
}

#[test]
fn demurrage_should_work() {
	new_test_ext().execute_with(|| {
		let alice = AccountKeyring::Alice.to_account_id();
		let cid = CommunityIdentifier::from_str("aaaaaaaaaa").unwrap();
		System::set_block_number(0);
		assert_ok!(EncointerBalances::issue(cid, &alice, BalanceType::from_num(1)));
		System::set_block_number(1);
		assert_eq!(
			EncointerBalances::balance(cid, &alice),
			exp::<BalanceType, BalanceType>(-DefaultDemurrage::get()).unwrap()
		);
		//one year later
		System::set_block_number(86400 / 5 * 356);
		let result: f64 = EncointerBalances::balance(cid, &alice).lossy_into();
		assert_abs_diff_eq!(result, 0.5, epsilon = 1.0e-12);
		let result: f64 = EncointerBalances::total_issuance(cid).lossy_into();
		assert_abs_diff_eq!(result, 0.5, epsilon = 1.0e-12);
	});
}

#[test]
fn transfer_with_demurrage_exceeding_amount_should_fail() {
	let alice = AccountKeyring::Alice.to_account_id();
	let bob = AccountKeyring::Bob.to_account_id();
	new_test_ext().execute_with(|| {
		let cid = CommunityIdentifier::from_str("aaaaaaaaaa").unwrap();
		System::set_block_number(0);
		assert_ok!(EncointerBalances::issue(cid, &alice, BalanceType::from_num(100)));
		//one year later
		System::set_block_number(86400 / 5 * 356);
		// balance should now be 50
		assert_noop!(
			EncointerBalances::transfer(Some(alice).into(), bob, cid, BalanceType::from_num(60)),
			Error::<TestRuntime>::BalanceTooLow,
		);
	});
}

#[test]
fn purge_balances_works() {
	new_test_ext().execute_with(|| {
		let cid = CommunityIdentifier::default();
		let alice = AccountKeyring::Alice.to_account_id();
		let bob = AccountKeyring::Bob.to_account_id();
		assert_ok!(EncointerBalances::issue(cid, &alice, BalanceType::from_num(50.1)));
		assert_eq!(EncointerBalances::balance(cid, &alice), BalanceType::from_num(50.1));
		assert_ok!(EncointerBalances::issue(cid, &bob, BalanceType::from_num(12)));
		assert_eq!(EncointerBalances::balance(cid, &bob), BalanceType::from_num(12));
		EncointerBalances::purge_balances(cid);
		assert_eq!(EncointerBalances::balance(cid, &alice), 0);
		assert_eq!(EncointerBalances::balance(cid, &bob), 0);
	})
}

#[test]
fn set_fee_conversion_factor_errs_with_bad_origin() {
	new_test_ext().execute_with(|| {
		assert_dispatch_err(
			EncointerBalances::set_fee_conversion_factor(
				Origin::signed(AccountKeyring::Bob.into()),
				5u32,
			),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn set_fee_conversion_factor_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(EncointerBalances::set_fee_conversion_factor(Origin::signed(master()), 5u32));

		assert_eq!(EncointerBalances::fee_conversion_factor(), 5u32);
		assert_ok!(EncointerBalances::set_fee_conversion_factor(Origin::signed(master()), 6u32,));

		assert_eq!(EncointerBalances::fee_conversion_factor(), 6u32);
	});
}

mod impl_fungibles {
	use super::*;
	use crate::impl_fungibles::{balance_type, fungible};

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
			// a delta of 10000 corresponds to 10000 * 10 ^ -18 = 10 ^ -14
			assert!(almost_eq(
				fungible(BalanceType::from_num(1f64)),
				1_000_000_000_000_000_000u128,
				10000
			));

			assert!(almost_eq(
				fungible(BalanceType::from_num(0.1f64)),
				0_100_000_000_000_000_000u128,
				10000
			));

			assert!(almost_eq(
				fungible(BalanceType::from_num(123.456f64)),
				123_456_000_000_000_000_000u128,
				10000
			));
		})
	}

	#[test]
	fn total_issuance_and_balance_works() {
		new_test_ext().execute_with(|| {
			let cid = CommunityIdentifier::default();
			let alice = AccountKeyring::Alice.to_account_id();
			assert_ok!(EncointerBalances::issue(cid, &alice, BalanceType::from_num(50.1)));
			assert!(
				almost_eq(
					<EncointerBalances as Inspect<
						<TestRuntime as frame_system::Config>::AccountId,
					>>::balance(cid, &alice),
					50_100_000_000_000_000_000u128,
					10000
				)
			);

			assert!(
				almost_eq(
					<EncointerBalances as Inspect<
						<TestRuntime as frame_system::Config>::AccountId,
					>>::reducible_balance(cid, &alice, false),
					50_100_000_000_000_000_000u128,
					10000
				)
			);

			assert!(
				almost_eq(
					<EncointerBalances as Inspect<
						<TestRuntime as frame_system::Config>::AccountId,
					>>::total_issuance(cid),
					50_100_000_000_000_000_000u128,
					10000
				)
			);
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
					fungible(BalanceType::from_num(4.5 * 10f64.powf(18f64)))
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
					fungible(BalanceType::from_num(4.5 * 10f64.powf(18f64)))
				) == DepositConsequence::Overflow
			);

			assert!(
				EncointerBalances::can_deposit(cid, &alice, fungible(BalanceType::from_num(1))) ==
					DepositConsequence::Success
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
			assert_ok!(EncointerBalances::issue(cid, &alice, BalanceType::from_num(10)));
			assert_ok!(EncointerBalances::issue(cid, &bob, BalanceType::from_num(1)));

			assert!(
				EncointerBalances::can_withdraw(wrong_cid, &alice, 10) ==
					WithdrawConsequence::UnknownAsset
			);

			assert!(
				EncointerBalances::can_withdraw(cid, &bob, fungible(BalanceType::from_num(12))) ==
					WithdrawConsequence::Underflow
			);

			assert!(
				EncointerBalances::can_withdraw(cid, &bob, fungible(BalanceType::from_num(0))) ==
					WithdrawConsequence::Success
			);

			assert!(
				EncointerBalances::can_withdraw(cid, &bob, fungible(BalanceType::from_num(2))) ==
					WithdrawConsequence::NoFunds
			);

			assert!(
				EncointerBalances::can_withdraw(cid, &bob, fungible(BalanceType::from_num(1))) ==
					WithdrawConsequence::Success
			);
		})
	}

	#[test]
	fn set_balance_and_set_total_issuance_works() {
		new_test_ext().execute_with(|| {
			let cid = CommunityIdentifier::default();
			let alice = AccountKeyring::Alice.to_account_id();
			assert_ok!(EncointerBalances::issue(cid, &alice, BalanceType::from_num(10)));

			assert!(
				almost_eq(
					<EncointerBalances as Inspect<
						<TestRuntime as frame_system::Config>::AccountId,
					>>::balance(cid, &alice),
					10_000_000_000_000_000_000u128,
					10000
				)
			);

			assert_ok!(EncointerBalances::set_balance(cid, &alice, 20_000_000_000_000_000_000u128));

			assert!(
				almost_eq(
					<EncointerBalances as Inspect<
						<TestRuntime as frame_system::Config>::AccountId,
					>>::balance(cid, &alice),
					20_000_000_000_000_000_000u128,
					10000
				)
			);

			assert!(
				almost_eq(
					<EncointerBalances as Inspect<
						<TestRuntime as frame_system::Config>::AccountId,
					>>::total_issuance(cid),
					10_000_000_000_000_000_000u128,
					10000
				)
			);

			EncointerBalances::set_total_issuance(cid, 30_000_000_000_000_000_000u128);

			assert!(
				almost_eq(
					<EncointerBalances as Inspect<
						<TestRuntime as frame_system::Config>::AccountId,
					>>::total_issuance(cid),
					30_000_000_000_000_000_000u128,
					10000
				)
			);
		})
	}
}
