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
use crate::mock::{EncointerReputationCommitments, RuntimeOrigin};
use codec::Encode;
use encointer_primitives::{
	ceremonies::Reputation,
	communities::{CommunityIdentifier, Degree, Location},
	reputation_commitments::{DescriptorType, FromStr},
};
use frame_support::{assert_err, assert_ok};
use mock::{new_test_ext, System, TestRuntime};
use sp_runtime::traits::{BlakeTwo256, Hash};
use test_utils::{helpers::last_event, storage::participant_reputation, AccountId, AccountKeyring};

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
			assert_ok!(EncointerReputationCommitments::do_commit_reputation(
				&alice.clone(),
				cid,
				11,
				0,
				None
			));
			assert_eq!(
				EncointerReputationCommitments::commitments(0, (&alice, cid, 11)),
				None
			);
			assert_eq!(
				last_event::<TestRuntime>(),
				Some(
					Event::CommitedReputation(alice.clone(), cid, 11, 0, None)
						.into()
				)
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
			assert_eq!(
				EncointerReputationCommitments::commitments(1, (&alice, cid, 11)),
				None
			);
			assert_eq!(
				last_event::<TestRuntime>(),
				Some(
					Event::CommitedReputation(alice.clone(), cid, 11, 1, None)
						.into()
				)
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
				EncointerReputationCommitments::commitments(0, (&alice, cid2, 12)),
				Some(hash)
			);
			assert_eq!(
				last_event::<TestRuntime>(),
				Some(
					Event::CommitedReputation(
						alice.clone(),
						cid2,
						12,
						0,
						Some(hash)
					)
					.into()
				)
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
