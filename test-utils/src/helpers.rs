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
use codec::Encode;
use encointer_primitives::communities::{CommunityIdentifier, Degree, Location};
use frame_support::traits::OriginTrait;
use sp_core::{blake2_256, sr25519, Pair};
use sp_keyring::AccountKeyring;

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
    .collect();
}

/// register a simple test community with N meetup locations and defined bootstrappers
pub fn register_test_community<Runtime>(
    custom_bootstrappers: Option<Vec<sr25519::Pair>>,
    n_locations: u32,
) -> CommunityIdentifier
where
    Runtime: encointer_communities::Config,
    Runtime: frame_system::Config<AccountId = AccountId>,
    <Runtime as frame_system::Config>::Origin: OriginTrait<AccountId = AccountId>,
{
    let bs: Vec<AccountId> = custom_bootstrappers
        .unwrap_or_else(|| bootstrappers())
        .into_iter()
        .map(|b| account_id(&b))
        .collect();

    let prime = &bs[0];

    let mut loc = Vec::with_capacity(n_locations as usize);
    for l in 0..n_locations {
        loc.push(Location {
            lat: Degree::from_num(l),
            lon: Degree::from_num(l),
        })
    }
    encointer_communities::Module::<Runtime>::new_community(
        Runtime::Origin::signed(prime.clone()),
        loc.clone(),
        bs.clone(),
        Default::default(),
        None,
        None,
    )
    .unwrap();
    CommunityIdentifier::from(blake2_256(&(loc, bs).encode()))
}
