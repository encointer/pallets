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

#[cfg(test)]
mod tests;

use jsonrpc_core::{Error, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::{Decode, Encode, HeaderT, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

use encointer_communities_rpc_runtime_api::CommunitiesApi as CommunitiesRuntimeApi;
use encointer_primitives::{
	balances::BalanceEntry,
	communities::{consts::CACHE_DIRTY_KEY, CidName, CommunityIdentifier, Location},
};
use parking_lot::RwLock;
use sc_rpc_api::DenyUnsafe;
use sp_api::offchain::{OffchainStorage, STORAGE_PREFIX};

const CIDS_KEY: &[u8; 4] = b"cids";

#[rpc]
pub trait CommunitiesApi<BlockHash, AccountId, BlockNumber>
where
	AccountId: 'static + Encode + Decode + Send + Sync,
	BlockNumber: 'static + Encode + Decode + Send + Sync,
{
	#[rpc(name = "encointer_getAllCommunities")]
	fn communities_get_all(&self, at: Option<BlockHash>) -> Result<Vec<CidName>>;

	#[rpc(name = "encointer_getLocations")]
	fn communities_get_locations(
		&self,
		cid: CommunityIdentifier,
		at: Option<BlockHash>,
	) -> Result<Vec<Location>>;

	#[rpc(name = "encointer_getAllBalances")]
	fn communities_get_all_balances(
		&self,
		account: AccountId,
		at: Option<BlockHash>,
	) -> Result<Vec<(CommunityIdentifier, BalanceEntry<BlockNumber>)>>;
}

pub struct Communities<Client, Block, S> {
	client: Arc<Client>,
	storage: Arc<RwLock<S>>,
	offchain_indexing: bool,
	_marker: std::marker::PhantomData<Block>,
	deny_unsafe: DenyUnsafe,
}

impl<C, Block, S> Communities<C, Block, S>
where
	S: 'static + OffchainStorage,
{
	/// Create new `Communities` with the given reference to the client and to the offchain storage
	pub fn new(
		client: Arc<C>,
		storage: S,
		offchain_indexing: bool,
		deny_unsafe: DenyUnsafe,
	) -> Self {
		Communities {
			client,
			storage: Arc::new(RwLock::new(storage)),
			offchain_indexing,
			_marker: Default::default(),
			deny_unsafe,
		}
	}

	/// Check if cache was marked dirty by the runtime
	pub fn cache_dirty(&self) -> bool {
		match self.storage.read().get(STORAGE_PREFIX, CACHE_DIRTY_KEY) {
			Some(d) => Decode::decode(&mut d.as_slice()).unwrap_or_else(|e| {
				log::error!("Cache dirty bit: {:?}", e);
				log::info!("Defaulting to dirty == true");
				true
			}),
			None => {
				log::warn!("Cache dirty bit is none. This is fine if no community is registered.");
				true
			},
		}
	}

	pub fn get_storage<V: Decode>(&self, key: &[u8]) -> Option<V> {
		match self.storage.read().get(STORAGE_PREFIX, key) {
			Some(v) => Some(Decode::decode(&mut v.as_slice()).unwrap()),
			None => None,
		}
	}

	pub fn set_storage<V: Encode>(&self, key: &[u8], val: &V) {
		self.storage.write().set(STORAGE_PREFIX, key, &val.encode());
	}
}

macro_rules! refresh_cache {
	($self:ident, $at:ident) => {
		log::info!("refreshing cache.....");
		let api = $self.client.runtime_api();
		let at = BlockId::hash($at.unwrap_or_else(|| $self.client.info().best_hash));
		let cids = api.get_cids(&at).map_err(runtime_error_into_rpc_err).unwrap();
		let mut cid_names: Vec<CidName> = vec![];

		cids.iter().for_each(|cid| {
			$self.get_storage(&cid.as_array()).map_or_else(
				|| warn_storage_inconsistency(cid),
				|name| cid_names.push(CidName::new(*cid, name)),
			)
		});

		$self.set_storage(CIDS_KEY, &cid_names);

		cids.iter().for_each(|cid| {
			let cache_key = &(CIDS_KEY, cid).encode()[..];
			let loc = api.get_locations(&at, &cid).map_err(runtime_error_into_rpc_err).unwrap();

			$self.set_storage(cache_key, &loc);
		});
		$self.set_storage(CACHE_DIRTY_KEY, &false);
	};
}

type BlockNumberFor<B> = <<B as BlockT>::Header as HeaderT>::Number;

impl<C, Block, S, AccountId>
	CommunitiesApi<<Block as BlockT>::Hash, AccountId, BlockNumberFor<Block>>
	for Communities<C, Block, S>
where
	AccountId: 'static + Clone + Encode + Decode + Send + Sync + PartialEq,
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: CommunitiesRuntimeApi<Block, AccountId, BlockNumberFor<Block>>,
	S: 'static + OffchainStorage,
{
	fn communities_get_all(&self, at: Option<<Block as BlockT>::Hash>) -> Result<Vec<CidName>> {
		if !self.offchain_indexing {
			return Err(offchain_indexing_disabled_error("communities_getAll"))
		}

		if self.cache_dirty() {
			refresh_cache!(self, at);
		}

		match self.get_storage(CIDS_KEY) {
			Some(cids) => {
				log::info!("Using cached community list: {:?}", cids);
				Ok(cids)
			},
			None => Err(storage_not_found_error(CIDS_KEY)),
		}
	}

	fn communities_get_locations(
		&self,
		cid: CommunityIdentifier,
		at: Option<<Block as BlockT>::Hash>,
	) -> Result<Vec<Location>> {
		if !self.offchain_indexing {
			return Err(offchain_indexing_disabled_error("communities_getAll"))
		}

		if self.cache_dirty() {
			refresh_cache!(self, at);
		}

		let cache_key = &(CIDS_KEY, cid).encode()[..];
		match self.get_storage::<Vec<Location>>(cache_key) {
			Some(loc) => {
				log::info!("Using cached location list with len {}", loc.len());
				Ok(loc)
			},
			None => Err(storage_not_found_error(cache_key)),
		}
	}

	fn communities_get_all_balances(
		&self,
		account: AccountId,
		at: Option<<Block as BlockT>::Hash>,
	) -> Result<Vec<(CommunityIdentifier, BalanceEntry<BlockNumberFor<Block>>)>> {
		self.deny_unsafe.check_if_safe()?;

		let api = self.client.runtime_api();
		let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
		return Ok(api.get_all_balances(&at, &account).map_err(runtime_error_into_rpc_err)?)
	}
}

const RUNTIME_ERROR: i64 = 1; // Arbitrary number, but substrate uses the same
const OFFCHAIN_INDEXING_DISABLED_ERROR: i64 = 2;
const STORAGE_NOT_FOUND_ERROR: i64 = 3;

/// Converts a runtime trap into an RPC error.
fn runtime_error_into_rpc_err(err: impl std::fmt::Debug) -> Error {
	Error {
		code: ErrorCode::ServerError(RUNTIME_ERROR),
		message: "Runtime trapped".into(),
		data: Some(format!("{:?}", err).into()),
	}
}

fn storage_not_found_error(key: impl std::fmt::Debug) -> Error {
	Error {
		code: ErrorCode::ServerError(STORAGE_NOT_FOUND_ERROR),
		message: "Offchain storage not found".into(),
		data: Some(format!("Key {:?}", key).into()),
	}
}

fn offchain_indexing_disabled_error(call: impl std::fmt::Debug) -> Error {
	Error {
		code: ErrorCode::ServerError(OFFCHAIN_INDEXING_DISABLED_ERROR),
		message: "This rpc is not allowed with offchain-indexing disabled".into(),
		data: Some(format!("call: {:?}", call).into()),
	}
}

/// This should never happen!
fn warn_storage_inconsistency(cid: &CommunityIdentifier) {
	log::warn!("Storage inconsistency. Could not find cid: {:?} in offchain storage. This is a fatal bug in the pallet", cid)
}
