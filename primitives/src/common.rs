use codec::{Decode, Encode};
use consts::MAX_HASH_SIZE;
use rstd::vec::Vec;
use sp_core::RuntimeDebug;

/// Substrate runtimes provide no string type. Hence, for arbitrary data of varying length the
/// `Vec<u8>` is used. In the polkadot-js the typedef `Text` is used to automatically
/// utf8 decode bytes into a string.
pub type PalletString = Vec<u8>;

pub type IpfsCid = PalletString;

pub fn validate_ipfs_cid(cid: &IpfsCid) -> Result<(), IpfsValidationError> {
    if cid.len() != MAX_HASH_SIZE {
        return Err(IpfsValidationError::InvalidLength);
    }
    Bs58verify::verify(&cid).map_err(|e| IpfsValidationError::InvalidBase58(e))
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub enum IpfsValidationError {
    InvalidLength,
    InvalidBase58(Bs58Error),
}

pub mod consts {
    // Only valid for current hashing algorithm of IPFS (sha256)
    // string length: 46 characters (base-58)
    pub const MAX_HASH_SIZE: usize = 46;
}

/// Simple Bs58 verification adapted from https://github.com/mycorrhiza/bs58-rs
pub struct Bs58verify {}

impl Bs58verify {
    const BITCOIN_ALPHABET: &'static [u8] =
        b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    const BITCOIN_DECODE_MAP: fn() -> [u8; 128] = || -> [u8; 128] {
        let mut decode = [0xFF; 128];

        let mut i = 0;
        while i < Self::BITCOIN_ALPHABET.len() {
            decode[Self::BITCOIN_ALPHABET[i] as usize] = i as u8;
            i += 1;
        }
        return decode;
    };

    pub fn verify(bytes: &[u8]) -> Result<(), Bs58Error> {
        for (i, c) in bytes.iter().enumerate() {
            if *c > 127 {
                return Err(Bs58Error::NonAsciiCharacter(i as u8));
            }

            if Self::BITCOIN_DECODE_MAP()[*c as usize] as usize == 0xFF {
                return Err(Bs58Error::NonBs58Character(i as u8));
            }
        }
        Ok(())
    }
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub enum Bs58Error {
    /// Non ascii character at index
    NonAsciiCharacter(u8),
    /// Non bs58 character at index
    NonBs58Character(u8),
}
