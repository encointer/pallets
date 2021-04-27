//! Runtime API definition required by Communities RPC extensions.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
use rstd::vec::Vec;

use encointer_primitives::{common::PalletString, communities::CommunityIdentifier};

sp_api::decl_runtime_apis! {

    pub trait CommunitiesApi {
        fn get_cids() -> Vec<CommunityIdentifier>;
        fn get_name(cid: &CommunityIdentifier) -> Option<PalletString>;
    }
}
