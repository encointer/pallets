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
//! provides functionality for
//! - requesting digital personhood uniqueness rating aka anti-sybil rating
//! -

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use encointer_primitives::{
    ceremonies::ProofOfAttendance,
    sybil::{
        sibling_junction, IssuePersonhoodUniquenessRatingCall, PersonhoodUniquenessRating,
        SybilResponse,
    },
};
use fixed::types::I16F16;
use frame_support::traits::Currency;
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, ensure, traits::PalletInfo,
};
use frame_system::ensure_signed;
use log::debug;
use polkadot_parachain::primitives::Sibling;
use rstd::prelude::*;
use sp_core::H256;
use sp_runtime::traits::{AccountIdConversion, CheckedConversion, IdentifyAccount, Member, Verify};
use xcm::v0::{Error as XcmError, OriginKind, SendXcm, Xcm};

const LOG: &str = "encointer";

pub trait Config: frame_system::Config {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
    /// The XCM sender module.
    type XcmSender: SendXcm;

    type Currency: Currency<<Self as frame_system::Config>::AccountId>;

    type Public: IdentifyAccount<AccountId = Self::AccountId>;
    type Signature: Verify<Signer = <Self as Config>::Public> + Member + Decode + Encode;
}

decl_storage! {
    trait Store for Module<T: Config> as EncointerSybilGate {
        /// XCM PersonhoodUniquenessRating requests sent to another parachain that have not yielded a response yet
        PendingRequests get(fn pending_requests): map hasher(identity) H256 => T::AccountId;
        /// The proof of attendances that have already been used in a previous request
        /// Membership checks are faster with maps than with vecs, see: https://substrate.dev/recipes/map-set.html.
        ///
        /// This is a double_map, as more requests might be added, and ProofOfAttendances are allowed to be used per request
        BurnedProofs get(fn burned_proofs): double_map hasher(blake2_128_concat) SybilResponse, hasher(blake2_128_concat) H256 => ();
    }
}

decl_event! {
    pub enum Event<T>
    where AccountId = <T as frame_system::Config>::AccountId,
    {
        /// An account has successfully sent a request to another parachain \[requester, request_hash, parachain\]
        PersonhoodUniquenessRatingRequestSentSuccess(AccountId, H256, u32),
        /// Failed to send request to another parachain \[requester, request_hash, xcm error\]
        PersonhoodUniquenessRatingRequestSentFailure(AccountId, H256, XcmError),
        /// Faucet dripped some funds to account \[funded_account\]
        FautetDrippedTo(AccountId),
        /// Faucet rejected dripping funds due to weak PersonhoodUniquenessRating \[rejected_account\]
        FaucetRejectedDueToWeakPersonhoodUniquenessRating(AccountId),
        /// Faucet rejected dripping funds due to reuse of ProofOfAttendances \[rejected_account\]
        FaucetRejectedDueToProofReuse(AccountId),
    }
}

decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        fn deposit_event() = default;
        type Error = Error<T>;

        #[weight = 5_000_000]
        /// ### Proof of PersonhoodRequest
        ///
        /// Request a PersonhoodUniquenessRating from an encointer-parachain.
        ///
        /// The `pallet_personhood_oracle_index` is the pallet's module index of the respective encointer-parachain's
        /// `pallet-encointer-personhood-oracle` pallet to query.
        fn request_personhood_uniqueness_rating(
            origin,
            parachain_id: u32,
            pallet_personhood_oracle_index: u8,
            proof_of_attendances: Vec<Vec<u8>>,
            requested_response: SybilResponse
        ) {
            let sender = ensure_signed(origin)?;

            let proofs =
                proof_of_attendances.into_iter().map(|proof| Decode::decode(&mut proof.as_slice()).unwrap())
                .collect::<Vec<ProofOfAttendance<T::Signature, T::AccountId>>>()
                .encode();

            // Get this pallet's runtime configuration specific module index.
            let sender_pallet_sybil_gate_index = <T as frame_system::Config>::PalletInfo::index::<Self>()
                .map(|i| i.checked_into::<u8>())
                .flatten()
                .ok_or("[EncointerSybilGate]: PalletIndex does not fix into u8. Consider giving it a smaller index.")?;

            let call = IssuePersonhoodUniquenessRatingCall::new(
                pallet_personhood_oracle_index,
                proofs,
                requested_response,
                sender_pallet_sybil_gate_index
            );
            let request_hash = call.request_hash();

            let message = Xcm::Transact { origin_type: OriginKind::SovereignAccount, require_weight_at_most: call.weight(), call: call.encode().into() };
            debug!(target: LOG, "[EncointerSybilGate]: Sending PersonhoodUniquenessRatingRequest to chain: {:?}", parachain_id);
            match T::XcmSender::send_xcm(sibling_junction(parachain_id).into(), message) {
                Ok(()) => {
                    <PendingRequests<T>>::insert(&request_hash, &sender);
                    Self::deposit_event(RawEvent::PersonhoodUniquenessRatingRequestSentSuccess(sender, request_hash, parachain_id))
                },
                Err(e) => Self::deposit_event(RawEvent::PersonhoodUniquenessRatingRequestSentFailure(sender, request_hash, e)),
            }
        }

        #[weight = 5_000_000]
        /// ### Faucet
        ///
        /// Faucet that funds accounts. Currently, this can only be called from other parachains, as
        /// the PersonhoodUniquenessRating can otherwise not be verified.
        fn faucet(
            origin,
            request_hash: H256,
            rating: PersonhoodUniquenessRating
        ) {
            let sender = ensure_signed(origin)?;
            Sibling::try_from_account(&sender).ok_or(<Error<T>>::OnlyParachainsAllowed)?;
            ensure!(<PendingRequests<T>>::contains_key(&request_hash), <Error<T>>::UnexpectedResponse);
            let account = <PendingRequests<T>>::take(&request_hash);
            debug!(target: LOG, "Received PersonhoodUniquenessRating for account: {:?}", account);

            for proof in rating.proofs() {
                if BurnedProofs::contains_key(SybilResponse::Faucet, proof) {
                    Self::deposit_event(RawEvent::FaucetRejectedDueToProofReuse(account));
                    // Even if the rest of the proofs have not been used, we return here, as the
                    // attested/last_n_ceremonies ratio might not be correct any more.
                    return Err(<Error<T>>::RequestContainsBurnedProofs)?;
                }
            }

            if rating.as_ratio::<I16F16>() < I16F16::from_num(0.5) {
                Self::deposit_event(RawEvent::FaucetRejectedDueToWeakPersonhoodUniquenessRating(account));
                return Err(<Error<T>>::PersonhoodUniquenessRatingTooWeak)?;
            } else {
                T::Currency::deposit_creating(&account, 1u32.into());
                rating.proofs().into_iter().for_each( |p|
                    BurnedProofs::insert(SybilResponse::Faucet, p, ())
               );
                Self::deposit_event(RawEvent::FautetDrippedTo(account))
            }
        }
    }
}

decl_error! {
    pub enum Error for Module<T: Config> {
        /// Your PersonhoodUniquenessRating is to weak
        PersonhoodUniquenessRatingTooWeak,
        /// The PersonhoodUniquenessRatingRequest contains ProofOfAttendances that have already been used
        RequestContainsBurnedProofs,
        /// Only other parachains can call this function
        OnlyParachainsAllowed,
        /// Received response to an unknown request
        UnexpectedResponse,
    }
}

#[cfg(test)]
mod tests;
