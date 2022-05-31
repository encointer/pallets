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

use jsonrpsee::{
	core::{JsonValue, RpcResult},
	proc_macros::rpc,
};
use sc_rpc::DenyUnsafe;
use sp_api::{Decode, Encode, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

use encointer_bazaar_rpc_runtime_api::BazaarApi as BazaarRuntimeApi;
use encointer_primitives::{
	bazaar::{BusinessData, BusinessIdentifier, OfferingData},
	communities::CommunityIdentifier,
};

#[rpc(client, server)]
pub trait BazaarApi<BlockHash, AccountId>
where
	AccountId: 'static + Encode + Decode + Send + Sync,
{
	#[rpc(name = "encointer_bazaarGetBusinesses")]
	fn get_businesses(
		&self,
		cid: CommunityIdentifier,
		at: Option<BlockHash>,
	) -> RpcResult<Vec<BusinessData>>;
	#[rpc(name = "encointer_bazaarGetOfferings")]
	fn get_offerings(
		&self,
		cid: CommunityIdentifier,
		at: Option<BlockHash>,
	) -> RpcResult<Vec<OfferingData>>;
	#[rpc(name = "encointer_bazaarGetOfferingsForBusiness")]
	fn get_offerings_for_business(
		&self,
		bid: BusinessIdentifier<AccountId>,
		at: Option<BlockHash>,
	) -> RpcResult<Vec<OfferingData>>;
}

pub struct Bazaar<Client, Block, AccountId> {
	client: Arc<Client>,
	_marker: std::marker::PhantomData<(Block, AccountId)>,
	deny_unsafe: DenyUnsafe,
}

impl<Client, Block, AccountId> Bazaar<Client, Block, AccountId> {
	/// Create new `Bazaar` instance with the given reference to the client.
	pub fn new(client: Arc<Client>, deny_unsafe: DenyUnsafe) -> Self {
		Bazaar { client, _marker: Default::default(), deny_unsafe }
	}
}

impl<Client, Block, AccountId> BazaarApi<<Block as BlockT>::Hash, AccountId>
	for Bazaar<Client, Block, AccountId>
where
	AccountId: 'static + Clone + Encode + Decode + Send + Sync,
	Block: BlockT,
	Client: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	Client::Api: BazaarRuntimeApi<Block, AccountId>,
{
	fn get_businesses(
		&self,
		cid: CommunityIdentifier,
		at: Option<<Block as BlockT>::Hash>,
	) -> RpcResult<Vec<BusinessData>> {
		self.deny_unsafe.check_if_safe()?;

		let api = self.client.runtime_api();
		let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
		return Ok(api
			.get_businesses(&at, &cid)
			.map_err(runtime_error_into_rpc_err)?
			.iter()
			.map(|bid| bid.1.clone())
			.collect())
	}

	fn get_offerings(
		&self,
		cid: CommunityIdentifier,
		at: Option<<Block as BlockT>::Hash>,
	) -> RpcResult<Vec<OfferingData>> {
		self.deny_unsafe.check_if_safe()?;

		let api = self.client.runtime_api();
		let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
		return Ok(api
			.get_businesses(&at, &cid)
			.map_err(runtime_error_into_rpc_err)?
			.iter()
			.flat_map(|bid| api.get_offerings(&at, &BusinessIdentifier::new(cid, bid.0.clone())))
			.flatten()
			.collect())
	}

	fn get_offerings_for_business(
		&self,
		bid: BusinessIdentifier<AccountId>,
		at: Option<<Block as BlockT>::Hash>,
	) -> RpcResult<Vec<OfferingData>> {
		self.deny_unsafe.check_if_safe()?;

		let api = self.client.runtime_api();
		let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
		return Ok(api.get_offerings(&at, &bid).map_err(runtime_error_into_rpc_err)?)
	}
}

const RUNTIME_ERROR: i64 = 1; // Arbitrary number, but substrate uses the same

/// Converts a runtime trap into an RPC error.
fn runtime_error_into_rpc_err(err: impl std::fmt::Debug) -> Error {
	Error {
		code: ErrorCode::ServerError(RUNTIME_ERROR),
		message: "Runtime trapped".into(),
		data: Some(format!("{:?}", err).into()),
	}
}
