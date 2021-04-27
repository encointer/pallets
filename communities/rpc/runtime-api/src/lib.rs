//! Runtime API definition required by Communities RPC extensions.

#![cfg_attr(not(feature = "std"), no_std)]

use encointer_primitives::{common::PalletString, communities::CommunityIdentifier};

sp_api::decl_runtime_apis! {

    pub trait CommunitiesApi {
        fn get_name(cid: CommunityIdentifier) -> PalletString;
    }
}
