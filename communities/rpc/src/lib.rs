use jsonrpc_core::{Error, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

use communities_rpc_runtime_api::CommunitiesApi as CommunitiesRuntimeApi;
use encointer_primitives::communities::CommunityIdentifier;

#[rpc]
pub trait CommunitiesApi<BlockHash> {
    #[rpc(name = "communities_getNames")]
    fn community_names(&self, at: Option<BlockHash>) -> Result<()>;
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
    fn community_names(&self, at: Option<<Block as BlockT>::Hash>) -> Result<()> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let _res = api.get_cids(&at);

        let _res = api.get_name(&at, CommunityIdentifier::default());
        Ok(())
    }
}
