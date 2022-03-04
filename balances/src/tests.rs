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
use frame_support::{assert_noop, assert_ok, traits::OnInitialize};
use mock::{new_test_ext, EncointerBalances, System, TestRuntime};
use sp_std::str::FromStr;
use test_utils::{helpers::last_event, AccountKeyring};

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
