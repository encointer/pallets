use jsonrpc_core::{Error, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::{Decode, Encode, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

use encointer_communities_rpc_runtime_api::CommunitiesApi as CommunitiesRuntimeApi;
use encointer_primitives::common::PalletString;
use encointer_primitives::communities::CommunityIdentifier;
use parking_lot::RwLock;
use sp_api::offchain::{OffchainStorage, STORAGE_PREFIX};

// Todo: consolidate declaration in primitives once sybil stuff is feature gated
const CACHE_DIRTY: &[u8] = b"dirty";

#[rpc]
pub trait CommunitiesApi<BlockHash> {
    #[rpc(name = "communities_getNames")]
    fn community_names(&self, at: Option<BlockHash>) -> Result<Vec<PalletString>>;
}

pub struct Communities<Client, Block, S> {
    client: Arc<Client>,
    storage: Arc<RwLock<S>>,
    _marker: std::marker::PhantomData<Block>,
}

impl<C, B, S> Communities<C, B, S> {
    /// Create new `Communities` with the given reference to the client.
    pub fn new(client: Arc<C>, storage: S) -> Self {
        Communities {
            client,
            storage: Arc::new(RwLock::new(storage)),
            _marker: Default::default(),
        }
    }
}

impl<C, Block, S> CommunitiesApi<<Block as BlockT>::Hash> for Communities<C, Block, S>
where
    Block: BlockT,
    C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: CommunitiesRuntimeApi<Block>,
    S: 'static + OffchainStorage,
{
    fn community_names(&self, at: Option<<Block as BlockT>::Hash>) -> Result<Vec<PalletString>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let option_dirty = self.storage.read().get(STORAGE_PREFIX, CACHE_DIRTY);

        if option_dirty.is_none() {
            log::warn!("Dirty is none")
        }

        let dirty = option_dirty.map_or_else(
            || true,
            |d| {
                Decode::decode(&mut d.as_slice()).unwrap_or_else(|e| {
                    log::warn!("{:?}", e);
                    true
                })
            },
        );

        let cache = || self.storage.read().get(STORAGE_PREFIX, b"cids");

        if !dirty & cache().is_some() {
            let c = Decode::decode(&mut cache().unwrap().as_slice()).unwrap();
            log::warn!("Using cached value: {:?}", c);
            return Ok(c);
        } else {
            let cids = api.get_cids(&at).map_err(runtime_error_into_rpc_err)?;
            println!("Cids {:?}", cids);
            let mut names: Vec<PalletString> = vec![];

            for cid in cids.iter() {
                api.get_name(&at, cid)
                    .map_err(runtime_error_into_rpc_err)?
                    // simply warn that about the cid in question and continue with other ones
                    .map_or_else(|| warn_storage_inconsistency(cid), |name| names.push(name));
            }

            self.storage
                .write()
                .set(STORAGE_PREFIX, b"cids", &names.encode());

            self.storage
                .write()
                .set(STORAGE_PREFIX, CACHE_DIRTY, &false.encode());

            Ok(names)
        }
    }
}

/// Arbitrary number, but substrate uses the same
const RUNTIME_ERROR: i64 = 1;

/// Converts a runtime trap into an RPC error.
fn runtime_error_into_rpc_err(err: impl std::fmt::Debug) -> Error {
    Error {
        code: ErrorCode::ServerError(RUNTIME_ERROR),
        message: "Runtime trapped".into(),
        data: Some(format!("{:?}", err).into()),
    }
}

/// This should never happen!
fn warn_storage_inconsistency(cid: &CommunityIdentifier) {
    log::warn!("Storage inconsistency. Could not find cid: {:?} in runtime storage. This is a fatal bug in the pallet", cid)
}
