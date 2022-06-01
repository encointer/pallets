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
use sp_core::offchain::storage::InMemOffchainStorage;

#[test]
fn caching_works() {
	let storage = InMemOffchainStorage::default();
	let client = ();
	let communities: CommunitiesRpc<_, (), _> =
		CommunitiesRpc::new(Arc::new(client), storage, true, DenyUnsafe::Yes);

	let cid_names = vec![CidName::new(Default::default(), "hello world".into())];

	assert!(communities.cache_dirty());
	communities.set_storage(CIDS_KEY, &cid_names);
	communities.set_storage(CACHE_DIRTY_KEY, &false);
	assert!(!communities.cache_dirty());
	assert_eq!(communities.get_storage(CIDS_KEY), Some(cid_names));
}
