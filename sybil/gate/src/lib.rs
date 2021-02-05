// Copyright (c) 2019 Alain Brenzikofer
// This file is part of Encointer
//
// Encointer is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Encointer is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Encointer.  If not, see <http://www.gnu.org/licenses/>.

//! # Encointer Sybil Proof Request Module (WIP)
//!
//! Note: This is a wip, we were able to successfully send XCMP messages and decode them
//! on the receiving parachain. However, we currently get a `XCMPERROR::BadOrigin` when executing
//! the XCM.
//!
//! provides functionality for
//! - requesting digital proof of personhood confidence aka anti-sybil confidence

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use encointer_primitives::sybil::{
    IssueProofOfPersonhoodConfidenceCall, ProofOfPersonhoodConfidence, ProofOfPersonhoodRequest,
};
use frame_support::{debug, decl_event, decl_module, decl_storage};
use frame_system::ensure_signed;
use rstd::prelude::*;
use sp_runtime::traits::{IdentifyAccount, Member, Verify};
use xcm::v0::{Error as XcmError, Junction, OriginKind, SendXcm, Xcm};

const LOG: &str = "encointer";

pub trait Trait: frame_system::Trait {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    /// The XCM sender module.
    type XcmSender: SendXcm;

    type Public: IdentifyAccount<AccountId = Self::AccountId>;
    type Signature: Verify<Signer = <Self as Trait>::Public> + Member + Decode + Encode;
}

decl_storage! {
    trait Store for Module<T: Trait> as EncointerSybilGate {
        ProofOfPersonhood get(fn proof_of_personhood_confidence): map hasher(blake2_128_concat) T::AccountId => ProofOfPersonhoodConfidence;
    }
}

decl_event! {
    pub enum Event<T>
    where AccountId = <T as frame_system::Trait>::AccountId,
    {
        ProofOfPersonHoodRequestSentSuccess(AccountId),
        ProofOfPersonHoodRequestSentFailure(AccountId, XcmError),
        StoredProofOfPersonHoodConfidence(AccountId),
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        #[weight = 5_000_000]
        /// ### Proof of PersonhoodRequest
        ///
        /// Request a ProofOfPersonHood from an encointer-parachain.
        ///
        /// The `pallet_sybil_proof_issuer_index` is the pallet index of the respective encointer-parachain's
        /// `pallet-encointer-sybil-proof-issuer` pallet to query. It was decided to put that as an argument,
        /// as there might be more than one encointer-chain running.
        fn request_proof_of_personhood_confidence(
            origin,
            parachain_id: u32,
            pallet_sybil_proof_issuer_index: u8,
            request: ProofOfPersonhoodRequest<T::Signature, T::AccountId>
        ) {
            debug::RuntimeLogger::init();
            let sender = ensure_signed(origin)?;
            let location = Junction::Parachain { id: parachain_id };

            // todo: use get the runtime configuration's specific pallet index. Currently, this corresponds
            // to the index given in our encointer parachain because we declare the module like this in
            // construct runtime:
            // `EncointerSybilGate: encointer_sybil_gate::{Module, Call, Storage, Event<T>} = 2,`
            let sender_pallet_sybil_gate_index = 15u8;
            // Todo: use actual call_index from proof issuer
            let call: IssueProofOfPersonhoodConfidenceCall<T::Signature, T::AccountId> = ([pallet_sybil_proof_issuer_index, 0], parachain_id, sender_pallet_sybil_gate_index, request);
            let message = Xcm::Transact { origin_type: OriginKind::SovereignAccount, call: call.encode() };
            debug::debug!(target: LOG, "sending ProofOfPersonhoodRequest to chain: {:?}", parachain_id);
            match T::XcmSender::send_xcm(location.into(), message) {
                Ok(()) => Self::deposit_event(RawEvent::ProofOfPersonHoodRequestSentSuccess(sender)),
                Err(e) => Self::deposit_event(RawEvent::ProofOfPersonHoodRequestSentFailure(sender, e)),
            }
        }

        #[weight = 5_000_000]
        fn set_proof_of_personhood_confidence(
            origin,
            account: T::AccountId,
            confidence: ProofOfPersonhoodConfidence
        ) {
            debug::RuntimeLogger::init();
            let sender = ensure_signed(origin)?;
            debug::debug!(target: LOG, "set ProofOfPersonhood Confidence for account: {:?}", account);
            <ProofOfPersonhood<T>>::insert(account, confidence);
            Self::deposit_event(RawEvent::StoredProofOfPersonHoodConfidence(sender))
        }
    }
}

#[cfg(test)]
mod tests;
