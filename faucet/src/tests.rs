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
use codec::Encode;
use encointer_primitives::{
	ceremonies::Reputation,
	communities::{Degree, Location},
	reputation_commitments::{DescriptorType, FromStr},
};
use frame_support::{assert_err, assert_ok};
use mock::{master, new_test_ext, RuntimeOrigin, TestRuntime};
use sp_runtime::DispatchError;
use test_utils::{helpers::*, storage::*, *};

#[test]
fn purpose_id_is_stored_correctly() {
	new_test_ext().execute_with(|| {
		assert_eq!(EncointerFaucet::reputation_commitments_purpose_id(), 0,);
		assert_eq!(
			EncointerReputationCommitments::purposes(0),
			DescriptorType::from_str("EncointerFaucet").unwrap(),
		);
	});
}

#[test]
fn set_drip_amount_errs_with_bad_origin() {
	new_test_ext().execute_with(|| {
		assert_dispatch_err(
			EncointerFaucet::set_drip_amount(
				RuntimeOrigin::signed(AccountKeyring::Bob.into()),
				10000u64,
			),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn set_drip_amount_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events

		assert_ok!(EncointerFaucet::set_drip_amount(RuntimeOrigin::signed(master()), 1001u64));

		assert_eq!(EncointerFaucet::drip_amount(), 1001u64);
		assert_ok!(EncointerFaucet::set_drip_amount(RuntimeOrigin::signed(master()), 1337u64));

		assert_eq!(EncointerFaucet::drip_amount(), 1337u64);

		assert_eq!(last_event::<TestRuntime>(), Some(Event::DripAmountUpdated(1337u64).into()));
	});
}

#[test]
fn pot_balance_works() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&EncointerFaucet::account_id(), 101);
		// free_balance - minimum_balance
		assert_eq!(EncointerFaucet::pot(), 100);
	});
}

#[test]
fn dripping_works() {
	let mut ext = new_test_ext();
	let alice = AccountId::from(AccountKeyring::Alice);
	let cid = CommunityIdentifier::default();
	let cid2 = CommunityIdentifier::new::<AccountId>(
		Location { lat: Degree::from_num(0.1), lon: Degree::from_num(0.1) },
		vec![],
	)
	.unwrap();
	ext.insert(participant_reputation((cid, 12), &alice), Reputation::VerifiedUnlinked.encode());

	ext.execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		Balances::make_free_balance_be(&EncointerFaucet::account_id(), 1000);
		assert_ok!(EncointerFaucet::set_drip_amount(RuntimeOrigin::signed(master()), 101u64));

		assert_ok!(EncointerFaucet::drip(RuntimeOrigin::signed(alice.clone()), cid, 12,));
		assert_eq!(Balances::free_balance(&alice), 101);
		assert_eq!(EncointerFaucet::pot(), 898);

		assert_eq!(last_event::<TestRuntime>(), Some(Event::Dripped(alice.clone(), 101).into()));

		assert_err!(
			EncointerFaucet::drip(RuntimeOrigin::signed(alice.clone()), cid, 12),
			encointer_reputation_commitments::Error::<TestRuntime>::AlreadyCommited
		);

		assert_err!(
			EncointerFaucet::drip(RuntimeOrigin::signed(alice.clone()), cid, 13),
			encointer_reputation_commitments::Error::<TestRuntime>::NoReputation
		);

		assert_err!(
			EncointerFaucet::drip(RuntimeOrigin::signed(alice.clone()), cid2, 12),
			encointer_reputation_commitments::Error::<TestRuntime>::NoReputation
		);
	})
}

#[test]
fn faucet_empty_works() {
	let mut ext = new_test_ext();
	let alice = AccountId::from(AccountKeyring::Alice);
	let cid = CommunityIdentifier::default();
	let cid2 = CommunityIdentifier::new::<AccountId>(
		Location { lat: Degree::from_num(0.1), lon: Degree::from_num(0.1) },
		vec![],
	)
	.unwrap();
	ext.insert(participant_reputation((cid, 12), &alice), Reputation::VerifiedUnlinked.encode());
	ext.insert(participant_reputation((cid, 13), &alice), Reputation::VerifiedUnlinked.encode());
	ext.insert(participant_reputation((cid2, 12), &alice), Reputation::VerifiedUnlinked.encode());
	ext.insert(participant_reputation((cid2, 13), &alice), Reputation::VerifiedUnlinked.encode());

	ext.execute_with(|| {
		Balances::make_free_balance_be(&EncointerFaucet::account_id(), 35);
		assert_ok!(EncointerFaucet::set_drip_amount(RuntimeOrigin::signed(master()), 10u64));

		assert_ok!(EncointerFaucet::drip(RuntimeOrigin::signed(alice.clone()), cid, 12,));
		assert_ok!(EncointerFaucet::drip(RuntimeOrigin::signed(alice.clone()), cid, 13,));
		assert_ok!(EncointerFaucet::drip(RuntimeOrigin::signed(alice.clone()), cid2, 12,));
		assert_eq!(Balances::free_balance(&alice), 30);
		assert_eq!(EncointerFaucet::pot(), 4);

		assert_err!(
			EncointerFaucet::drip(RuntimeOrigin::signed(alice.clone()), cid2, 13),
			Error::<TestRuntime>::FaucetEmpty
		);
		assert_eq!(Balances::free_balance(&alice), 30);
		assert_eq!(EncointerFaucet::pot(), 4);
	})
}
