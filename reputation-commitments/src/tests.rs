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

//! Unit tests for the encointer_reputation_commitments module.

use super::*;
use crate::mock::{
	EncointerCeremonies, EncointerReputationCommitments, EncointerScheduler, RuntimeOrigin,
	Timestamp,
};
use encointer_primitives::{
	ceremonies::Reputation,
	communities::{CommunityIdentifier, Degree, Location},
	reputation_commitments::{DescriptorType, FromStr},
};
use frame_support::{
	assert_err, assert_ok,
	traits::{OnFinalize, OnInitialize},
};
use mock::{new_test_ext, System, TestRuntime};
use parity_scale_codec::Encode;
use sp_runtime::traits::{BlakeTwo256, Hash};
use test_utils::{
	helpers::{event_deposited, last_event, register_test_community},
	storage::participant_reputation,
	AccountId, AccountKeyring, BLOCKTIME, GENESIS_TIME,
};

pub fn set_timestamp(t: u64) {
	let _ = pallet_timestamp::Pallet::<TestRuntime>::set(RuntimeOrigin::none(), t);
}

/// Run until a particular block.
fn run_to_block(n: u64) {
	while System::block_number() < n {
		if System::block_number() > 1 {
			System::on_finalize(System::block_number());
		}
		set_timestamp(GENESIS_TIME + BLOCKTIME * n);
		Timestamp::on_finalize(System::block_number());
		System::set_block_number(System::block_number() + 1);
		System::on_initialize(System::block_number());
	}
}

/// Progress blocks until the phase changes
fn run_to_next_phase() {
	let phase = EncointerScheduler::current_phase();
	let mut blocknr = System::block_number();
	while phase == EncointerScheduler::current_phase() {
		blocknr += 1;
		run_to_block(blocknr);
	}
}

#[test]
fn register_purpose_works() {
	new_test_ext().execute_with(|| {
		let alice = AccountId::from(AccountKeyring::Alice);
		let some_description = DescriptorType::from_str("Some Description").unwrap();
		let some_description2 = DescriptorType::from_str("Some Description 2").unwrap();
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		assert_ok!(EncointerReputationCommitments::do_register_purpose(some_description.clone()));
		assert_eq!(EncointerReputationCommitments::current_purpose_id(), 1);
		assert_eq!(EncointerReputationCommitments::purposes(0), some_description.clone());

		// use extrinsic
		assert_ok!(EncointerReputationCommitments::register_purpose(
			RuntimeOrigin::signed(alice.clone()),
			some_description2.clone()
		));
		assert_eq!(EncointerReputationCommitments::current_purpose_id(), 2);
		assert_eq!(EncointerReputationCommitments::purposes(0), some_description);
		assert_eq!(EncointerReputationCommitments::purposes(1), some_description2);

		assert_eq!(
			last_event::<TestRuntime>(),
			Some(Event::RegisteredCommitmentPurpose(1, some_description2).into())
		);
	});
}

#[test]
fn commitments_work() {
	new_test_ext().execute_with(|| {
		let mut ext = new_test_ext();
		let alice = AccountId::from(AccountKeyring::Alice);
		let bob = AccountId::from(AccountKeyring::Bob);
		let cid = CommunityIdentifier::default();
		let cid2 = CommunityIdentifier::new::<AccountId>(
			Location { lat: Degree::from_num(0.1), lon: Degree::from_num(0.1) },
			vec![],
		)
		.unwrap();
		ext.insert(
			participant_reputation((cid, 11), &alice),
			Reputation::VerifiedUnlinked.encode(),
		);
		ext.insert(
			participant_reputation((cid2, 12), &alice),
			Reputation::VerifiedUnlinked.encode(),
		);

		ext.insert(participant_reputation((cid, 11), &bob), Reputation::VerifiedUnlinked.encode());

		ext.execute_with(|| {
			System::set_block_number(System::block_number() + 1); // this is needed to assert events

			// register purposes
			assert_ok!(EncointerReputationCommitments::do_register_purpose(
				DescriptorType::from_str("Some Description").unwrap()
			));
			assert_ok!(EncointerReputationCommitments::do_register_purpose(
				DescriptorType::from_str("Some Description2").unwrap()
			));

			// commit without hash
			// alice
			assert_ok!(EncointerReputationCommitments::do_commit_reputation(
				&alice.clone(),
				cid,
				11,
				0,
				None
			));
			assert!(Commitments::<TestRuntime>::contains_key((cid, 11), (0, &alice)));
			assert_eq!(
				last_event::<TestRuntime>(),
				Some(Event::CommitedReputation(cid, 11, 0, alice.clone(), None).into())
			);

			// bob
			assert_ok!(EncointerReputationCommitments::do_commit_reputation(
				&bob.clone(),
				cid,
				11,
				0,
				None
			));
			assert!(Commitments::<TestRuntime>::contains_key((cid, 11), (0, &bob)));
			assert_eq!(
				last_event::<TestRuntime>(),
				Some(Event::CommitedReputation(cid, 11, 0, bob.clone(), None).into())
			);

			// same reputation, different purpose
			// use extrinsic
			assert_ok!(EncointerReputationCommitments::commit_reputation(
				RuntimeOrigin::signed(alice.clone()),
				cid,
				11,
				1,
				None
			));
			assert!(Commitments::<TestRuntime>::contains_key((cid, 11), (1, &alice)));
			assert_eq!(
				last_event::<TestRuntime>(),
				Some(Event::CommitedReputation(cid, 11, 1, alice.clone(), None).into())
			);

			// commit with hash
			// use extrinsic
			let hash = "Some value".using_encoded(BlakeTwo256::hash);

			assert_ok!(EncointerReputationCommitments::commit_reputation(
				RuntimeOrigin::signed(alice.clone()),
				cid2,
				12,
				0,
				Some(hash)
			));
			assert_eq!(
				EncointerReputationCommitments::commitments((cid2, 12), (0, &alice)),
				Some(hash)
			);
			assert_eq!(
				last_event::<TestRuntime>(),
				Some(Event::CommitedReputation(cid2, 12, 0, alice.clone(), Some(hash)).into())
			);

			// already commited
			assert_err!(
				EncointerReputationCommitments::do_commit_reputation(
					&alice.clone(),
					cid,
					11,
					0,
					None
				),
				Error::<TestRuntime>::AlreadyCommited
			);

			// no reputation
			assert_err!(
				EncointerReputationCommitments::do_commit_reputation(
					&alice.clone(),
					cid,
					13,
					0,
					None
				),
				Error::<TestRuntime>::NoReputation
			);

			assert_err!(
				EncointerReputationCommitments::do_commit_reputation(
					&alice.clone(),
					cid2,
					11,
					0,
					None
				),
				Error::<TestRuntime>::NoReputation
			);

			// inexistent purpose
			assert_err!(
				EncointerReputationCommitments::do_commit_reputation(
					&alice.clone(),
					cid,
					11,
					2,
					None
				),
				Error::<TestRuntime>::InexistentPurpose
			);
		})
	});
}

#[test]
fn purging_works() {
	new_test_ext().execute_with(|| {
		let mut ext = new_test_ext();
		let alice = AccountId::from(AccountKeyring::Alice);
		let bob = AccountId::from(AccountKeyring::Bob);
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);

		//ext.insert(community_identifiers(), vec![cid, cid2].encode());

		ext.insert(participant_reputation((cid, 1), &alice), Reputation::VerifiedUnlinked.encode());
		ext.insert(
			participant_reputation((cid2, 1), &alice),
			Reputation::VerifiedUnlinked.encode(),
		);
		ext.insert(participant_reputation((cid, 1), &bob), Reputation::VerifiedUnlinked.encode());
		ext.insert(participant_reputation((cid2, 1), &bob), Reputation::VerifiedUnlinked.encode());
		ext.insert(participant_reputation((cid, 2), &alice), Reputation::VerifiedUnlinked.encode());
		ext.insert(
			participant_reputation((cid2, 2), &alice),
			Reputation::VerifiedUnlinked.encode(),
		);
		ext.insert(participant_reputation((cid, 2), &bob), Reputation::VerifiedUnlinked.encode());
		ext.insert(participant_reputation((cid2, 2), &bob), Reputation::VerifiedUnlinked.encode());

		ext.execute_with(|| {
			System::set_block_number(System::block_number() + 1); // this is needed to assert events
														 // re-register because of different ext
			let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
			let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);

			// register purposes
			assert_ok!(EncointerReputationCommitments::do_register_purpose(
				DescriptorType::from_str("Some Description").unwrap()
			));
			assert_ok!(EncointerReputationCommitments::do_register_purpose(
				DescriptorType::from_str("Some Description2").unwrap()
			));

			// commit
			assert_ok!(EncointerReputationCommitments::do_commit_reputation(
				&alice.clone(),
				cid,
				1,
				0,
				None
			));
			assert_ok!(EncointerReputationCommitments::do_commit_reputation(
				&bob.clone(),
				cid2,
				1,
				0,
				None
			));
			assert_ok!(EncointerReputationCommitments::do_commit_reputation(
				&alice.clone(),
				cid,
				1,
				1,
				None
			));
			assert_ok!(EncointerReputationCommitments::do_commit_reputation(
				&bob.clone(),
				cid,
				1,
				1,
				None
			));

			assert_ok!(EncointerReputationCommitments::do_commit_reputation(
				&alice.clone(),
				cid2,
				2,
				0,
				None
			));
			assert_ok!(EncointerReputationCommitments::do_commit_reputation(
				&bob.clone(),
				cid,
				2,
				0,
				None
			));
			assert_ok!(EncointerReputationCommitments::do_commit_reputation(
				&alice.clone(),
				cid,
				2,
				1,
				None
			));
			assert_ok!(EncointerReputationCommitments::do_commit_reputation(
				&bob.clone(),
				cid2,
				2,
				1,
				None
			));

			assert!(Commitments::<TestRuntime>::contains_key((cid, 1), (0, &alice)));
			assert!(Commitments::<TestRuntime>::contains_key((cid2, 1), (0, &bob)));
			assert!(Commitments::<TestRuntime>::contains_key((cid, 1), (1, &alice)));
			assert!(Commitments::<TestRuntime>::contains_key((cid, 1), (1, &bob)));

			assert!(Commitments::<TestRuntime>::contains_key((cid2, 2), (0, &alice)));
			assert!(Commitments::<TestRuntime>::contains_key((cid, 2), (0, &bob)));
			assert!(Commitments::<TestRuntime>::contains_key((cid, 2), (1, &alice)));
			assert!(Commitments::<TestRuntime>::contains_key((cid2, 2), (1, &bob)));

			let reputation_lifetime = EncointerCeremonies::reputation_lifetime();

			for _ in 0..reputation_lifetime {
				run_to_next_phase();
				run_to_next_phase();
				run_to_next_phase();
			}

			assert!(Commitments::<TestRuntime>::contains_key((cid, 1), (0, &alice)));
			assert!(Commitments::<TestRuntime>::contains_key((cid2, 1), (0, &bob)));
			assert!(Commitments::<TestRuntime>::contains_key((cid, 1), (1, &alice)));
			assert!(Commitments::<TestRuntime>::contains_key((cid, 1), (1, &bob)));

			assert!(Commitments::<TestRuntime>::contains_key((cid2, 2), (0, &alice)));
			assert!(Commitments::<TestRuntime>::contains_key((cid, 2), (0, &bob)));
			assert!(Commitments::<TestRuntime>::contains_key((cid, 2), (1, &alice)));
			assert!(Commitments::<TestRuntime>::contains_key((cid2, 2), (1, &bob)));

			run_to_next_phase();
			run_to_next_phase();
			run_to_next_phase();

			assert!(event_deposited::<TestRuntime>(Event::CommitmentRegistryPurged(1).into()));

			assert!(!Commitments::<TestRuntime>::contains_key((cid, 1), (0, &alice)));
			assert!(!Commitments::<TestRuntime>::contains_key((cid2, 1), (0, &bob)));
			assert!(!Commitments::<TestRuntime>::contains_key((cid, 1), (1, &alice)));
			assert!(!Commitments::<TestRuntime>::contains_key((cid, 1), (1, &bob)));

			assert!(Commitments::<TestRuntime>::contains_key((cid2, 2), (0, &alice)));
			assert!(Commitments::<TestRuntime>::contains_key((cid, 2), (0, &bob)));
			assert!(Commitments::<TestRuntime>::contains_key((cid, 2), (1, &alice)));
			assert!(Commitments::<TestRuntime>::contains_key((cid2, 2), (1, &bob)));

			run_to_next_phase();
			run_to_next_phase();
			run_to_next_phase();

			assert!(event_deposited::<TestRuntime>(Event::CommitmentRegistryPurged(2).into()));

			assert!(!Commitments::<TestRuntime>::contains_key((cid, 1), (0, &alice)));
			assert!(!Commitments::<TestRuntime>::contains_key((cid2, 1), (0, &bob)));
			assert!(!Commitments::<TestRuntime>::contains_key((cid, 1), (1, &alice)));
			assert!(!Commitments::<TestRuntime>::contains_key((cid, 1), (1, &bob)));

			assert!(!Commitments::<TestRuntime>::contains_key((cid2, 2), (0, &alice)));
			assert!(!Commitments::<TestRuntime>::contains_key((cid, 2), (0, &bob)));
			assert!(!Commitments::<TestRuntime>::contains_key((cid, 2), (1, &alice)));
			assert!(!Commitments::<TestRuntime>::contains_key((cid2, 2), (1, &bob)));
		})
	})
}
