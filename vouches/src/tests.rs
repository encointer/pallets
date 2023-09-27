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
use crate::mock::{EncointerVouches, RuntimeOrigin, Timestamp};
use codec::Encode;
use encointer_primitives::{
	common::{BoundedIpfsCid, FromStr},
	vouches::{PresenceType, VouchType},
};
use frame_support::{
	assert_err, assert_ok,
	traits::{OnFinalize, OnInitialize},
};
use mock::{new_test_ext, System, TestRuntime};
use sp_runtime::traits::{BlakeTwo256, Hash};
use test_utils::{
	helpers::{event_deposited, last_event, register_test_community},
	storage::participant_reputation,
	AccountId, AccountKeyring, BLOCKTIME, GENESIS_TIME,
};

#[test]
fn vouch_for_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		pallet_timestamp::Pallet::<TestRuntime>::set(RuntimeOrigin::none(), 42);

		let alice = AccountId::from(AccountKeyring::Alice);
		let bob = AccountId::from(AccountKeyring::Bob);
		let charlie = AccountId::from(AccountKeyring::Charlie);

		let vouch_type = VouchType::EncounteredHuman(PresenceType::Physical);
		let mut qualities = VouchQualityBoundedVec::default();
		qualities
			.try_push(VouchQuality::Badge(
				BoundedIpfsCid::from_str("QmPK1s3pNYLi9ERiq3BDxKa4XosgWwFRQUydHUtz4YgpqB").unwrap(),
			))
			.unwrap();

		assert_ok!(EncointerVouches::vouch_for(
			RuntimeOrigin::signed(alice.clone()),
			charlie.clone(),
			vouch_type,
			qualities.clone(),
		));

		assert_eq!(
			EncointerVouches::vouches(charlie, alice)[0],
			Vouch { protected: false, timestamp: 42, vouch_type, qualities }
		);
	});
}
