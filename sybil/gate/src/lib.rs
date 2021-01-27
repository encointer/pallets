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

//! # Encointer Sybil Proof Request Module
//!
//! provides functionality for
//! - requesting digital proof of personhood confidence aka anti-sybil confidence

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{debug, decl_event, decl_module, decl_storage, RuntimeDebug};
use frame_system::ensure_signed;
use rstd::prelude::*;
use sp_core::H256;
use xcm::v0::{Error as XcmError, Junction, OriginKind, SendXcm, Xcm};

const LOG: &str = "encointer";

pub trait Trait: frame_system::Trait {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    /// The XCM sender module.
    type XcmSender: SendXcm;
    /// Runtime Call type, used for cross-messaging calls.
    type Call: Encode + From<<Self as frame_system::Trait>::Call>;
}

pub type ProofOfPersonhoodRequest = Vec<(H256, Vec<u8>)>; // Todo: Replace with ProofOfAttendance Type
pub type IssueProofOfPersonhoodCall = ([u8; 2], ProofOfPersonhoodRequest);

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct ProofOfPersonhoodConfidence {
    attested: u8,
    last_n_ceremonies: u8,
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
        fn request_proof_of_personhood_confidence(
            origin,
            parachain_id: u32,
            pallet_sybil_proof_issuer_index: u8,
            request: ProofOfPersonhoodRequest
        ) {
            let sender = ensure_signed(origin)?;
            let location = Junction::Parachain { id: parachain_id };
            // Todo: use actual call_index from proof issuer
            let call: IssueProofOfPersonhoodCall = ([pallet_sybil_proof_issuer_index, 0], vec![]);
            let message = Xcm::Transact { origin_type: OriginKind::SovereignAccount, call: call.encode() };
            match T::XcmSender::send_xcm(location.into(), message.into()) {
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
            let sender = ensure_signed(origin)?;
            <ProofOfPersonhood<T>>::insert(account, confidence);
            Self::deposit_event(RawEvent::StoredProofOfPersonHoodConfidence(sender))
        }
    }
}

#[cfg(test)]
mod tests;
