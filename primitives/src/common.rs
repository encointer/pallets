use codec::{Decode, Encode};
use sp_core::RuntimeDebug;

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
pub enum IpfsValidationError {
    /// Invalid length supplied. Should be 46. Is: \[length\]
    InvalidLength(u8),
    InvalidBase58(Bs58Error),
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
