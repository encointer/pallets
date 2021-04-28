use jsonrpc_core::{Result, Error, ErrorCode};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

use encointer_communities_rpc_runtime_api::CommunitiesApi as CommunitiesRuntimeApi;
use encointer_primitives::common::PalletString;
use encointer_primitives::communities::CommunityIdentifier;

#[rpc]
pub trait CommunitiesApi<BlockHash> {
    #[rpc(name = "communities_getNames")]
    fn community_names(&self, at: Option<BlockHash>) -> Result<Vec<PalletString>>;
}

pub struct Communities<Client, Block> {
    client: Arc<Client>,
    _marker: std::marker::PhantomData<Block>,
}

impl<C, B> Communities<C, B> {
    /// Create new `Communities` with the given reference to the client.
    pub fn new(client: Arc<C>) -> Self {
        Communities {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block> CommunitiesApi<<Block as BlockT>::Hash> for Communities<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: CommunitiesRuntimeApi<Block>,
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