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

//! # Encointer Sybil Proof Issuer Module
//!
//! provides functionality for
//! - issuing and verifying digital proof of personhood confidence aka anti-sybil confidence

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{debug, decl_event, decl_module, dispatch::DispatchResult, ensure};
use frame_system::ensure_signed;
use rstd::prelude::*;
use sp_runtime::traits::{IdentifyAccount, Member, Verify};
use xcm::v0::{Error as XcmError, Junction, OriginKind, SendXcm, Xcm};

use encointer_primitives::{
    ceremonies::{ProofOfAttendance, Reputation},
    sybil::ProofOfPersonhoodConfidence,
};

const LOG: &str = "encointer";

pub trait Trait:
    frame_system::Trait
    + encointer_ceremonies::Trait
    + encointer_scheduler::Trait
    + encointer_balances::Trait
    + encointer_communities::Trait
{
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    /// The XCM sender module.
    type XcmSender: SendXcm;

    type Public: IdentifyAccount<AccountId = Self::AccountId>;
    type Signature: Verify<Signer = <Self as Trait>::Public> + Member + Decode + Encode;
}

pub type SetProofOfPersonhoodCall = ([u8; 2], ProofOfPersonhoodConfidence);

decl_event! {
    pub enum Event<T>
    where AccountId = <T as frame_system::Trait>::AccountId,
    {
        ProofOfPersonHoodSentSuccess(AccountId),
        ProofOfPersonHoodSentFailure(AccountId, XcmError),
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        #[weight = 5_000_000]
        fn issue_proof_of_personhood_confidence(
        origin,
        sender_parachain_id: u32,
        pallet_sybil_gate_index: u8,
        proof_of_person_hood_request: Vec<u8>
        ) {
            debug::debug!(target: LOG, "received proof of personhood request from parachain: {:?}", sender_parachain_id);
            debug::debug!(target: LOG, "request: {:?}", proof_of_person_hood_request);
            let sender = ensure_signed(origin)?;
            let location = Junction::Parachain { id: sender_parachain_id };
            // Todo: Actually verify request
            // Todo: use actual call_index from sybil gate
            let call: SetProofOfPersonhoodCall = ([pallet_sybil_gate_index, 0], ProofOfPersonhoodConfidence::default());
            let message = Xcm::Transact { origin_type: OriginKind::SovereignAccount, call: call.encode() };
            match T::XcmSender::send_xcm(location.into(), message.into()) {
                Ok(()) => Self::deposit_event(RawEvent::ProofOfPersonHoodSentSuccess(sender)),
                Err(e) => Self::deposit_event(RawEvent::ProofOfPersonHoodSentFailure(sender, e)),
            }
        }
    }
}

impl<T: Trait> Module<T> {
    fn verify(p: ProofOfAttendance<<T as Trait>::Signature, T::AccountId>) -> DispatchResult {
        ensure!(
            <encointer_ceremonies::Module<T>>::participant_reputation(
                &(p.community_identifier, p.ceremony_index),
                &p.attendee_public
            ) == Reputation::VerifiedUnlinked,
            "former attendance has not been verified or has already been linked to other account"
        );
        Self::verify_attendee_signature(p)
    }

    fn verify_attendee_signature(
        proof: ProofOfAttendance<<T as Trait>::Signature, T::AccountId>,
    ) -> DispatchResult {
        ensure!(
            proof.attendee_signature.verify(
                &(proof.prover_public, proof.ceremony_index).encode()[..],
                &proof.attendee_public,
            ),
            "bad attendee signature"
        );
        Ok(())
    }
}
