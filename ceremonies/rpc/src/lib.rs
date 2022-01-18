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

use jsonrpc_core::{Error, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sc_rpc::DenyUnsafe;
use sp_api::{Decode, Encode, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

use encointer_ceremonies_rpc_runtime_api::CeremoniesApi as CeremoniesRuntimeApi;
use encointer_primitives::{ceremonies::Reputation, communities::CommunityIdentifier};

#[rpc]
pub trait CeremoniesApi<BlockHash, AccountId>
where
	AccountId: 'static + Encode + Decode + Send + Sync,
{
	#[rpc(name = "ceremonies_getReputations")]
	fn get_reputations(
		&self,
		account: AccountId,
		at: Option<BlockHash>,
	) -> Result<Vec<(CommunityIdentifier, Reputation)>>;
}

pub struct Ceremonies<Client, Block, AccountId> {
	client: Arc<Client>,
	_marker: std::marker::PhantomData<(Block, AccountId)>,
	deny_unsafe: DenyUnsafe,
}

impl<Client, Block, AccountId> Ceremonies<Client, Block, AccountId> {
	/// Create new `Ceremonies` instance with the given reference to the client.
	pub fn new(client: Arc<Client>, deny_unsafe: DenyUnsafe) -> Self {
		Ceremonies { client, _marker: Default::default(), deny_unsafe }
	}
}

impl<Client, Block, AccountId> CeremoniesApi<<Block as BlockT>::Hash, AccountId>
	for Ceremonies<Client, Block, AccountId>
where
	AccountId: 'static + Clone + Encode + Decode + Send + Sync + PartialEq,
	Block: BlockT,
	Client: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	Client::Api: CeremoniesRuntimeApi<Block, AccountId>,
{
	fn get_reputations(
		&self,
		account: AccountId,
		at: Option<<Block as BlockT>::Hash>,
	) -> Result<Vec<(CommunityIdentifier, Reputation)>> {
		self.deny_unsafe.check_if_safe()?;

		let api = self.client.runtime_api();
		let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
		return Ok(api
			.get_reputations(&at)
			.map_err(runtime_error_into_rpc_err)?
			.iter()
			.filter(|t| t.1 == account)
			.map(|t| (t.0 .0.clone(), t.2.clone()))
			.collect())
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
