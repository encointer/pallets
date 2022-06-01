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

mod error;
#[cfg(test)]
mod tests;

use crate::error::Error;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
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

#[rpc(client, server)]
pub trait CommunitiesApi<BlockHash, AccountId, BlockNumber>
where
	AccountId: 'static + Encode + Decode + Send + Sync,
	BlockNumber: 'static + Encode + Decode + Send + Sync,
{
	#[method(name = "encointer_getAllCommunities")]
	fn communities_get_all(&self, at: Option<BlockHash>) -> RpcResult<Vec<CidName>>;

	#[method(name = "encointer_getLocations")]
	fn communities_get_locations(
		&self,
		cid: CommunityIdentifier,
		at: Option<BlockHash>,
	) -> RpcResult<Vec<Location>>;

	#[method(name = "encointer_getAllBalances")]
	fn communities_get_all_balances(
		&self,
		account: AccountId,
		at: Option<BlockHash>,
	) -> RpcResult<Vec<(CommunityIdentifier, BalanceEntry<BlockNumber>)>>;
}

pub struct CommunitiesRpc<Client, Block, S> {
	client: Arc<Client>,
	storage: Arc<RwLock<S>>,
	offchain_indexing: bool,
	_marker: std::marker::PhantomData<Block>,
	deny_unsafe: DenyUnsafe,
}

impl<C, Block, S> CommunitiesRpc<C, Block, S>
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
		CommunitiesRpc {
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
		let cids = api.get_cids(&at).map_err(|e| Error::Runtime(e.into()))?;
		let mut cid_names: Vec<CidName> = vec![];

		cids.iter().for_each(|cid| {
			$self.get_storage(&cid.as_array()).map_or_else(
				|| warn_storage_inconsistency(cid),
				|name| cid_names.push(CidName::new(*cid, name)),
			)
		});

		$self.set_storage(CIDS_KEY, &cid_names);

		for cid in cids.iter() {
			let cache_key = &(CIDS_KEY, cid).encode()[..];
			let loc = api.get_locations(&at, &cid).map_err(|e| Error::Runtime(e.into()))?;

			$self.set_storage(cache_key, &loc);
		}
		$self.set_storage(CACHE_DIRTY_KEY, &false);
	};
}

type BlockNumberFor<B> = <<B as BlockT>::Header as HeaderT>::Number;

impl<C, Block, S, AccountId>
	CommunitiesApiServer<<Block as BlockT>::Hash, AccountId, BlockNumberFor<Block>>
	for CommunitiesRpc<C, Block, S>
where
	AccountId: 'static + Clone + Encode + Decode + Send + Sync + PartialEq,
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: CommunitiesRuntimeApi<Block, AccountId, BlockNumberFor<Block>>,
	S: 'static + OffchainStorage,
{
	fn communities_get_all(&self, at: Option<<Block as BlockT>::Hash>) -> RpcResult<Vec<CidName>> {
		if !self.offchain_indexing {
			return Err(Error::OffchainIndexingDisabled("communities_getAll".to_string()).into())
		}

		if self.cache_dirty() {
			refresh_cache!(self, at);
		}

		match self.get_storage(CIDS_KEY) {
			Some(cids) => {
				log::info!("Using cached community list: {:?}", cids);
				Ok(cids)
			},
			None => Err(Error::OffchainStorageNotFound(format!("{:?}", CIDS_KEY)).into()),
		}
	}

	fn communities_get_locations(
		&self,
		cid: CommunityIdentifier,
		at: Option<<Block as BlockT>::Hash>,
	) -> RpcResult<Vec<Location>> {
		if !self.offchain_indexing {
			return Err(Error::OffchainIndexingDisabled("communities_getAll".to_string()).into())
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
			None => Err(Error::OffchainStorageNotFound(format!("{:?}", cache_key)).into()),
		}
	}

	fn communities_get_all_balances(
		&self,
		account: AccountId,
		at: Option<<Block as BlockT>::Hash>,
	) -> RpcResult<Vec<(CommunityIdentifier, BalanceEntry<BlockNumberFor<Block>>)>> {
		self.deny_unsafe.check_if_safe()?;

		let api = self.client.runtime_api();
		let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
		Ok(api.get_all_balances(&at, &account).map_err(|e| Error::Runtime(e.into()))?)
	}
}

/// This should never happen!
fn warn_storage_inconsistency(cid: &CommunityIdentifier) {
	log::warn!("Storage inconsistency. Could not find cid: {:?} in offchain storage. This is a fatal bug in the pallet", cid)
}
