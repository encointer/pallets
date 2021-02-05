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

use super::*;
use crate::{Module, Trait};
use encointer_ceremonies::Module as EncointerCeremoniesModule;
use frame_support::assert_ok;
use sp_core::H256;
use sp_keyring::AccountKeyring;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
};

use encointer_primitives::sybil::ProofOfPersonhoodRequest;

use test_utils::*;

pub type EncointerScheduler = encointer_scheduler::Module<TestRuntime>;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TestRuntime;

impl_frame_system!(TestRuntime);
impl_timestamp!(TestRuntime, EncointerScheduler);
impl_outer_origin_for_runtime!(TestRuntime);

impl_encointer_ceremonies!(TestRuntime);
impl_encointer_communities!(TestRuntime);
impl_encointer_balances!(TestRuntime);
impl_encointer_scheduler!(TestRuntime, EncointerCeremoniesModule);

impl Trait for TestRuntime {
    type Event = ();
    type XcmSender = ();
    type Signature = Signature;
}

fn new_test_ext() -> sp_io::TestExternalities {
    let storage = frame_system::GenesisConfig::default()
        .build_storage::<TestRuntime>()
        .unwrap();
    storage.into()
}

type SybilGate = Module<TestRuntime>;

#[test]
fn issue_proof_of_personhood_is_ok() {
    new_test_ext().execute_with(|| {
        assert_ok!(SybilGate::issue_proof_of_personhood_confidence(
            Origin::signed(AccountKeyring::Alice.into()),
            2,
            1,
            ProofOfPersonhoodRequest::default()
        ));
    })
}
