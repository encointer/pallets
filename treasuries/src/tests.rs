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
use crate::mock::{Balances, EncointerBalances, EncointerTreasuries, RuntimeOrigin, System};
use approx::assert_abs_diff_eq;
use encointer_primitives::treasuries::SwapNativeOption;
use frame_support::{assert_err, assert_ok};
use mock::{new_test_ext, TestRuntime};
use rstest::rstest;
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

		assert_ok!(EncointerTreasuries::do_spend_native(Some(cid), &beneficiary, amount));
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
#[rstest(
	burn,
	native_allowance,
	rate_float,
	case(false, 10_000_000_000_000, 0.000000000001),
	case(true, 10_000_000_000_000, 0.000000000001),
	case(false, 110_000_000, 0.000_000_2),
	case(true, 110_000_000, 0.000_000_2)
)]
fn swap_native_partial_works(burn: bool, native_allowance: Balance, rate_float: f64) {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let beneficiary = AccountId::from(AccountKeyring::Alice);
		let rate = Some(BalanceType::from_num(rate_float));
		let cid = CommunityIdentifier::default();
		let community_balance = 10_000.0;
		let swap_option: SwapNativeOption<Balance, Moment> = SwapNativeOption {
			cid,
			native_allowance,
			rate,
			do_burn: burn,
			valid_from: None,
			valid_until: None,
		};

		let treasury = EncointerTreasuries::get_community_treasury_account_unchecked(Some(cid));
		Balances::make_free_balance_be(&treasury, native_allowance * 2);
		EncointerBalances::issue(cid, &beneficiary, BalanceType::from_num(community_balance)).unwrap();

		assert_ok!(EncointerTreasuries::do_issue_swap_native_option(
			cid,
			&beneficiary,
			swap_option
		));
		assert_eq!(EncointerTreasuries::swap_native_options(cid, &beneficiary), Some(swap_option));

		let swap_native_amount = native_allowance / 10;
		assert_ok!(EncointerTreasuries::swap_native(
			RuntimeOrigin::signed(beneficiary.clone()),
			cid,
			swap_native_amount
		));

		assert_eq!(Balances::free_balance(&treasury), native_allowance * 2 - native_allowance / 10);
		assert_eq!(Balances::free_balance(&beneficiary), swap_native_amount);

		let swap_native = BalanceType::from_num::<u64>(swap_native_amount.try_into().unwrap());
		assert_abs_diff_eq!(
			EncointerBalances::balance(cid, &beneficiary).to_num::<f64>(),
			community_balance - swap_native.to_num::<f64>() * rate_float,
			epsilon = 0.0001
		);
		// remaining allowance must decrease
		assert_eq!(
			EncointerTreasuries::swap_native_options(cid, &beneficiary)
				.unwrap()
				.native_allowance,
			native_allowance * 9 / 10
		);
		assert!(event_deposited::<TestRuntime>(
			Event::<TestRuntime>::SpentNative {
				treasury: treasury.clone(),
				beneficiary: beneficiary.clone(),
				amount: swap_native_amount
			}
			.into()
		));
		if burn {
			assert!(event_deposited::<TestRuntime>(
				pallet_encointer_balances::Event::<TestRuntime>::Burned(
					cid,
					beneficiary.clone(),
					BalanceType::from_num(swap_native_amount) * rate.unwrap()
				)
				.into()
			));
		} else {
			assert!(event_deposited::<TestRuntime>(
				pallet_encointer_balances::Event::<TestRuntime>::Transferred(
					cid,
					beneficiary.clone(),
					treasury.clone(),
					BalanceType::from_num(swap_native_amount) * rate.unwrap()
				)
				.into()
			));
		}
	});
}
#[test]
fn swap_native_without_option_fails() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let beneficiary = AccountId::from(AccountKeyring::Alice);
		let cid = CommunityIdentifier::default();
		let swap_native_amount = 50_000_000;
		assert_err!(
			EncointerTreasuries::swap_native(
				RuntimeOrigin::signed(beneficiary.clone()),
				cid,
				swap_native_amount
			),
			Error::<TestRuntime>::NoValidSwapOption
		);
	});
}
#[rstest(burn, case(false), case(true))]
fn swap_native_insufficient_cc_fails(burn: bool) {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let beneficiary = AccountId::from(AccountKeyring::Alice);
		let native_allowance: BalanceOf<TestRuntime> = 100_000_000;
		let rate_float = 0.000_000_2;
		let rate = Some(BalanceType::from_num(rate_float));
		let cid = CommunityIdentifier::default();
		let swap_option: SwapNativeOption<Balance, Moment> = SwapNativeOption {
			cid,
			native_allowance,
			rate,
			do_burn: burn,
			valid_from: None,
			valid_until: None,
		};

		let treasury = EncointerTreasuries::get_community_treasury_account_unchecked(Some(cid));
		Balances::make_free_balance_be(&treasury, 51_000_000);
		EncointerBalances::issue(cid, &beneficiary, BalanceType::from_num(1)).unwrap();

		assert_ok!(EncointerTreasuries::do_issue_swap_native_option(
			cid,
			&beneficiary,
			swap_option
		));
		assert_eq!(EncointerTreasuries::swap_native_options(cid, &beneficiary), Some(swap_option));

		let swap_native_amount = 50_000_000;
		assert_err!(
			EncointerTreasuries::swap_native(
				RuntimeOrigin::signed(beneficiary.clone()),
				cid,
				swap_native_amount
			),
			pallet_encointer_balances::Error::<TestRuntime>::BalanceTooLow
		);
	});
}

#[rstest(burn, case(false), case(true))]
fn swap_native_insufficient_treasury_funds_fails(burn: bool) {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let beneficiary = AccountId::from(AccountKeyring::Alice);
		let native_allowance: BalanceOf<TestRuntime> = 100_000_000;
		let rate_float = 0.000_000_2;
		let rate = Some(BalanceType::from_num(rate_float));
		let cid = CommunityIdentifier::default();
		let swap_option: SwapNativeOption<Balance, Moment> = SwapNativeOption {
			cid,
			native_allowance,
			rate,
			do_burn: burn,
			valid_from: None,
			valid_until: None,
		};

		let treasury = EncointerTreasuries::get_community_treasury_account_unchecked(Some(cid));
		Balances::make_free_balance_be(&treasury, 49_000_000);
		EncointerBalances::issue(cid, &beneficiary, BalanceType::from_num(1)).unwrap();

		assert_ok!(EncointerTreasuries::do_issue_swap_native_option(
			cid,
			&beneficiary,
			swap_option
		));
		assert_eq!(EncointerTreasuries::swap_native_options(cid, &beneficiary), Some(swap_option));

		let swap_native_amount = 50_000_000;
		assert_err!(
			EncointerTreasuries::swap_native(
				RuntimeOrigin::signed(beneficiary.clone()),
				cid,
				swap_native_amount
			),
			Error::<TestRuntime>::InsufficientNativeFunds
		);
	});
}

#[rstest(burn, case(false), case(true))]
fn swap_native_insufficient_allowance_fails(burn: bool) {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let beneficiary = AccountId::from(AccountKeyring::Alice);
		let native_allowance: BalanceOf<TestRuntime> = 49_000_000;
		let rate_float = 0.000_000_2;
		let rate = Some(BalanceType::from_num(rate_float));
		let cid = CommunityIdentifier::default();
		let swap_option: SwapNativeOption<Balance, Moment> = SwapNativeOption {
			cid,
			native_allowance,
			rate,
			do_burn: burn,
			valid_from: None,
			valid_until: None,
		};

		let treasury = EncointerTreasuries::get_community_treasury_account_unchecked(Some(cid));
		Balances::make_free_balance_be(&treasury, 51_000_000);
		EncointerBalances::issue(cid, &beneficiary, BalanceType::from_num(1)).unwrap();

		assert_ok!(EncointerTreasuries::do_issue_swap_native_option(
			cid,
			&beneficiary,
			swap_option
		));
		assert_eq!(EncointerTreasuries::swap_native_options(cid, &beneficiary), Some(swap_option));

		let swap_native_amount = 50_000_000;
		assert_err!(
			EncointerTreasuries::swap_native(
				RuntimeOrigin::signed(beneficiary.clone()),
				cid,
				swap_native_amount
			),
			Error::<TestRuntime>::InsufficientAllowance
		);
	});
}
