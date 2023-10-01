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
use crate::mock::{EncointerVouches, RuntimeOrigin};
use encointer_primitives::{
	common::{BoundedIpfsCid, FromStr},
	vouches::{PresenceType, VouchKind},
};
use frame_support::assert_ok;
use mock::{new_test_ext, System, TestRuntime};
use test_utils::{AccountId, AccountKeyring};

#[test]
fn vouch_for_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let _ = pallet_timestamp::Pallet::<TestRuntime>::set(RuntimeOrigin::none(), 42);

		let alice = AccountId::from(AccountKeyring::Alice);
		let charlie = AccountId::from(AccountKeyring::Charlie);

		let vouch_kind = VouchKind::EncounteredHuman(PresenceType::LivePhysical);
		let quality = VouchQuality::Badge(
			BoundedIpfsCid::from_str("QmPK1s3pNYLi9ERiq3BDxKa4XosgWwFRQUydHUtz4YgpqB").unwrap(),
		);

		assert_ok!(EncointerVouches::vouch_for(
			RuntimeOrigin::signed(alice.clone()),
			charlie.clone(),
			vouch_kind,
			quality.clone(),
		));

		assert_eq!(
			EncointerVouches::vouches(charlie, alice)[0],
			Vouch { protected: false, timestamp: 42, vouch_kind, quality }
		);
	});
}
