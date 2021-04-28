use jsonrpc_core::{Result, Error, ErrorCode};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

use encointer_communities_rpc_runtime_api::CommunitiesApi as CommunitiesRuntimeApi;
use encointer_primitives::common::PalletString;
use encointer_primitives::communities::CommunityIdentifier;
use sp_api::offchain::OffchainStorage;

#[rpc]
pub trait CommunitiesApi<BlockHash> {
    #[rpc(name = "communities_getNames")]
    fn community_names(&self, at: Option<BlockHash>) -> Result<Vec<PalletString>>;
}

pub struct Communities<Client, Block, S> {
    client: Arc<Client>,
    storage: Option<S>,
    _marker: std::marker::PhantomData<Block>,
}

impl<C, B, S> Communities<C, B, S> {
    /// Create new `Communities` with the given reference to the client.
    pub fn new(client: Arc<C>, storage: Option<S>) -> Self {
        if storage.is_none() {
            log::warn!("Offchain caching disabled, due to lack of offchain storage support in backend.");
        }

        Communities {
            client,
            storage,
            _marker: Default::default(),
        }
    }
}

impl<C, Block, S> CommunitiesApi<<Block as BlockT>::Hash> for Communities<C, Block, S>
where
    Block: BlockT,
    C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: CommunitiesRuntimeApi<Block>,
    S: 'static + OffchainStorage
{
    fn community_names(&self, at: Option<<Block as BlockT>::Hash>) -> Result<Vec<PalletString>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        let cids = api.get_cids(&at)
            .map_err(runtime_error_into_rpc_err)?;

        println!("Cids {:?}", cids);
        let mut names :Vec<PalletString> = vec![];

        for cid in cids.iter() {
            api.get_name(&at, cid)
                .map_err(runtime_error_into_rpc_err)?
                .map_or_else(|| warn_storage_inconsistency(cid) ,|name| names.push(name));
        }

        Ok(names)
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