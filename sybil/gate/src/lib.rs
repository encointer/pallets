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
use fixed::types::I16F16;
use frame_support::traits::Currency;
use frame_support::{
    debug, decl_error, decl_event, decl_module, decl_storage, ensure, traits::PalletInfo,
};
use frame_system::ensure_signed;
use polkadot_parachain::primitives::Sibling;
use rstd::prelude::*;
use sp_runtime::traits::{AccountIdConversion, CheckedConversion, IdentifyAccount, Member, Verify};
use xcm::v0::{Error as XcmError, Junction, OriginKind, SendXcm, Xcm};

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
        PendingRequests get(fn pending_requests): map hasher(blake2_128_concat) T::AccountId => ();
    }
}

decl_event! {
    pub enum Event<T>
    where AccountId = <T as frame_system::Config>::AccountId,
    {
        ProofOfPersonHoodRequestSentSuccess(AccountId),
        ProofOfPersonHoodRequestSentFailure(AccountId, XcmError),
        FautetDrippedTo(AccountId),
        FaucetRejectedDueToWeakProofofPersonhood(AccountId),
    }
}

decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        fn deposit_event() = default;
        type Error = Error<T>;

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

            // Get this pallet's runtime configuration specific module index.
            let sender_pallet_sybil_gate_index = <T as frame_system::Config>::PalletInfo::index::<Self>()
                .map(|i| i.checked_into::<u8>())
                .flatten()
                .ok_or("[EncointerSybilGate]: PalletIndex does not fix into u8. Consider giving it a smaller index.")?;

            let call = IssueProofOfPersonhoodConfidenceCall::new(pallet_sybil_proof_issuer_index, sender.clone(), request, sender_pallet_sybil_gate_index);
            let message = Xcm::Transact { origin_type: OriginKind::SovereignAccount, call: call.encode() };
            debug::debug!(target: LOG, "[EncointerSybilGate]: Sending ProofOfPersonhoodRequest to chain: {:?}", parachain_id);
            match T::XcmSender::send_xcm(location.into(), message) {
                Ok(()) => {
                    <PendingRequests<T>>::insert(&sender, ());
                    Self::deposit_event(RawEvent::ProofOfPersonHoodRequestSentSuccess(sender))
                },
                Err(e) => Self::deposit_event(RawEvent::ProofOfPersonHoodRequestSentFailure(sender, e)),
            }
        }

        #[weight = 5_000_000]
        fn faucet(
            origin,
            account: T::AccountId,
            confidence: ProofOfPersonhoodConfidence
        ) {
            let sender = ensure_signed(origin)?;
            Sibling::try_from_account(&sender).ok_or(<Error<T>>::OnlyParachainsAllowed)?;

            debug::RuntimeLogger::init();
            debug::debug!(target: LOG, "set ProofOfPersonhood Confidence for account: {:?}", account);

            ensure!(<PendingRequests<T>>::contains_key(&account), <Error<T>>::UnexpectedAccount);
            <PendingRequests<T>>::remove(&account);

            if confidence.as_ratio::<I16F16>() < I16F16::from_num(0.5) {
                Self::deposit_event(RawEvent::FaucetRejectedDueToWeakProofofPersonhood(account))
            } else {
                T::Currency::deposit_creating(&account, 1u32.into());
                Self::deposit_event(RawEvent::FautetDrippedTo(account))
            }
        }
    }
}

decl_error! {
    pub enum Error for Module<T: Config> {
        /// Your ProofOfPersonhood is to weak
        ProofOfPersonhoodToWeak,
        /// Only other parachains can call this function
        OnlyParachainsAllowed,
        /// This account has no pending SybilGate requests
        UnexpectedAccount,
    }
}

#[cfg(test)]
mod tests;
