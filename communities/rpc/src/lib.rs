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
    /// Create new `Communities` with the given reference to the client.
    pub fn new(client: Arc<C>, storage: S) -> Self {
        Communities {
            client,
            storage: Arc::new(RwLock::new(storage)),
            _marker: Default::default(),
        }
    }

    pub fn cache_dirty(&self) -> bool {
        let option_dirty = self.storage.read().get(STORAGE_PREFIX, CACHE_DIRTY_KEY);

        if option_dirty.is_none() {
            log::warn!("Dirty is none")
        }

        option_dirty.map_or_else(
            || true,
            |d| {
                Decode::decode(&mut d.as_slice()).unwrap_or_else(|e| {
                    log::warn!("{:?}", e);
                    true
                })
            },
        )
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
        if !self.cache_dirty() {
            let cids = self.get_storage(b"cids");
            if cids.is_some() {
                log::info!("Using cached community names: {:?}", cids);
                return Ok(cids.unwrap());
            }
        }

        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        let cids = api.get_cids(&at).map_err(runtime_error_into_rpc_err)?;
        let mut cid_names: Vec<CidName> = vec![];

        for cid in cids.iter() {
            match self.get_storage(cid.as_ref()) {
                Some(name) => cid_names.push(CidName::new(*cid, name)),
                None => api
                    .get_name(&at, cid)
                    .map_err(runtime_error_into_rpc_err)?
                    // simply warn that about the cid in question and continue with other ones
                    .map_or_else(
                        || warn_storage_inconsistency(cid),
                        |name| cid_names.push(CidName::new(*cid, name)),
                    ),
            };
        }

        self.set_storage(b"cids", &cid_names);
        self.set_storage(CACHE_DIRTY_KEY, &false);

        Ok(cid_names)
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
