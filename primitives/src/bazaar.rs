pub type ShopIdentifier = Vec<u8>;
pub type ArticleIdentifier = Vec<u8>;

pub mod consts {
    // Only valid for current hashing algorithm of IPFS (sha256)
    // string length: 46 characters (base-58)
    pub const MAX_HASH_SIZE: usize = 46;
}
