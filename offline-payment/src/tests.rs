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

use crate::{
	compute_commitment, compute_nullifier, mock::*, Error, Event, OfflineIdentities,
	OfflinePaymentProof, UsedNullifiers,
};
use encointer_primitives::{balances::BalanceType, communities::CommunityIdentifier};
use frame_support::{assert_noop, assert_ok};
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

fn test_zk_secret() -> [u8; 32] {
	[1u8; 32]
}

fn test_nonce() -> [u8; 32] {
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

#[test]
fn register_offline_identity_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		let zk_secret = test_zk_secret();
		let commitment = compute_commitment(&zk_secret);

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
		let zk_secret = test_zk_secret();
		let commitment = compute_commitment(&zk_secret);

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
fn submit_offline_payment_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		let zk_secret = test_zk_secret();
		let nonce = test_nonce();
		let commitment = compute_commitment(&zk_secret);
		let nullifier = compute_nullifier(&zk_secret, &nonce);
		let amount = BalanceType::from_num(10);

		// Setup: register identity and fund account
		let cid = setup_community_with_balance(&alice(), BalanceType::from_num(100));

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		// Create proof
		let proof = OfflinePaymentProof::new(zk_secret, nonce);

		// Submit payment (Charlie submits on behalf of Alice -> Bob)
		assert_ok!(EncointerOfflinePayment::submit_offline_payment(
			RuntimeOrigin::signed(charlie()),
			proof,
			alice(),
			bob(),
			amount,
			cid,
			nullifier
		));

		// Verify nullifier is marked as used
		assert!(UsedNullifiers::<TestRuntime>::contains_key(&nullifier));

		// Verify balance changed
		assert_eq!(
			pallet_encointer_balances::Pallet::<TestRuntime>::balance(cid, &alice()),
			BalanceType::from_num(90)
		);
		assert_eq!(
			pallet_encointer_balances::Pallet::<TestRuntime>::balance(cid, &bob()),
			amount
		);

		// Check event
		System::assert_last_event(
			Event::<TestRuntime>::OfflinePaymentSettled {
				sender: alice(),
				recipient: bob(),
				cid,
				amount,
				nullifier,
			}
			.into(),
		);
	});
}

#[test]
fn submit_offline_payment_fails_with_duplicate_nullifier() {
	new_test_ext().execute_with(|| {
		let zk_secret = test_zk_secret();
		let nonce = test_nonce();
		let commitment = compute_commitment(&zk_secret);
		let nullifier = compute_nullifier(&zk_secret, &nonce);
		let amount = BalanceType::from_num(10);

		let cid = setup_community_with_balance(&alice(), BalanceType::from_num(100));

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		let proof = OfflinePaymentProof::new(zk_secret, nonce);

		// First submission succeeds
		assert_ok!(EncointerOfflinePayment::submit_offline_payment(
			RuntimeOrigin::signed(charlie()),
			proof.clone(),
			alice(),
			bob(),
			amount,
			cid,
			nullifier
		));

		// Second submission with same nullifier fails
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
			Error::<TestRuntime>::NullifierAlreadyUsed
		);
	});
}

#[test]
fn submit_offline_payment_fails_with_invalid_proof() {
	new_test_ext().execute_with(|| {
		let zk_secret = test_zk_secret();
		let wrong_secret = [99u8; 32];
		let nonce = test_nonce();
		let commitment = compute_commitment(&zk_secret);
		let nullifier = compute_nullifier(&wrong_secret, &nonce); // Using wrong secret
		let amount = BalanceType::from_num(10);

		let cid = setup_community_with_balance(&alice(), BalanceType::from_num(100));

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		// Proof with wrong secret
		let proof = OfflinePaymentProof::new(wrong_secret, nonce);

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
			Error::<TestRuntime>::InvalidProof
		);
	});
}

#[test]
fn submit_offline_payment_fails_with_nullifier_mismatch() {
	new_test_ext().execute_with(|| {
		let zk_secret = test_zk_secret();
		let nonce = test_nonce();
		let commitment = compute_commitment(&zk_secret);
		let wrong_nullifier = [99u8; 32]; // Doesn't match proof
		let amount = BalanceType::from_num(10);

		let cid = setup_community_with_balance(&alice(), BalanceType::from_num(100));

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		let proof = OfflinePaymentProof::new(zk_secret, nonce);

		assert_noop!(
			EncointerOfflinePayment::submit_offline_payment(
				RuntimeOrigin::signed(charlie()),
				proof,
				alice(),
				bob(),
				amount,
				cid,
				wrong_nullifier
			),
			Error::<TestRuntime>::NullifierMismatch
		);
	});
}

#[test]
fn submit_offline_payment_fails_with_unregistered_sender() {
	new_test_ext().execute_with(|| {
		let zk_secret = test_zk_secret();
		let nonce = test_nonce();
		let nullifier = compute_nullifier(&zk_secret, &nonce);
		let amount = BalanceType::from_num(10);

		let cid = setup_community_with_balance(&alice(), BalanceType::from_num(100));

		// Alice has NOT registered offline identity
		let proof = OfflinePaymentProof::new(zk_secret, nonce);

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
		let zk_secret = test_zk_secret();
		let nonce = test_nonce();
		let commitment = compute_commitment(&zk_secret);
		let nullifier = compute_nullifier(&zk_secret, &nonce);

		let cid = setup_community_with_balance(&alice(), BalanceType::from_num(100));

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		let proof = OfflinePaymentProof::new(zk_secret, nonce);

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
		let zk_secret = test_zk_secret();
		let nonce = test_nonce();
		let commitment = compute_commitment(&zk_secret);
		let nullifier = compute_nullifier(&zk_secret, &nonce);
		let amount = BalanceType::from_num(10);

		let cid = setup_community_with_balance(&alice(), BalanceType::from_num(100));

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		let proof = OfflinePaymentProof::new(zk_secret, nonce);

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
fn multiple_payments_with_different_nullifiers_work() {
	new_test_ext().execute_with(|| {
		let zk_secret = test_zk_secret();
		let commitment = compute_commitment(&zk_secret);
		let amount = BalanceType::from_num(10);

		let cid = setup_community_with_balance(&alice(), BalanceType::from_num(100));

		assert_ok!(EncointerOfflinePayment::register_offline_identity(
			RuntimeOrigin::signed(alice()),
			commitment
		));

		// First payment
		let nonce1 = [1u8; 32];
		let nullifier1 = compute_nullifier(&zk_secret, &nonce1);
		let proof1 = OfflinePaymentProof::new(zk_secret, nonce1);

		assert_ok!(EncointerOfflinePayment::submit_offline_payment(
			RuntimeOrigin::signed(charlie()),
			proof1,
			alice(),
			bob(),
			amount,
			cid,
			nullifier1
		));

		// Second payment with different nonce
		let nonce2 = [2u8; 32];
		let nullifier2 = compute_nullifier(&zk_secret, &nonce2);
		let proof2 = OfflinePaymentProof::new(zk_secret, nonce2);

		assert_ok!(EncointerOfflinePayment::submit_offline_payment(
			RuntimeOrigin::signed(charlie()),
			proof2,
			alice(),
			bob(),
			amount,
			cid,
			nullifier2
		));

		// Verify balances
		assert_eq!(
			pallet_encointer_balances::Pallet::<TestRuntime>::balance(cid, &alice()),
			BalanceType::from_num(80)
		);
		assert_eq!(
			pallet_encointer_balances::Pallet::<TestRuntime>::balance(cid, &bob()),
			BalanceType::from_num(20)
		);
	});
}

#[test]
fn proof_computation_is_deterministic() {
	let zk_secret = test_zk_secret();
	let nonce = test_nonce();

	let commitment1 = compute_commitment(&zk_secret);
	let commitment2 = compute_commitment(&zk_secret);
	assert_eq!(commitment1, commitment2);

	let nullifier1 = compute_nullifier(&zk_secret, &nonce);
	let nullifier2 = compute_nullifier(&zk_secret, &nonce);
	assert_eq!(nullifier1, nullifier2);

	let proof = OfflinePaymentProof::new(zk_secret, nonce);
	assert_eq!(proof.compute_commitment(), commitment1);
	assert_eq!(proof.compute_nullifier(), nullifier1);
}
