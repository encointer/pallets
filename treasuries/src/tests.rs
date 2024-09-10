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

//! Unit tests for the encointer_treasuries module.

use super::*;
use crate::mock::{Balances, EncointerTreasuries, System};
use frame_support::assert_ok;
use mock::{new_test_ext, TestRuntime};
use sp_core::crypto::Ss58Codec;
use std::str::FromStr;
use test_utils::{helpers::*, *};

pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[test]
fn treasury_spending_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let beneficiary = AccountId::from(AccountKeyring::Alice);
		let amount: BalanceOf<TestRuntime> = 100_000_000;
		let cid = CommunityIdentifier::default();

		let treasury = EncointerTreasuries::get_community_treasury_account_unchecked(Some(cid));
		Balances::make_free_balance_be(&treasury, 500_000_000);

		assert_ok!(EncointerTreasuries::do_spend_native(Some(cid), beneficiary.clone(), amount));
		assert_eq!(Balances::free_balance(&treasury), 400_000_000);
		assert_eq!(Balances::free_balance(&beneficiary), amount);
		assert_eq!(
			last_event::<TestRuntime>(),
			Some(Event::SpentNative { treasury, beneficiary, amount }.into())
		);
	});
}

#[test]
fn treasury_getter_works() {
	new_test_ext().execute_with(|| {
		let treasury_account = EncointerTreasuries::get_community_treasury_account_unchecked(None);
		assert_eq!(
			treasury_account.to_ss58check(),
			"5FU79FVXdN3RYSj8857XjNT2SgeRrhk8iUzjb75X1yc8YDkZ"
		);
		let cid = CommunityIdentifier::default();
		let treasury_account =
			EncointerTreasuries::get_community_treasury_account_unchecked(Some(cid));
		assert_eq!(
			treasury_account.to_ss58check(),
			"5D58hM12H6Gkc1h1chuzbbJ3FitGHAyTM6ECkdz4hi5dFheH"
		);
		let cid = CommunityIdentifier::from_str("sqm1v79dF6b").expect("invalid community id");
		let treasury_account =
			EncointerTreasuries::get_community_treasury_account_unchecked(Some(cid));
		assert_eq!(
			treasury_account.to_ss58check(),
			"5CWoc3mGF9VEnuZzBbPWxhKPvY743AGwxUbvkYQHS8yWZbem"
		)
	});
}
