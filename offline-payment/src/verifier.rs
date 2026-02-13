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

//! Groth16 ZK proof verification for offline payments.
//!
//! This module provides on-chain verification of Groth16 proofs on the BN254 curve.

use ark_bn254::{g1::G1Affine, g2::G2Affine, Bn254, Fr};
use ark_ff::PrimeField;
use ark_groth16::{Groth16, PreparedVerifyingKey, Proof, VerifyingKey};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use sp_std::vec::Vec;

/// Maximum size of a serialized Groth16 proof (2 G1 + 1 G2 on BN254)
pub const MAX_PROOF_SIZE: usize = 192;

/// Maximum size of a serialized verification key
pub const MAX_VK_SIZE: usize = 2048;

/// Number of public inputs for our circuit
pub const NUM_PUBLIC_INPUTS: usize = 5;

/// Groth16 proof wrapper for SCALE encoding
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Groth16Proof {
	pub a: G1Affine,
	pub b: G2Affine,
	pub c: G1Affine,
}

impl Groth16Proof {
	/// Deserialize from bytes
	pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
		let proof = Proof::<Bn254>::deserialize_compressed(bytes).ok()?;
		Some(Self { a: proof.a, b: proof.b, c: proof.c })
	}

	/// Serialize to bytes
	pub fn to_bytes(&self) -> Vec<u8> {
		let proof = Proof::<Bn254> { a: self.a, b: self.b, c: self.c };
		let mut bytes = Vec::new();
		proof.serialize_compressed(&mut bytes).expect("serialization should not fail");
		bytes
	}

	/// Convert to arkworks Proof type
	pub fn to_ark_proof(&self) -> Proof<Bn254> {
		Proof { a: self.a, b: self.b, c: self.c }
	}
}

/// Groth16 verification key wrapper
#[derive(Clone, Debug)]
pub struct Groth16VerifyingKey {
	inner: VerifyingKey<Bn254>,
}

impl Groth16VerifyingKey {
	/// Deserialize from bytes
	pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
		let vk = VerifyingKey::<Bn254>::deserialize_compressed(bytes).ok()?;
		Some(Self { inner: vk })
	}

	/// Serialize to bytes
	pub fn to_bytes(&self) -> Vec<u8> {
		let mut bytes = Vec::new();
		self.inner
			.serialize_compressed(&mut bytes)
			.expect("serialization should not fail");
		bytes
	}

	/// Get the inner verification key
	pub fn inner(&self) -> &VerifyingKey<Bn254> {
		&self.inner
	}
}

/// Public inputs for the offline payment circuit
#[derive(Clone, Debug)]
pub struct PublicInputs {
	/// Commitment = Poseidon(zk_secret)
	pub commitment: Fr,
	/// Recipient hash = Poseidon(recipient_pubkey)
	pub recipient_hash: Fr,
	/// Amount as field element
	pub amount: Fr,
	/// Asset hash (community ID hash or native-token sentinel), chain-bound in extrinsic
	pub asset_hash: Fr,
	/// Nullifier = Poseidon(zk_secret, nonce)
	pub nullifier: Fr,
}

impl PublicInputs {
	/// Convert 32-byte array to field element
	pub fn bytes_to_field(bytes: &[u8; 32]) -> Fr {
		Fr::from_le_bytes_mod_order(bytes)
	}

	/// Create from raw byte arrays
	pub fn from_bytes(
		commitment: &[u8; 32],
		recipient_hash: &[u8; 32],
		amount: &[u8; 32],
		asset_hash: &[u8; 32],
		nullifier: &[u8; 32],
	) -> Self {
		Self {
			commitment: Self::bytes_to_field(commitment),
			recipient_hash: Self::bytes_to_field(recipient_hash),
			amount: Self::bytes_to_field(amount),
			asset_hash: Self::bytes_to_field(asset_hash),
			nullifier: Self::bytes_to_field(nullifier),
		}
	}

	/// Convert to vector of field elements for verification
	pub fn to_vec(&self) -> Vec<Fr> {
		sp_std::vec![
			self.commitment,
			self.recipient_hash,
			self.amount,
			self.asset_hash,
			self.nullifier,
		]
	}
}

/// Verify a Groth16 proof against public inputs
///
/// Returns `true` if the proof is valid, `false` otherwise.
pub fn verify_groth16_proof(
	vk: &Groth16VerifyingKey,
	proof: &Groth16Proof,
	public_inputs: &PublicInputs,
) -> bool {
	let ark_proof = proof.to_ark_proof();
	let inputs = public_inputs.to_vec();

	// Prepare the verifying key for efficient verification
	let pvk: PreparedVerifyingKey<Bn254> = vk.inner().clone().into();

	// Verify the proof
	Groth16::<Bn254>::verify_proof(&pvk, &ark_proof, &inputs).unwrap_or(false)
}

/// Convert a u128 amount to a 32-byte field element representation
pub fn amount_to_bytes(amount: u128) -> [u8; 32] {
	let mut bytes = [0u8; 32];
	bytes[..16].copy_from_slice(&amount.to_le_bytes());
	bytes
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_bytes_to_field_roundtrip() {
		let original = [1u8; 32];
		let field = PublicInputs::bytes_to_field(&original);

		// Field element should be non-zero
		assert_ne!(field, Fr::from(0u64));
	}

	#[test]
	fn test_amount_to_bytes() {
		let amount: u128 = 1_000_000_000_000;
		let bytes = amount_to_bytes(amount);

		// First 16 bytes should contain the amount
		let recovered = u128::from_le_bytes(bytes[..16].try_into().unwrap());
		assert_eq!(recovered, amount);

		// Last 16 bytes should be zero
		assert_eq!(&bytes[16..], &[0u8; 16]);
	}
}
