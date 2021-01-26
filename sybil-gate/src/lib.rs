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

//! # Encointer SybilGate Module
//!
//! provides functionality for
//! - issuing and verifying digital proof of personhood confidence aka anti-sybil confidence

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{decl_event, decl_module, dispatch::DispatchResult, ensure};
use frame_system::ensure_signed;
use rstd::prelude::*;
use sp_runtime::traits::{IdentifyAccount, Member, Verify};
use xcm::v0::{Error as XcmError, Junction, OriginKind, SendXcm, Xcm};

use encointer_ceremonies::{ProofOfAttendance, Reputation};

pub trait Trait: frame_system::Trait + encointer_ceremonies::Trait {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    /// The XCM sender module.
    type XcmSender: SendXcm;
    /// Runtime Call type, used for cross-messaging calls.
    type Call: Encode + From<<Self as frame_system::Trait>::Call>;

    type Public: IdentifyAccount<AccountId = Self::AccountId>;
    type Signature: Verify<Signer = <Self as Trait>::Public> + Member + Decode + Encode;
}

decl_event! {
    pub enum Event<T>
    where AccountId = <T as frame_system::Trait>::AccountId,
    {
        /// Record sent to another location.
        RecordSentSuccess(AccountId),
        /// Record didn't send, error attached.
        RecordSentFailure(AccountId, XcmError),
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        #[weight = 5_000_000]
        // fn record(origin, parachain_id: u32, record: T::Record) {
        fn record(origin, parachain_id: u32) {
            let sender = ensure_signed(origin)?;
            let location = Junction::Parachain { id: parachain_id };
            // let call: <T as Trait>::Call = datalog::Call::<T>::record(record).into();
            // let message = Xcm::Transact { origin_type: OriginKind::SovereignAccount, call: call.encode() };
            // match T::XcmSender::send_xcm(location.into(), message.into()) {
            //     Ok(()) => Self::deposit_event(RawEvent::RecordSentSuccess(sender)),
            //     Err(e) => Self::deposit_event(RawEvent::RecordSentFailure(sender, e)),
            // }
            ()
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
