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

//! Helper functions to manipulate the storage, to get a specific state in the tests

use sp_core::twox_128;

use encointer_primitives::ceremonies::CommunityCeremony;
use frame_support::pallet_prelude::Encode;

pub type StorageKey = Vec<u8>;

pub fn current_ceremony_index() -> StorageKey {
	storage_key("EncointerScheduler", "CurrentCeremonyIndex")
}

pub fn community_identifiers() -> StorageKey {
	storage_key("EncointerCommunities", "CommunityIdentifiers")
}

pub fn participant_reputation<AccountId: Encode>(
	c: CommunityCeremony,
	account: AccountId,
) -> StorageKey {
	storage_double_map_key("EncointerCeremonies", "ParticipantReputation", &c, &account)
}

pub fn storage_key(module: &str, storage_key_name: &str) -> StorageKey {
	let mut key = twox_128(module.as_bytes()).to_vec();
	key.extend(&twox_128(storage_key_name.as_bytes()));
	key
}

pub fn storage_double_map_key<K, Q>(
	module: &str,
	storage_key_name: &str,
	key1: K,
	key2: Q,
) -> StorageKey
where
	K: Encode,
	Q: Encode,
{
	let mut bytes = sp_core::twox_128(module.as_bytes()).to_vec();
	bytes.extend(&sp_core::twox_128(storage_key_name.as_bytes())[..]);
	bytes.extend(key_hash(&key1));
	bytes.extend(key_hash(&key2));
	bytes
}

pub fn key_hash<K: Encode>(key: &K) -> StorageKey {
	let encoded_key = key.encode();
	let x: &[u8] = encoded_key.as_slice();
	sp_core::blake2_128(x).iter().chain(x.iter()).cloned().collect::<Vec<_>>()
}
