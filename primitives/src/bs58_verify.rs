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
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;

#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};

/// Simple Bs58 verification adapted from https://github.com/mycorrhiza/bs58-rs
pub struct Bs58verify {}

impl Bs58verify {
    /// Obtained with:
    /// ```rust
    ///     const BITCOIN_ALPHABET: &'static [u8] =
    ///         b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    ///
    ///     const BITCOIN_DECODE_MAP: fn() -> [u8; 128] = || -> [u8; 128] {
    ///         let mut decode = [0xFF; 128];
    ///
    ///        let mut i = 0;
    ///         while i < BITCOIN_ALPHABET.len() {
    ///             decode[BITCOIN_ALPHABET[i] as usize] = i as u8;
    ///             i += 1;
    ///         }
    ///         return decode;
    ///     };
    /// ```
    const BITCOIN_DECODE_MAP: [u8; 128] = [
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0xFF, 0x11,
        0x12, 0x13, 0x14, 0x15, 0xFF, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F,
        0x20, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28,
        0x29, 0x2A, 0x2B, 0xFF, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36,
        0x37, 0x38, 0x39, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    ];

    pub fn verify(bytes: &[u8]) -> Result<(), Bs58Error> {
        for (i, c) in bytes.iter().enumerate() {
            if *c > 127 {
                return Err(Bs58Error::NonAsciiCharacter(i as u8));
            }

            if Self::BITCOIN_DECODE_MAP[*c as usize] as usize == 0xFF {
                return Err(Bs58Error::NonBs58Character(i as u8));
            }
        }
        Ok(())
    }
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub enum Bs58Error {
    /// Non ascii character at index
    NonAsciiCharacter(u8),
    /// Non bs58 character at index
    NonBs58Character(u8),
}
