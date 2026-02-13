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

use crate::{mock::*, Error, Event, Groth16ProofBytes, OfflineIdentities, UsedNullifiers};
use encointer_primitives::{balances::BalanceType, communities::CommunityIdentifier};
use frame_support::{assert_noop, assert_ok, traits::Currency, BoundedVec};
use sp_keyring::Sr25519Keyring;
use test_utils::helpers::register_test_community;

fn alice() -> <TestRuntime as frame_system::Config>::AccountId {
	Sr25519Keyring::Alice.to_account_id()
}

fn bob() -> <TestRuntime as frame_system::Config>::AccountId {
	Sr25519Keyring::Bob.to_account_id()
}

fn charlie() -> <TestRuntime as frame_system::Config>::AccountId {
	Sr25519Keyring::Charlie.to_account_id()
}

fn test_commitment() -> [u8; 32] {
	[1u8; 32]
}

fn test_nullifier() -> [u8; 32] {
	[2u8; 32]
}

fn setup_community_with_balance(
	account: &<TestRuntime as frame_system::Config>::AccountId,
	balance: BalanceType,
) -> CommunityIdentifier {
	let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
	let _ = pallet_encointer_balances::Pallet::<TestRuntime>::issue(cid, account, balance);
	cid
}

fn fund_native(account: &<TestRuntime as frame_system::Config>::AccountId, amount: u128) {
	Balances::make_free_balance_be(account, amount);
}

// ============ Register Offline Identity Tests ============

#[test]
fn register_offline_identity_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		let commitment = test_commitment();

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		assert_eq!(OfflineIdentities::<TestRuntime>::get(alice()), Some(commitment));

		// Check event
		System::assert_last_event(
			Event::<TestRuntime>::OfflineIdentityRegistered { who: alice(), commitment }.into(),
		);
	});
}

#[test]
fn register_offline_identity_fails_if_already_registered() {
	new_test_ext().execute_with(|| {
		let commitment = test_commitment();

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		assert_noop!(
			EncointerOfflinePayment::register_offline_identity(
				RuntimeOrigin::signed(alice()),
				commitment
			),
			Error::<TestRuntime>::AlreadyRegistered
		);
	});
}

#[test]
fn different_accounts_can_register_different_commitments() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		let commitment1 = [1u8; 32];
		let commitment2 = [2u8; 32];

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment1
		));

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(bob()),
			commitment2
		));

		assert_eq!(OfflineIdentities::<TestRuntime>::get(alice()), Some(commitment1));
		assert_eq!(OfflineIdentities::<TestRuntime>::get(bob()), Some(commitment2));
	});
}

// ============ Submit Offline Payment Error Tests ============

#[test]
fn submit_offline_payment_fails_without_verification_key() {
	new_test_ext().execute_with(|| {
		let commitment = test_commitment();
		let nullifier = test_nullifier();
		let amount = BalanceType::from_num(10);

		let cid = setup_community_with_balance(&alice(), BalanceType::from_num(100));

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		// Create dummy proof bytes
		let proof_bytes: BoundedVec<u8, MaxProofSize> =
			BoundedVec::try_from(vec![0u8; 128]).unwrap();
		let proof = Groth16ProofBytes { proof_bytes };

		assert_noop!(
			EncointerOfflinePayment::submit_offline_payment(
				RuntimeOrigin::signed(charlie()),
				proof,
				alice(),
				bob(),
				amount,
				cid,
				nullifier
			),
			Error::<TestRuntime>::NoVerificationKey
		);
	});
}

#[test]
fn submit_offline_payment_fails_with_unregistered_sender() {
	new_test_ext().execute_with(|| {
		let nullifier = test_nullifier();
		let amount = BalanceType::from_num(10);

		let cid = setup_community_with_balance(&alice(), BalanceType::from_num(100));

		// Alice has NOT registered offline identity
		let proof_bytes: BoundedVec<u8, MaxProofSize> =
			BoundedVec::try_from(vec![0u8; 128]).unwrap();
		let proof = Groth16ProofBytes { proof_bytes };

		assert_noop!(
			EncointerOfflinePayment::submit_offline_payment(
				RuntimeOrigin::signed(charlie()),
				proof,
				alice(),
				bob(),
				amount,
				cid,
				nullifier
			),
			Error::<TestRuntime>::NoOfflineIdentity
		);
	});
}

#[test]
fn submit_offline_payment_fails_with_zero_amount() {
	new_test_ext().execute_with(|| {
		let commitment = test_commitment();
		let nullifier = test_nullifier();

		let cid = setup_community_with_balance(&alice(), BalanceType::from_num(100));

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		let proof_bytes: BoundedVec<u8, MaxProofSize> =
			BoundedVec::try_from(vec![0u8; 128]).unwrap();
		let proof = Groth16ProofBytes { proof_bytes };

		assert_noop!(
			EncointerOfflinePayment::submit_offline_payment(
				RuntimeOrigin::signed(charlie()),
				proof,
				alice(),
				bob(),
				BalanceType::from_num(0),
				cid,
				nullifier
			),
			Error::<TestRuntime>::AmountMustBePositive
		);
	});
}

#[test]
fn submit_offline_payment_fails_when_sender_equals_recipient() {
	new_test_ext().execute_with(|| {
		let commitment = test_commitment();
		let nullifier = test_nullifier();
		let amount = BalanceType::from_num(10);

		let cid = setup_community_with_balance(&alice(), BalanceType::from_num(100));

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		let proof_bytes: BoundedVec<u8, MaxProofSize> =
			BoundedVec::try_from(vec![0u8; 128]).unwrap();
		let proof = Groth16ProofBytes { proof_bytes };

		assert_noop!(
			EncointerOfflinePayment::submit_offline_payment(
				RuntimeOrigin::signed(charlie()),
				proof,
				alice(),
				alice(), // Same as sender
				amount,
				cid,
				nullifier
			),
			Error::<TestRuntime>::SenderEqualsRecipient
		);
	});
}

#[test]
fn submit_offline_payment_fails_with_insufficient_balance() {
	new_test_ext().execute_with(|| {
		let commitment = test_commitment();
		let nullifier = test_nullifier();
		let amount = BalanceType::from_num(200); // More than Alice has

		let cid = setup_community_with_balance(&alice(), BalanceType::from_num(100));

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		let proof_bytes: BoundedVec<u8, MaxProofSize> =
			BoundedVec::try_from(vec![0u8; 128]).unwrap();
		let proof = Groth16ProofBytes { proof_bytes };

		assert_noop!(
			EncointerOfflinePayment::submit_offline_payment(
				RuntimeOrigin::signed(charlie()),
				proof,
				alice(),
				bob(),
				amount,
				cid,
				nullifier
			),
			Error::<TestRuntime>::InsufficientBalance
		);
	});
}

// ============ Set Verification Key Tests ============

#[test]
fn set_verification_key_fails_for_non_root() {
	new_test_ext().execute_with(|| {
		let vk_bytes: BoundedVec<u8, MaxVkSize> = BoundedVec::try_from(vec![0u8; 100]).unwrap();

		assert_noop!(
			EncointerOfflinePayment::set_verification_key(RuntimeOrigin::signed(alice()), vk_bytes),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn set_verification_key_fails_with_invalid_vk() {
	new_test_ext().execute_with(|| {
		// Invalid VK bytes (not a valid serialized verification key)
		let vk_bytes: BoundedVec<u8, MaxVkSize> = BoundedVec::try_from(vec![0u8; 100]).unwrap();

		assert_noop!(
			EncointerOfflinePayment::set_verification_key(RuntimeOrigin::root(), vk_bytes),
			Error::<TestRuntime>::VkDeserializationFailed
		);
	});
}

// ============ Nullifier Tests ============

#[test]
fn nullifier_storage_works() {
	new_test_ext().execute_with(|| {
		let nullifier = test_nullifier();

		// Initially not used
		assert!(!UsedNullifiers::<TestRuntime>::contains_key(nullifier));

		// Mark as used
		UsedNullifiers::<TestRuntime>::insert(nullifier, ());

		// Now it's used
		assert!(UsedNullifiers::<TestRuntime>::contains_key(nullifier));
	});
}

// ============ Helper Function Tests ============

#[test]
fn derive_zk_secret_is_deterministic() {
	let seed = b"test-seed";
	let secret1 = crate::derive_zk_secret(seed);
	let secret2 = crate::derive_zk_secret(seed);
	assert_eq!(secret1, secret2);
}

#[test]
fn derive_zk_secret_is_different_for_different_seeds() {
	let secret1 = crate::derive_zk_secret(b"seed1");
	let secret2 = crate::derive_zk_secret(b"seed2");
	assert_ne!(secret1, secret2);
}

#[test]
fn hash_recipient_is_deterministic() {
	let recipient = b"alice";
	let hash1 = crate::hash_recipient(recipient);
	let hash2 = crate::hash_recipient(recipient);
	assert_eq!(hash1, hash2);
}

#[test]
fn balance_to_bytes_works() {
	let amount = BalanceType::from_num(100);
	let bytes = crate::balance_to_bytes(amount);

	// Should be 32 bytes
	assert_eq!(bytes.len(), 32);

	// Last 16 bytes should be zero (padding)
	assert_eq!(&bytes[16..], &[0u8; 16]);
}

// ============ Full E2E ZK Test ============

#[test]
fn e2e_zk_payment_works() {
	use crate::{
		circuit::{compute_commitment, poseidon_config},
		prover::{
			bytes32_to_field, field_to_bytes32, generate_proof, proof_to_bytes, TrustedSetup,
			TEST_SETUP_SEED,
		},
	};
	use ark_bn254::Fr;
	use parity_scale_codec::Encode;

	new_test_ext().execute_with(|| {
		System::set_block_number(1);

		// Step 1: Generate trusted setup (in production, this is done once via MPC)
		let setup = TrustedSetup::generate_with_seed(TEST_SETUP_SEED);
		let vk_bytes = setup.verifying_key_bytes();

		// Step 2: Set the verification key via sudo
		let bounded_vk: BoundedVec<u8, MaxVkSize> =
			BoundedVec::try_from(vk_bytes).expect("VK too large");
		assert_ok!(EncointerOfflinePayment::set_verification_key(
			RuntimeOrigin::root(),
			bounded_vk
		));

		// Step 3: Setup - create community and fund Alice
		let cid = setup_community_with_balance(&alice(), BalanceType::from_num(100));

		// Step 4: Generate ZK secret and commitment using Poseidon
		let poseidon = poseidon_config();
		let zk_secret = Fr::from(12345u64);
		let commitment_field = compute_commitment(&poseidon, &zk_secret);
		let commitment = field_to_bytes32(&commitment_field);

		// Step 5: Register offline identity with the Poseidon commitment
		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		// Step 6: Generate the ZK proof for a payment
		let nonce = Fr::from(67890u64);
		let amount = BalanceType::from_num(10);

		// Hash public inputs
		let recipient_hash_bytes = crate::hash_recipient(&bob().encode());
		let cid_hash_bytes = crate::hash_cid(&cid);
		let amount_bytes = crate::balance_to_bytes(amount);

		let recipient_hash = bytes32_to_field(&recipient_hash_bytes);
		let cid_hash = bytes32_to_field(&cid_hash_bytes);
		let amount_field = bytes32_to_field(&amount_bytes);

		let (proof, public_inputs) = generate_proof(
			&setup.proving_key,
			zk_secret,
			nonce,
			recipient_hash,
			amount_field,
			cid_hash,
		)
		.expect("Proof generation failed");

		let proof_bytes = proof_to_bytes(&proof);
		let nullifier_field = public_inputs[4]; // nullifier is the 5th public input
		let nullifier = field_to_bytes32(&nullifier_field);

		// Step 7: Submit the offline payment
		let bounded_proof: BoundedVec<u8, MaxProofSize> =
			BoundedVec::try_from(proof_bytes).expect("Proof too large");
		let proof_struct = Groth16ProofBytes { proof_bytes: bounded_proof };

		assert_ok!(EncointerOfflinePayment::submit_offline_payment(
			RuntimeOrigin::signed(charlie()), // Anyone can submit
			proof_struct,
			alice(),
			bob(),
			amount,
			cid,
			nullifier
		));

		// Step 8: Verify the payment was processed
		assert_eq!(
			pallet_encointer_balances::Pallet::<TestRuntime>::balance(cid, &alice()),
			BalanceType::from_num(90)
		);
		assert_eq!(
			pallet_encointer_balances::Pallet::<TestRuntime>::balance(cid, &bob()),
			BalanceType::from_num(10)
		);

		// Step 9: Verify nullifier is marked as used (double-spend prevention)
		assert!(UsedNullifiers::<TestRuntime>::contains_key(nullifier));

		// Step 10: Try to double-spend - should fail
		let bounded_proof2: BoundedVec<u8, MaxProofSize> =
			BoundedVec::try_from(vec![0u8; 128]).unwrap();
		let proof_struct2 = Groth16ProofBytes { proof_bytes: bounded_proof2 };
		assert_noop!(
			EncointerOfflinePayment::submit_offline_payment(
				RuntimeOrigin::signed(charlie()),
				proof_struct2,
				alice(),
				bob(),
				amount,
				cid,
				nullifier // Same nullifier
			),
			Error::<TestRuntime>::NullifierAlreadyUsed
		);
	});
}

#[test]
fn generate_benchmark_fixtures() {
	use crate::{
		circuit::{compute_commitment, poseidon_config},
		prover::{
			bytes32_to_field, field_to_bytes32, proof_to_bytes, TrustedSetup, TEST_SETUP_SEED,
		},
	};
	use ark_bn254::Fr;
	use parity_scale_codec::Encode;
	use sp_io::hashing::blake2_256;
	use sp_runtime::codec::DecodeAll;

	// Reproduce frame_benchmarking::account() logic
	fn bench_account(
		name: &str,
		index: u32,
		seed: u32,
	) -> <TestRuntime as frame_system::Config>::AccountId {
		let entropy = (name, index, seed).using_encoded(blake2_256);
		<TestRuntime as frame_system::Config>::AccountId::decode_all(&mut &entropy[..]).unwrap()
	}

	// Same accounts as the benchmark
	let _sender = bench_account("sender", 0, 0);
	let recipient = bench_account("recipient", 1, 1);

	// Same community as create_community() in benchmark
	use encointer_primitives::communities::{CommunityIdentifier, Degree, Location};
	let location = Location { lat: Degree::from_num(1i32), lon: Degree::from_num(1i32) };
	let bs: Vec<<TestRuntime as frame_system::Config>::AccountId> = vec![
		bench_account("alice", 1, 1),
		bench_account("bob", 2, 2),
		bench_account("charlie", 3, 3),
	];
	let cid = CommunityIdentifier::new(location, bs).unwrap();

	// Same proof parameters as the benchmark
	let poseidon = poseidon_config();
	let zk_secret = Fr::from(12345u64);
	let nonce = Fr::from(67890u64);
	let amount = BalanceType::from_num(10);

	let commitment_field = compute_commitment(&poseidon, &zk_secret);
	let commitment = field_to_bytes32(&commitment_field);
	let recipient_hash = bytes32_to_field(&crate::hash_recipient(&recipient.encode()));
	let cid_hash = bytes32_to_field(&crate::hash_cid(&cid));
	let amount_field = bytes32_to_field(&crate::balance_to_bytes(amount));

	// Generate trusted setup and proof with deterministic RNG
	let setup = TrustedSetup::generate_with_seed(TEST_SETUP_SEED);
	let vk_bytes = setup.verifying_key_bytes();

	use crate::circuit::OfflinePaymentCircuit;
	use ark_groth16::Groth16;
	use ark_snark::SNARK;
	use ark_std::rand::{rngs::StdRng, SeedableRng};

	let circuit = OfflinePaymentCircuit::new(
		poseidon,
		zk_secret,
		nonce,
		recipient_hash,
		amount_field,
		cid_hash,
	);
	let public_inputs = circuit.public_inputs();
	let mut rng = StdRng::seed_from_u64(42); // deterministic
	let proof = Groth16::<ark_bn254::Bn254>::prove(&setup.proving_key, circuit, &mut rng)
		.expect("proving failed");
	let proof_bytes = proof_to_bytes(&proof);
	let nullifier = field_to_bytes32(&public_inputs[4]);

	// Verify it works
	assert!(crate::prover::verify_proof(&setup.verifying_key, &proof, &public_inputs));

	fn bytes_to_rust_array(name: &str, bytes: &[u8]) {
		print!("const {}: [u8; {}] = [\n\t", name, bytes.len());
		for (i, b) in bytes.iter().enumerate() {
			if i > 0 && i % 15 == 0 {
				print!("\n\t");
			}
			if i + 1 < bytes.len() {
				print!("0x{b:02x}, ");
			} else {
				print!("0x{b:02x},");
			}
		}
		println!("\n];");
	}

	println!("// ===== BENCHMARK FIXTURE DATA =====");
	bytes_to_rust_array("VK_BYTES", &vk_bytes);
	bytes_to_rust_array("PROOF_BYTES", &proof_bytes);
	bytes_to_rust_array("COMMITMENT", &commitment);
	bytes_to_rust_array("NULLIFIER", &nullifier);
}

#[test]
fn e2e_invalid_proof_rejected() {
	use crate::prover::{TrustedSetup, TEST_SETUP_SEED};

	new_test_ext().execute_with(|| {
		System::set_block_number(1);

		// Setup verification key
		let setup = TrustedSetup::generate_with_seed(TEST_SETUP_SEED);
		let vk_bytes = setup.verifying_key_bytes();
		let bounded_vk: BoundedVec<u8, MaxVkSize> =
			BoundedVec::try_from(vk_bytes).expect("VK too large");
		assert_ok!(EncointerOfflinePayment::set_verification_key(
			RuntimeOrigin::root(),
			bounded_vk
		));

		// Setup community
		let cid = setup_community_with_balance(&alice(), BalanceType::from_num(100));

		// Register identity
		let commitment = [1u8; 32];
		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		// Try to submit an invalid proof (garbage bytes that won't deserialize)
		let fake_proof_bytes: BoundedVec<u8, MaxProofSize> =
			BoundedVec::try_from(vec![0u8; 128]).unwrap();
		let fake_proof = Groth16ProofBytes { proof_bytes: fake_proof_bytes };
		let nullifier = [2u8; 32];
		let amount = BalanceType::from_num(10);

		assert_noop!(
			EncointerOfflinePayment::submit_offline_payment(
				RuntimeOrigin::signed(charlie()),
				fake_proof,
				alice(),
				bob(),
				amount,
				cid,
				nullifier
			),
			Error::<TestRuntime>::ProofDeserializationFailed
		);
	});
}

// ============ Native Token Helper Tests ============

#[test]
fn native_token_cid_hash_is_deterministic() {
	let h1 = crate::native_token_cid_hash();
	let h2 = crate::native_token_cid_hash();
	assert_eq!(h1, h2);
	// Must differ from any real community hash
	assert_ne!(h1, [0u8; 32]);
}

#[test]
fn native_balance_to_bytes_works() {
	let amount: u128 = 1_000_000_000_000;
	let bytes = crate::native_balance_to_bytes(amount);
	let recovered = u128::from_le_bytes(bytes[..16].try_into().unwrap());
	assert_eq!(recovered, amount);
	assert_eq!(&bytes[16..], &[0u8; 16]);
}

// ============ Submit Native Offline Payment Error Tests ============

#[test]
fn submit_native_fails_zero_amount() {
	new_test_ext().execute_with(|| {
		let commitment = test_commitment();
		let nullifier = test_nullifier();

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		let proof =
			Groth16ProofBytes { proof_bytes: BoundedVec::try_from(vec![0u8; 128]).unwrap() };

		assert_noop!(
			EncointerOfflinePayment::submit_native_offline_payment(
				RuntimeOrigin::signed(charlie()),
				proof,
				alice(),
				bob(),
				0u128,
				nullifier
			),
			Error::<TestRuntime>::AmountMustBePositive
		);
	});
}

#[test]
fn submit_native_fails_self_send() {
	new_test_ext().execute_with(|| {
		let commitment = test_commitment();
		let nullifier = test_nullifier();

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		let proof =
			Groth16ProofBytes { proof_bytes: BoundedVec::try_from(vec![0u8; 128]).unwrap() };

		assert_noop!(
			EncointerOfflinePayment::submit_native_offline_payment(
				RuntimeOrigin::signed(charlie()),
				proof,
				alice(),
				alice(),
				100u128,
				nullifier
			),
			Error::<TestRuntime>::SenderEqualsRecipient
		);
	});
}

#[test]
fn submit_native_fails_no_identity() {
	new_test_ext().execute_with(|| {
		let nullifier = test_nullifier();

		let proof =
			Groth16ProofBytes { proof_bytes: BoundedVec::try_from(vec![0u8; 128]).unwrap() };

		assert_noop!(
			EncointerOfflinePayment::submit_native_offline_payment(
				RuntimeOrigin::signed(charlie()),
				proof,
				alice(),
				bob(),
				100u128,
				nullifier
			),
			Error::<TestRuntime>::NoOfflineIdentity
		);
	});
}

#[test]
fn submit_native_fails_used_nullifier() {
	new_test_ext().execute_with(|| {
		let commitment = test_commitment();
		let nullifier = test_nullifier();

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));
		fund_native(&alice(), 1000);

		UsedNullifiers::<TestRuntime>::insert(nullifier, ());

		let proof =
			Groth16ProofBytes { proof_bytes: BoundedVec::try_from(vec![0u8; 128]).unwrap() };

		assert_noop!(
			EncointerOfflinePayment::submit_native_offline_payment(
				RuntimeOrigin::signed(charlie()),
				proof,
				alice(),
				bob(),
				100u128,
				nullifier
			),
			Error::<TestRuntime>::NullifierAlreadyUsed
		);
	});
}

#[test]
fn submit_native_fails_no_vk() {
	new_test_ext().execute_with(|| {
		let commitment = test_commitment();
		let nullifier = test_nullifier();

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));
		fund_native(&alice(), 1000);

		let proof =
			Groth16ProofBytes { proof_bytes: BoundedVec::try_from(vec![0u8; 128]).unwrap() };

		assert_noop!(
			EncointerOfflinePayment::submit_native_offline_payment(
				RuntimeOrigin::signed(charlie()),
				proof,
				alice(),
				bob(),
				100u128,
				nullifier
			),
			Error::<TestRuntime>::NoVerificationKey
		);
	});
}

#[test]
fn submit_native_fails_insufficient_balance() {
	new_test_ext().execute_with(|| {
		let commitment = test_commitment();
		let nullifier = test_nullifier();

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));
		// Alice has 0 native balance

		let proof =
			Groth16ProofBytes { proof_bytes: BoundedVec::try_from(vec![0u8; 128]).unwrap() };

		assert_noop!(
			EncointerOfflinePayment::submit_native_offline_payment(
				RuntimeOrigin::signed(charlie()),
				proof,
				alice(),
				bob(),
				100u128,
				nullifier
			),
			Error::<TestRuntime>::InsufficientBalance
		);
	});
}

#[test]
fn submit_native_fails_invalid_proof() {
	use crate::prover::{TrustedSetup, TEST_SETUP_SEED};

	new_test_ext().execute_with(|| {
		let setup = TrustedSetup::generate_with_seed(TEST_SETUP_SEED);
		let bounded_vk: BoundedVec<u8, MaxVkSize> =
			BoundedVec::try_from(setup.verifying_key_bytes()).expect("VK too large");
		assert_ok!(EncointerOfflinePayment::set_verification_key(
			RuntimeOrigin::root(),
			bounded_vk
		));

		let commitment = test_commitment();
		let nullifier = test_nullifier();

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));
		fund_native(&alice(), 1000);

		let proof =
			Groth16ProofBytes { proof_bytes: BoundedVec::try_from(vec![0u8; 128]).unwrap() };

		assert_noop!(
			EncointerOfflinePayment::submit_native_offline_payment(
				RuntimeOrigin::signed(charlie()),
				proof,
				alice(),
				bob(),
				100u128,
				nullifier
			),
			Error::<TestRuntime>::ProofDeserializationFailed
		);
	});
}

// ============ Cross-domain Nullifier Test ============

#[test]
fn nullifier_shared_between_cc_and_native() {
	new_test_ext().execute_with(|| {
		let nullifier = test_nullifier();
		let commitment = test_commitment();

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		let cid = setup_community_with_balance(&alice(), BalanceType::from_num(100));
		fund_native(&alice(), 1000);

		// Mark nullifier as used (simulates a successful CC payment)
		UsedNullifiers::<TestRuntime>::insert(nullifier, ());

		// Native payment with same nullifier should fail
		let proof =
			Groth16ProofBytes { proof_bytes: BoundedVec::try_from(vec![0u8; 128]).unwrap() };
		assert_noop!(
			EncointerOfflinePayment::submit_native_offline_payment(
				RuntimeOrigin::signed(charlie()),
				proof,
				alice(),
				bob(),
				100u128,
				nullifier
			),
			Error::<TestRuntime>::NullifierAlreadyUsed
		);

		// Similarly, CC payment with a nullifier used by native should fail
		let nullifier2 = [3u8; 32];
		UsedNullifiers::<TestRuntime>::insert(nullifier2, ());

		let proof2 =
			Groth16ProofBytes { proof_bytes: BoundedVec::try_from(vec![0u8; 128]).unwrap() };
		assert_noop!(
			EncointerOfflinePayment::submit_offline_payment(
				RuntimeOrigin::signed(charlie()),
				proof2,
				alice(),
				bob(),
				BalanceType::from_num(10),
				cid,
				nullifier2
			),
			Error::<TestRuntime>::NullifierAlreadyUsed
		);
	});
}

// ============ Native E2E ZK Test ============

#[test]
fn e2e_native_zk_payment_works() {
	use crate::{
		circuit::{compute_commitment, poseidon_config},
		prover::{
			bytes32_to_field, field_to_bytes32, generate_proof, proof_to_bytes, TrustedSetup,
			TEST_SETUP_SEED,
		},
	};
	use ark_bn254::Fr;
	use parity_scale_codec::Encode;

	new_test_ext().execute_with(|| {
		System::set_block_number(1);

		let setup = TrustedSetup::generate_with_seed(TEST_SETUP_SEED);
		let bounded_vk: BoundedVec<u8, MaxVkSize> =
			BoundedVec::try_from(setup.verifying_key_bytes()).expect("VK too large");
		assert_ok!(EncointerOfflinePayment::set_verification_key(
			RuntimeOrigin::root(),
			bounded_vk
		));

		fund_native(&alice(), 1000);

		let poseidon = poseidon_config();
		let zk_secret = Fr::from(12345u64);
		let commitment_field = compute_commitment(&poseidon, &zk_secret);
		let commitment = field_to_bytes32(&commitment_field);

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		let nonce = Fr::from(99999u64); // different nonce from CC test → different nullifier
		let amount: u128 = 100;

		let recipient_hash_bytes = crate::hash_recipient(&bob().encode());
		let cid_hash_bytes = crate::native_token_cid_hash();
		let amount_bytes = crate::native_balance_to_bytes(amount);

		let recipient_hash = bytes32_to_field(&recipient_hash_bytes);
		let cid_hash = bytes32_to_field(&cid_hash_bytes);
		let amount_field = bytes32_to_field(&amount_bytes);

		let (proof, public_inputs) = generate_proof(
			&setup.proving_key,
			zk_secret,
			nonce,
			recipient_hash,
			amount_field,
			cid_hash,
		)
		.expect("Proof generation failed");

		let proof_bytes = proof_to_bytes(&proof);
		let nullifier = field_to_bytes32(&public_inputs[4]);

		let bounded_proof: BoundedVec<u8, MaxProofSize> =
			BoundedVec::try_from(proof_bytes).expect("Proof too large");
		let proof_struct = Groth16ProofBytes { proof_bytes: bounded_proof };

		assert_ok!(EncointerOfflinePayment::submit_native_offline_payment(
			RuntimeOrigin::signed(charlie()),
			proof_struct,
			alice(),
			bob(),
			amount,
			nullifier
		));

		assert_eq!(<Balances as Currency<_>>::free_balance(&alice()), 900);
		assert_eq!(<Balances as Currency<_>>::free_balance(&bob()), 100);

		assert!(UsedNullifiers::<TestRuntime>::contains_key(nullifier));

		// Double-spend should fail
		let proof2 =
			Groth16ProofBytes { proof_bytes: BoundedVec::try_from(vec![0u8; 128]).unwrap() };
		assert_noop!(
			EncointerOfflinePayment::submit_native_offline_payment(
				RuntimeOrigin::signed(charlie()),
				proof2,
				alice(),
				bob(),
				amount,
				nullifier
			),
			Error::<TestRuntime>::NullifierAlreadyUsed
		);
	});
}

// ============ Native Benchmark Fixture Generation ============

#[test]
fn generate_native_benchmark_fixtures() {
	use crate::{
		circuit::{compute_commitment, poseidon_config, OfflinePaymentCircuit},
		prover::{
			bytes32_to_field, field_to_bytes32, proof_to_bytes, TrustedSetup, TEST_SETUP_SEED,
		},
	};
	use ark_bn254::Fr;
	use ark_groth16::Groth16;
	use ark_snark::SNARK;
	use ark_std::rand::{rngs::StdRng, SeedableRng};
	use parity_scale_codec::Encode;
	use sp_io::hashing::blake2_256;
	use sp_runtime::codec::DecodeAll;

	fn bench_account(
		name: &str,
		index: u32,
		seed: u32,
	) -> <TestRuntime as frame_system::Config>::AccountId {
		let entropy = (name, index, seed).using_encoded(blake2_256);
		<TestRuntime as frame_system::Config>::AccountId::decode_all(&mut &entropy[..]).unwrap()
	}

	let recipient = bench_account("recipient", 1, 1);

	let poseidon = poseidon_config();
	let zk_secret = Fr::from(12345u64);
	// Use a different nonce from CC fixtures to get a distinct nullifier
	let nonce = Fr::from(11111u64);
	let amount: u128 = 10;

	let commitment_field = compute_commitment(&poseidon, &zk_secret);
	let commitment = field_to_bytes32(&commitment_field);
	let recipient_hash = bytes32_to_field(&crate::hash_recipient(&recipient.encode()));
	let cid_hash = bytes32_to_field(&crate::native_token_cid_hash());
	let amount_field = bytes32_to_field(&crate::native_balance_to_bytes(amount));

	let setup = TrustedSetup::generate_with_seed(TEST_SETUP_SEED);

	let circuit = OfflinePaymentCircuit::new(
		poseidon,
		zk_secret,
		nonce,
		recipient_hash,
		amount_field,
		cid_hash,
	);
	let public_inputs = circuit.public_inputs();
	let mut rng = StdRng::seed_from_u64(42);
	let proof = Groth16::<ark_bn254::Bn254>::prove(&setup.proving_key, circuit, &mut rng)
		.expect("proving failed");
	let proof_bytes = proof_to_bytes(&proof);
	let nullifier = field_to_bytes32(&public_inputs[4]);

	// Verify commitment matches the CC fixtures (same zk_secret)
	assert_eq!(
		commitment,
		[
			0xba, 0xa7, 0xb4, 0x72, 0x5c, 0xa5, 0xb6, 0x73, 0x73, 0xab, 0xb8, 0x4f, 0xac, 0x19,
			0xdf, 0x8b, 0xcc, 0x17, 0xff, 0xcf, 0x5d, 0xa6, 0x5e, 0x62, 0xe2, 0x8b, 0x86, 0x2d,
			0x54, 0xf0, 0x08, 0x25,
		]
	);

	assert!(crate::prover::verify_proof(&setup.verifying_key, &proof, &public_inputs));

	fn bytes_to_rust_array(name: &str, bytes: &[u8]) {
		print!("const {}: [u8; {}] = [\n\t", name, bytes.len());
		for (i, b) in bytes.iter().enumerate() {
			if i > 0 && i % 15 == 0 {
				print!("\n\t");
			}
			if i + 1 < bytes.len() {
				print!("0x{b:02x}, ");
			} else {
				print!("0x{b:02x},");
			}
		}
		println!("\n];");
	}

	println!("// ===== NATIVE BENCHMARK FIXTURE DATA =====");
	bytes_to_rust_array("NATIVE_PROOF_BYTES", &proof_bytes);
	bytes_to_rust_array("NATIVE_NULLIFIER", &nullifier);
}
