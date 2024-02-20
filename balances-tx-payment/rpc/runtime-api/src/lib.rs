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

//! Runtime API definition required by Ceremonies RPC extensions.

#![cfg_attr(not(feature = "std"), no_std)]

use core::fmt;
use frame_support::pallet_prelude::TypeInfo;
use parity_scale_codec::{Decode, Encode};
#[cfg(not(feature = "std"))]
use sp_std::vec::Vec;

/// Error type of this RPC api.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Debug, TypeInfo)]
pub enum Error {
	/// The call to runtime failed.
	RuntimeError,
	/// Decode Error
	DecodeError,
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let msg = match self {
			Error::RuntimeError => "Runtime Error",
			Error::DecodeError => "Decode Error",
		};
		write!(f, "{msg}")
	}
}

impl From<Error> for i32 {
	fn from(e: Error) -> i32 {
		match e {
			Error::RuntimeError => 1,
			Error::DecodeError => 2,
		}
	}
}

sp_api::decl_runtime_apis! {
	pub trait BalancesTxPaymentApi<Balance, AssetId, AssetBalance> where
		Balance: Encode + Decode,
		AssetId: Encode + Decode,
		AssetBalance: Encode + Decode,
	{
		fn balance_to_asset_balance(balance:Balance, asset_id:AssetId) -> Result<AssetBalance, Error>;
	}
}
