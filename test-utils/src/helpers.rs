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

use crate::AccountId;
use encointer_primitives::communities::{CommunityIdentifier, Degree, Location};
use frame_support::{pallet_prelude::DispatchResultWithPostInfo, traits::OriginTrait};
use sp_core::{sr25519, Pair};
use sp_keyring::AccountKeyring;
use sp_runtime::DispatchError;

/// shorthand to convert Pair to AccountId
pub fn account_id(pair: &sr25519::Pair) -> AccountId {
	pair.public().into()
}

/// All well-known keys are bootstrappers for easy testing afterwards
pub fn bootstrappers() -> Vec<sr25519::Pair> {
	return vec![
		AccountKeyring::Alice,
		AccountKeyring::Bob,
		AccountKeyring::Charlie,
		AccountKeyring::Dave,
		AccountKeyring::Eve,
		AccountKeyring::Ferdie,
	]
	.iter()
	.map(|k| k.pair())
	.collect()
}

/// register a simple test community with a specified location and defined bootstrappers
pub fn register_test_community<Runtime>(
	custom_bootstrappers: Option<Vec<sr25519::Pair>>,
	lat: f64,
	lon: f64,
) -> CommunityIdentifier
where
	Runtime: encointer_communities::Config,
	Runtime: frame_system::Config<AccountId = AccountId>,
	<Runtime as frame_system::Config>::Origin: OriginTrait<AccountId = AccountId>,
{
	let bs: Vec<AccountId> = custom_bootstrappers
		.unwrap_or_else(bootstrappers)
		.into_iter()
		.map(|b| account_id(&b))
		.collect();

	let prime = &bs[0];

	let location = Location { lat: Degree::from_num(lat), lon: Degree::from_num(lon) };
	encointer_communities::Pallet::<Runtime>::new_community(
		Runtime::Origin::signed(prime.clone()),
		location,
		bs.clone(),
		Default::default(),
		None,
		None,
	)
	.unwrap();
	CommunityIdentifier::new(location, bs).unwrap()
}

pub fn get_num_events<T: frame_system::Config>() -> usize {
	frame_system::Pallet::<T>::events().len()
}
pub fn events<T: frame_system::Config>() -> Vec<T::Event> {
	let events = frame_system::Pallet::<T>::events()
		.into_iter()
		.map(|evt| evt.event)
		.collect::<Vec<_>>();
	frame_system::Pallet::<T>::reset_events();
	events
}

pub fn last_event<T: frame_system::Config>() -> Option<T::Event> {
	event_at_index::<T>(get_num_events::<T>() - 1)
}

pub fn event_at_index<T: frame_system::Config>(index: usize) -> Option<T::Event> {
	let events = frame_system::Pallet::<T>::events();
	if events.len() < index {
		return None
	}
	let frame_system::EventRecord { event, .. } = &events[index];
	Some(event.clone())
}

pub fn event_deposited<T: frame_system::Config>(desired_event: T::Event) -> bool {
	let events = frame_system::Pallet::<T>::events();
	for eventrec in events.iter() {
		let frame_system::EventRecord { event, .. } = eventrec;
		if *event == desired_event {
			return true
		}
	}
	false
}

pub fn assert_dispatch_err(actual: DispatchResultWithPostInfo, expected: DispatchError) {
	assert_eq!(actual.unwrap_err().error, expected)
}

pub fn almost_eq(a: u128, b: u128, delta: u128) -> bool {
	let diff = if a > b { a - b } else { b - a };
	diff < delta
}
