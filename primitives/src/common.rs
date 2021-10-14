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

use codec::{Decode, Encode};
use sp_core::RuntimeDebug;

#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};

use crate::bs58_verify::{Bs58Error, Bs58verify};

#[cfg(not(feature = "std"))]
use rstd::vec::Vec;

/// Substrate runtimes provide no string type. Hence, for arbitrary data of varying length the
/// `Vec<u8>` is used. In the polkadot-js the typedef `Text` is used to automatically
/// utf8 decode bytes into a string.
#[cfg(not(feature = "std"))]
pub type PalletString = Vec<u8>;

#[cfg(feature = "std")]
pub type PalletString = String;

pub trait AsByteOrNoop {
    fn as_bytes_or_noop(&self) -> &[u8];
}

impl AsByteOrNoop for PalletString {
    #[cfg(feature = "std")]
    fn as_bytes_or_noop(&self) -> &[u8] {
        self.as_bytes()
    }

    #[cfg(not(feature = "std"))]
    fn as_bytes_or_noop(&self) -> &[u8] {
        self
    }
}

pub type IpfsCid = PalletString;

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

pub fn validate_ipfs_cid(cid: &IpfsCid) -> Result<(), IpfsValidationError> {
    if cid.len() != MAX_HASH_SIZE {
        return Err(IpfsValidationError::InvalidLength(cid.len() as u8));
    }
    Bs58verify::verify(&cid.as_bytes_or_noop()).map_err(|e| IpfsValidationError::InvalidBase58(e))
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub enum IpfsValidationError {
    /// Invalid length supplied. Should be 46. Is: \[length\]
    InvalidLength(u8),
    InvalidBase58(Bs58Error),
}
