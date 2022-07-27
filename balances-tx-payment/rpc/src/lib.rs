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

use codec::Codec;
use encointer_balances_tx_payment_rpc_runtime_api::{
	BalancesTxPaymentApi as BalancesTxPaymentApiRuntimeApi, Error,
};
//use encointer_rpc::Error;
use jsonrpsee::{
	core::{Error as JsonRpseeError, RpcResult},
	proc_macros::rpc,
	types::error::{CallError, ErrorCode, ErrorObject},
};
pub use pallet_transaction_payment::RuntimeDispatchInfo;
use pallet_transaction_payment::{FeeDetails, InclusionFee};
use pallet_transaction_payment_rpc::TransactionPaymentApiServer;
use sp_api::{Decode, Encode, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_core::Bytes;
use sp_rpc::number::NumberOrHex;
use sp_runtime::{
	generic::BlockId,
	traits::{Block as BlockT, MaybeDisplay},
};
use std::sync::Arc;

#[rpc(client, server)]
pub trait BalancesTxPaymentApi<BlockHash, AssetId>
where
	AssetId: 'static + Encode + Decode + Send + Sync,
{
	#[method(name = "encointer_queryAssetFeeDetails", blocking)]
	fn query_asset_fee_details(
		&self,
		asset_id: AssetId,
		encoded_xt: Bytes,
		at: Option<BlockHash>,
	) -> RpcResult<FeeDetails<NumberOrHex>>;
}

pub struct BalancesTxPaymentRpc<C, P, Q, R, S> {
	client: Arc<C>,
	transaction_payment: P,
	_marker: std::marker::PhantomData<Q>,
	_marker2: std::marker::PhantomData<R>,
	_marker3: std::marker::PhantomData<S>,
}

impl<C, P, Q, R, S> BalancesTxPaymentRpc<C, P, Q, R, S> {
	pub fn new(client: Arc<C>, transaction_payment: P) -> Self {
		BalancesTxPaymentRpc {
			client,
			transaction_payment,
			_marker: Default::default(),
			_marker2: Default::default(),
			_marker3: Default::default(),
		}
	}
}

impl<C, P, Block, AssetId, Balance, AssetBalance>
	BalancesTxPaymentApiServer<<Block as BlockT>::Hash, AssetId>
	for BalancesTxPaymentRpc<C, P, Block, Balance, AssetBalance>
where
	AssetId: 'static + Clone + Copy + Encode + Decode + Send + Sync + PartialEq,
	AssetBalance: 'static
		+ Clone
		+ Encode
		+ Decode
		+ Send
		+ Sync
		+ PartialEq
		+ Into<NumberOrHex>
		+ MaybeDisplay
		+ Copy,
	Block: BlockT,
	P: TransactionPaymentApiServer<<Block as BlockT>::Hash, RuntimeDispatchInfo<Balance>>,
	C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
	C::Api: BalancesTxPaymentApiRuntimeApi<Block, Balance, AssetId, AssetBalance>,
	Balance:
		Codec + MaybeDisplay + Copy + TryInto<NumberOrHex> + Send + Sync + 'static + From<u128>,
{
	fn query_asset_fee_details(
		&self,
		asset_id: AssetId,
		encoded_xt: Bytes,
		at: Option<Block::Hash>,
	) -> RpcResult<FeeDetails<NumberOrHex>> {
		let api = self.client.runtime_api();

		let balance_fee_details = self.transaction_payment.query_fee_details(encoded_xt, at)?;

		let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

		let try_into_rpc_balance = |value: AssetBalance| {
			value.try_into().map_err(|_| {
				JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
					ErrorCode::InvalidParams.code(),
					format!("{} doesn't fit in NumberOrHex representation", value),
					None::<()>,
				)))
			})
		};

		Ok(FeeDetails {
			inclusion_fee: if let Some(inclusion_fee) = balance_fee_details.inclusion_fee {
				let base_fee = api
					.balance_to_asset_balance(
						&at,
						inclusion_fee.base_fee.into_u256().as_u128().into(),
						asset_id,
					)
					.map_err(|e| {
						CallError::Custom(ErrorObject::owned(
							Error::RuntimeError.into(),
							"Unable to query balance conversion.",
							Some(e.to_string()),
						))
					})?
					.map_err(|_e| {
						CallError::Custom(ErrorObject::owned(
							Error::RuntimeError.into(),
							"Unable to query balance conversion.",
							Some("Unable to query balance conversion."),
						))
					})?;

				let len_fee = api
					.balance_to_asset_balance(
						&at,
						inclusion_fee.len_fee.into_u256().as_u128().into(),
						asset_id,
					)
					.map_err(|e| {
						CallError::Custom(ErrorObject::owned(
							Error::RuntimeError.into(),
							"Unable to query fee details.",
							Some(e.to_string()),
						))
					})?
					.map_err(|_e| {
						CallError::Custom(ErrorObject::owned(
							Error::RuntimeError.into(),
							"Unable to query balance conversion.",
							Some("Unable to query balance conversion."),
						))
					})?;

				let adjusted_weight_fee = api
					.balance_to_asset_balance(
						&at,
						inclusion_fee.adjusted_weight_fee.into_u256().as_u128().into(),
						asset_id,
					)
					.map_err(|e| {
						CallError::Custom(ErrorObject::owned(
							Error::RuntimeError.into(),
							"Unable to query fee details.",
							Some(e.to_string()),
						))
					})?
					.map_err(|_e| {
						CallError::Custom(ErrorObject::owned(
							Error::RuntimeError.into(),
							"Unable to query balance conversion.",
							Some("Unable to query balance conversion."),
						))
					})?;

				Some(InclusionFee {
					base_fee: try_into_rpc_balance(base_fee)?,
					len_fee: try_into_rpc_balance(len_fee)?,
					adjusted_weight_fee: try_into_rpc_balance(adjusted_weight_fee)?,
				})
			} else {
				None
			},
			tip: Default::default(),
		})
	}
}
