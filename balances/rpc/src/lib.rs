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

use encointer_balances_rpc_runtime_api::BalancesApi as BalancesRuntimeApi;
use encointer_primitives::{balances::BalanceEntry, communities::CommunityIdentifier};
use jsonrpc_core::{Error, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sc_rpc::DenyUnsafe;
use sp_api::{Decode, Encode, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_runtime::{
	generic::BlockId,
	traits::{Block as BlockT, Header as HeaderT},
};
use std::sync::Arc;

#[rpc]
pub trait BalancesApi<BlockHash, AccountId, BlockNumber>
where
	AccountId: 'static + Encode + Decode + Send + Sync,
	BlockNumber: 'static + Encode + Decode + Send + Sync,
{
	#[rpc(name = "encointerBalances_getAllBalances")]
	fn get_all_balances(
		&self,
		account: AccountId,
		at: Option<BlockHash>,
	) -> Result<Vec<(CommunityIdentifier, BalanceEntry<BlockNumber>)>>;
}

pub struct Balances<Client, Block, AccountId> {
	client: Arc<Client>,
	_marker: std::marker::PhantomData<(Block, AccountId)>,
	deny_unsafe: DenyUnsafe,
}

impl<Client, Block, AccountId> Balances<Client, Block, AccountId> {
	/// Create new `Balances` instance with the given reference to the client.
	pub fn new(client: Arc<Client>, deny_unsafe: DenyUnsafe) -> Self {
		Balances { client, _marker: Default::default(), deny_unsafe }
	}
}

type BlockNumberFor<B> = <<B as BlockT>::Header as HeaderT>::Number;

impl<Client, Block, AccountId>
	BalancesApi<<Block as BlockT>::Hash, AccountId, BlockNumberFor<Block>>
	for Balances<Client, Block, AccountId>
where
	AccountId: 'static + Clone + Encode + Decode + Send + Sync + PartialEq,
	Block: BlockT,
	Client: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	Client::Api: BalancesRuntimeApi<Block, AccountId, BlockNumberFor<Block>>,
{
	fn get_all_balances(
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

/// Converts a runtime trap into an RPC error.
fn runtime_error_into_rpc_err(err: impl std::fmt::Debug) -> Error {
	Error {
		code: ErrorCode::ServerError(RUNTIME_ERROR),
		message: "Runtime trapped".into(),
		data: Some(format!("{:?}", err).into()),
	}
}
