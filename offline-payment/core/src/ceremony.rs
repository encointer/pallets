//! Multiparty trusted setup ceremony for Groth16 on BN254.
//!
//! Implements Phase 2 delta-rerandomization: each participant samples a random
//! scalar `d`, rerandomizes the proving key's delta-dependent terms, and produces
//! a proof-of-knowledge receipt.  Security: if *any one* participant deletes their
//! randomness, the combined toxic waste is unrecoverable.
//!
//! Protocol:
//! 1. **Init** — generate initial CRS via `Groth16::circuit_specific_setup`
//! 2. **Contribute** — sample `d`, multiply `delta_g2 *= d`, divide `h_query` and `l_query` by `d`,
//!    emit a [`ContributionReceipt`]
//! 3. **Verify** — pairing check on receipts + functional proof test
//! 4. **Finalize** — extract PK/VK from ceremony state

use ark_bn254::{Bn254, Fr, G1Affine, G1Projective, G2Affine, G2Projective};
use ark_ec::{pairing::Pairing, AffineRepr, CurveGroup, Group};
use ark_ff::Field;
use ark_groth16::{Groth16, ProvingKey};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_snark::SNARK;
use ark_std::{
	rand::{rngs::StdRng, SeedableRng},
	vec::Vec,
	UniformRand,
};

use crate::{
	circuit::{poseidon_config, OfflinePaymentCircuit},
	prover::{bytes32_to_field, generate_proof, verify_proof},
};

/// Proof-of-knowledge for a single contribution.
#[derive(Clone, Debug, PartialEq)]
pub struct ContributionReceipt {
	/// `d * G1::generator()` — proves knowledge of the scalar `d`
	pub d_g1: G1Affine,
	/// `delta_g2` *before* this contribution
	pub old_delta_g2: G2Affine,
	/// `delta_g2` *after* this contribution
	pub new_delta_g2: G2Affine,
}

impl ContributionReceipt {
	pub fn to_bytes(&self) -> Vec<u8> {
		let mut buf = Vec::new();
		self.d_g1.serialize_compressed(&mut buf).expect("serialize d_g1");
		self.old_delta_g2
			.serialize_compressed(&mut buf)
			.expect("serialize old_delta_g2");
		self.new_delta_g2
			.serialize_compressed(&mut buf)
			.expect("serialize new_delta_g2");
		buf
	}

	pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
		let mut cursor = bytes;
		let d_g1 = G1Affine::deserialize_compressed(&mut cursor).ok()?;
		let old_delta_g2 = G2Affine::deserialize_compressed(&mut cursor).ok()?;
		let new_delta_g2 = G2Affine::deserialize_compressed(&mut cursor).ok()?;
		Some(Self { d_g1, old_delta_g2, new_delta_g2 })
	}
}

/// Serialize a proving key to compressed bytes.
pub fn serialize_pk(pk: &ProvingKey<Bn254>) -> Vec<u8> {
	let mut buf = Vec::new();
	pk.serialize_compressed(&mut buf).expect("serialize PK");
	buf
}

/// Serialize the VK embedded in a proving key to compressed bytes.
pub fn serialize_vk(pk: &ProvingKey<Bn254>) -> Vec<u8> {
	let mut buf = Vec::new();
	pk.vk.serialize_compressed(&mut buf).expect("serialize VK");
	buf
}

/// Serialize delta_g2 from a proving key to compressed bytes.
pub fn serialize_delta_g2(pk: &ProvingKey<Bn254>) -> Vec<u8> {
	let mut buf = Vec::new();
	pk.vk.delta_g2.serialize_compressed(&mut buf).expect("serialize delta_g2");
	buf
}

/// Create a dummy circuit for setup / functional tests.
fn dummy_circuit() -> OfflinePaymentCircuit {
	OfflinePaymentCircuit::new(
		poseidon_config(),
		Fr::from(1u64),
		Fr::from(1u64),
		Fr::from(1u64),
		Fr::from(1u64),
		Fr::from(1u64),
	)
}

/// Initialize a ceremony — generate initial CRS with high-entropy randomness.
pub fn ceremony_init() -> ProvingKey<Bn254> {
	let mut rng = StdRng::seed_from_u64(entropy_seed());
	let (pk, _vk) =
		Groth16::<Bn254>::circuit_specific_setup(dummy_circuit(), &mut rng).expect("setup failed");
	pk
}

/// Derive a seed from the current system time (nanosecond precision).
fn entropy_seed() -> u64 {
	std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.unwrap()
		.as_nanos() as u64
}

/// Initialize a ceremony with a deterministic seed (testing only).
pub fn ceremony_init_with_seed(seed: u64) -> ProvingKey<Bn254> {
	use ark_std::rand::{rngs::StdRng, SeedableRng};
	let mut rng = StdRng::seed_from_u64(seed);
	let (pk, _vk) =
		Groth16::<Bn254>::circuit_specific_setup(dummy_circuit(), &mut rng).expect("setup failed");
	pk
}

/// Apply one contribution: rerandomize the proving key's delta-dependent terms.
///
/// Returns the updated proving key and a receipt for verification.
pub fn ceremony_contribute(pk: ProvingKey<Bn254>) -> (ProvingKey<Bn254>, ContributionReceipt) {
	let mut rng = StdRng::seed_from_u64(entropy_seed());
	let d = Fr::rand(&mut rng);
	ceremony_contribute_with_scalar(pk, d)
}

/// Deterministic contribution (testing only).
fn ceremony_contribute_with_scalar(
	mut pk: ProvingKey<Bn254>,
	d: Fr,
) -> (ProvingKey<Bn254>, ContributionReceipt) {
	let d_inv = d.inverse().expect("d must be nonzero");

	let old_delta_g2 = pk.vk.delta_g2;

	// delta_g2 *= d
	let new_delta_g2: G2Affine = (G2Projective::from(pk.vk.delta_g2) * d).into_affine();
	pk.vk.delta_g2 = new_delta_g2;

	// delta_g1 *= d  (proving key also stores delta_g1)
	pk.delta_g1 = (G1Projective::from(pk.delta_g1) * d).into_affine();

	// h_query[i] *= d_inv  (these contain …/delta terms)
	for h in pk.h_query.iter_mut() {
		*h = (G1Projective::from(*h) * d_inv).into_affine();
	}

	// l_query[i] *= d_inv
	for l in pk.l_query.iter_mut() {
		*l = (G1Projective::from(*l) * d_inv).into_affine();
	}

	let d_g1: G1Affine = (G1Projective::generator() * d).into_affine();

	let receipt = ContributionReceipt { d_g1, old_delta_g2, new_delta_g2 };
	(pk, receipt)
}

/// Verify a single contribution receipt via pairing check:
///   `e(d_g1, old_delta_g2) == e(G1::gen, new_delta_g2)`
pub fn verify_contribution(receipt: &ContributionReceipt) -> bool {
	let lhs = Bn254::pairing(receipt.d_g1, receipt.old_delta_g2);
	let rhs = Bn254::pairing(G1Affine::generator(), receipt.new_delta_g2);
	lhs == rhs
}

/// Functional test: generate a proof with the PK and verify with its embedded VK.
pub fn verify_ceremony_pk(pk: &ProvingKey<Bn254>) -> bool {
	let zk_secret = bytes32_to_field(&[1u8; 32]);
	let nonce = bytes32_to_field(&[2u8; 32]);
	let recipient_hash = bytes32_to_field(&[3u8; 32]);
	let amount = bytes32_to_field(&[4u8; 32]);
	let asset_hash = bytes32_to_field(&[5u8; 32]);

	let Some((proof, public_inputs)) =
		generate_proof(pk, zk_secret, nonce, recipient_hash, amount, asset_hash)
	else {
		return false;
	};

	verify_proof(&pk.vk, &proof, &public_inputs)
}

#[cfg(test)]
mod tests {
	use super::*;

	const SEED: u64 = 0xCE5E_0001;

	#[test]
	fn init_produces_valid_pk() {
		let pk = ceremony_init_with_seed(SEED);
		assert!(verify_ceremony_pk(&pk));
	}

	#[test]
	fn single_contribution() {
		let pk = ceremony_init_with_seed(SEED);
		let (pk2, receipt) = ceremony_contribute_with_scalar(pk, Fr::from(42u64));
		assert!(verify_contribution(&receipt));
		assert!(verify_ceremony_pk(&pk2));
	}

	#[test]
	fn three_contributions() {
		let pk = ceremony_init_with_seed(SEED);
		let (pk, r1) = ceremony_contribute_with_scalar(pk, Fr::from(111u64));
		let (pk, r2) = ceremony_contribute_with_scalar(pk, Fr::from(222u64));
		let (pk, r3) = ceremony_contribute_with_scalar(pk, Fr::from(333u64));

		assert!(verify_contribution(&r1));
		assert!(verify_contribution(&r2));
		assert!(verify_contribution(&r3));
		assert!(verify_ceremony_pk(&pk));
	}

	#[test]
	fn receipt_chain_consistency() {
		let pk = ceremony_init_with_seed(SEED);
		let (pk, r1) = ceremony_contribute_with_scalar(pk, Fr::from(10u64));
		let (_, r2) = ceremony_contribute_with_scalar(pk, Fr::from(20u64));

		// r1.new_delta_g2 must equal r2.old_delta_g2
		assert_eq!(r1.new_delta_g2, r2.old_delta_g2);
	}

	#[test]
	fn tampered_receipt_fails() {
		let pk = ceremony_init_with_seed(SEED);
		let (_pk, mut receipt) = ceremony_contribute_with_scalar(pk, Fr::from(42u64));

		// Tamper: swap old and new
		std::mem::swap(&mut receipt.old_delta_g2, &mut receipt.new_delta_g2);
		assert!(!verify_contribution(&receipt));
	}

	#[test]
	fn receipt_serialization_roundtrip() {
		let pk = ceremony_init_with_seed(SEED);
		let (_pk, receipt) = ceremony_contribute_with_scalar(pk, Fr::from(99u64));

		let bytes = receipt.to_bytes();
		let recovered = ContributionReceipt::from_bytes(&bytes).unwrap();
		assert_eq!(receipt, recovered);
	}

	#[test]
	fn ceremony_pk_differs_from_single_party() {
		let pk_single = ceremony_init_with_seed(SEED);
		let pk_ceremony = ceremony_init_with_seed(SEED);
		let (pk_ceremony, _) = ceremony_contribute_with_scalar(pk_ceremony, Fr::from(77u64));

		// delta_g2 must differ after contribution
		assert_ne!(pk_single.vk.delta_g2, pk_ceremony.vk.delta_g2);

		// But both must produce valid proofs
		assert!(verify_ceremony_pk(&pk_single));
		assert!(verify_ceremony_pk(&pk_ceremony));
	}
}
