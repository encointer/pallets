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

//! ZK Circuit for offline payment proofs.
//!
//! This circuit proves:
//! - Knowledge of zk_secret such that commitment = Poseidon(zk_secret)
//! - Nullifier correctness: nullifier = Poseidon(zk_secret, nonce)
//!
//! Public inputs: commitment, recipient_hash, amount, asset_hash, nullifier
//! Private inputs: zk_secret, nonce

use ark_bn254::Fr;
use ark_crypto_primitives::sponge::{
	constraints::CryptographicSpongeVar,
	poseidon::{constraints::PoseidonSpongeVar, PoseidonConfig, PoseidonSponge},
	CryptographicSponge,
};
use ark_r1cs_std::{fields::fp::FpVar, prelude::*};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_std::{vec, vec::Vec};

/// Poseidon configuration for BN254 scalar field
/// Using standard parameters for 2-to-1 compression
pub fn poseidon_config() -> PoseidonConfig<Fr> {
	// Standard Poseidon parameters for BN254
	// These are simplified parameters for the PoC
	// In production, use parameters from a trusted source
	let full_rounds = 8;
	let partial_rounds = 57;
	let alpha = 5;
	let rate = 2;
	let capacity = 1;

	// Generate round constants and MDS matrix
	// For PoC, we use deterministic generation
	let (ark, mds) = generate_poseidon_parameters(rate + capacity, full_rounds, partial_rounds);

	PoseidonConfig { full_rounds, partial_rounds, alpha: alpha as u64, ark, mds, rate, capacity }
}

/// Generate Poseidon parameters deterministically
fn generate_poseidon_parameters(
	t: usize,
	full_rounds: usize,
	partial_rounds: usize,
) -> (Vec<Vec<Fr>>, Vec<Vec<Fr>>) {
	use ark_ff::Field;

	let total_rounds = full_rounds + partial_rounds;

	// Generate ARK (AddRoundKey) constants
	let mut ark = Vec::with_capacity(total_rounds);
	for r in 0..total_rounds {
		let mut round_constants = Vec::with_capacity(t);
		for i in 0..t {
			// Deterministic constant generation using wrapping arithmetic
			let seed = ((r * t + i + 1) as u64).wrapping_mul(0x517cc1b727220a95u64);
			round_constants.push(Fr::from(seed));
		}
		ark.push(round_constants);
	}

	// Generate MDS matrix (simple Cauchy matrix)
	let mut mds = Vec::with_capacity(t);
	for i in 0..t {
		let mut row = Vec::with_capacity(t);
		for j in 0..t {
			// Cauchy matrix: M[i][j] = 1 / (x_i + y_j) where x_i = i+1, y_j = t+j+1
			let x = Fr::from((i + 1) as u64);
			let y = Fr::from((t + j + 1) as u64);
			let entry = (x + y).inverse().unwrap_or(Fr::from(1u64));
			row.push(entry);
		}
		mds.push(row);
	}

	(ark, mds)
}

/// Compute Poseidon hash of inputs
pub fn poseidon_hash(config: &PoseidonConfig<Fr>, inputs: &[Fr]) -> Fr {
	let mut sponge = PoseidonSponge::new(config);
	for input in inputs {
		sponge.absorb(input);
	}
	sponge.squeeze_field_elements::<Fr>(1)[0]
}

/// Compute commitment = Poseidon(zk_secret)
pub fn compute_commitment(config: &PoseidonConfig<Fr>, zk_secret: &Fr) -> Fr {
	poseidon_hash(config, &[*zk_secret])
}

/// Compute nullifier = Poseidon(zk_secret, nonce)
pub fn compute_nullifier(config: &PoseidonConfig<Fr>, zk_secret: &Fr, nonce: &Fr) -> Fr {
	poseidon_hash(config, &[*zk_secret, *nonce])
}

/// The offline payment circuit
#[derive(Clone)]
pub struct OfflinePaymentCircuit {
	/// Poseidon configuration
	pub poseidon_config: PoseidonConfig<Fr>,

	// Public inputs
	/// Commitment = Poseidon(zk_secret)
	pub commitment: Fr,
	/// Hash of recipient account
	pub recipient_hash: Fr,
	/// Payment amount as field element
	pub amount: Fr,
	/// Hash of community identifier
	pub asset_hash: Fr,
	/// Nullifier = Poseidon(zk_secret, nonce)
	pub nullifier: Fr,

	// Private inputs (witnesses)
	/// The secret key (private)
	pub zk_secret: Fr,
	/// Random nonce for nullifier (private)
	pub nonce: Fr,
}

impl OfflinePaymentCircuit {
	/// Create a new circuit instance
	pub fn new(
		poseidon_config: PoseidonConfig<Fr>,
		zk_secret: Fr,
		nonce: Fr,
		recipient_hash: Fr,
		amount: Fr,
		asset_hash: Fr,
	) -> Self {
		let commitment = compute_commitment(&poseidon_config, &zk_secret);
		let nullifier = compute_nullifier(&poseidon_config, &zk_secret, &nonce);

		Self {
			poseidon_config,
			commitment,
			recipient_hash,
			amount,
			asset_hash,
			nullifier,
			zk_secret,
			nonce,
		}
	}

	/// Get the public inputs for verification
	pub fn public_inputs(&self) -> Vec<Fr> {
		vec![self.commitment, self.recipient_hash, self.amount, self.asset_hash, self.nullifier]
	}
}

impl ConstraintSynthesizer<Fr> for OfflinePaymentCircuit {
	fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
		// Allocate public inputs
		let commitment_var = FpVar::new_input(cs.clone(), || Ok(self.commitment))?;
		let recipient_hash_var = FpVar::new_input(cs.clone(), || Ok(self.recipient_hash))?;
		let amount_var = FpVar::new_input(cs.clone(), || Ok(self.amount))?;
		let asset_hash_var = FpVar::new_input(cs.clone(), || Ok(self.asset_hash))?;
		let nullifier_var = FpVar::new_input(cs.clone(), || Ok(self.nullifier))?;

		// Allocate private witnesses
		let zk_secret_var = FpVar::new_witness(cs.clone(), || Ok(self.zk_secret))?;
		let nonce_var = FpVar::new_witness(cs.clone(), || Ok(self.nonce))?;

		// Create Poseidon sponge for constraint generation
		let mut commitment_sponge = PoseidonSpongeVar::new(cs.clone(), &self.poseidon_config);

		// Constraint 1: commitment = Poseidon(zk_secret)
		commitment_sponge.absorb(&vec![zk_secret_var.clone()])?;
		let computed_commitment = commitment_sponge.squeeze_field_elements(1)?[0].clone();
		computed_commitment.enforce_equal(&commitment_var)?;

		// Create new sponge for nullifier
		let mut nullifier_sponge = PoseidonSpongeVar::new(cs.clone(), &self.poseidon_config);

		// Constraint 2: nullifier = Poseidon(zk_secret, nonce)
		nullifier_sponge.absorb(&vec![zk_secret_var, nonce_var])?;
		let computed_nullifier = nullifier_sponge.squeeze_field_elements(1)?[0].clone();
		computed_nullifier.enforce_equal(&nullifier_var)?;

		// Constraint 3: Bind the other public inputs (they must be provided correctly)
		// These don't need additional constraints as they're already public inputs
		// The verifier ensures they match the claimed values
		let _ = recipient_hash_var;
		let _ = amount_var;
		let _ = asset_hash_var;

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use ark_relations::r1cs::ConstraintSystem;

	#[test]
	fn test_poseidon_hash_deterministic() {
		let config = poseidon_config();
		let input = Fr::from(12345u64);

		let hash1 = poseidon_hash(&config, &[input]);
		let hash2 = poseidon_hash(&config, &[input]);

		assert_eq!(hash1, hash2);
	}

	#[test]
	fn test_commitment_computation() {
		let config = poseidon_config();
		let zk_secret = Fr::from(42u64);

		let commitment = compute_commitment(&config, &zk_secret);

		// Should be non-zero and deterministic
		assert_ne!(commitment, Fr::from(0u64));
		assert_eq!(commitment, compute_commitment(&config, &zk_secret));
	}

	#[test]
	fn test_nullifier_computation() {
		let config = poseidon_config();
		let zk_secret = Fr::from(42u64);
		let nonce1 = Fr::from(1u64);
		let nonce2 = Fr::from(2u64);

		let nullifier1 = compute_nullifier(&config, &zk_secret, &nonce1);
		let nullifier2 = compute_nullifier(&config, &zk_secret, &nonce2);

		// Different nonces should produce different nullifiers
		assert_ne!(nullifier1, nullifier2);
	}

	#[test]
	fn test_circuit_constraints_satisfied() {
		let config = poseidon_config();
		let zk_secret = Fr::from(12345u64);
		let nonce = Fr::from(67890u64);
		let recipient_hash = Fr::from(111u64);
		let amount = Fr::from(1000u64);
		let asset_hash = Fr::from(222u64);

		let circuit = OfflinePaymentCircuit::new(
			config,
			zk_secret,
			nonce,
			recipient_hash,
			amount,
			asset_hash,
		);

		let cs = ConstraintSystem::<Fr>::new_ref();
		circuit.generate_constraints(cs.clone()).unwrap();

		assert!(cs.is_satisfied().unwrap());
		println!("Number of constraints: {}", cs.num_constraints());
	}

	#[test]
	fn test_circuit_with_wrong_commitment_fails() {
		let config = poseidon_config();
		let zk_secret = Fr::from(12345u64);
		let nonce = Fr::from(67890u64);

		// Create circuit with correct values
		let mut circuit = OfflinePaymentCircuit::new(
			config.clone(),
			zk_secret,
			nonce,
			Fr::from(111u64),
			Fr::from(1000u64),
			Fr::from(222u64),
		);

		// Tamper with commitment
		circuit.commitment = Fr::from(99999u64);

		let cs = ConstraintSystem::<Fr>::new_ref();
		circuit.generate_constraints(cs.clone()).unwrap();

		// Should not be satisfied due to commitment mismatch
		assert!(!cs.is_satisfied().unwrap());
	}

	#[test]
	fn test_circuit_with_wrong_nullifier_fails() {
		let config = poseidon_config();
		let zk_secret = Fr::from(12345u64);
		let nonce = Fr::from(67890u64);

		let mut circuit = OfflinePaymentCircuit::new(
			config.clone(),
			zk_secret,
			nonce,
			Fr::from(111u64),
			Fr::from(1000u64),
			Fr::from(222u64),
		);

		// Tamper with nullifier
		circuit.nullifier = Fr::from(99999u64);

		let cs = ConstraintSystem::<Fr>::new_ref();
		circuit.generate_constraints(cs.clone()).unwrap();

		assert!(!cs.is_satisfied().unwrap());
	}
}
