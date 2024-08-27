use encointer_primitives::communities::CommunityIdentifier;
use encointer_rpc::Error;
use jsonrpsee::{
    core::RpcResult,
    proc_macros::rpc,
    types::{error::ErrorObject, ErrorObjectOwned},
};
use pallet_encointer_treasuries_rpc_runtime_api::TreasuriesApi as TreasuriesRuntimeApi;
use parity_scale_codec::{Decode, Encode};
use sc_rpc_api::DenyUnsafe;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;
use std::sync::Arc;

#[rpc(client, server)]
pub trait TreasuriesApi<AccountId>
where
    AccountId: 'static + Encode + Decode + Send + Sync,
{
    #[method(name = "encointer_getCommunityTreasuryAccountUnchecked")]
    fn get_community_treasury_account_unchecked(
        &self,
        cid: CommunityIdentifier,
    ) -> RpcResult<AccountId>;
}

pub struct TreasuriesRpc<Client, Block, AccountId> {
    client: Arc<Client>,
    _marker: std::marker::PhantomData<(Block, AccountId)>,
}

impl<Client, Block, AccountId> TreasuriesRpc<Client, Block, AccountId> {
    pub fn new(client: Arc<Client>) -> Self {
        TreasuriesRpc { client, _marker: Default::default() }
    }
}

impl<Client, Block, AccountId> TreasuriesApiServer<AccountId>
for TreasuriesRpc<Client, Block, AccountId>
where
    AccountId: 'static + Clone + Encode + Decode + Send + Sync,
    Block: BlockT,
    Client: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    Client::Api: TreasuriesRuntimeApi<Block, AccountId>,
{
    fn get_community_treasury_account_unchecked(
        &self,
        cid: CommunityIdentifier,
    ) -> RpcResult<AccountId> {
        let api = self.client.runtime_api();
        let at = self.client.info().best_hash;
        Ok(api
            .get_community_treasury_account_unchecked(at, &cid)
            .map_err(|e| Error::Runtime(e.into()))?)
    }
}
