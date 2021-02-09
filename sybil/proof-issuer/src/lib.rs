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

//! # Encointer Sybil Proof Issuer Module (WIP, untested)
//!
//! provides functionality for
//! - issuing and verifying digital proof of personhood confidence aka anti-sybil confidence

#![cfg_attr(not(feature = "std"), no_std)]

use codec::Encode;
use frame_support::{debug, decl_event, decl_module, dispatch::DispatchResult, ensure};
use frame_system::ensure_signed;
use polkadot_parachain::primitives::Sibling;
use rstd::prelude::*;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::traits::Verify;
use xcm::v0::{Error as XcmError, Junction, OriginKind, SendXcm, Xcm};

use encointer_primitives::{
    ceremonies::{ProofOfAttendance, Reputation},
    sybil::{ProofOfPersonhoodConfidence, ProofOfPersonhoodRequest, SetProofOfPersonHoodCall},
};

const LOG: &str = "encointer";

use encointer_ceremonies::Config as Ceremonies;
use sp_runtime::sp_std::cmp::min;
use sp_runtime::DispatchError;

pub trait Config:
    frame_system::Config
    + Ceremonies
    + encointer_scheduler::Config
    + encointer_balances::Config
    + encointer_communities::Config
{
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
    /// The XCM sender module.
    type XcmSender: SendXcm;
}

pub type SetProofOfPersonhoodCall<AccountId> = ([u8; 2], AccountId, ProofOfPersonhoodConfidence);

decl_event! {
    pub enum Event<T>
    where AccountId = <T as frame_system::Config>::AccountId,
    {
        ProofOfPersonHoodRequestReceived(AccountId),
        ProofOfPersonHoodSentSuccess(AccountId),
        ProofOfPersonHoodSentFailure(AccountId, XcmError),
    }
}

decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        #[weight = 5_000_000]
        fn issue_proof_of_personhood_confidence(
        origin,
        requester: T::AccountId,
        proof_of_person_hood_request: ProofOfPersonhoodRequest<<T as Ceremonies>::Signature, T::AccountId>,
        sender_sybil_gate: u8
        ) {
            debug::RuntimeLogger::init();
            let sender = ensure_signed(origin)?;
            let para_id: u32 = Sibling::try_from_account(&sender)
                .ok_or("[EncointerSybilGate]: Can only call `issue_proof_of_personhood` from another parachain")?
                .into();

            debug::debug!(target: LOG, "received proof of personhood from parachain: {:?}", para_id);
            debug::debug!(target: LOG, "received proof of personhood request: {:?}", proof_of_person_hood_request);

            let confidence = Self::verify(proof_of_person_hood_request).unwrap_or_else(|_| ProofOfPersonhoodConfidence::default());

            let location = Junction::Parachain { id: para_id };
            // Todo: Don't use hardcoded 1 here
            let call =  SetProofOfPersonHoodCall::new(sender_sybil_gate, requester.clone(), confidence);
            let message = Xcm::Transact { origin_type: OriginKind::SovereignAccount, call: call.encode() };
            match T::XcmSender::send_xcm(location.into(), message.into()) {
                Ok(()) => Self::deposit_event(RawEvent::ProofOfPersonHoodSentSuccess(requester)),
                Err(e) => Self::deposit_event(RawEvent::ProofOfPersonHoodSentFailure(requester, e)),
            }
        }
    }
}

impl<T: Config> Module<T> {
    fn verify(
        request: ProofOfPersonhoodRequest<<T as Ceremonies>::Signature, T::AccountId>,
    ) -> Result<ProofOfPersonhoodConfidence, DispatchError> {
        let mut c_index_min = 0;
        let mut n_attested = 0;
        for (cid, poa) in request.iter() {
            if !<encointer_communities::Module<T>>::community_identifiers().contains(&cid) {
                continue;
            }
            if Self::verify_proof_of_attendance(&poa).is_ok() {
                c_index_min = min(poa.ceremony_index, c_index_min);
                n_attested += 1;
            }
        }
        let last_n_ceremonies = <encointer_scheduler::Module<T>>::current_ceremony_index()
            .checked_sub(c_index_min)
            .expect("Proofs can't be valid with bogus ceremony index; qed");

        Ok(ProofOfPersonhoodConfidence::new(
            n_attested,
            last_n_ceremonies,
        ))
    }

    fn verify_proof_of_attendance(
        p: &ProofOfAttendance<<T as Ceremonies>::Signature, T::AccountId>,
    ) -> DispatchResult {
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
        proof: &ProofOfAttendance<<T as Ceremonies>::Signature, T::AccountId>,
    ) -> DispatchResult {
        ensure!(
            proof.attendee_signature.verify(
                &(proof.prover_public.clone(), proof.ceremony_index.clone()).encode()[..],
                &proof.attendee_public,
            ),
            "bad attendee signature"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests;
