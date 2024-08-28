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

//! Unit tests for the encointer democracy module.

use super::*;
use crate::mock::{
    EncointerBalances, EncointerCeremonies, EncointerCommunities, EncointerScheduler, EncointerTreasuries, Balances, Timestamp,
};
use encointer_primitives::{
    balances::Demurrage,
    ceremonies::{InactivityTimeoutType, Reputation},
    common::{FromStr, PalletString},
    communities::{
        CommunityIdentifier, CommunityMetadata as CommunityMetadataType, Degree, GeoHash, Location,
        NominalIncome as NominalIncomeType,
    },
    democracy::{ProposalAction, ProposalActionIdentifier, ProposalState, Tally, Vote},
};
use frame_support::{
    assert_err, assert_ok,
    traits::{OnFinalize, OnInitialize},
};
use mock::{new_test_ext, EncointerDemocracy, RuntimeOrigin, System, TestRuntime};
use sp_runtime::BoundedVec;
use std::str::FromStr as StdFromStr;
use test_utils::{
    helpers::{
        account_id, add_population, event_at_index, get_num_events, last_event,
        register_test_community,
    },
    *,
};

fn create_cid() -> CommunityIdentifier {
    return register_test_community::<TestRuntime>(None, 0.0, 0.0);
}

fn alice() -> AccountId {
    AccountKeyring::Alice.into()
}

fn bob() -> AccountId {
    AccountKeyring::Bob.into()
}

fn advance_n_blocks(n: u64) {
    let mut blocknr = System::block_number();
    for _ in 0..n {
        blocknr += 1;
        run_to_block(blocknr);
    }
}

pub fn set_timestamp(t: u64) {
    let _ = pallet_timestamp::Pallet::<TestRuntime>::set(RuntimeOrigin::none(), t);
}

/// Run until a particular block.
fn run_to_block(n: u64) {
    while System::block_number() < n {
        if System::block_number() > 1 {
            System::on_finalize(System::block_number());
        }
        set_timestamp(Timestamp::get() + BLOCKTIME);
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
fn proposal_submission_works() {
    new_test_ext().execute_with(|| {
        let cid = create_cid();
        let now = Timestamp::get();
        let alice = alice();

        // invalid
        EncointerCeremonies::fake_reputation((cid, 2), &alice, Reputation::VerifiedLinked(0));
        // valid
        EncointerCeremonies::fake_reputation((cid, 3), &alice, Reputation::VerifiedLinked(0));
        EncointerCeremonies::fake_reputation((cid, 4), &alice, Reputation::VerifiedLinked(0));
        EncointerCeremonies::fake_reputation((cid, 5), &alice, Reputation::VerifiedLinked(0));

        let proposal_action =
            ProposalAction::UpdateNominalIncome(cid, NominalIncomeType::from(100u32));

        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice),
			proposal_action.clone()
		));
        assert_eq!(EncointerDemocracy::proposal_count(), 1);
        let proposal = EncointerDemocracy::proposals(1).unwrap();
        assert_eq!(proposal.state, ProposalState::Ongoing);
        assert_eq!(proposal.action, proposal_action);
        assert_eq!(proposal.start, now);
        assert_eq!(proposal.electorate_size, 3);
        assert!(EncointerDemocracy::tallies(1).is_some());
    });
}

#[test]
fn proposal_submission_fails_if_proposal_in_enactment_queue() {
    new_test_ext().execute_with(|| {
        let cid = create_cid();
        let proposal_action =
            ProposalAction::UpdateNominalIncome(cid, NominalIncomeType::from(100u32));

        EnactmentQueue::<TestRuntime>::insert(proposal_action.clone().get_identifier(), 100);

        assert_err!(
			EncointerDemocracy::submit_proposal(
				RuntimeOrigin::signed(alice()),
				proposal_action.clone()
			),
			Error::<TestRuntime>::ProposalWaitingForEnactment
		);
    });
}

#[test]
fn eligible_reputations_works_with_different_reputations() {
    new_test_ext().execute_with(|| {
        let cid = create_cid();
        let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);
        let alice = alice();

        let proposal_action = ProposalAction::SetInactivityTimeout(8);
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action
		));

        EncointerCeremonies::fake_reputation((cid, 4), &alice, Reputation::Unverified);
        assert_eq!(
            EncointerDemocracy::validate_and_commit_reputations(
                1,
                &alice,
                &BoundedVec::try_from(vec![(cid, 4)]).unwrap(),
            ),
            Ok(0)
        );

        EncointerCeremonies::fake_reputation((cid, 5), &alice, Reputation::UnverifiedReputable);
        assert_eq!(
            EncointerDemocracy::validate_and_commit_reputations(
                1,
                &alice,
                &BoundedVec::try_from(vec![(cid, 5)]).unwrap(),
            ),
            Ok(0)
        );

        EncointerCeremonies::fake_reputation((cid2, 4), &alice, Reputation::VerifiedUnlinked);
        assert_eq!(
            EncointerDemocracy::validate_and_commit_reputations(
                1,
                &alice,
                &BoundedVec::try_from(vec![(cid2, 4)]).unwrap(),
            ),
            Ok(1)
        );

        EncointerCeremonies::fake_reputation((cid2, 3), &alice, Reputation::VerifiedLinked(0));
        assert_eq!(
            EncointerDemocracy::validate_and_commit_reputations(
                1,
                &alice,
                &BoundedVec::try_from(vec![(cid2, 3)]).unwrap(),
            ),
            Ok(1)
        );

        let cid3 = register_test_community::<TestRuntime>(None, 20.0, 10.0);
        let cid4 = register_test_community::<TestRuntime>(None, 10.0, 20.0);

        EncointerCeremonies::fake_reputation((cid3, 4), &alice, Reputation::Unverified);
        EncointerCeremonies::fake_reputation((cid3, 5), &alice, Reputation::UnverifiedReputable);
        EncointerCeremonies::fake_reputation((cid4, 4), &alice, Reputation::VerifiedUnlinked);
        EncointerCeremonies::fake_reputation((cid4, 3), &alice, Reputation::VerifiedLinked(0));
        assert_eq!(
            EncointerDemocracy::validate_and_commit_reputations(
                1,
                &alice,
                &BoundedVec::try_from(vec![(cid3, 5), (cid3, 4), (cid4, 4), (cid4, 3)]).unwrap(),
            ),
            Ok(2)
        );
    });
}

#[test]
fn eligible_reputations_works_with_used_reputations() {
    new_test_ext().execute_with(|| {
        let cid = create_cid();
        let alice = alice();

        let proposal_action = ProposalAction::SetInactivityTimeout(8);
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action
		));

        EncointerCeremonies::fake_reputation((cid, 5), &alice, Reputation::VerifiedLinked(0));

        // commit reputation
        EncointerDemocracy::validate_and_commit_reputations(
            1,
            &alice,
            &BoundedVec::try_from(vec![(cid, 5)]).unwrap(),
        )
            .ok();

        EncointerCeremonies::fake_reputation((cid, 4), &alice, Reputation::VerifiedLinked(0));

        assert_eq!(
            EncointerDemocracy::validate_and_commit_reputations(
                1,
                &alice,
                &BoundedVec::try_from(vec![(cid, 5), (cid, 4)]).unwrap(),
            ),
            Ok(1)
        )
    });
}

#[test]
fn eligible_reputations_works_with_inexistent_reputations() {
    new_test_ext().execute_with(|| {
        let cid = create_cid();
        let alice = alice();

        let proposal_action = ProposalAction::SetInactivityTimeout(8);
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action
		));

        EncointerCeremonies::fake_reputation((cid, 4), &alice, Reputation::VerifiedLinked(0));

        assert_eq!(
            EncointerDemocracy::validate_and_commit_reputations(
                1,
                &alice,
                &BoundedVec::try_from(vec![(cid, 5), (cid, 4)]).unwrap(),
            ),
            Ok(1)
        )
    });
}

#[test]
fn eligible_reputations_works_with_cids() {
    new_test_ext().execute_with(|| {
        let cid = create_cid();
        let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);
        let alice = alice();

        let proposal_action =
            ProposalAction::UpdateNominalIncome(cid, NominalIncomeType::from(100u32));
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action
		));

        EncointerCeremonies::fake_reputation((cid, 5), &alice, Reputation::VerifiedLinked(0));
        EncointerCeremonies::fake_reputation((cid2, 5), &alice, Reputation::VerifiedLinked(0));

        assert_eq!(
            EncointerDemocracy::validate_and_commit_reputations(
                1,
                &alice,
                &BoundedVec::try_from(vec![(cid, 5), (cid2, 5)]).unwrap(),
            ),
            Ok(1)
        )
    });
}

#[test]
fn eligible_reputations_fails_with_invalid_cindex() {
    new_test_ext().execute_with(|| {
        let cid = create_cid();
        let alice = alice();

        let proposal_action = ProposalAction::SetInactivityTimeout(8);
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action
		));

        EncointerCeremonies::fake_reputation((cid, 1), &alice, Reputation::VerifiedLinked(0));
        EncointerCeremonies::fake_reputation((cid, 4), &alice, Reputation::VerifiedLinked(0));
        EncointerCeremonies::fake_reputation((cid, 6), &alice, Reputation::VerifiedLinked(0));

        assert_eq!(
            EncointerDemocracy::validate_and_commit_reputations(
                1,
                &alice,
                &BoundedVec::try_from(vec![(cid, 1), (cid, 4), (cid, 6)]).unwrap(),
            ),
            Ok(1)
        )
    });
}

#[test]
fn voting_works() {
    new_test_ext().execute_with(|| {
        let cid = create_cid();
        let alice = alice();
        let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);

        let proposal_action =
            ProposalAction::SetInactivityTimeout(InactivityTimeoutType::from(100u32));

        EncointerCeremonies::fake_reputation((cid, 3), &alice, Reputation::Unverified);
        EncointerCeremonies::fake_reputation((cid, 4), &alice, Reputation::VerifiedLinked(0));
        EncointerCeremonies::fake_reputation((cid, 5), &alice, Reputation::VerifiedLinked(0));

        assert_err!(
			EncointerDemocracy::vote(
				RuntimeOrigin::signed(alice.clone()),
				1,
				Vote::Aye,
				BoundedVec::try_from(vec![(cid, 3), (cid, 4), (cid, 5)]).unwrap()
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
			BoundedVec::try_from(vec![(cid, 3), (cid, 4), (cid, 5)]).unwrap()
		));

        let mut tally = EncointerDemocracy::tallies(1).unwrap();
        assert_eq!(tally.turnout, 2);
        assert_eq!(tally.ayes, 2);

        EncointerCeremonies::fake_reputation((cid2, 4), &alice, Reputation::Unverified);
        EncointerCeremonies::fake_reputation((cid2, 5), &alice, Reputation::VerifiedLinked(0));
        EncointerCeremonies::fake_reputation((cid, 2), &alice, Reputation::VerifiedLinked(0));
        EncointerCeremonies::fake_reputation((cid2, 6), &alice, Reputation::VerifiedLinked(0));

        assert_ok!(EncointerDemocracy::vote(
			RuntimeOrigin::signed(alice.clone()),
			1,
			Vote::Nay,
			BoundedVec::try_from(vec![
				(cid, 2),  // invalid because out of range
				(cid, 4),  // invalid because already used
				(cid2, 4), // invalid because unverified
				(cid2, 5), // valid
				(cid2, 6), // invalid because out of range
				(cid2, 3), // invlalid non-existent
			])
			.unwrap()
		));

        tally = EncointerDemocracy::tallies(1).unwrap();
        assert_eq!(tally.turnout, 3);
        assert_eq!(tally.ayes, 2);
    });
}

#[test]
fn do_update_proposal_state_fails_with_inexistent_proposal() {
    new_test_ext().execute_with(|| {
        assert_err!(
			EncointerDemocracy::do_update_proposal_state(1),
			Error::<TestRuntime>::InexistentProposal
		);
    });
}

#[test]
fn do_update_proposal_state_fails_with_wrong_state() {
    new_test_ext().execute_with(|| {
        let cid = create_cid();
        let proposal: Proposal<Moment, AccountId, Balance> = Proposal {
            start: Moment::from(1u64),
            start_cindex: 1,
            action: ProposalAction::UpdateNominalIncome(cid, NominalIncomeType::from(100u32)),
            state: ProposalState::Rejected,
            electorate_size: 0,
        };
        Proposals::<TestRuntime>::insert(1, proposal);

        let proposal2: Proposal<Moment, AccountId, Balance> = Proposal {
            start: Moment::from(1u64),
            start_cindex: 1,
            action: ProposalAction::UpdateNominalIncome(cid, NominalIncomeType::from(100u32)),
            state: ProposalState::Approved,
            electorate_size: 0,
        };
        Proposals::<TestRuntime>::insert(2, proposal2);

        let proposal3: Proposal<Moment, AccountId, Balance> = Proposal {
            start: Moment::from(1u64),
            start_cindex: 1,
            action: ProposalAction::UpdateNominalIncome(cid, NominalIncomeType::from(100u32)),
            state: ProposalState::SupersededBy { id: 2 },
            electorate_size: 0,
        };
        Proposals::<TestRuntime>::insert(3, proposal3);

        assert_err!(
			EncointerDemocracy::do_update_proposal_state(1),
			Error::<TestRuntime>::ProposalCannotBeUpdated
		);

        assert_err!(
			EncointerDemocracy::do_update_proposal_state(2),
			Error::<TestRuntime>::ProposalCannotBeUpdated
		);

        assert_err!(
			EncointerDemocracy::do_update_proposal_state(3),
			Error::<TestRuntime>::ProposalCannotBeUpdated
		);
    });
}

#[test]
fn do_update_proposal_state_cancels_superseded_proposal() {
    new_test_ext().execute_with(|| {
        let proposal_action = ProposalAction::SetInactivityTimeout(8);

        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice()),
			proposal_action
		));

        //another proposal of same action has been scheduled for enactment
        LastApprovedProposalForAction::<TestRuntime>::insert(
            ProposalActionIdentifier::SetInactivityTimeout,
            (3 * BLOCKTIME, 2),
        );

        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Ongoing);

        advance_n_blocks(5);

        assert_ok!(EncointerDemocracy::do_update_proposal_state(1));

        assert_eq!(
            EncointerDemocracy::proposals(1).unwrap().state,
            ProposalState::SupersededBy { id: 2 }
        );
    });
}

#[test]
fn do_update_proposal_state_does_not_cancel_no_supersession_actions() {
    new_test_ext().execute_with(|| {
        let cid = create_cid();
        let petition1_text = PalletString::try_from("freedom for all".as_bytes().to_vec()).unwrap();
        let proposal1_action = ProposalAction::Petition(Some(cid), petition1_text.clone());

        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice()),
			proposal1_action
		));

        //another proposal of same action has been scheduled for enactment
        LastApprovedProposalForAction::<TestRuntime>::insert(
            ProposalActionIdentifier::Petition(Some(cid)),
            (3 * BLOCKTIME, 2),
        );

        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Ongoing);

        advance_n_blocks(5);

        assert_ok!(EncointerDemocracy::do_update_proposal_state(1));

        // we do not want petitions to cancel others
        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Ongoing);
    });
}

#[test]
fn do_update_proposal_state_works_with_too_old_proposal() {
    new_test_ext().execute_with(|| {
        let proposal_action = ProposalAction::SetInactivityTimeout(8);

        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice()),
			proposal_action
		));

        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Ongoing);

        advance_n_blocks(40);

        assert_ok!(EncointerDemocracy::do_update_proposal_state(1));
        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Ongoing);
        advance_n_blocks(1);

        assert_ok!(EncointerDemocracy::do_update_proposal_state(1));
        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Rejected);
    });
}

#[test]
fn do_update_proposal_state_works() {
    new_test_ext().execute_with(|| {
        let proposal_action = ProposalAction::SetInactivityTimeout(8);

        let alice = alice();
        let cid = register_test_community::<TestRuntime>(None, 10.0, 10.0);

        // make sure electorate is 100
        let pairs = add_population(100, 0);
        for p in pairs {
            EncointerCeremonies::fake_reputation(
                (cid, 5),
                &account_id(&p),
                Reputation::VerifiedLinked(0),
            );
        }

        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice),
			proposal_action.clone()
		));

        assert_ok!(EncointerDemocracy::do_update_proposal_state(1));
        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Ongoing);

        // propsal is passing
        Tallies::<TestRuntime>::insert(1, Tally { turnout: 100, ayes: 100 });

        assert_eq!(EncointerDemocracy::do_update_proposal_state(1).unwrap(), false);
        assert_eq!(
            EncointerDemocracy::proposals(1).unwrap().state,
            ProposalState::Confirming { since: 0 }
        );

        // not passing anymore
        Tallies::<TestRuntime>::insert(1, Tally { turnout: 100, ayes: 0 });

        assert_eq!(EncointerDemocracy::do_update_proposal_state(1).unwrap(), false);
        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Ongoing);

        // nothing changes if repeated
        assert_eq!(EncointerDemocracy::do_update_proposal_state(1).unwrap(), false);
        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Ongoing);

        // passing
        Tallies::<TestRuntime>::insert(1, Tally { turnout: 100, ayes: 100 });

        assert_eq!(EncointerDemocracy::do_update_proposal_state(1).unwrap(), false);
        assert_eq!(
            EncointerDemocracy::proposals(1).unwrap().state,
            ProposalState::Confirming { since: 0 }
        );

        assert_eq!(
            EncointerDemocracy::enactment_queue(proposal_action.clone().get_identifier()),
            None
        );
        // should even work if proposal is too old before update is called.
        advance_n_blocks(41);
        // proposal is enacted
        assert_eq!(EncointerDemocracy::do_update_proposal_state(1).unwrap(), true);
        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Approved);
        assert_eq!(
            EncointerDemocracy::enactment_queue(proposal_action.clone().get_identifier()).unwrap(),
            1
        );
    });
}

#[test]
fn update_proposal_state_extrinsic_works() {
    new_test_ext().execute_with(|| {
        let proposal_action = ProposalAction::SetInactivityTimeout(8);

        let alice = alice();
        let cid = register_test_community::<TestRuntime>(None, 10.0, 10.0);

        EncointerCeremonies::fake_reputation((cid, 3), &alice, Reputation::VerifiedLinked(0));
        EncointerCeremonies::fake_reputation((cid, 4), &alice, Reputation::VerifiedLinked(0));
        EncointerCeremonies::fake_reputation((cid, 5), &alice, Reputation::VerifiedLinked(0));

        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action
		));

        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Ongoing);
        // propsal is passing
        Tallies::<TestRuntime>::insert(1, Tally { turnout: 3, ayes: 3 });
        EncointerDemocracy::update_proposal_state(RuntimeOrigin::signed(alice), 1).unwrap();
        assert_eq!(
            EncointerDemocracy::proposals(1).unwrap().state,
            ProposalState::Confirming { since: 0 }
        );
    });
}

#[test]
fn test_get_electorate_works() {
    new_test_ext().execute_with(|| {
        let cid = create_cid();
        let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);
        let alice = alice();
        let bob = bob();

        EncointerCeremonies::fake_reputation((cid, 4), &alice, Reputation::VerifiedLinked(0));
        EncointerCeremonies::fake_reputation((cid, 5), &alice, Reputation::VerifiedLinked(0));
        EncointerCeremonies::fake_reputation((cid2, 3), &bob, Reputation::VerifiedLinked(0));
        EncointerCeremonies::fake_reputation((cid2, 4), &bob, Reputation::VerifiedLinked(0));
        EncointerCeremonies::fake_reputation((cid2, 5), &bob, Reputation::VerifiedLinked(0));

        let proposal_action = ProposalAction::SetInactivityTimeout(8);
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action.clone()
		));

        let proposal_action2 =
            ProposalAction::UpdateNominalIncome(cid, NominalIncomeType::from(100u32));
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action.clone()
		));

        assert_eq!(EncointerDemocracy::get_electorate(7, proposal_action).unwrap(), 5);
        assert_eq!(EncointerDemocracy::get_electorate(7, proposal_action2).unwrap(), 2);
    });
}

#[test]
fn is_passing_works() {
    new_test_ext().execute_with(|| {
        let alice = alice();
        let cid = register_test_community::<TestRuntime>(None, 10.0, 10.0);

        // electorate is 100
        let pairs = add_population(100, 0);
        for p in pairs {
            EncointerCeremonies::fake_reputation(
                (cid, 5),
                &account_id(&p),
                Reputation::VerifiedLinked(0),
            );
        }

        let proposal_action = ProposalAction::SetInactivityTimeout(8);
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action
		));

        // turnout below threshold
        Tallies::<TestRuntime>::insert(1, Tally { turnout: 1, ayes: 1 });
        assert_eq!(EncointerDemocracy::is_passing(1).unwrap(), false);

        // low turnout, 60 % approval
        Tallies::<TestRuntime>::insert(1, Tally { turnout: 10, ayes: 6 });
        assert_eq!(EncointerDemocracy::is_passing(1).unwrap(), false);

        // low turnout 90 % approval
        Tallies::<TestRuntime>::insert(1, Tally { turnout: 10, ayes: 9 });
        assert_eq!(EncointerDemocracy::is_passing(1).unwrap(), true);

        // high turnout, 60 % approval
        Tallies::<TestRuntime>::insert(1, Tally { turnout: 100, ayes: 60 });
        assert_eq!(EncointerDemocracy::is_passing(1).unwrap(), true);

        // high turnout 90 % approval
        Tallies::<TestRuntime>::insert(1, Tally { turnout: 100, ayes: 90 });
        assert_eq!(EncointerDemocracy::is_passing(1).unwrap(), true);
    });
}

#[test]
fn enactment_updates_proposal_metadata_and_enactment_queue() {
    new_test_ext().execute_with(|| {
        let cid = create_cid();
        let alice = alice();

        let proposal_action = ProposalAction::SetInactivityTimeout(8);
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action.clone()
		));

        let proposal_action2 =
            ProposalAction::UpdateNominalIncome(cid, NominalIncomeType::from(100u32));
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action2.clone()
		));

        EnactmentQueue::<TestRuntime>::insert(proposal_action.clone().get_identifier(), 1);
        EnactmentQueue::<TestRuntime>::insert(proposal_action2.clone().get_identifier(), 2);

        run_to_next_phase();
        // first assigning phase after proposal lifetime ended

        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Enacted);

        assert_eq!(EncointerDemocracy::proposals(2).unwrap().state, ProposalState::Enacted);

        assert_eq!(EncointerDemocracy::enactment_queue(proposal_action.get_identifier()), None);

        assert_eq!(EncointerDemocracy::enactment_queue(proposal_action2.get_identifier()), None);
    });
}

#[test]
fn proposal_happy_flow() {
    new_test_ext().execute_with(|| {
        System::set_block_number(System::block_number() + 1); // this is needed to assert events
        let cid = create_cid();
        let cid2 = register_test_community::<TestRuntime>(None, 10.0, 10.0);
        let alice = alice();

        EncointerCeremonies::fake_reputation((cid, 3), &alice, Reputation::VerifiedLinked(0));
        EncointerCeremonies::fake_reputation((cid, 4), &alice, Reputation::VerifiedLinked(0));
        EncointerCeremonies::fake_reputation((cid, 5), &alice, Reputation::VerifiedLinked(0));
        EncointerCeremonies::fake_reputation((cid2, 3), &alice, Reputation::VerifiedLinked(0));

        let proposal_action =
            ProposalAction::UpdateNominalIncome(cid, NominalIncomeType::from(13037u32));
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action.clone()
		));
        assert_eq!(
            last_event::<TestRuntime>(),
            Some(
                Event::ProposalSubmitted {
                    proposal_id: 1,
                    proposal_action: proposal_action.clone()
                }
                    .into()
            )
        );

        assert_ok!(EncointerDemocracy::vote(
			RuntimeOrigin::signed(alice.clone()),
			1,
			Vote::Aye,
			BoundedVec::try_from(vec![(cid, 3), (cid, 4), (cid, 5),]).unwrap()
		));

        assert_eq!(
            last_event::<TestRuntime>(),
            Some(Event::VotePlaced { proposal_id: 1, vote: Vote::Aye, num_votes: 3 }.into())
        );

        // let pass the proposal lifetime
        advance_n_blocks(40);

        assert_ok!(EncointerDemocracy::update_proposal_state(
			RuntimeOrigin::signed(alice.clone()),
			1,
		));

        assert_eq!(
            last_event::<TestRuntime>(),
            Some(
                Event::ProposalStateUpdated {
                    proposal_id: 1,
                    proposal_state: ProposalState::Approved
                }
                    .into()
            )
        );

        run_to_next_phase();
        // first assigning phase after proposal lifetime ended

        assert_eq!(
            event_at_index::<TestRuntime>(get_num_events::<TestRuntime>() - 2),
            Some(Event::ProposalEnacted { proposal_id: 1 }.into())
        );

        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Enacted);
        assert_eq!(
            EncointerDemocracy::enactment_queue(proposal_action.clone().get_identifier()),
            None
        );
        assert_eq!(EncointerCommunities::nominal_income(cid), NominalIncomeType::from(13037u32));
    });
}

#[test]
fn enact_add_location_works() {
    new_test_ext().execute_with(|| {
        let cid = create_cid();
        let alice = alice();
        let location = Location { lat: Degree::from_num(10.0), lon: Degree::from_num(10.0) };
        let proposal_action = ProposalAction::AddLocation(cid, location);
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action.clone()
		));

        let geo_hash = GeoHash::try_from_params(location.lat, location.lon).unwrap();

        // directly inject the proposal into the enactment queue
        EnactmentQueue::<TestRuntime>::insert(proposal_action.clone().get_identifier(), 1);

        assert_eq!(EncointerCommunities::locations(cid, geo_hash.clone()).len(), 0);

        run_to_next_phase();
        // first assigning phase after proposal lifetime ended

        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Enacted);
        assert_eq!(EncointerDemocracy::enactment_queue(proposal_action.get_identifier()), None);
        assert_eq!(EncointerCommunities::locations(cid, geo_hash).len(), 1);
    });
}

#[test]
fn enact_remove_location_works() {
    new_test_ext().execute_with(|| {
        let cid = create_cid();
        let alice = alice();
        let location = Location { lat: Degree::from_num(10.0), lon: Degree::from_num(10.0) };
        let _ = EncointerCommunities::do_add_location(cid, location);
        let proposal_action = ProposalAction::RemoveLocation(cid, location);
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action.clone()
		));

        let geo_hash = GeoHash::try_from_params(location.lat, location.lon).unwrap();

        // directly inject the proposal into the enactment queue
        EnactmentQueue::<TestRuntime>::insert(proposal_action.clone().get_identifier(), 1);
        assert_eq!(EncointerCommunities::locations(cid, geo_hash.clone()).len(), 1);

        run_to_next_phase();
        // first assigning phase after proposal lifetime ended

        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Enacted);
        assert_eq!(EncointerDemocracy::enactment_queue(proposal_action.get_identifier()), None);
        assert_eq!(EncointerCommunities::locations(cid, geo_hash).len(), 0);
    });
}

#[test]
fn enact_update_community_metadata_works() {
    new_test_ext().execute_with(|| {
        let cid = create_cid();
        let alice = alice();
        let community_metadata: CommunityMetadataType = CommunityMetadataType {
            name: PalletString::from_str("abc1337").unwrap(),
            symbol: PalletString::from_str("DEF").unwrap(),
            ..Default::default()
        };

        let proposal_action = ProposalAction::UpdateCommunityMetadata(cid, community_metadata);
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action.clone()
		));

        // directly inject the proposal into the enactment queue
        EnactmentQueue::<TestRuntime>::insert(proposal_action.clone().get_identifier(), 1);

        run_to_next_phase();
        // first assigning phase after proposal lifetime ended

        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Enacted);
        assert_eq!(EncointerDemocracy::enactment_queue(proposal_action.get_identifier()), None);
        assert_eq!(
            EncointerCommunities::community_metadata(cid).name,
            PalletString::from_str("abc1337").unwrap()
        );
    });
}

#[test]
fn enact_update_demurrage_works() {
    new_test_ext().execute_with(|| {
        let cid = create_cid();
        let alice = alice();
        let demurrage = Demurrage::from_num(0.0001337);

        let proposal_action = ProposalAction::UpdateDemurrage(cid, demurrage);
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action.clone()
		));

        // directly inject the proposal into the enactment queue
        EnactmentQueue::<TestRuntime>::insert(proposal_action.clone().get_identifier(), 1);

        assert!(EncointerBalances::demurrage_per_block(&cid) != demurrage);

        run_to_next_phase();
        // first assigning phase after proposal lifetime ended

        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Enacted);
        assert_eq!(EncointerDemocracy::enactment_queue(proposal_action.get_identifier()), None);
        assert_eq!(EncointerBalances::demurrage_per_block(&cid), demurrage);
    });
}

#[test]
fn enact_update_nominal_income_works() {
    new_test_ext().execute_with(|| {
        let cid = create_cid();
        let alice = alice();
        let proposal_action =
            ProposalAction::UpdateNominalIncome(cid, NominalIncomeType::from(13037u32));
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action.clone()
		));

        // directly inject the proposal into the enactment queue
        EnactmentQueue::<TestRuntime>::insert(proposal_action.clone().get_identifier(), 1);

        run_to_next_phase();
        // first assigning phase after proposal lifetime ended

        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Enacted);
        assert_eq!(EncointerDemocracy::enactment_queue(proposal_action.get_identifier()), None);
        assert_eq!(EncointerCommunities::nominal_income(cid), NominalIncomeType::from(13037u32));
    });
}

#[test]
fn enact_set_inactivity_timeout_works() {
    new_test_ext().execute_with(|| {
        let alice = alice();
        let proposal_action =
            ProposalAction::SetInactivityTimeout(InactivityTimeoutType::from(13037u32));
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action.clone()
		));

        // directly inject the proposal into the enactment queue
        EnactmentQueue::<TestRuntime>::insert(proposal_action.clone().get_identifier(), 1);

        run_to_next_phase();
        // first assigning phase after proposal lifetime ended

        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Enacted);
        assert_eq!(
            EncointerDemocracy::enactment_queue(proposal_action.clone().get_identifier()),
            None
        );
        assert_eq!(
            EncointerCeremonies::inactivity_timeout(),
            InactivityTimeoutType::from(13037u32)
        );
    });
}

#[test]
fn enact_petition_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(System::block_number() + 1); // this is needed to assert events

        let cid = create_cid();
        let alice = alice();
        let petition_text = PalletString::try_from("freedom for all".as_bytes().to_vec()).unwrap();
        let proposal_action = ProposalAction::Petition(Some(cid), petition_text.clone());
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action.clone()
		));

        // directly inject the proposal into the enactment queue
        EnactmentQueue::<TestRuntime>::insert(proposal_action.clone().get_identifier(), 1);

        run_to_next_phase();
        // first assigning phase after proposal lifetime ended

        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Enacted);
        assert_eq!(EncointerDemocracy::enactment_queue(proposal_action.get_identifier()), None);

        match event_at_index::<TestRuntime>(get_num_events::<TestRuntime>() - 3).unwrap() {
            mock::RuntimeEvent::EncointerDemocracy(Event::PetitionApproved {
                                                       cid: maybe_cid,
                                                       text,
                                                   }) => {
                assert_eq!(maybe_cid, Some(cid));
                assert_eq!(text, petition_text);
            }
            _ => panic!("Wrong event"),
        };
    });
}

#[test]
fn enact_spend_native_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(System::block_number() + 1); // this is needed to assert events
        let beneficiary = AccountId::from(AccountKeyring::Alice);
        let amount: BalanceOf<TestRuntime> = 100_000_000;
        let cid = CommunityIdentifier::default();

        let treasury = EncointerTreasuries::get_community_treasury_account_unchecked(Some(cid));
        Balances::make_free_balance_be(&treasury, 500_000_000);

        let alice = alice();
        let proposal_action = ProposalAction::SpendNative(Some(cid), beneficiary.clone(), amount);
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action.clone()
		));

        // directly inject the proposal into the enactment queue
        EnactmentQueue::<TestRuntime>::insert(proposal_action.clone().get_identifier(), 1);

        run_to_next_phase();
        // first assigning phase after proposal lifetime ended

        assert_eq!(EncointerDemocracy::proposals(1).unwrap().state, ProposalState::Enacted);
        assert_eq!(EncointerDemocracy::enactment_queue(proposal_action.get_identifier()), None);

        assert_eq!(Balances::free_balance(&treasury), 400_000_000);
        assert_eq!(Balances::free_balance(&beneficiary), amount);
    });
}

#[test]
fn enactment_error_fires_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(System::block_number() + 1); // this is needed to assert events

        // use inexisting community to cause an enactment error
        let cid = CommunityIdentifier::from_str("u0qj944rhWE").unwrap();

        let alice = alice();
        let proposal_action =
            ProposalAction::UpdateNominalIncome(cid, NominalIncomeType::from(13037u32));
        assert_ok!(EncointerDemocracy::submit_proposal(
			RuntimeOrigin::signed(alice.clone()),
			proposal_action.clone()
		));

        // directly inject the proposal into the enactment queue
        EnactmentQueue::<TestRuntime>::insert(proposal_action.clone().get_identifier(), 1);

        run_to_next_phase();
        // first assigning phase after proposal lifetime ended

        match event_at_index::<TestRuntime>(get_num_events::<TestRuntime>() - 2).unwrap() {
            mock::RuntimeEvent::EncointerDemocracy(Event::EnactmentFailed {
                                                       proposal_id: 1,
                                                       reason: _r,
                                                   }) => (),
            _ => panic!("Wrong event"),
        };
    });
}
