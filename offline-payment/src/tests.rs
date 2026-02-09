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
use frame_support::{assert_noop, assert_ok, BoundedVec};
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

		assert_eq!(OfflineIdentities::<TestRuntime>::get(&alice()), Some(commitment));

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

		assert_eq!(OfflineIdentities::<TestRuntime>::get(&alice()), Some(commitment1));
		assert_eq!(OfflineIdentities::<TestRuntime>::get(&bob()), Some(commitment2));
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
		assert!(!UsedNullifiers::<TestRuntime>::contains_key(&nullifier));

		// Mark as used
		UsedNullifiers::<TestRuntime>::insert(&nullifier, ());

		// Now it's used
		assert!(UsedNullifiers::<TestRuntime>::contains_key(&nullifier));
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
	use crate::circuit::{compute_commitment, compute_nullifier, poseidon_config};
	use crate::prover::{
		bytes32_to_field, field_to_bytes32, generate_proof, proof_to_bytes, TrustedSetup,
		TEST_SETUP_SEED,
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
		assert_ok!(EncointerOfflinePayment::set_verification_key(RuntimeOrigin::root(), bounded_vk));

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
		assert!(UsedNullifiers::<TestRuntime>::contains_key(&nullifier));

		// Step 10: Try to double-spend - should fail
		let bounded_proof2: BoundedVec<u8, MaxProofSize> = BoundedVec::try_from(vec![0u8; 128]).unwrap();
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
fn e2e_invalid_proof_rejected() {
	use crate::prover::{TrustedSetup, TEST_SETUP_SEED};

	new_test_ext().execute_with(|| {
		System::set_block_number(1);

		// Setup verification key
		let setup = TrustedSetup::generate_with_seed(TEST_SETUP_SEED);
		let vk_bytes = setup.verifying_key_bytes();
		let bounded_vk: BoundedVec<u8, MaxVkSize> =
			BoundedVec::try_from(vk_bytes).expect("VK too large");
		assert_ok!(EncointerOfflinePayment::set_verification_key(RuntimeOrigin::root(), bounded_vk));

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
