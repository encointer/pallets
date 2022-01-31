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
	fixed::types::I16F16,
	sybil::{
		sibling_junction, CallMetadata, IssuePersonhoodUniquenessRatingCall,
		PersonhoodUniquenessRating, SybilResponse,
	},
};
use frame_support::{
	traits::{Currency, Get, PalletInfo},
	weights::GetDispatchInfo,
	Parameter,
};
use frame_system::ensure_signed;
use log::debug;
use polkadot_parachain::primitives::Sibling;
use sp_core::H256;
use sp_runtime::traits::{AccountIdConversion, CheckedConversion, IdentifyAccount, Member, Verify};
use sp_std::prelude::*;
use xcm::v1::{Error as XcmError, OriginKind, SendXcm, Xcm};

const LOG: &str = "encointer";

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// The XCM sender module.
		type XcmSender: SendXcm;

		type Currency: Currency<<Self as frame_system::Config>::AccountId>;

		type Public: IdentifyAccount<AccountId = Self::AccountId>;
		type Signature: Verify<Signer = <Self as Config>::Public> + Member + Decode + Encode;

		/// The outer call dispatch type.
		type Call: Parameter + GetDispatchInfo;

		type IssuePersonhoodUniquenessRatingWeight: Get<u64>;
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// ### Proof of PersonhoodRequest
		///
		/// Request a PersonhoodUniquenessRating from an encointer-parachain.
		///
		/// The `pallet_personhood_oracle_index` is the pallet's module index of the respective encointer-parachain's
		/// `pallet-encointer-personhood-oracle` pallet to query.
		#[pallet::weight(5_000_000)]
		pub fn request_personhood_uniqueness_rating(
			origin: OriginFor<T>,
			parachain_id: u32,
			pallet_personhood_oracle_index: u8,
			proof_of_attendances: Vec<Vec<u8>>,
			requested_response: SybilResponse,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			let resp_index = requested_response as u8;

			let proofs = proof_of_attendances
				.into_iter()
				.map(|proof| Decode::decode(&mut proof.as_slice()).unwrap())
				.collect::<Vec<ProofOfAttendance<T::Signature, T::AccountId>>>()
				.encode();

			// Get this pallet's runtime configuration specific module index.
			let sender_pallet_sybil_gate_index = <T as frame_system::Config>::PalletInfo::index::<Self>()
				.map(|i| i.checked_into::<u8>())
				.flatten()
				.ok_or("[EncointerSybilGate]: PalletIndex does not fix into u8. Consider giving it a smaller index.")?;

			// Get the weight of the response dynamically
			let resp_call: <T as Config>::Call = (
				[sender_pallet_sybil_gate_index, resp_index],
				H256::default(),
				PersonhoodUniquenessRating::default()
			)
				.using_encoded(|mut c| Decode::decode(&mut c).ok())
				.ok_or("[EncointerSybilGate]: Could not transform response call into runtime dispatchable")?;

			let call = IssuePersonhoodUniquenessRatingCall::new(
				pallet_personhood_oracle_index,
				proofs,
				CallMetadata::new(
					sender_pallet_sybil_gate_index,
					resp_index,
					resp_call.get_dispatch_info().weight,
				),
			);

			let request_hash = call.request_hash();

			let message = Xcm::Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: T::IssuePersonhoodUniquenessRatingWeight::get(),
				call: call.encode().into(),
			};
			debug!(
				target: LOG,
				"[EncointerSybilGate]: Sending PersonhoodUniquenessRatingRequest to chain: {:?}",
				parachain_id
			);
			match T::XcmSender::send_xcm(sibling_junction(parachain_id), message) {
				Ok(()) => {
					<PendingRequests<T>>::insert(&request_hash, &sender);
					Self::deposit_event(Event::PersonhoodUniquenessRatingRequestSentSuccess(
						sender,
						request_hash,
						parachain_id,
					))
				},
				Err(e) => Self::deposit_event(Event::PersonhoodUniquenessRatingRequestSentFailure(
					sender,
					request_hash,
					e,
				)),
			}

			Ok(().into())
		}

		/// ### Faucet
		///
		/// Faucet that funds accounts. Currently, this can only be called from other parachains, as
		/// the PersonhoodUniquenessRating can otherwise not be verified.
		#[pallet::weight(5_000_000)]
		pub fn faucet(
			origin: OriginFor<T>,
			request_hash: H256,
			rating: PersonhoodUniquenessRating,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			Sibling::try_from_account(&sender).ok_or(<Error<T>>::OnlyParachainsAllowed)?;

			let account = <PendingRequests<T>>::take(&request_hash)
				.ok_or_else(|| <Error<T>>::UnexpectedResponse)?;

			debug!(target: LOG, "Received PersonhoodUniquenessRating for account: {:?}", account);

			for proof in rating.proofs() {
				if BurnedProofs::<T>::contains_key(SybilResponse::Faucet, proof) {
					Self::deposit_event(Event::FaucetRejectedDueToProofReuse(account));
					// Even if the rest of the proofs have not been used, we return here, as the
					// attested/last_n_ceremonies ratio might not be correct any more.
					return Err(<Error<T>>::RequestContainsBurnedProofs)?
				}
			}

			if rating.as_ratio::<I16F16>() < I16F16::from_num(0.5) {
				Self::deposit_event(Event::FaucetRejectedDueToWeakPersonhoodUniquenessRating(
					account,
				));
				return Err(<Error<T>>::PersonhoodUniquenessRatingTooWeak)?
			} else {
				T::Currency::deposit_creating(&account, 1u32.into());
				rating
					.proofs()
					.into_iter()
					.for_each(|p| BurnedProofs::<T>::insert(SybilResponse::Faucet, p, ()));
				Self::deposit_event(Event::FautetDrippedTo(account))
			}

			Ok(().into())
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// An account has successfully sent a request to another parachain \[requester, request_hash, parachain\]
		PersonhoodUniquenessRatingRequestSentSuccess(T::AccountId, H256, u32),
		/// Failed to send request to another parachain \[requester, request_hash, xcm error\]
		PersonhoodUniquenessRatingRequestSentFailure(T::AccountId, H256, XcmError),
		/// Faucet dripped some funds to account \[funded_account\]
		FautetDrippedTo(T::AccountId),
		/// Faucet rejected dripping funds due to weak PersonhoodUniquenessRating \[rejected_account\]
		FaucetRejectedDueToWeakPersonhoodUniquenessRating(T::AccountId),
		/// Faucet rejected dripping funds due to reuse of ProofOfAttendances \[rejected_account\]
		FaucetRejectedDueToProofReuse(T::AccountId),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Your PersonhoodUniquenessRating is to weak
		PersonhoodUniquenessRatingTooWeak,
		/// The PersonhoodUniquenessRatingRequest contains ProofOfAttendances that have already been used
		RequestContainsBurnedProofs,
		/// Only other parachains can call this function
		OnlyParachainsAllowed,
		/// Received response to an unknown request
		UnexpectedResponse,
	}

	/// XCM PersonhoodUniquenessRating requests sent to another parachain that have not yielded a response yet
	#[pallet::storage]
	#[pallet::getter(fn pending_requests)]
	pub(super) type PendingRequests<T: Config> =
		StorageMap<_, Identity, H256, T::AccountId, OptionQuery>;

	/// The proof of attendances that have already been used in a previous request
	/// Membership checks are faster with maps than with vecs, see: https://substrate.dev/recipes/map-set.html.
	///
	/// This is a double_map, as more requests might be added, and ProofOfAttendances are allowed to be used per request
	#[pallet::storage]
	#[pallet::getter(fn burned_proofs)]
	pub(super) type BurnedProofs<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		SybilResponse,
		Blake2_128Concat,
		H256,
		(),
		ValueQuery,
	>;
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
