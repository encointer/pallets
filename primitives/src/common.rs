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

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub enum Bs58Error {
    /// Non ascii character at index
    NonAsciiCharacter(u8),
    /// Non bs58 character at index
    NonBs58Character(u8),
}
