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
//! - issuing personhood uniqueness rating aka anti-sybil rating

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{decl_error, decl_event, decl_module, dispatch::DispatchResult, ensure};
use frame_system::ensure_signed;
use log::{debug, warn};
use polkadot_parachain::primitives::Sibling;
use rstd::prelude::*;
use sp_core::H256;
use sp_runtime::traits::{AccountIdConversion, Verify};
use xcm::v1::{Error as XcmError, OriginKind, SendXcm, Xcm};

use encointer_primitives::{
    ceremonies::{ProofOfAttendance, Reputation},
    sybil::{
        sibling_junction, OpaqueRequest, PersonhoodUniquenessRating, RequestHash, SybilResponseCall, CallMetadata
    },
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
    type Event: From<Event> + Into<<Self as frame_system::Config>::Event>;
    /// The XCM sender module.
    type XcmSender: SendXcm;
}

type ProofOfAttendanceOf<T> =
    ProofOfAttendance<<T as Ceremonies>::Signature, <T as frame_system::Config>::AccountId>;

decl_event! {
    pub enum Event
    {
        /// Received PersonhoodUniquenessRating request \[request_hash, parachain\]
        PersonhoodUniquenessRatingRequestReceived(H256, u32),
        /// Successfully sent PersonhoodUniquenessRating response \[request_hash, parachain\]
        PersonhoodUniquenessRatingSentSuccess(H256, u32),
        /// Failed to send PersonhoodUniquenessRating response \[request_hash, parachain\]
        PersonhoodUniquenessRatingSentFailure(H256, u32, XcmError),
    }
}

decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        fn deposit_event() = default;
        type Error = Error<T>;

        /// Returns `PersonhoodUniquenessRating` based on the  `ProofOfAttendance`s that are
        /// encodedly passed as an `OpaqueRequest` to this call.
        ///
        /// The response is sent back to the sending chain again as an XCM and calls the function
        /// defined in the `CallMetadata`.
        ///
        /// Todo: The total weight of this call should include the weight of `CallMetadata` that
        /// is passed here.
        #[weight = 5_000_000]
        fn issue_personhood_uniqueness_rating(
        origin,
        rating_request: OpaqueRequest,
        response: CallMetadata
        ) {
            let sender = ensure_signed(origin)?;
            let para_id: u32 = Sibling::try_from_account(&sender)
                .ok_or(<Error<T>>::UnableToDecodeRequest)?
                .into();
            let request = <Vec<ProofOfAttendanceOf<T>>>::decode(&mut rating_request.as_slice())
                .map_err(|_| <Error<T>>::OnlyParachainsAllowed)?;

            debug!(target: LOG, "received proof of personhood-oracle from parachain: {:?}", para_id);
            debug!(target: LOG, "received proof of personhood-oracle request: {:?}", request);

            let confidence = Self::verify(request).unwrap_or_else(|_| PersonhoodUniquenessRating::default());

            let request_hash = rating_request.hash();
            let call =  SybilResponseCall::new(&response, request_hash, confidence);
            let message = Xcm::Transact { origin_type: OriginKind::SovereignAccount, require_weight_at_most: response.weight(), call: call.encode().into() };

            match T::XcmSender::send_xcm(sibling_junction(para_id).into(), message) {
                Ok(()) => Self::deposit_event(Event::PersonhoodUniquenessRatingSentSuccess(request_hash, para_id)),
                Err(e) => Self::deposit_event(Event::PersonhoodUniquenessRatingSentFailure(request_hash, para_id, e)),
            }
        }
    }
}

decl_error! {
    pub enum Error for Module<T: Config> {
        /// Only other parachains can call this function
        OnlyParachainsAllowed,
        /// Unable to decode PersonhoodUniquenessRating request
        UnableToDecodeRequest,
        /// former attendance has not been verified or has already been linked to other account
        AttendanceUsed,
        /// Bad Signature
        BadSignature,
    }
}

impl<T: Config> Module<T> {
    /// Verifies ProofOfAttendances and returns a PersonhoodUniquenessRating upon success
    pub fn verify(
        request: Vec<ProofOfAttendance<<T as Ceremonies>::Signature, T::AccountId>>,
    ) -> Result<PersonhoodUniquenessRating, DispatchError> {
        let mut c_index_min = <encointer_scheduler::Module<T>>::current_ceremony_index();
        let mut n_attested = 0;
        let mut attested = Vec::new();

        for proof in request.iter() {
            if !<encointer_communities::Module<T>>::community_identifiers()
                .contains(&proof.community_identifier)
            {
                warn!(
                    target: LOG,
                    "Received ProofOfAttendance for unknown cid: {:?}",
                    proof.community_identifier
                );
                continue;
            }
            if Self::verify_proof_of_attendance(&proof).is_ok() {
                c_index_min = min(proof.ceremony_index, c_index_min);
                n_attested += 1;
                attested.push(proof.hash())
            }
        }
        let last_n_ceremonies = <encointer_scheduler::Module<T>>::current_ceremony_index()
            .checked_sub(c_index_min)
            .expect("Proofs can't be valid with bogus ceremony index; qed");

        Ok(PersonhoodUniquenessRating::new(
            n_attested,
            last_n_ceremonies,
            attested,
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
            Error::<T>::AttendanceUsed
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
            Error::<T>::BadSignature
        );
        Ok(())
    }
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

