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
	circuit::{compute_commitment, poseidon_config},
	prover::{
		bytes32_to_field, field_to_bytes32, generate_proof, proof_to_bytes, TrustedSetup,
		TEST_SETUP_SEED,
	},
	*,
};
use ark_bn254::Fr;
use encointer_primitives::{
	balances::BalanceType,
	communities::{CommunityIdentifier, CommunityMetadata, Degree, Location},
};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use parity_scale_codec::Encode;

fn create_community<T: Config>() -> CommunityIdentifier {
	let alice: T::AccountId = account("alice", 1, 1);
	let bob: T::AccountId = account("bob", 2, 2);
	let charlie: T::AccountId = account("charlie", 3, 3);

	let location = Location { lat: Degree::from_num(1i32), lon: Degree::from_num(1i32) };
	let bs = vec![alice, bob, charlie];

	pallet_encointer_communities::Pallet::<T>::set_min_solar_trip_time_s(RawOrigin::Root.into(), 1)
		.ok();
	pallet_encointer_communities::Pallet::<T>::set_max_speed_mps(RawOrigin::Root.into(), 83).ok();

	pallet_encointer_communities::Pallet::<T>::new_community(
		RawOrigin::Root.into(),
		location,
		bs.clone(),
		CommunityMetadata::default(),
		None,
		None,
	)
	.ok();

	CommunityIdentifier::new(location, bs).unwrap()
}

benchmarks! {
	register_offline_identity {
		let caller: T::AccountId = account("caller", 0, 0);
		let commitment = [1u8; 32];
	}: _(RawOrigin::Signed(caller.clone()), commitment)
	verify {
		assert_eq!(OfflineIdentities::<T>::get(&caller), Some(commitment));
	}

	submit_offline_payment {
		// Generate trusted setup (deterministic, not measured)
		let setup = TrustedSetup::generate_with_seed(TEST_SETUP_SEED);
		let vk_bytes = setup.verifying_key_bytes();

		let sender: T::AccountId = account("sender", 0, 0);
		let recipient: T::AccountId = account("recipient", 1, 1);
		let submitter: T::AccountId = account("submitter", 2, 2);

		// Create community and fund sender
		let cid = create_community::<T>();
		pallet_encointer_balances::Pallet::<T>::issue(
			cid, &sender, BalanceType::from_num(100),
		).unwrap();

		// Derive ZK commitment via Poseidon
		let poseidon = poseidon_config();
		let zk_secret = Fr::from(12345u64);
		let nonce = Fr::from(67890u64);
		let commitment_field = compute_commitment(&poseidon, &zk_secret);
		let commitment = field_to_bytes32(&commitment_field);

		// Compute public input hashes (must match what the pallet recomputes)
		let amount = BalanceType::from_num(10);
		let recipient_hash = bytes32_to_field(&hash_recipient(&recipient.encode()));
		let cid_hash = bytes32_to_field(&hash_cid(&cid));
		let amount_field = bytes32_to_field(&balance_to_bytes(amount));

		// Generate real Groth16 proof
		let (proof, public_inputs) = generate_proof(
			&setup.proving_key,
			zk_secret,
			nonce,
			recipient_hash,
			amount_field,
			cid_hash,
		).expect("proof generation must succeed");

		let nullifier = field_to_bytes32(&public_inputs[4]);

		// Populate storage (not measured)
		let bounded_vk: BoundedVec<u8, T::MaxVkSize> =
			BoundedVec::try_from(vk_bytes).expect("VK within bounds");
		VerificationKey::<T>::put(bounded_vk);
		OfflineIdentities::<T>::insert(&sender, commitment);

		let bounded_proof: BoundedVec<u8, T::MaxProofSize> =
			BoundedVec::try_from(proof_to_bytes(&proof)).expect("proof within bounds");
		let proof_struct = Groth16ProofBytes { proof_bytes: bounded_proof };
	}: submit_offline_payment(
		RawOrigin::Signed(submitter),
		proof_struct,
		sender.clone(),
		recipient.clone(),
		amount,
		cid,
		nullifier
	)
	verify {
		assert!(UsedNullifiers::<T>::contains_key(nullifier));
		assert!(pallet_encointer_balances::Pallet::<T>::balance(cid, &sender) < BalanceType::from_num(100));
	}

	set_verification_key {
		let setup = TrustedSetup::generate_with_seed(TEST_SETUP_SEED);
		let vk_bytes = setup.verifying_key_bytes();
		let bounded_vk: BoundedVec<u8, T::MaxVkSize> =
			BoundedVec::try_from(vk_bytes).expect("VK within bounds");
	}: _(RawOrigin::Root, bounded_vk)
	verify {
		assert!(VerificationKey::<T>::get().is_some());
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::TestRuntime);
