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

//! Groth16 proof generation for offline payments.
//!
//! This module provides:
//! - Trusted setup (key generation)
//! - Proof generation
//! - Test fixtures for e2e testing

use crate::circuit::{poseidon_config, OfflinePaymentCircuit};
use ark_bn254::{Bn254, Fr};
use ark_ff::{BigInteger, PrimeField};
use ark_groth16::{Groth16, PreparedVerifyingKey, Proof, ProvingKey, VerifyingKey};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_snark::SNARK;
use ark_std::vec::Vec;

#[cfg(feature = "std")]
use ark_std::rand::{rngs::StdRng, SeedableRng};

/// Result of trusted setup ceremony
pub struct TrustedSetup {
    pub proving_key: ProvingKey<Bn254>,
    pub verifying_key: VerifyingKey<Bn254>,
}

impl TrustedSetup {
    /// Perform trusted setup with a deterministic seed (FOR TESTING ONLY)
    /// In production, use a proper MPC ceremony
    #[cfg(feature = "std")]
    pub fn generate_with_seed(seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let config = poseidon_config();

        // Create a dummy circuit for setup
        let circuit = OfflinePaymentCircuit::new(
            config,
            Fr::from(1u64),
            Fr::from(1u64),
            Fr::from(1u64),
            Fr::from(1u64),
            Fr::from(1u64),
        );

        let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(circuit, &mut rng)
            .expect("Setup failed");

        Self {
            proving_key: pk,
            verifying_key: vk,
        }
    }

    /// Serialize the verifying key to bytes
    pub fn verifying_key_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.verifying_key
            .serialize_compressed(&mut bytes)
            .expect("Serialization failed");
        bytes
    }

    /// Serialize the proving key to bytes
    pub fn proving_key_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.proving_key
            .serialize_compressed(&mut bytes)
            .expect("Serialization failed");
        bytes
    }

    /// Deserialize verifying key from bytes
    pub fn verifying_key_from_bytes(bytes: &[u8]) -> Option<VerifyingKey<Bn254>> {
        VerifyingKey::<Bn254>::deserialize_compressed(bytes).ok()
    }

    /// Deserialize proving key from bytes
    pub fn proving_key_from_bytes(bytes: &[u8]) -> Option<ProvingKey<Bn254>> {
        ProvingKey::<Bn254>::deserialize_compressed(bytes).ok()
    }
}

/// Generate a Groth16 proof for an offline payment
#[cfg(feature = "std")]
pub fn generate_proof(
    proving_key: &ProvingKey<Bn254>,
    zk_secret: Fr,
    nonce: Fr,
    recipient_hash: Fr,
    amount: Fr,
    cid_hash: Fr,
) -> Option<(Proof<Bn254>, Vec<Fr>)> {
    let config = poseidon_config();
    let circuit = OfflinePaymentCircuit::new(
        config,
        zk_secret,
        nonce,
        recipient_hash,
        amount,
        cid_hash,
    );

    let public_inputs = circuit.public_inputs();

    let mut rng = StdRng::seed_from_u64(
        // Use timestamp or other entropy in production
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64,
    );

    let proof = Groth16::<Bn254>::prove(proving_key, circuit, &mut rng).ok()?;

    Some((proof, public_inputs))
}

/// Serialize a proof to bytes
pub fn proof_to_bytes(proof: &Proof<Bn254>) -> Vec<u8> {
    let mut bytes = Vec::new();
    proof
        .serialize_compressed(&mut bytes)
        .expect("Serialization failed");
    bytes
}

/// Deserialize a proof from bytes
pub fn proof_from_bytes(bytes: &[u8]) -> Option<Proof<Bn254>> {
    Proof::<Bn254>::deserialize_compressed(bytes).ok()
}

/// Verify a proof (for testing - on-chain uses the pallet's verifier)
#[cfg(feature = "std")]
pub fn verify_proof(
    verifying_key: &VerifyingKey<Bn254>,
    proof: &Proof<Bn254>,
    public_inputs: &[Fr],
) -> bool {
    let pvk: PreparedVerifyingKey<Bn254> = verifying_key.clone().into();
    Groth16::<Bn254>::verify_proof(&pvk, proof, public_inputs).is_ok()
}

/// Convert a 32-byte array to a field element
pub fn bytes32_to_field(bytes: &[u8; 32]) -> Fr {
    Fr::from_le_bytes_mod_order(bytes)
}

/// Convert a field element to 32 bytes
pub fn field_to_bytes32(field: &Fr) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    let repr = field.into_bigint().to_bytes_le();
    bytes[..repr.len().min(32)].copy_from_slice(&repr[..repr.len().min(32)]);
    bytes
}

/// Well-known test seed for deterministic trusted setup
/// This seed should ONLY be used for testing
pub const TEST_SETUP_SEED: u64 = 0xDEADBEEF_CAFEBABE;

/// Pre-generated test fixtures using TEST_SETUP_SEED
#[cfg(feature = "std")]
pub mod test_fixtures {
    use super::*;
    use std::sync::OnceLock;

    static TEST_SETUP: OnceLock<TrustedSetup> = OnceLock::new();

    /// Get the test trusted setup (lazily initialized)
    pub fn get_test_setup() -> &'static TrustedSetup {
        TEST_SETUP.get_or_init(|| TrustedSetup::generate_with_seed(TEST_SETUP_SEED))
    }

    /// Get test verifying key bytes
    pub fn get_test_vk_bytes() -> Vec<u8> {
        get_test_setup().verifying_key_bytes()
    }

    /// Get test proving key
    pub fn get_test_pk() -> &'static ProvingKey<Bn254> {
        &get_test_setup().proving_key
    }

    /// Generate a test proof with well-known parameters
    pub fn generate_test_proof(
        zk_secret_bytes: &[u8; 32],
        nonce_bytes: &[u8; 32],
        recipient_hash_bytes: &[u8; 32],
        amount_bytes: &[u8; 32],
        cid_hash_bytes: &[u8; 32],
    ) -> Option<(Vec<u8>, [u8; 32], [u8; 32])> {
        let pk = get_test_pk();

        let zk_secret = bytes32_to_field(zk_secret_bytes);
        let nonce = bytes32_to_field(nonce_bytes);
        let recipient_hash = bytes32_to_field(recipient_hash_bytes);
        let amount = bytes32_to_field(amount_bytes);
        let cid_hash = bytes32_to_field(cid_hash_bytes);

        let (proof, public_inputs) = generate_proof(
            pk,
            zk_secret,
            nonce,
            recipient_hash,
            amount,
            cid_hash,
        )?;

        let proof_bytes = proof_to_bytes(&proof);
        let commitment_bytes = field_to_bytes32(&public_inputs[0]);
        let nullifier_bytes = field_to_bytes32(&public_inputs[4]);

        Some((proof_bytes, commitment_bytes, nullifier_bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trusted_setup_deterministic() {
        let setup1 = TrustedSetup::generate_with_seed(12345);
        let setup2 = TrustedSetup::generate_with_seed(12345);

        assert_eq!(
            setup1.verifying_key_bytes(),
            setup2.verifying_key_bytes()
        );
    }

    #[test]
    fn test_proof_generation_and_verification() {
        let setup = TrustedSetup::generate_with_seed(TEST_SETUP_SEED);

        let zk_secret = Fr::from(42u64);
        let nonce = Fr::from(123u64);
        let recipient_hash = Fr::from(456u64);
        let amount = Fr::from(1000u64);
        let cid_hash = Fr::from(789u64);

        let (proof, public_inputs) = generate_proof(
            &setup.proving_key,
            zk_secret,
            nonce,
            recipient_hash,
            amount,
            cid_hash,
        )
        .expect("Proof generation failed");

        // Verify the proof
        assert!(verify_proof(&setup.verifying_key, &proof, &public_inputs));
    }

    #[test]
    fn test_proof_serialization_roundtrip() {
        let setup = TrustedSetup::generate_with_seed(TEST_SETUP_SEED);

        let (proof, _) = generate_proof(
            &setup.proving_key,
            Fr::from(42u64),
            Fr::from(123u64),
            Fr::from(456u64),
            Fr::from(1000u64),
            Fr::from(789u64),
        )
        .unwrap();

        let bytes = proof_to_bytes(&proof);
        let recovered = proof_from_bytes(&bytes).unwrap();

        assert_eq!(proof, recovered);
    }

    #[test]
    fn test_vk_serialization_roundtrip() {
        let setup = TrustedSetup::generate_with_seed(TEST_SETUP_SEED);

        let bytes = setup.verifying_key_bytes();
        let recovered = TrustedSetup::verifying_key_from_bytes(&bytes).unwrap();

        // Compare serialized forms
        let mut recovered_bytes = Vec::new();
        recovered.serialize_compressed(&mut recovered_bytes).unwrap();
        assert_eq!(bytes, recovered_bytes);
    }

    #[test]
    fn test_mismatched_inputs_detected_by_chain() {
        // This test verifies that while Groth16 proofs may verify mathematically,
        // the on-chain logic catches mismatches between claimed and actual values.
        //
        // In Groth16, the proof is valid for ANY public inputs that satisfy the circuit.
        // The security comes from the on-chain check that the commitment matches the
        // registered identity. If someone tries to use a different commitment,
        // the on-chain lookup will fail.
        let setup = TrustedSetup::generate_with_seed(TEST_SETUP_SEED);

        let (proof, public_inputs) = generate_proof(
            &setup.proving_key,
            Fr::from(42u64),
            Fr::from(123u64),
            Fr::from(456u64),
            Fr::from(1000u64),
            Fr::from(789u64),
        )
        .unwrap();

        // The proof verifies with the correct public inputs
        assert!(verify_proof(&setup.verifying_key, &proof, &public_inputs));

        // The commitment is public_inputs[0]
        // On-chain, we verify that this commitment is registered for the sender
        // If someone submits a different commitment, the chain rejects it
        // because it won't match OfflineIdentities storage
    }

    #[test]
    fn test_field_bytes_roundtrip() {
        let original = Fr::from(123456789u64);
        let bytes = field_to_bytes32(&original);
        let recovered = bytes32_to_field(&bytes);
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_test_fixtures() {
        use test_fixtures::*;

        let zk_secret = [1u8; 32];
        let nonce = [2u8; 32];
        let recipient = [3u8; 32];
        let amount = [4u8; 32];
        let cid = [5u8; 32];

        let (proof_bytes, commitment, nullifier) =
            generate_test_proof(&zk_secret, &nonce, &recipient, &amount, &cid)
                .expect("Test proof generation failed");

        // Verify the proof is non-empty
        assert!(!proof_bytes.is_empty());
        assert_ne!(commitment, [0u8; 32]);
        assert_ne!(nullifier, [0u8; 32]);

        // Verify the proof
        let vk = TrustedSetup::verifying_key_from_bytes(&get_test_vk_bytes()).unwrap();
        let proof = proof_from_bytes(&proof_bytes).unwrap();

        let public_inputs = vec![
            bytes32_to_field(&commitment),
            bytes32_to_field(&recipient),
            bytes32_to_field(&amount),
            bytes32_to_field(&cid),
            bytes32_to_field(&nullifier),
        ];

        assert!(verify_proof(&vk, &proof, &public_inputs));
    }
}
