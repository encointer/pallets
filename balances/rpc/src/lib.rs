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

use jsonrpc_core::Result;
use jsonrpc_derive::rpc;
use sp_api::{offchain::OffchainStorage, Decode, Encode, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_runtime::{
	generic,
	traits::{Block as BlockT, Dispatchable, Extrinsic},
};
use std::sync::Arc;

use sc_transaction_pool_api::{
	error::IntoPoolError, BlockHash, InPoolTransaction, TransactionFor, TransactionPool,
	TransactionSource, TransactionStatus, TxHash,
};

use encointer_balances_rpc_runtime_api::BalancesApi as BalancesRuntimeApi;
use encointer_primitives::{balances::BalanceType, communities::CommunityIdentifier};

#[rpc]
pub trait BalancesApi<AccountId>
where
	AccountId: 'static + Encode + Decode + Send + Sync,
{
	#[rpc(name = "encointer_pendingIncomingTransferFor")]
	fn pending_incoming_transfers_for(
		&self,
		account: AccountId,
	) -> Result<Vec<(AccountId, CommunityIdentifier, BalanceType)>>;
}

pub struct Balances<Client, P, AccountId> {
	client: Arc<Client>,
	pool: Arc<P>,
	_marker: std::marker::PhantomData<(AccountId)>,
}

impl<Client, P, AccountId> Balances<Client, P, AccountId>
where
	P: TransactionPool + Sync + Send + 'static,
	AccountId: 'static + Encode + Decode + Send + Sync,
	Client: Send + Sync + 'static + ProvideRuntimeApi<P::Block> + HeaderBackend<P::Block>,
	Client::Api: BalancesRuntimeApi<P::Block, AccountId>,
{
	/// Create new `Balances` instance with the given reference to the client.
	pub fn new(client: Arc<Client>, pool: Arc<P>) -> Self {
		Balances { client, pool, _marker: Default::default() }
	}
}

impl<Client, P, AccountId> BalancesApi<AccountId> for Balances<Client, P, AccountId>
where
	P: TransactionPool + Sync + Send + 'static,
	<P::Block as BlockT>::Extrinsic: Extrinsic,
	AccountId: 'static + Clone + Encode + Decode + Send + Sync + PartialEq,
	Client: Send + Sync + 'static + ProvideRuntimeApi<P::Block> + HeaderBackend<P::Block>,
	Client::Api: BalancesRuntimeApi<P::Block, AccountId>,
{
	fn pending_incoming_transfers_for(
		&self,
		account_id: AccountId,
	) -> Result<Vec<(AccountId, CommunityIdentifier, BalanceType)>> {
		Ok(self
			.pool
			.ready()
			.map(|tx| {
				if tx.data().is_signed().unwrap_or(false) {
					let call = tx.data().call().function;
				}
				(tx.data().origin(), CommunityIdentifier::default(), 0.into())
			})
			.collect())
	}
}
