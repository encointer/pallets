use jsonrpc_core::{Error, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::{Decode, Encode, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

use encointer_communities_rpc_runtime_api::CommunitiesApi as CommunitiesRuntimeApi;
use encointer_primitives::communities::{consts::CACHE_DIRTY_KEY, CidName, CommunityIdentifier};
use parking_lot::RwLock;
use sp_api::offchain::{OffchainStorage, STORAGE_PREFIX};

#[rpc]
pub trait CommunitiesApi<BlockHash> {
    #[rpc(name = "communities_getCidNames")]
    fn community_cid_names(&self, at: Option<BlockHash>) -> Result<Vec<CidName>>;
}

pub struct Communities<Client, Block, S> {
    client: Arc<Client>,
    storage: Arc<RwLock<S>>,
    _marker: std::marker::PhantomData<Block>,
}

impl<C, B, S> Communities<C, B, S>
where
    S: 'static + OffchainStorage,
{
    /// Create new `Communities` with the given reference to the client and to the offchain storage
    pub fn new(client: Arc<C>, storage: S) -> Self {
        Communities {
            client,
            storage: Arc::new(RwLock::new(storage)),
            _marker: Default::default(),
        }
    }

    /// Check cache was marked dirty by the runtime
    pub fn cache_dirty(&self) -> bool {
        match self.storage.read().get(STORAGE_PREFIX, CACHE_DIRTY_KEY) {
            Some(d) => Decode::decode(&mut d.as_slice()).unwrap_or_else(|e| {
                log::error!("Cache dirty bit: {:?}", e);
                log::info!("Defaulting to dirty == true");
                true
            }),
            None => {
                // can also be none if no community was registered.
                log::warn!("Cache dirty bit is none, is offchain-indexing enabled?");
                true
            }
        }
    }

    pub fn get_storage<V: Decode>(&self, key: &[u8]) -> Option<V> {
        match self.storage.read().get(STORAGE_PREFIX, key) {
            Some(v) => Some(Decode::decode(&mut v.as_slice()).unwrap()),
            None => None,
        }
    }

    pub fn set_storage<V: Encode>(&self, key: &[u8], val: &V) {
        self.storage.write().set(STORAGE_PREFIX, key, &val.encode());
    }
}

impl<C, Block, S> CommunitiesApi<<Block as BlockT>::Hash> for Communities<C, Block, S>
where
    Block: BlockT,
    C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: CommunitiesRuntimeApi<Block>,
    S: 'static + OffchainStorage,
{
    fn community_cid_names(&self, at: Option<<Block as BlockT>::Hash>) -> Result<Vec<CidName>> {
        let cids_key = b"cids";
        if !self.cache_dirty() {
            return match self.get_storage(cids_key) {
                Some(cids) => {
                    // should only be None if no community was registered
                    log::info!("Using cached community names: {:?}", cids);
                    Ok(cids)
                }
                None => Err(storage_not_found_error(cids_key)),
            };
        }

        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        let cids = api.get_cids(&at).map_err(runtime_error_into_rpc_err)?;
        let mut cid_names: Vec<CidName> = vec![];

        cids.iter().for_each(|cid| {
            self.get_storage(cid.as_ref()).map_or_else(
                || warn_storage_inconsistency(cid),
                |name| cid_names.push(CidName::new(*cid, name)),
            )
        });

        self.set_storage(cids_key, &cid_names);
        self.set_storage(CACHE_DIRTY_KEY, &false);

        Ok(cid_names)
    }
}

/// Arbitrary number, but substrate uses the same
const RUNTIME_ERROR: i64 = 1;
const STORAGE_NOT_FOUND_ERROR: i64 = 1;

/// Converts a runtime trap into an RPC error.
fn runtime_error_into_rpc_err(err: impl std::fmt::Debug) -> Error {
    Error {
        code: ErrorCode::ServerError(RUNTIME_ERROR),
        message: "Runtime trapped".into(),
        data: Some(format!("{:?}", err).into()),
    }
}

fn storage_not_found_error(key: impl std::fmt::Debug) -> Error {
    Error {
        code: ErrorCode::ServerError(STORAGE_NOT_FOUND_ERROR),
        message: "Offchain storage not found".into(),
        data: Some(format!("Key {:?}", key).into()),
    }
}

/// This should never happen!
fn warn_storage_inconsistency(cid: &CommunityIdentifier) {
    log::warn!("Storage inconsistency. Could not find cid: {:?} in runtime storage. This is a fatal bug in the pallet", cid)
}
