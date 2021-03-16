use rstd::vec::Vec;

/// Substrate runtimes provide no string type. Hence, for arbitrary data of varying length the
/// `Vec<u8>` is used. In the polkadot-js the typedef `Text` is used to automatically
/// utf8 decode bytes into a string.
pub type PalletString = Vec<u8>;

pub type IpfsCid = PalletString;

pub mod consts {
    // Only valid for current hashing algorithm of IPFS (sha256)
    // string length: 46 characters (base-58)
    pub const MAX_HASH_SIZE: usize = 46;
}
