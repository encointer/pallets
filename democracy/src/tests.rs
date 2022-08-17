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
use encointer_primitives::{
	communities::NominalIncome as NominalIncomeType,
	democracy::{ProposalAccessPolicy, ProposalAction, ProposalState},
};
use frame_support::{assert_err, assert_ok};
use mock::{new_test_ext, EncointerDemocracy, Origin, System, TestRuntime};

use encointer_primitives::communities::CommunityIdentifier;
use test_utils::{
	helpers::{last_event, register_test_community},
	*,
};

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
#[test]
fn proposal_submission_works() {
	new_test_ext().execute_with(|| {
		let cid = create_cid();
		let proposal: Proposal<BlockNumber> = Proposal {
			start: BlockNumber::from(1u64),
			action: ProposalAction::UpdateNominalIncome(cid, NominalIncomeType::from(100i32)),
			state: ProposalState::Ongoing,
			access_policy: ProposalAccessPolicy::Community(cid),
		};

		assert_ok!(EncointerDemocracy::submit_proposal(Origin::signed(alice()), proposal.clone()));
		assert_eq!(EncointerDemocracy::proposal_count(), 1);
		assert_eq!(EncointerDemocracy::proposals(1).unwrap(), proposal);
	});
}
