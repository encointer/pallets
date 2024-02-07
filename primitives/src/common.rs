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

use crate::bs58_verify::{Bs58Error, Bs58verify};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::Len;
use scale_info::TypeInfo;
#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};
use sp_core::{bounded::BoundedVec, ConstU32, RuntimeDebug};

#[cfg(not(feature = "std"))]
use sp_std::vec::Vec;

/// Substrate runtimes provide no string type. Hence, for arbitrary data of varying length the
/// `Vec<u8>` is used. In the polkadot-js the typedef `Text` is used to automatically
/// utf8 decode bytes into a string.

pub type PalletString = BoundedVec<u8, ConstU32<256>>;

pub trait FromStr: Sized {
	type Err;
	fn from_str(inp: &str) -> Result<Self, Self::Err>;
}

impl FromStr for PalletString {
	type Err = Vec<u8>;
	fn from_str(inp: &str) -> Result<Self, Self::Err> {
		Self::try_from(inp.as_bytes().to_vec())
	}
}

pub trait AsByteOrNoop {
	fn as_bytes_or_noop(&self) -> &[u8];
}

impl AsByteOrNoop for PalletString {
	fn as_bytes_or_noop(&self) -> &[u8] {
		self
	}
}

pub type BoundedIpfsCid = PalletString;

pub fn validate_ascii(bytes: &[u8]) -> Result<(), u8> {
	for (i, c) in bytes.iter().enumerate() {
		if *c > 127 {
			return Err(i as u8);
		}
	}
	Ok(())
}

// Only valid for current hashing algorithm of IPFS (sha256)
// string length: 46 bs58 characters (bs58 -> 1 byte/char)
pub const MAX_HASH_SIZE: usize = 46;

pub fn validate_ipfs_cid(cid: &BoundedIpfsCid) -> Result<(), IpfsValidationError> {
	if cid.len() != MAX_HASH_SIZE {
		return Err(IpfsValidationError::InvalidLength(cid.len() as u8));
	}
	Bs58verify::verify(cid.as_bytes_or_noop()).map_err(IpfsValidationError::InvalidBase58)
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub enum IpfsValidationError {
	/// Invalid length supplied. Should be 46. Is: \[length\]
	InvalidLength(u8),
	InvalidBase58(Bs58Error),
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn pallet_string_cropping_works() {
		let data = vec![1u8; 22];
		assert_eq!(PalletString::truncate_from(data.clone()), data);
		let data = vec![1u8; 300];
		assert_eq!(PalletString::truncate_from(data.clone()), data[..256].to_vec());
	}
}
