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

//! # Offline Payment Pallet
//!
//! This pallet enables offline payments using ZK proofs with nullifiers.
//!
//! ## Overview
//!
//! Users register an offline identity (commitment) linked to their account.
//! Payments can be made offline by generating a proof, which is later
//! submitted to the chain for settlement. Nullifiers prevent double-spending.
//!
//! ## PoC Note
//!
//! This PoC uses Blake2 hashes instead of ZK proofs. The verification logic
//! mirrors what a real ZK circuit (Groth16) would prove:
//! - commitment = Hash(zk_secret)
//! - nullifier = Hash(zk_secret, nonce)
//!
//! In production, replace the proof verification with arkworks Groth16.

#![cfg_attr(not(feature = "std"), no_std)]

use core::marker::PhantomData;
use encointer_primitives::{balances::BalanceType, communities::CommunityIdentifier};
use frame_support::traits::Get;
use frame_system::ensure_signed;
use log::info;
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_io::hashing::blake2_256;
use sp_std::vec::Vec;
pub use weights::WeightInfo;

const LOG: &str = "encointer::offline-payment";

pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod weights;

/// Domain separator for commitment derivation
pub const COMMITMENT_DOMAIN: &[u8] = b"encointer-offline-commitment";
/// Domain separator for nullifier derivation
pub const NULLIFIER_DOMAIN: &[u8] = b"encointer-offline-nullifier";

/// Maximum proof size in bytes (placeholder for Groth16 proof ~192 bytes)
pub const MAX_PROOF_SIZE: u32 = 256;

/// Offline payment proof structure.
///
/// In production ZK: this would be a Groth16 proof (~192 bytes).
/// In PoC: contains the secret and nonce to verify commitment/nullifier.
#[derive(Clone, Encode, Decode, DecodeWithMemTracking, Eq, PartialEq, Debug, TypeInfo, MaxEncodedLen)]
pub struct OfflinePaymentProof {
	/// The ZK secret (in production ZK, this is hidden in the witness)
	pub zk_secret: [u8; 32],
	/// Random nonce for this payment
	pub nonce: [u8; 32],
}

impl OfflinePaymentProof {
	/// Create a new proof from secret and nonce
	pub fn new(zk_secret: [u8; 32], nonce: [u8; 32]) -> Self {
		Self { zk_secret, nonce }
	}

	/// Compute the commitment that should match the registered one
	pub fn compute_commitment(&self) -> [u8; 32] {
		compute_commitment(&self.zk_secret)
	}

	/// Compute the nullifier for this proof
	pub fn compute_nullifier(&self) -> [u8; 32] {
		compute_nullifier(&self.zk_secret, &self.nonce)
	}
}

/// Compute commitment from zk_secret: Hash(domain || zk_secret)
pub fn compute_commitment(zk_secret: &[u8; 32]) -> [u8; 32] {
	let mut input = [0u8; 64];
	input[..32].copy_from_slice(&blake2_256(COMMITMENT_DOMAIN));
	input[32..].copy_from_slice(zk_secret);
	blake2_256(&input)
}

/// Compute nullifier from zk_secret and nonce: Hash(domain || zk_secret || nonce)
pub fn compute_nullifier(zk_secret: &[u8; 32], nonce: &[u8; 32]) -> [u8; 32] {
	let mut input = [0u8; 96];
	input[..32].copy_from_slice(&blake2_256(NULLIFIER_DOMAIN));
	input[32..64].copy_from_slice(zk_secret);
	input[64..].copy_from_slice(nonce);
	blake2_256(&input)
}

/// Derive zk_secret from account seed bytes with domain separation
pub fn derive_zk_secret(seed_bytes: &[u8]) -> [u8; 32] {
	let mut input = Vec::with_capacity(seed_bytes.len() + COMMITMENT_DOMAIN.len());
	input.extend_from_slice(seed_bytes);
	input.extend_from_slice(COMMITMENT_DOMAIN);
	blake2_256(&input)
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ pallet_encointer_balances::Config
		+ pallet_encointer_communities::Config
	{
		/// The overarching event type
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Weight information for extrinsics
		type WeightInfo: WeightInfo;

		/// Maximum size of proof in bytes
		#[pallet::constant]
		type MaxProofSize: Get<u32>;
	}

	/// Maps account → commitment (Hash of zk_secret)
	/// Set once via register_offline_identity()
	#[pallet::storage]
	#[pallet::getter(fn offline_identities)]
	pub type OfflineIdentities<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, [u8; 32], OptionQuery>;

	/// Set of spent nullifiers. Prevents double-submission of the same proof.
	#[pallet::storage]
	#[pallet::getter(fn used_nullifiers)]
	pub type UsedNullifiers<T: Config> =
		StorageMap<_, Blake2_128Concat, [u8; 32], (), OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Offline identity registered for an account
		OfflineIdentityRegistered {
			who: T::AccountId,
			commitment: [u8; 32],
		},
		/// Offline payment settled successfully
		OfflinePaymentSettled {
			sender: T::AccountId,
			recipient: T::AccountId,
			cid: CommunityIdentifier,
			amount: BalanceType,
			nullifier: [u8; 32],
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Account already has a registered offline identity
		AlreadyRegistered,
		/// Sender does not have a registered offline identity
		NoOfflineIdentity,
		/// This nullifier has already been used (double-spend attempt)
		NullifierAlreadyUsed,
		/// Proof verification failed - commitment mismatch
		InvalidProof,
		/// Nullifier in proof doesn't match computed nullifier
		NullifierMismatch,
		/// Amount must be positive
		AmountMustBePositive,
		/// Sender cannot be the same as recipient
		SenderEqualsRecipient,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Register an offline identity (commitment) for the caller's account.
		///
		/// This is a one-time setup that links a ZK commitment to the account.
		/// The commitment is Hash(zk_secret) where zk_secret is derived from
		/// the account's seed.
		///
		/// # Arguments
		/// * `commitment` - The commitment value (Hash of zk_secret)
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::register_offline_identity())]
		pub fn register_offline_identity(
			origin: OriginFor<T>,
			commitment: [u8; 32],
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				!OfflineIdentities::<T>::contains_key(&who),
				Error::<T>::AlreadyRegistered
			);

			OfflineIdentities::<T>::insert(&who, commitment);

			info!(target: LOG, "offline identity registered: {:?}", who);
			Self::deposit_event(Event::OfflineIdentityRegistered { who, commitment });

			Ok(())
		}

		/// Submit an offline payment proof for settlement.
		///
		/// Anyone can submit a proof - the submitter doesn't need to be the sender.
		/// This allows either party (buyer or seller) to settle when they come online.
		///
		/// # Arguments
		/// * `proof` - The ZK proof (in PoC: contains zk_secret and nonce)
		/// * `sender` - The account sending funds
		/// * `recipient` - The account receiving funds
		/// * `amount` - The amount to transfer
		/// * `cid` - The community identifier
		/// * `nullifier` - The unique nullifier for this payment
		#[pallet::call_index(1)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::submit_offline_payment())]
		pub fn submit_offline_payment(
			origin: OriginFor<T>,
			proof: OfflinePaymentProof,
			sender: T::AccountId,
			recipient: T::AccountId,
			amount: BalanceType,
			cid: CommunityIdentifier,
			nullifier: [u8; 32],
		) -> DispatchResult {
			// Anyone can submit
			let _submitter = ensure_signed(origin)?;

			// Validate inputs
			ensure!(amount > BalanceType::from_num(0), Error::<T>::AmountMustBePositive);
			ensure!(sender != recipient, Error::<T>::SenderEqualsRecipient);

			// 1. Sender must have registered offline identity
			let commitment = OfflineIdentities::<T>::get(&sender)
				.ok_or(Error::<T>::NoOfflineIdentity)?;

			// 2. Nullifier must be fresh
			ensure!(
				!UsedNullifiers::<T>::contains_key(&nullifier),
				Error::<T>::NullifierAlreadyUsed
			);

			// 3. Verify proof: commitment matches
			let computed_commitment = proof.compute_commitment();
			ensure!(computed_commitment == commitment, Error::<T>::InvalidProof);

			// 4. Verify proof: nullifier matches
			let computed_nullifier = proof.compute_nullifier();
			ensure!(computed_nullifier == nullifier, Error::<T>::NullifierMismatch);

			// 5. Execute transfer
			pallet_encointer_balances::Pallet::<T>::do_transfer(
				cid,
				&sender,
				&recipient,
				amount,
			)?;

			// 6. Mark nullifier as used
			UsedNullifiers::<T>::insert(&nullifier, ());

			info!(
				target: LOG,
				"offline payment settled: {:?} -> {:?}, amount: {:?}, cid: {:?}",
				sender, recipient, amount, cid
			);
			Self::deposit_event(Event::OfflinePaymentSettled {
				sender,
				recipient,
				cid,
				amount,
				nullifier,
			});

			Ok(())
		}
	}
}
