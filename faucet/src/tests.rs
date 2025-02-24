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

//! Unit tests for the encointer_faucet module.

use super::*;
use crate::mock::{Balances, EncointerFaucet, EncointerReputationCommitments, System};
use encointer_primitives::{
	ceremonies::Reputation,
	faucet::FromStr,
	reputation_commitments::{DescriptorType, FromStr as DescriptorFromStr},
};
use frame_support::{assert_err, assert_ok};
use mock::{new_test_ext, RuntimeOrigin, TestRuntime};
use parity_scale_codec::Encode;
use sp_core::bounded_vec;
use sp_runtime::{AccountId32, DispatchError};
use test_utils::{helpers::*, storage::*, *};

pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

fn new_faucet(
	origin: RuntimeOrigin,
	name: FaucetNameType,
	amount: BalanceOf<TestRuntime>,
	whitelist: Option<WhiteListType>,
	drip_amount: BalanceOf<TestRuntime>,
) -> AccountId32 {
	assert_ok!(EncointerFaucet::create_faucet(origin, name, amount, whitelist, drip_amount));

	if let mock::RuntimeEvent::EncointerFaucet(Event::FaucetCreated(faucet_account, _)) =
		last_event::<TestRuntime>().unwrap()
	{
		faucet_account
	} else {
		panic!("Faucet not found");
	}
}
#[test]
fn faucet_creation_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let alice = AccountId::from(AccountKeyring::Alice);
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);

		// insert some purposes
		EncointerReputationCommitments::do_register_purpose(
			DescriptorType::from_str("Some Description").unwrap(),
		)
		.ok();
		EncointerReputationCommitments::do_register_purpose(
			DescriptorType::from_str("Some Description 2").unwrap(),
		)
		.ok();

		let whitelist_input: WhiteListType = bounded_vec![cid, cid2];
		Balances::make_free_balance_be(&alice, 114);
		let faucet_account = new_faucet(
			RuntimeOrigin::signed(alice.clone()),
			FaucetNameType::from_str("Some Faucet Name").unwrap(),
			100,
			Some(whitelist_input.clone()),
			10,
		);

		let faucet = EncointerFaucet::faucets(&faucet_account).unwrap();

		assert_eq!(faucet.name, FaucetNameType::from_str("Some Faucet Name").unwrap());
		assert_eq!(faucet.purpose_id, 2);
		assert_eq!(faucet.whitelist, Some(whitelist_input));
		assert_eq!(faucet.drip_amount, 10);
		assert_eq!(faucet.creator, alice.clone());
		assert_eq!(Balances::free_balance(&alice), 1);
		assert_eq!(Balances::reserved_balance(&alice), 13);
		assert_eq!(Balances::free_balance(&faucet_account), 100);
	});
}

#[test]
fn faucet_creation_fails_with_insufficient_balance() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let alice = AccountId::from(AccountKeyring::Alice);
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);

		let whitelist_input: WhiteListType = bounded_vec![cid, cid2];
		Balances::make_free_balance_be(&alice, 112);

		assert_err!(
			EncointerFaucet::create_faucet(
				RuntimeOrigin::signed(alice.clone()),
				FaucetNameType::from_str("Some Faucet Name").unwrap(),
				100,
				Some(whitelist_input.clone()),
				10
			),
			Error::<TestRuntime>::InsuffiecientBalance
		);
	});
}

#[test]
fn faucet_creation_fails_with_duplicate() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let alice = AccountId::from(AccountKeyring::Alice);
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);

		let whitelist_input: WhiteListType = bounded_vec![cid, cid2];
		Balances::make_free_balance_be(&alice, 100);

		assert_ok!(EncointerFaucet::create_faucet(
			RuntimeOrigin::signed(alice.clone()),
			FaucetNameType::from_str("Some Faucet Name").unwrap(),
			10,
			Some(whitelist_input.clone()),
			2
		));

		assert_ok!(EncointerFaucet::create_faucet(
			RuntimeOrigin::signed(alice.clone()),
			FaucetNameType::from_str("Some Faucet Name 2").unwrap(),
			10,
			Some(whitelist_input.clone()),
			2
		));

		assert_err!(
			EncointerFaucet::create_faucet(
				RuntimeOrigin::signed(alice.clone()),
				FaucetNameType::from_str("Some Faucet Name").unwrap(),
				10,
				Some(whitelist_input.clone()),
				2
			),
			Error::<TestRuntime>::FaucetAlreadyExists
		);
	});
}

#[test]
fn faucet_creation_fails_with_too_small_drip_amount() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let alice = AccountId::from(AccountKeyring::Alice);
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);

		let whitelist_input: WhiteListType = bounded_vec![cid, cid2];
		Balances::make_free_balance_be(&alice, 100);

		assert_err!(
			EncointerFaucet::create_faucet(
				RuntimeOrigin::signed(alice.clone()),
				FaucetNameType::from_str("Some Faucet Name").unwrap(),
				10,
				Some(whitelist_input.clone()),
				1
			),
			Error::<TestRuntime>::DripAmountTooSmall
		);
	});
}

#[test]
fn faucet_creation_fails_with_invalid_cid() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let alice = AccountId::from(AccountKeyring::Alice);
		let cid = register_test_community::<TestRuntime>(None, 10.0, 10.0);
		let cid2 = CommunityIdentifier::default();

		let whitelist_input: WhiteListType = bounded_vec![cid, cid2];

		assert_err!(
			EncointerFaucet::create_faucet(
				RuntimeOrigin::signed(alice.clone()),
				FaucetNameType::from_str("Some Faucet Name").unwrap(),
				10,
				Some(whitelist_input.clone()),
				1
			),
			Error::<TestRuntime>::InvalidCommunityIdentifierInWhitelist
		);
	});
}

#[test]
fn dripping_works() {
	new_test_ext().execute_with(|| {
		let mut ext = new_test_ext();
		let alice = AccountId::from(AccountKeyring::Alice);
		let bob = AccountId::from(AccountKeyring::Bob);
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);

		ext.insert(participant_reputation((cid, 12), &bob), Reputation::VerifiedUnlinked.encode());

		ext.execute_with(|| {
			// re-register because of different ext
			let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
			let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);
			System::set_block_number(System::block_number() + 1); // this is needed to assert events

			let whitelist_input: WhiteListType = bounded_vec![cid, cid2];
			Balances::make_free_balance_be(&alice, 1000);

			let faucet_account1 = new_faucet(
				RuntimeOrigin::signed(alice.clone()),
				FaucetNameType::from_str("Some Faucet Name").unwrap(),
				100,
				Some(whitelist_input.clone()),
				10,
			);

			let faucet_account2 = new_faucet(
				RuntimeOrigin::signed(alice.clone()),
				FaucetNameType::from_str("Some Faucet Name 2").unwrap(),
				100,
				Some(whitelist_input.clone()),
				9,
			);

			assert_ok!(EncointerFaucet::drip(
				RuntimeOrigin::signed(bob.clone()),
				faucet_account1.clone(),
				cid,
				12,
			));
			assert_eq!(Balances::free_balance(&bob), 10);
			assert_eq!(Balances::free_balance(&faucet_account1), 90);
			assert_eq!(
				last_event::<TestRuntime>(),
				Some(Event::Dripped(faucet_account1.clone(), bob.clone(), 10).into())
			);

			assert_ok!(EncointerFaucet::drip(
				RuntimeOrigin::signed(bob.clone()),
				faucet_account2.clone(),
				cid,
				12,
			));
			assert_eq!(Balances::free_balance(&bob), 19);
			assert_eq!(Balances::free_balance(&faucet_account2), 91);
			assert_eq!(
				last_event::<TestRuntime>(),
				Some(Event::Dripped(faucet_account2.clone(), bob.clone(), 9).into())
			);

			assert_err!(
				EncointerFaucet::drip(
					RuntimeOrigin::signed(bob.clone()),
					faucet_account1.clone(),
					cid,
					12
				),
				pallet_encointer_reputation_commitments::Error::<TestRuntime>::AlreadyCommited
			);

			assert_err!(
				EncointerFaucet::drip(
					RuntimeOrigin::signed(bob.clone()),
					faucet_account1.clone(),
					cid,
					13
				),
				pallet_encointer_reputation_commitments::Error::<TestRuntime>::NoReputation
			);

			assert_err!(
				EncointerFaucet::drip(
					RuntimeOrigin::signed(bob.clone()),
					faucet_account1.clone(),
					cid2,
					12
				),
				pallet_encointer_reputation_commitments::Error::<TestRuntime>::NoReputation
			);
		})
	})
}

#[test]
fn faucet_empty_works() {
	new_test_ext().execute_with(|| {
		let mut ext = new_test_ext();
		let alice = AccountId::from(AccountKeyring::Alice);
		let bob = AccountId::from(AccountKeyring::Bob);
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);
		ext.insert(
			participant_reputation((cid, 12), &alice),
			Reputation::VerifiedUnlinked.encode(),
		);
		ext.insert(
			participant_reputation((cid, 13), &alice),
			Reputation::VerifiedUnlinked.encode(),
		);
		ext.insert(
			participant_reputation((cid2, 12), &alice),
			Reputation::VerifiedUnlinked.encode(),
		);
		ext.insert(
			participant_reputation((cid2, 13), &alice),
			Reputation::VerifiedUnlinked.encode(),
		);

		ext.execute_with(|| {
			// re-register because of different ext
			let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
			let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);
			System::set_block_number(System::block_number() + 1); // this is needed to assert events
			let whitelist_input: WhiteListType = bounded_vec![cid, cid2];
			Balances::make_free_balance_be(&bob, 1000);

			let faucet_account = new_faucet(
				RuntimeOrigin::signed(bob.clone()),
				FaucetNameType::from_str("Some Faucet Name").unwrap(),
				35,
				Some(whitelist_input.clone()),
				10,
			);

			assert_ok!(EncointerFaucet::drip(
				RuntimeOrigin::signed(alice.clone()),
				faucet_account.clone(),
				cid,
				12,
			));
			assert_ok!(EncointerFaucet::drip(
				RuntimeOrigin::signed(alice.clone()),
				faucet_account.clone(),
				cid,
				13,
			));
			assert_ok!(EncointerFaucet::drip(
				RuntimeOrigin::signed(alice.clone()),
				faucet_account.clone(),
				cid2,
				12,
			));
			assert_eq!(Balances::free_balance(&alice), 30);
			assert_eq!(Balances::free_balance(&faucet_account), 5);

			assert_err!(
				EncointerFaucet::drip(
					RuntimeOrigin::signed(alice.clone()),
					faucet_account.clone(),
					cid2,
					13
				),
				Error::<TestRuntime>::FaucetEmpty
			);
			assert_eq!(Balances::free_balance(&alice), 30);
			assert_eq!(Balances::free_balance(&faucet_account), 5);
		})
	})
}

#[test]
fn dripping_fails_when_cid_not_whitelisted() {
	new_test_ext().execute_with(|| {
		let mut ext = new_test_ext();
		let alice = AccountId::from(AccountKeyring::Alice);
		let bob = AccountId::from(AccountKeyring::Bob);
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);
		ext.insert(
			participant_reputation((cid, 12), &alice),
			Reputation::VerifiedUnlinked.encode(),
		);
		ext.insert(
			participant_reputation((cid2, 13), &alice),
			Reputation::VerifiedUnlinked.encode(),
		);

		ext.execute_with(|| {
			System::set_block_number(System::block_number() + 1); // this is needed to assert events
													  // re-register because of different ext
			let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
			let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);
			let whitelist_input: WhiteListType = bounded_vec![cid];
			Balances::make_free_balance_be(&bob, 1000);

			let faucet_account = new_faucet(
				RuntimeOrigin::signed(bob.clone()),
				FaucetNameType::from_str("Some Faucet Name").unwrap(),
				35,
				Some(whitelist_input.clone()),
				10,
			);

			assert_ok!(EncointerFaucet::drip(
				RuntimeOrigin::signed(alice.clone()),
				faucet_account.clone(),
				cid,
				12,
			));
			assert_err!(
				EncointerFaucet::drip(
					RuntimeOrigin::signed(alice.clone()),
					faucet_account.clone(),
					cid2,
					13,
				),
				Error::<TestRuntime>::CommunityNotInWhitelist
			);
		})
	})
}

#[test]
fn dripping_fails_with_inexistent_faucet() {
	new_test_ext().execute_with(|| {
		let alice = AccountId::from(AccountKeyring::Alice);
		let bob = AccountId::from(AccountKeyring::Bob);
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);

		assert_err!(
			EncointerFaucet::drip(RuntimeOrigin::signed(alice.clone()), bob.clone(), cid, 13,),
			Error::<TestRuntime>::InexsistentFaucet
		);
	})
}

#[test]
fn dripping_works_with_whitelist_bypass() {
	new_test_ext().execute_with(|| {
		let mut ext = new_test_ext();
		let alice = AccountId::from(AccountKeyring::Alice);
		let bob = AccountId::from(AccountKeyring::Bob);
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);
		ext.insert(
			participant_reputation((cid, 12), &alice),
			Reputation::VerifiedUnlinked.encode(),
		);
		ext.insert(
			participant_reputation((cid2, 13), &alice),
			Reputation::VerifiedUnlinked.encode(),
		);

		ext.execute_with(|| {
			System::set_block_number(System::block_number() + 1); // this is needed to assert events
													  // re-register because of different ext
			let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
			let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);
			Balances::make_free_balance_be(&bob, 1000);

			let faucet_account = new_faucet(
				RuntimeOrigin::signed(bob.clone()),
				FaucetNameType::from_str("Some Faucet Name").unwrap(),
				35,
				None,
				10,
			);

			assert_ok!(EncointerFaucet::drip(
				RuntimeOrigin::signed(alice.clone()),
				faucet_account.clone(),
				cid,
				12,
			));
			assert_ok!(EncointerFaucet::drip(
				RuntimeOrigin::signed(alice.clone()),
				faucet_account.clone(),
				cid2,
				13,
			));
		})
	})
}

#[test]
fn dissolve_faucet_works() {
	let mut ext = new_test_ext();
	let alice = AccountId::from(AccountKeyring::Alice);
	let bob = AccountId::from(AccountKeyring::Bob);

	ext.execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let whitelist_input: WhiteListType = bounded_vec![cid];
		Balances::make_free_balance_be(&bob, 1000);

		let faucet_account = new_faucet(
			RuntimeOrigin::signed(bob.clone()),
			FaucetNameType::from_str("Some Faucet Name").unwrap(),
			35,
			Some(whitelist_input.clone()),
			10,
		);

		assert_eq!(Balances::free_balance(&bob), 952);
		assert_eq!(Balances::reserved_balance(&bob), 13);

		assert_eq!(Balances::free_balance(&faucet_account), 35);
		assert_ok!(EncointerFaucet::dissolve_faucet(
			RuntimeOrigin::root(),
			faucet_account.clone(),
			alice.clone()
		));
		assert_eq!(Balances::free_balance(&faucet_account), 0);
		assert_eq!(Balances::free_balance(&alice), 35);
		assert_eq!(Balances::free_balance(&bob), 965);
		assert_eq!(Balances::reserved_balance(&bob), 0);

		assert_eq!(
			last_event::<TestRuntime>(),
			Some(Event::FaucetDissolved(faucet_account.clone()).into())
		);
	})
}

#[test]
fn dissolve_faucet_fails_if_not_root() {
	let mut ext = new_test_ext();
	let alice = AccountId::from(AccountKeyring::Alice);
	let bob = AccountId::from(AccountKeyring::Bob);

	ext.execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let whitelist_input: WhiteListType = bounded_vec![cid];
		Balances::make_free_balance_be(&bob, 1000);

		let faucet_account = new_faucet(
			RuntimeOrigin::signed(bob.clone()),
			FaucetNameType::from_str("Some Faucet Name").unwrap(),
			35,
			Some(whitelist_input.clone()),
			10,
		);

		assert_err!(
			EncointerFaucet::dissolve_faucet(
				RuntimeOrigin::signed(bob.clone()),
				faucet_account.clone(),
				alice.clone()
			),
			DispatchError::BadOrigin
		);
	})
}

#[test]
fn dissolve_faucet_fails_with_inexistent_faucet() {
	new_test_ext().execute_with(|| {
		let alice = AccountId::from(AccountKeyring::Alice);
		let bob = AccountId::from(AccountKeyring::Bob);

		assert_err!(
			EncointerFaucet::dissolve_faucet(RuntimeOrigin::root(), bob.clone(), alice.clone()),
			Error::<TestRuntime>::InexsistentFaucet
		);
	})
}

#[test]
fn close_faucet_works() {
	new_test_ext().execute_with(|| {
		let mut ext = new_test_ext();
		let alice = AccountId::from(AccountKeyring::Alice);
		let bob = AccountId::from(AccountKeyring::Bob);
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);

		ext.insert(
			participant_reputation((cid, 12), &alice),
			Reputation::VerifiedUnlinked.encode(),
		);

		ext.execute_with(|| {
			let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
			System::set_block_number(System::block_number() + 1); // this is needed to assert events
			let whitelist_input: WhiteListType = bounded_vec![cid];
			Balances::make_free_balance_be(&bob, 1000);

			let faucet_account = new_faucet(
				RuntimeOrigin::signed(bob.clone()),
				FaucetNameType::from_str("Some Faucet Name").unwrap(),
				35,
				Some(whitelist_input.clone()),
				12,
			);

			assert_eq!(Balances::free_balance(&bob), 952);
			assert_eq!(Balances::reserved_balance(&bob), 13);

			assert_eq!(Balances::free_balance(&faucet_account), 35);

			// after one drip the faucet balance is 23
			// 23 < 24 (2 * drip_amount)
			assert_ok!(EncointerFaucet::drip(
				RuntimeOrigin::signed(alice.clone()),
				faucet_account.clone(),
				cid,
				12,
			));

			assert_ok!(EncointerFaucet::close_faucet(
				RuntimeOrigin::signed(bob.clone()),
				faucet_account.clone(),
			));

			assert_eq!(Balances::free_balance(&faucet_account), 0);
			assert_eq!(Balances::free_balance(&alice), 12);
			assert_eq!(Balances::free_balance(&bob), 965);
			assert_eq!(Balances::free_balance(EncointerFaucet::catch_basin_account_id()), 23);
			assert_eq!(Balances::reserved_balance(&bob), 0);

			assert_eq!(
				last_event::<TestRuntime>(),
				Some(Event::FaucetClosed(faucet_account.clone()).into())
			);
		})
	})
}

#[test]
fn close_faucet_fails_if_not_creator() {
	let mut ext = new_test_ext();
	let alice = AccountId::from(AccountKeyring::Alice);
	let bob = AccountId::from(AccountKeyring::Bob);

	ext.execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let whitelist_input: WhiteListType = bounded_vec![cid];
		Balances::make_free_balance_be(&bob, 1000);

		let faucet_account = new_faucet(
			RuntimeOrigin::signed(bob.clone()),
			FaucetNameType::from_str("Some Faucet Name").unwrap(),
			35,
			Some(whitelist_input.clone()),
			10,
		);

		assert_err!(
			EncointerFaucet::close_faucet(
				RuntimeOrigin::signed(alice.clone()),
				faucet_account.clone()
			),
			Error::<TestRuntime>::NotCreator
		);
	})
}

#[test]
fn close_faucet_fails_if_not_empty() {
	let mut ext = new_test_ext();
	let bob = AccountId::from(AccountKeyring::Bob);

	ext.execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let whitelist_input: WhiteListType = bounded_vec![cid];
		Balances::make_free_balance_be(&bob, 1000);

		let faucet_account = new_faucet(
			RuntimeOrigin::signed(bob.clone()),
			FaucetNameType::from_str("Some Faucet Name").unwrap(),
			35,
			Some(whitelist_input.clone()),
			10,
		);

		assert_err!(
			EncointerFaucet::close_faucet(
				RuntimeOrigin::signed(bob.clone()),
				faucet_account.clone()
			),
			Error::<TestRuntime>::FaucetNotEmpty
		);
	})
}

#[test]
fn close_faucet_fails_with_inexistent_faucet() {
	new_test_ext().execute_with(|| {
		let bob = AccountId::from(AccountKeyring::Bob);

		assert_err!(
			EncointerFaucet::close_faucet(RuntimeOrigin::signed(bob.clone()), bob.clone()),
			Error::<TestRuntime>::InexsistentFaucet
		);
	})
}

#[test]
fn set_reserve_amount_errs_with_bad_origin() {
	new_test_ext().execute_with(|| {
		assert_dispatch_err(
			EncointerFaucet::set_reserve_amount(
				RuntimeOrigin::signed(AccountKeyring::Bob.into()),
				1,
			),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn set_reserve_amount_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(EncointerFaucet::set_reserve_amount(
			RuntimeOrigin::signed(AccountKeyring::Alice.into()),
			2
		));

		assert_eq!(EncointerFaucet::reserve_amount(), 2);
		assert_ok!(EncointerFaucet::set_reserve_amount(
			RuntimeOrigin::signed(AccountKeyring::Alice.into()),
			3
		));

		assert_eq!(EncointerFaucet::reserve_amount(), 3);
	});
}
