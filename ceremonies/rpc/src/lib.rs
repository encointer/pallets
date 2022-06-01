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

use encointer_rpc::Error;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use parking_lot::RwLock;
use sc_rpc::DenyUnsafe;
use sp_api::{offchain::OffchainStorage, Decode, Encode, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

use encointer_ceremonies_rpc_runtime_api::CeremoniesApi as CeremoniesRuntimeApi;
use encointer_primitives::{
	ceremonies::{
		consts::STORAGE_PREFIX, reputation_cache_dirty_key, reputation_cache_key,
		AggregatedAccountData, CommunityReputation,
	},
	communities::CommunityIdentifier,
	scheduler::CeremonyIndexType,
};

#[rpc(client, server)]
pub trait CeremoniesApi<BlockHash, AccountId, Moment>
where
	AccountId: 'static + Encode + Decode + Send + Sync,
	Moment: 'static + Encode + Decode + Send + Sync,
{
	#[method(name = "encointer_getReputations", blocking)]
	fn get_reputations(
		&self,
		account: AccountId,
		at: Option<BlockHash>,
	) -> RpcResult<Vec<(CeremonyIndexType, CommunityReputation)>>;

	// For rpc calls that need a while block should help the rpc server to
	// spawn it with `tokio.spawn_blocking` to keep the rpc server responsive
	// for calls that take longer. (not 100% sure if I understand correctly.)
	#[method(name = "encointer_getAggregatedAccountData", blocking)]
	fn get_aggregated_account_data(
		&self,
		cid: CommunityIdentifier,
		account: AccountId,
		at: Option<BlockHash>,
	) -> RpcResult<AggregatedAccountData<AccountId, Moment>>;
}

pub struct CeremoniesRpc<Client, Block, AccountId, Moment, S> {
	client: Arc<Client>,
	deny_unsafe: DenyUnsafe,
	storage: Arc<RwLock<S>>,
	#[allow(unused)]
	offchain_indexing: bool,
	_marker: std::marker::PhantomData<(Block, AccountId, Moment)>,
}

impl<Client, Block, AccountId, Moment, S> CeremoniesRpc<Client, Block, AccountId, Moment, S>
where
	S: 'static + OffchainStorage,
	Block: sp_api::BlockT,
	AccountId: 'static + Encode + Decode + Send + Sync,
	Moment: 'static + Encode + Decode + Send + Sync,
	Client: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	Client::Api: CeremoniesRuntimeApi<Block, AccountId, Moment>,
	encointer_primitives::ceremonies::AggregatedAccountData<AccountId, Moment>: Decode,
{
	/// Create new `Ceremonies` instance with the given reference to the client.
	pub fn new(
		client: Arc<Client>,
		deny_unsafe: DenyUnsafe,
		storage: S,
		offchain_indexing: bool,
	) -> Self {
		CeremoniesRpc {
			client,
			_marker: Default::default(),
			deny_unsafe,
			storage: Arc::new(RwLock::new(storage)),
			offchain_indexing,
		}
	}

	/// Check if cache was marked dirty by the runtime
	pub fn cache_dirty(&self, key: &[u8]) -> bool {
		match self.storage.read().get(STORAGE_PREFIX, key) {
			Some(d) => Decode::decode(&mut d.as_slice()).unwrap_or_else(|e| {
				log::error!("Cache dirty bit: {:?}", e);
				log::info!("{:?}: Defaulting to dirty == true", key);
				true
			}),
			None => {
				log::warn!("{:?}: Cache dirty bit is none.", key);
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

	pub fn refresh_reputation_cache(
		&self,
		account: AccountId,
		at: Option<<Block as BlockT>::Hash>,
	) -> RpcResult<()> {
		let api = self.client.runtime_api();
		let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
		let reputations =
			api.get_reputations(&at, &account).map_err(|e| Error::Runtime(e.into()))?;
		let cache_key = &reputation_cache_key(&account);
		self.set_storage::<Vec<(CeremonyIndexType, CommunityReputation)>>(cache_key, &reputations);
		self.set_storage(&reputation_cache_dirty_key(&account), &false);
		Ok(())
	}
}

impl<Client, Block, AccountId, Moment, S>
	CeremoniesApiServer<<Block as BlockT>::Hash, AccountId, Moment>
	for CeremoniesRpc<Client, Block, AccountId, Moment, S>
where
	AccountId: 'static + Clone + Encode + Decode + Send + Sync + PartialEq,
	Moment: 'static + Clone + Encode + Decode + Send + Sync + PartialEq,
	Block: BlockT,
	Client: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	Client::Api: CeremoniesRuntimeApi<Block, AccountId, Moment>,
	S: 'static + OffchainStorage,
	encointer_primitives::ceremonies::AggregatedAccountData<AccountId, Moment>: Decode,
{
	fn get_reputations(
		&self,
		account: AccountId,
		at: Option<<Block as BlockT>::Hash>,
	) -> RpcResult<Vec<(CeremonyIndexType, CommunityReputation)>> {
		self.deny_unsafe.check_if_safe()?;
		let api = self.client.runtime_api();
		let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
		Ok(api.get_reputations(&at, &account).map_err(|e| Error::Runtime(e.into()))?)

		// This part was broken, the cache was never marked as dirty: https://github.com/encointer/pallets/issues/220
		//
		// if !self.offchain_indexing {
		// 	return Err(offchain_indexing_disabled_error("ceremonies_getReputations"))
		// }
		//
		// if self.cache_dirty(&reputation_cache_dirty_key(&account)) {
		// 	self.refresh_reputation_cache(account.clone(), at);
		// }
		//
		// let cache_key = &reputation_cache_key(&account);
		// match self.get_storage::<Vec<(CeremonyIndexType, CommunityReputation)>>(cache_key) {
		// 	Some(reputation_list) => Ok(reputation_list),
		// 	None => Err(storage_not_found_error(cache_key)),
		// }
	}

	fn get_aggregated_account_data(
		&self,
		cid: CommunityIdentifier,
		account: AccountId,
		at: Option<<Block as BlockT>::Hash>,
	) -> RpcResult<AggregatedAccountData<AccountId, Moment>> {
		self.deny_unsafe.check_if_safe()?;
		let api = self.client.runtime_api();
		let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
		Ok(api
			.get_aggregated_account_data(&at, cid, &account)
			.map_err(|e| Error::Runtime(e.into()))?)
	}
}
