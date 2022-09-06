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

//! Unit tests for the tokens module.

use super::*;
use crate::mock::{EncointerCeremonies, EncointerScheduler};
use encointer_primitives::{
	ceremonies::Reputation,
	communities::{CommunityIdentifier, NominalIncome as NominalIncomeType},
	democracy::{ProposalAction, ProposalState, Vote, VoteEntry},
};
use frame_support::{assert_err, assert_ok, traits::OnInitialize};
use mock::{new_test_ext, EncointerDemocracy, Origin, System, TestRuntime};
use sp_runtime::BoundedVec;
use test_utils::{helpers::register_test_community, *};

fn create_cid() -> CommunityIdentifier {
	return register_test_community::<TestRuntime>(None, 0.0, 0.0)
}

fn alice() -> AccountId {
	AccountKeyring::Alice.into()
}

fn bob() -> AccountId {
	AccountKeyring::Bob.into()
}

type BlockNumber = <TestRuntime as frame_system::Config>::BlockNumber;

fn advance_n_blocks(n: u64) {
	let target_block = System::block_number() + n;
	for _ in 0..n {
		System::set_block_number(System::block_number() + 1);
		System::on_initialize(System::block_number());
	}
}

#[test]
fn proposal_submission_works() {
	new_test_ext().execute_with(|| {
		let cid = create_cid();
		let block = System::block_number();
		let proposal_action =
			ProposalAction::UpdateNominalIncome(cid, NominalIncomeType::from(100i32));

		assert_ok!(EncointerDemocracy::submit_proposal(
			Origin::signed(alice()),
			proposal_action.clone()
		));
		assert_eq!(EncointerDemocracy::proposal_count(), 1);
		let proposal = EncointerDemocracy::proposals(1).unwrap();
		assert_eq!(proposal.state, ProposalState::Ongoing);
		assert_eq!(proposal.action, proposal_action);
		assert_eq!(proposal.start, block);
		assert!(EncointerDemocracy::tallies(1).is_some());
	});
}

#[test]
fn valid_reputations_works_with_different_reputations() {
	new_test_ext().execute_with(|| {
		let cid = create_cid();
		let alice = alice();

		EncointerCeremonies::fake_reputation((cid, 1), &alice, Reputation::Unverified);
		assert!(EncointerDemocracy::valid_reputations(
			1,
			&alice,
			&BoundedVec::try_from(vec![(cid, 1)]).unwrap(),
			None
		)
		.unwrap()
		.is_empty());

		EncointerCeremonies::fake_reputation((cid, 2), &alice, Reputation::UnverifiedReputable);
		assert!(EncointerDemocracy::valid_reputations(
			1,
			&alice,
			&BoundedVec::try_from(vec![(cid, 2)]).unwrap(),
			None
		)
		.unwrap()
		.is_empty());

		EncointerCeremonies::fake_reputation((cid, 3), &alice, Reputation::VerifiedUnlinked);
		assert_eq!(
			EncointerDemocracy::valid_reputations(
				1,
				&alice,
				&BoundedVec::try_from(vec![(cid, 3)]).unwrap(),
				None
			)
			.unwrap()
			.len(),
			1
		);

		EncointerCeremonies::fake_reputation((cid, 4), &alice, Reputation::VerifiedLinked);
		assert_eq!(
			EncointerDemocracy::valid_reputations(
				1,
				&alice,
				&BoundedVec::try_from(vec![(cid, 4)]).unwrap(),
				None
			)
			.unwrap()
			.len(),
			1
		);

		let valid_reputations = EncointerDemocracy::valid_reputations(
			1,
			&alice,
			&BoundedVec::try_from(vec![(cid, 1), (cid, 2), (cid, 3), (cid, 4)]).unwrap(),
			None,
		)
		.unwrap();
		assert_eq!(valid_reputations.len(), 2);

		assert_eq!(valid_reputations.first().unwrap().1, 3);

		assert_eq!(valid_reputations.last().unwrap().1, 4);
	});
}

#[test]
fn valid_reputations_works_with_used_reputations() {
	new_test_ext().execute_with(|| {
		let cid = create_cid();
		let alice = alice();

		EncointerCeremonies::fake_reputation((cid, 1), &alice, Reputation::VerifiedLinked);
		// use this reputation for a vote
		VoteEntries::<TestRuntime>::insert(1, (alice.clone(), (cid, 1)), ());

		EncointerCeremonies::fake_reputation((cid, 2), &alice, Reputation::VerifiedLinked);

		let valid_reputations = EncointerDemocracy::valid_reputations(
			1,
			&alice,
			&BoundedVec::try_from(vec![(cid, 1), (cid, 2)]).unwrap(),
			None,
		)
		.unwrap();
		assert_eq!(valid_reputations.len(), 1);
		assert_eq!(valid_reputations.first().unwrap().1, 2);
	});
}

#[test]
fn valid_reputations_works_with_inexistent_reputations() {
	new_test_ext().execute_with(|| {
		let cid = create_cid();
		let alice = alice();

		EncointerCeremonies::fake_reputation((cid, 1), &alice, Reputation::VerifiedLinked);

		let valid_reputations = EncointerDemocracy::valid_reputations(
			1,
			&alice,
			&BoundedVec::try_from(vec![(cid, 1), (cid, 2)]).unwrap(),
			None,
		)
		.unwrap();
		assert_eq!(valid_reputations.len(), 1);
		assert_eq!(valid_reputations.first().unwrap().1, 1);
	});
}

#[test]
fn valid_reputations_works_with_cids() {
	new_test_ext().execute_with(|| {
		let cid = create_cid();
		let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);
		let alice = alice();

		EncointerCeremonies::fake_reputation((cid, 1), &alice, Reputation::VerifiedLinked);
		EncointerCeremonies::fake_reputation((cid2, 1), &alice, Reputation::VerifiedLinked);

		let valid_reputations = EncointerDemocracy::valid_reputations(
			1,
			&alice,
			&BoundedVec::try_from(vec![(cid, 1), (cid2, 1)]).unwrap(),
			Some(cid2),
		)
		.unwrap();
		assert_eq!(valid_reputations.len(), 1);
		assert_eq!(valid_reputations.first().unwrap(), &(cid2, 1u32));
	});
}

#[test]
fn voting_works() {
	new_test_ext().execute_with(|| {
		let cid = create_cid();
		let alice = alice();
		let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);

		let proposal_action =
			ProposalAction::UpdateNominalIncome(cid, NominalIncomeType::from(100i32));

		EncointerCeremonies::fake_reputation((cid, 1), &alice, Reputation::Unverified);
		EncointerCeremonies::fake_reputation((cid, 2), &alice, Reputation::VerifiedLinked);
		EncointerCeremonies::fake_reputation((cid, 3), &alice, Reputation::VerifiedLinked);

		assert_err!(
			EncointerDemocracy::vote(
				Origin::signed(alice.clone()),
				1,
				Vote::Aye,
				BoundedVec::try_from(vec![(cid, 1), (cid, 2), (cid, 3)]).unwrap()
			),
			Error::<TestRuntime>::InexistentProposal
		);

		assert_ok!(EncointerDemocracy::submit_proposal(
			Origin::signed(alice.clone()),
			proposal_action.clone()
		));

		assert_ok!(EncointerDemocracy::vote(
			Origin::signed(alice.clone()),
			1,
			Vote::Aye,
			BoundedVec::try_from(vec![(cid, 1), (cid, 2), (cid, 3)]).unwrap()
		));

		let mut tally = EncointerDemocracy::tallies(1).unwrap();
		assert_eq!(tally.turnout, 2);
		assert_eq!(tally.ayes, 2);

		EncointerCeremonies::fake_reputation((cid, 4), &alice, Reputation::Unverified);
		EncointerCeremonies::fake_reputation((cid, 5), &alice, Reputation::VerifiedLinked);
		EncointerCeremonies::fake_reputation((cid2, 6), &alice, Reputation::VerifiedLinked);

		assert_ok!(EncointerDemocracy::vote(
			Origin::signed(alice.clone()),
			1,
			Vote::Nay,
			// 3 is invalid because already used
			// 4 is invalid because unverified
			// 6 and 7 is invalid because of wrong cid
			// 8 is invalid because it does not exist
			BoundedVec::try_from(vec![
				(cid, 3),
				(cid, 4),
				(cid, 5),
				(cid2, 6),
				(cid2, 7),
				(cid, 8)
			])
			.unwrap()
		));

		tally = EncointerDemocracy::tallies(1).unwrap();
		assert_eq!(tally.turnout, 3);
		assert_eq!(tally.ayes, 2);
	});
}
