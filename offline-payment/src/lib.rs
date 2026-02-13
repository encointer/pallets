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
//! This pallet enables offline payments using Groth16 ZK proofs with nullifiers.
//!
//! ## Overview
//!
//! Users register an offline identity (commitment) linked to their account.
//! Payments can be made offline by generating a ZK proof, which is later
//! submitted to the chain for settlement. Nullifiers prevent double-spending.
//!
//! ## ZK Circuit
//!
//! The circuit proves:
//! - Knowledge of zk_secret such that commitment = Poseidon(zk_secret)
//! - Nullifier correctness: nullifier = Poseidon(zk_secret, nonce)
//! - Amount is properly bounded (fits in 128 bits)
//!
//! Public inputs: commitment, recipient_hash, amount, cid_hash, nullifier
//! Private inputs: zk_secret, nonce

#![cfg_attr(not(feature = "std"), no_std)]

use core::marker::PhantomData;
use encointer_primitives::{balances::BalanceType, communities::CommunityIdentifier};
use frame_support::traits::{Currency, ExistenceRequirement, Get};
use frame_system::ensure_signed;
use log::info;
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_io::hashing::blake2_256;
use sp_runtime::SaturatedConversion;
use sp_std::vec::Vec;

pub use weights::WeightInfo;

const LOG: &str = "encointer::offline-payment";

pub use pallet::*;
pub mod verifier;

/// Balance type for the native token currency
pub type BalanceOf<T> =
	<<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[cfg(feature = "std")]
pub mod circuit;
#[cfg(feature = "std")]
pub mod prover;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod weights;

/// Domain separator for commitment derivation (used in Poseidon hash)
pub const COMMITMENT_DOMAIN: &[u8] = b"encointer-offline-commitment";
/// Domain separator for nullifier derivation
pub const NULLIFIER_DOMAIN: &[u8] = b"encointer-offline-nullifier";

/// Maximum size of a Groth16 proof in bytes (2 G1 + 1 G2 on BN254)
pub const MAX_PROOF_SIZE: u32 = 256;
/// Maximum size of verification key in bytes
pub const MAX_VK_SIZE: u32 = 2048;

/// Groth16 proof for offline payment.
///
/// Contains the serialized proof bytes that can be verified on-chain.
#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(S))]
pub struct Groth16ProofBytes<S: Get<u32>> {
	/// Serialized Groth16 proof (compressed format)
	pub proof_bytes: frame_support::BoundedVec<u8, S>,
}

impl<S: Get<u32>> Clone for Groth16ProofBytes<S> {
	fn clone(&self) -> Self {
		Self { proof_bytes: self.proof_bytes.clone() }
	}
}

impl<S: Get<u32>> PartialEq for Groth16ProofBytes<S> {
	fn eq(&self, other: &Self) -> bool {
		self.proof_bytes == other.proof_bytes
	}
}

impl<S: Get<u32>> Eq for Groth16ProofBytes<S> {}

impl<S: Get<u32>> core::fmt::Debug for Groth16ProofBytes<S> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("Groth16ProofBytes")
			.field("proof_bytes", &self.proof_bytes)
			.finish()
	}
}

impl<S: Get<u32>> Groth16ProofBytes<S> {
	/// Create from raw proof bytes
	pub fn from_bytes(bytes: Vec<u8>) -> Option<Self> {
		let bounded = frame_support::BoundedVec::try_from(bytes).ok()?;
		Some(Self { proof_bytes: bounded })
	}
}

/// Compute commitment using Blake2 hash (for deriving zk_secret from seed).
/// The actual commitment in the circuit uses Poseidon, but we use Blake2
/// for the initial derivation from account seed.
pub fn derive_zk_secret(seed_bytes: &[u8]) -> [u8; 32] {
	let mut input = Vec::with_capacity(seed_bytes.len() + COMMITMENT_DOMAIN.len());
	input.extend_from_slice(seed_bytes);
	input.extend_from_slice(COMMITMENT_DOMAIN);
	blake2_256(&input)
}

/// Compute hash of recipient account for public input
pub fn hash_recipient(recipient: &[u8]) -> [u8; 32] {
	blake2_256(recipient)
}

/// Compute hash of community identifier for public input
pub fn hash_cid(cid: &CommunityIdentifier) -> [u8; 32] {
	blake2_256(&cid.encode())
}

/// Convert BalanceType to bytes for public input
pub fn balance_to_bytes(amount: BalanceType) -> [u8; 32] {
	// BalanceType is i64F64, we need to convert to bytes
	let bits = amount.to_bits();
	let mut bytes = [0u8; 32];
	bytes[..16].copy_from_slice(&bits.to_le_bytes());
	bytes
}

/// Compute the sentinel CID hash for native token payments.
/// Uses `blake2_256(b"encointer-native-token")` instead of a real community identifier.
pub fn native_token_cid_hash() -> [u8; 32] {
	blake2_256(b"encointer-native-token")
}

/// Convert a native token u128 amount to a 32-byte field element representation.
/// Delegates to `verifier::amount_to_bytes`.
pub fn native_balance_to_bytes(amount: u128) -> [u8; 32] {
	verifier::amount_to_bytes(amount)
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
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Weight information for extrinsics
		type WeightInfo: WeightInfo;

		/// Native token currency (e.g. `pallet_balances`)
		type Currency: Currency<Self::AccountId>;

		/// Maximum size of proof in bytes
		#[pallet::constant]
		type MaxProofSize: Get<u32>;

		/// Maximum size of verification key in bytes
		#[pallet::constant]
		type MaxVkSize: Get<u32>;
	}

	/// Maps account → commitment (Poseidon hash of zk_secret)
	/// Set once via register_offline_identity()
	#[pallet::storage]
	#[pallet::getter(fn offline_identities)]
	pub type OfflineIdentities<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, [u8; 32], OptionQuery>;

	/// Set of spent nullifiers. Prevents double-submission of the same proof.
	#[pallet::storage]
	#[pallet::getter(fn used_nullifiers)]
	pub type UsedNullifiers<T: Config> = StorageMap<_, Blake2_128Concat, [u8; 32], (), OptionQuery>;

	/// Groth16 verification key, set by governance.
	/// Serialized ark-groth16 VerifyingKey<Bn254>.
	#[pallet::storage]
	#[pallet::getter(fn verification_key)]
	pub type VerificationKey<T: Config> =
		StorageValue<_, BoundedVec<u8, T::MaxVkSize>, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Offline identity registered for an account
		OfflineIdentityRegistered { who: T::AccountId, commitment: [u8; 32] },
		/// Offline payment settled successfully
		OfflinePaymentSettled {
			sender: T::AccountId,
			recipient: T::AccountId,
			cid: CommunityIdentifier,
			amount: BalanceType,
			nullifier: [u8; 32],
		},
		/// Native token offline payment settled successfully
		NativeOfflinePaymentSettled {
			sender: T::AccountId,
			recipient: T::AccountId,
			amount: BalanceOf<T>,
			nullifier: [u8; 32],
		},
		/// Verification key was set
		VerificationKeySet,
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Account already has a registered offline identity
		AlreadyRegistered,
		/// Sender does not have a registered offline identity
		NoOfflineIdentity,
		/// This nullifier has already been used (double-spend attempt)
		NullifierAlreadyUsed,
		/// ZK proof verification failed
		InvalidProof,
		/// Failed to deserialize the proof
		ProofDeserializationFailed,
		/// No verification key has been set
		NoVerificationKey,
		/// Failed to deserialize the verification key
		VkDeserializationFailed,
		/// Amount must be positive
		AmountMustBePositive,
		/// Sender cannot be the same as recipient
		SenderEqualsRecipient,
		/// Sender has insufficient balance for this payment
		InsufficientBalance,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Register an offline identity (commitment) for the caller's account.
		///
		/// This is a one-time setup that links a ZK commitment to the account.
		/// The commitment should be Poseidon(zk_secret) where zk_secret is
		/// derived deterministically from the account's seed.
		///
		/// # Arguments
		/// * `commitment` - The Poseidon hash commitment of the zk_secret
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::register_offline_identity())]
		pub fn register_offline_identity(
			origin: OriginFor<T>,
			commitment: [u8; 32],
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(!OfflineIdentities::<T>::contains_key(&who), Error::<T>::AlreadyRegistered);

			OfflineIdentities::<T>::insert(&who, commitment);

			info!(target: LOG, "offline identity registered: {who:?}");
			Self::deposit_event(Event::OfflineIdentityRegistered { who, commitment });

			Ok(())
		}

		/// Submit an offline payment ZK proof for settlement.
		///
		/// Anyone can submit a proof - the submitter doesn't need to be the sender.
		/// This allows either party (buyer or seller) to settle when they come online.
		///
		/// The proof verifies:
		/// - Prover knows zk_secret such that commitment = Poseidon(zk_secret)
		/// - nullifier = Poseidon(zk_secret, nonce) for some nonce
		/// - All public inputs match the claimed values
		///
		/// # Arguments
		/// * `proof` - The Groth16 proof bytes
		/// * `sender` - The account sending funds (must have registered commitment)
		/// * `recipient` - The account receiving funds
		/// * `amount` - The amount to transfer
		/// * `cid` - The community identifier
		/// * `nullifier` - The unique nullifier for this payment
		#[pallet::call_index(1)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::submit_offline_payment())]
		pub fn submit_offline_payment(
			origin: OriginFor<T>,
			proof: Groth16ProofBytes<T::MaxProofSize>,
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
			let commitment =
				OfflineIdentities::<T>::get(&sender).ok_or(Error::<T>::NoOfflineIdentity)?;

			// 2. Nullifier must be fresh
			ensure!(
				!UsedNullifiers::<T>::contains_key(nullifier),
				Error::<T>::NullifierAlreadyUsed
			);

			// 3. Check sender has sufficient balance (fail-fast before expensive proof
			//    verification)
			ensure!(
				pallet_encointer_balances::Pallet::<T>::balance(cid, &sender) >= amount,
				Error::<T>::InsufficientBalance
			);

			// 4. Get verification key
			let vk_bytes = VerificationKey::<T>::get().ok_or(Error::<T>::NoVerificationKey)?;

			// 5. Deserialize verification key
			let vk = verifier::Groth16VerifyingKey::from_bytes(&vk_bytes)
				.ok_or(Error::<T>::VkDeserializationFailed)?;

			// 6. Deserialize proof
			let zk_proof = verifier::Groth16Proof::from_bytes(&proof.proof_bytes)
				.ok_or(Error::<T>::ProofDeserializationFailed)?;

			// 7. Construct public inputs
			let recipient_hash = hash_recipient(&recipient.encode());
			let cid_hash = hash_cid(&cid);
			let amount_bytes = balance_to_bytes(amount);

			let public_inputs = verifier::PublicInputs::from_bytes(
				&commitment,
				&recipient_hash,
				&amount_bytes,
				&cid_hash,
				&nullifier,
			);

			// 8. Verify the ZK proof
			ensure!(
				verifier::verify_groth16_proof(&vk, &zk_proof, &public_inputs),
				Error::<T>::InvalidProof
			);

			// 9. Execute transfer
			pallet_encointer_balances::Pallet::<T>::do_transfer(cid, &sender, &recipient, amount)?;

			// 10. Mark nullifier as used
			UsedNullifiers::<T>::insert(nullifier, ());

			info!(
				target: LOG,
				"offline payment settled: {sender:?} -> {recipient:?}, amount: {amount:?}, cid: {cid:?}"
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

		/// Submit a native token offline payment ZK proof for settlement.
		///
		/// Same ZK circuit as CC payments, but uses a sentinel CID hash
		/// (`blake2_256(b"encointer-native-token")`) and transfers native currency
		/// via `T::Currency::transfer()`.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::submit_native_offline_payment())]
		pub fn submit_native_offline_payment(
			origin: OriginFor<T>,
			proof: Groth16ProofBytes<T::MaxProofSize>,
			sender: T::AccountId,
			recipient: T::AccountId,
			amount: BalanceOf<T>,
			nullifier: [u8; 32],
		) -> DispatchResult {
			let _submitter = ensure_signed(origin)?;

			ensure!(!amount.is_zero(), Error::<T>::AmountMustBePositive);
			ensure!(sender != recipient, Error::<T>::SenderEqualsRecipient);

			let commitment =
				OfflineIdentities::<T>::get(&sender).ok_or(Error::<T>::NoOfflineIdentity)?;

			ensure!(
				!UsedNullifiers::<T>::contains_key(nullifier),
				Error::<T>::NullifierAlreadyUsed
			);

			ensure!(T::Currency::free_balance(&sender) >= amount, Error::<T>::InsufficientBalance);

			let vk_bytes = VerificationKey::<T>::get().ok_or(Error::<T>::NoVerificationKey)?;
			let vk = verifier::Groth16VerifyingKey::from_bytes(&vk_bytes)
				.ok_or(Error::<T>::VkDeserializationFailed)?;
			let zk_proof = verifier::Groth16Proof::from_bytes(&proof.proof_bytes)
				.ok_or(Error::<T>::ProofDeserializationFailed)?;

			let recipient_hash = hash_recipient(&recipient.encode());
			let cid_hash = native_token_cid_hash();
			let amount_u128: u128 = amount.saturated_into();
			let amount_bytes = native_balance_to_bytes(amount_u128);

			let public_inputs = verifier::PublicInputs::from_bytes(
				&commitment,
				&recipient_hash,
				&amount_bytes,
				&cid_hash,
				&nullifier,
			);

			ensure!(
				verifier::verify_groth16_proof(&vk, &zk_proof, &public_inputs),
				Error::<T>::InvalidProof
			);

			T::Currency::transfer(&sender, &recipient, amount, ExistenceRequirement::KeepAlive)?;

			UsedNullifiers::<T>::insert(nullifier, ());

			info!(
				target: LOG,
				"native offline payment settled: {sender:?} -> {recipient:?}, amount: {amount:?}"
			);
			Self::deposit_event(Event::NativeOfflinePaymentSettled {
				sender,
				recipient,
				amount,
				nullifier,
			});

			Ok(())
		}

		/// Set the Groth16 verification key (governance/sudo only).
		///
		/// The verification key must be generated from the trusted setup
		/// ceremony for the offline payment circuit.
		///
		/// # Arguments
		/// * `vk` - Serialized verification key bytes
		#[pallet::call_index(2)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::set_verification_key())]
		pub fn set_verification_key(
			origin: OriginFor<T>,
			vk: BoundedVec<u8, T::MaxVkSize>,
		) -> DispatchResult {
			ensure_root(origin)?;

			// Validate that the vk can be deserialized
			verifier::Groth16VerifyingKey::from_bytes(&vk)
				.ok_or(Error::<T>::VkDeserializationFailed)?;

			VerificationKey::<T>::put(vk);
			Self::deposit_event(Event::VerificationKeySet);

			info!(target: LOG, "verification key set");
			Ok(())
		}
	}
}
