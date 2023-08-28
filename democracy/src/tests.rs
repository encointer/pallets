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

use crate::mock::Timestamp;

use frame_support::traits::OnFinalize;

use encointer_primitives::{
	ceremonies::Reputation,
	communities::{CommunityIdentifier, NominalIncome as NominalIncomeType},
	democracy::{ProposalAction, ProposalActionIdentifier, ProposalState, Tally, Vote},
};
use frame_support::{assert_err, assert_ok, traits::OnInitialize};
use frame_system::pallet_prelude::BlockNumberFor;
use mock::{new_test_ext, EncointerDemocracy, RuntimeOrigin, System, TestRuntime};
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

type BlockNumber = BlockNumberFor<TestRuntime>;

fn advance_n_blocks(n: u64) {
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
			RuntimeOrigin::signed(alice()),
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

		let proposal_action = ProposalAction::SetInactivityTimeout(8);
		assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action
		));

		EncointerCeremonies::fake_reputation((cid, 4), &alice, Reputation::Unverified);
		assert!(EncointerDemocracy::valid_reputations(
			1,
			&alice,
			&BoundedVec::try_from(vec![(cid, 4)]).unwrap(),
			None
		)
		.unwrap()
		.is_empty());

		EncointerCeremonies::fake_reputation((cid, 5), &alice, Reputation::UnverifiedReputable);
		assert!(EncointerDemocracy::valid_reputations(
			1,
			&alice,
			&BoundedVec::try_from(vec![(cid, 5)]).unwrap(),
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

		EncointerCeremonies::fake_reputation((cid, 2), &alice, Reputation::VerifiedLinked);
		assert_eq!(
			EncointerDemocracy::valid_reputations(
				1,
				&alice,
				&BoundedVec::try_from(vec![(cid, 2)]).unwrap(),
				None
			)
			.unwrap()
			.len(),
			1
		);

		let valid_reputations = EncointerDemocracy::valid_reputations(
			1,
			&alice,
			&BoundedVec::try_from(vec![(cid, 5), (cid, 4), (cid, 3), (cid, 2)]).unwrap(),
			None,
		)
		.unwrap();
		assert_eq!(valid_reputations.len(), 2);

		assert_eq!(valid_reputations.first().unwrap().1, 3);

		assert_eq!(valid_reputations.last().unwrap().1, 2);
	});
}

#[test]
fn valid_reputations_works_with_used_reputations() {
	new_test_ext().execute_with(|| {
		let cid = create_cid();
		let alice = alice();

		let proposal_action = ProposalAction::SetInactivityTimeout(8);
		assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action
		));

		EncointerCeremonies::fake_reputation((cid, 5), &alice, Reputation::VerifiedLinked);
		// use this reputation for a vote
		VoteEntries::<TestRuntime>::insert(1, (alice.clone(), (cid, 5)), ());

		EncointerCeremonies::fake_reputation((cid, 4), &alice, Reputation::VerifiedLinked);

		let valid_reputations = EncointerDemocracy::valid_reputations(
			1,
			&alice,
			&BoundedVec::try_from(vec![(cid, 5), (cid, 4)]).unwrap(),
			None,
		)
		.unwrap();
		assert_eq!(valid_reputations.len(), 1);
		assert_eq!(valid_reputations.first().unwrap().1, 4);
	});
}

#[test]
fn valid_reputations_works_with_inexistent_reputations() {
	new_test_ext().execute_with(|| {
		let cid = create_cid();
		let alice = alice();

		let proposal_action = ProposalAction::SetInactivityTimeout(8);
		assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action
		));

		EncointerCeremonies::fake_reputation((cid, 4), &alice, Reputation::VerifiedLinked);

		let valid_reputations = EncointerDemocracy::valid_reputations(
			1,
			&alice,
			&BoundedVec::try_from(vec![(cid, 4), (cid, 5)]).unwrap(),
			None,
		)
		.unwrap();
		assert_eq!(valid_reputations.len(), 1);
		assert_eq!(valid_reputations.first().unwrap().1, 4);
	});
}

#[test]
fn valid_reputations_works_with_cids() {
	new_test_ext().execute_with(|| {
		let cid = create_cid();
		let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);
		let alice = alice();

		let proposal_action = ProposalAction::SetInactivityTimeout(8);
		assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action
		));

		EncointerCeremonies::fake_reputation((cid, 5), &alice, Reputation::VerifiedLinked);
		EncointerCeremonies::fake_reputation((cid2, 5), &alice, Reputation::VerifiedLinked);

		let valid_reputations = EncointerDemocracy::valid_reputations(
			1,
			&alice,
			&BoundedVec::try_from(vec![(cid, 5), (cid2, 5)]).unwrap(),
			Some(cid2),
		)
		.unwrap();
		assert_eq!(valid_reputations.len(), 1);
		assert_eq!(valid_reputations.first().unwrap(), &(cid2, 5u32));
	});
}

#[test]
fn valid_reputations_fails_with_invalid_cindex() {
	new_test_ext().execute_with(|| {
		let cid = create_cid();
		let alice = alice();

		let proposal_action = ProposalAction::SetInactivityTimeout(8);
		assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action
		));

		EncointerCeremonies::fake_reputation((cid, 1), &alice, Reputation::VerifiedLinked);
		EncointerCeremonies::fake_reputation((cid, 4), &alice, Reputation::VerifiedLinked);
		EncointerCeremonies::fake_reputation((cid, 6), &alice, Reputation::VerifiedLinked);

		let valid_reputations = EncointerDemocracy::valid_reputations(
			1,
			&alice,
			&BoundedVec::try_from(vec![(cid, 1), (cid, 4), (cid, 6)]).unwrap(),
			Some(cid),
		)
		.unwrap();
		assert_eq!(valid_reputations.len(), 1);
		assert_eq!(valid_reputations.first().unwrap(), &(cid, 4u32));
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
				RuntimeOrigin::signed(alice.clone()),
				1,
				Vote::Aye,
				BoundedVec::try_from(vec![(cid, 1), (cid, 2), (cid, 3)]).unwrap()
			),
			Error::<TestRuntime>::InexistentProposal
		);

		assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action.clone()
		));

		assert_ok!(EncointerDemocracy::vote(
			RuntimeOrigin::signed(alice.clone()),
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
			RuntimeOrigin::signed(alice.clone()),
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

#[test]
fn update_proposal_state_fails_with_inexistent_proposal() {
	new_test_ext().execute_with(|| {
		assert_err!(
			EncointerDemocracy::update_proposal_state(1),
			Error::<TestRuntime>::InexistentProposal
		);
	});
}

#[test]
fn update_proposal_state_fails_with_wrong_state() {
	new_test_ext().execute_with(|| {
		let cid = create_cid();
		let proposal: Proposal<BlockNumber> = Proposal {
			start: BlockNumber::from(1u64),
			start_cindex: 1,
			action: ProposalAction::UpdateNominalIncome(cid, NominalIncomeType::from(100i32)),
			state: ProposalState::Cancelled,
		};
		Proposals::<TestRuntime>::insert(1, proposal);

		let proposal2: Proposal<BlockNumber> = Proposal {
			start: BlockNumber::from(1u64),
			start_cindex: 1,
			action: ProposalAction::UpdateNominalIncome(cid, NominalIncomeType::from(100i32)),
			state: ProposalState::Approved,
		};
		Proposals::<TestRuntime>::insert(2, proposal2);

		assert_err!(
			EncointerDemocracy::update_proposal_state(1),
			Error::<TestRuntime>::ProposalCannotBeUpdated
		);

		assert_err!(
			EncointerDemocracy::update_proposal_state(2),
			Error::<TestRuntime>::ProposalCannotBeUpdated
		);
	});
}

#[test]
fn update_proposal_state_works_with_cancelled_proposal() {
	new_test_ext().execute_with(|| {
		let proposal_action = ProposalAction::SetInactivityTimeout(8);

		assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice()),
			proposal_action
		));

		CancelledAtBlock::<TestRuntime>::insert(ProposalActionIdentifier::SetInactivityTimeout, 3);

		assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Ongoing);

		advance_n_blocks(5);

		assert_ok!(EncointerDemocracy::update_proposal_state(1));

		assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Cancelled);
	});
}

#[test]
fn update_proposal_state_works_with_too_old_proposal() {
	new_test_ext().execute_with(|| {
		let proposal_action = ProposalAction::SetInactivityTimeout(8);

		assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice()),
			proposal_action
		));

		assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Ongoing);

		advance_n_blocks(40);

		assert_ok!(EncointerDemocracy::update_proposal_state(1));
		assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Ongoing);

		advance_n_blocks(1);

		assert_ok!(EncointerDemocracy::update_proposal_state(1));
		assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Cancelled);
	});
}

#[test]
fn update_proposal_state_works() {
	new_test_ext().execute_with(|| {
		let proposal_action = ProposalAction::SetInactivityTimeout(8);

		assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice()),
			proposal_action
		));

		assert_ok!(EncointerDemocracy::update_proposal_state(1));
		assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Ongoing);

		// propsal is passing
		Tallies::<TestRuntime>::insert(1, Tally { turnout: 100, ayes: 100 });

		assert_eq!(EncointerDemocracy::update_proposal_state(1).unwrap(), false);
		assert_eq!(
			EncointerDemocracy::proposals(1).unwrap().state,
			ProposalState::Confirming { since: 0 }
		);

		// not passing anymore
		Tallies::<TestRuntime>::insert(1, Tally { turnout: 100, ayes: 0 });

		assert_eq!(EncointerDemocracy::update_proposal_state(1).unwrap(), false);
		assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Ongoing);

		// nothing changes if repeated
		assert_eq!(EncointerDemocracy::update_proposal_state(1).unwrap(), false);
		assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Ongoing);

		// passing
		Tallies::<TestRuntime>::insert(1, Tally { turnout: 100, ayes: 100 });

		assert_eq!(EncointerDemocracy::update_proposal_state(1).unwrap(), false);
		assert_eq!(
			EncointerDemocracy::proposals(1).unwrap().state,
			ProposalState::Confirming { since: 0 }
		);

		advance_n_blocks(11);
		// proposal is enacted
		assert_eq!(EncointerDemocracy::update_proposal_state(1).unwrap(), true);
		assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Approved);
	});
}
