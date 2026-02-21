use crate::{mock::*, BandersnatchPublicKey, Error, RingComputationPhase, MAX_REPUTATION_LEVELS};
use encointer_primitives::{ceremonies::Reputation, scheduler::CeremonyPhaseType};
use frame_support::{assert_noop, assert_ok};
use test_utils::helpers::{account_id, add_population, bootstrappers, register_test_community};

/// Advance ceremony phase from Registering to Assigning (required for `initiate_rings`).
fn advance_to_assigning() {
	assert_eq!(EncointerScheduler::current_phase(), CeremonyPhaseType::Registering);
	assert_ok!(EncointerScheduler::next_phase(RuntimeOrigin::root()));
	assert_eq!(EncointerScheduler::current_phase(), CeremonyPhaseType::Assigning);
}

/// Run ring computation to completion. Returns the number of steps taken.
fn run_computation_to_completion(caller: &test_utils::AccountId) -> u32 {
	let mut steps = 0u32;
	while EncointerReputationRings::continue_ring_computation(RuntimeOrigin::signed(caller.clone()))
		.is_ok()
	{
		steps += 1;
		assert!(steps < 100, "Ring computation didn't complete in 100 steps");
	}
	steps
}

fn fake_bandersnatch_key(seed: u8) -> BandersnatchPublicKey {
	let mut key = [0u8; 32];
	key[0] = seed;
	key
}

fn fake_bandersnatch_key_u32(seed: u32) -> BandersnatchPublicKey {
	let mut key = [0u8; 32];
	key[..4].copy_from_slice(&seed.to_le_bytes());
	key
}

/// Register bandersnatch keys for a set of accounts and fake their reputation
/// in the specified community for the given ceremony indices.
///
/// Returns the account IDs.
fn setup_community_with_reputations(
	cid: encointer_primitives::communities::CommunityIdentifier,
	accounts: &[sp_core::sr25519::Pair],
	// (account_index, ceremony_index) pairs that should have verified reputation
	verified_ceremonies: &[(usize, u32)],
) {
	for (i, pair) in accounts.iter().enumerate() {
		let acc = account_id(pair);
		assert_ok!(EncointerReputationRings::register_bandersnatch_key(
			RuntimeOrigin::signed(acc.clone()),
			fake_bandersnatch_key(i as u8 + 1),
		));
	}
	for &(account_idx, cindex) in verified_ceremonies {
		let acc = account_id(&accounts[account_idx]);
		pallet_encointer_ceremonies::Pallet::<TestRuntime>::fake_reputation(
			(cid, cindex),
			&acc,
			Reputation::VerifiedLinked(cindex),
		);
	}
}

// -- Key registration tests --

#[test]
fn register_bandersnatch_key_works() {
	new_test_ext().execute_with(|| {
		let alice = account_id(&bootstrappers()[0]);
		let key = fake_bandersnatch_key(42);

		assert_ok!(EncointerReputationRings::register_bandersnatch_key(
			RuntimeOrigin::signed(alice.clone()),
			key,
		));

		assert_eq!(EncointerReputationRings::bandersnatch_key(&alice), Some(key));
	});
}

#[test]
fn register_bandersnatch_key_update_works() {
	new_test_ext().execute_with(|| {
		let alice = account_id(&bootstrappers()[0]);
		let key1 = fake_bandersnatch_key(1);
		let key2 = fake_bandersnatch_key(2);

		assert_ok!(EncointerReputationRings::register_bandersnatch_key(
			RuntimeOrigin::signed(alice.clone()),
			key1,
		));
		assert_ok!(EncointerReputationRings::register_bandersnatch_key(
			RuntimeOrigin::signed(alice.clone()),
			key2,
		));

		assert_eq!(EncointerReputationRings::bandersnatch_key(&alice), Some(key2));
	});
}

// -- Ring initiation tests --

#[test]
fn initiate_rings_works() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);
		let alice = account_id(&bootstrappers()[0]);

		// Current ceremony index is 7 (from genesis), so ceremony 6 is valid.
		advance_to_assigning();
		assert_ok!(EncointerReputationRings::initiate_rings(RuntimeOrigin::signed(alice), cid, 6,));

		let state = EncointerReputationRings::pending_ring_computation().unwrap();
		assert_eq!(state.community, cid);
		assert_eq!(state.ceremony_index, 6);
		assert_eq!(
			state.phase,
			RingComputationPhase::CollectingMembers { next_ceremony_offset: 0 }
		);
	});
}

#[test]
fn initiate_rings_fails_if_computation_in_progress() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);
		let alice = account_id(&bootstrappers()[0]);

		advance_to_assigning();
		assert_ok!(EncointerReputationRings::initiate_rings(
			RuntimeOrigin::signed(alice.clone()),
			cid,
			6,
		));
		assert_noop!(
			EncointerReputationRings::initiate_rings(RuntimeOrigin::signed(alice), cid, 5,),
			Error::<TestRuntime>::ComputationAlreadyInProgress
		);
	});
}

#[test]
fn initiate_rings_fails_for_unknown_community() {
	new_test_ext().execute_with(|| {
		let alice = account_id(&bootstrappers()[0]);
		let fake_cid = encointer_primitives::communities::CommunityIdentifier::default();

		advance_to_assigning();
		assert_noop!(
			EncointerReputationRings::initiate_rings(RuntimeOrigin::signed(alice), fake_cid, 6,),
			Error::<TestRuntime>::CommunityNotFound
		);
	});
}

#[test]
fn initiate_rings_fails_for_invalid_ceremony_index() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);
		let alice = account_id(&bootstrappers()[0]);

		advance_to_assigning();
		// Current index is 7. Index 7 is current (not yet complete).
		assert_noop!(
			EncointerReputationRings::initiate_rings(RuntimeOrigin::signed(alice.clone()), cid, 7,),
			Error::<TestRuntime>::InvalidCeremonyIndex
		);

		// Index 0 is invalid.
		assert_noop!(
			EncointerReputationRings::initiate_rings(RuntimeOrigin::signed(alice), cid, 0,),
			Error::<TestRuntime>::InvalidCeremonyIndex
		);
	});
}

#[test]
fn initiate_rings_fails_during_wrong_phase() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);
		let alice = account_id(&bootstrappers()[0]);

		// Genesis starts in Registering.
		assert_eq!(EncointerScheduler::current_phase(), CeremonyPhaseType::Registering);
		assert_noop!(
			EncointerReputationRings::initiate_rings(RuntimeOrigin::signed(alice.clone()), cid, 6,),
			Error::<TestRuntime>::WrongPhase
		);

		// Advance Registering → Assigning → Attesting.
		assert_ok!(EncointerScheduler::next_phase(RuntimeOrigin::root()));
		assert_ok!(EncointerScheduler::next_phase(RuntimeOrigin::root()));
		assert_eq!(EncointerScheduler::current_phase(), CeremonyPhaseType::Attesting);

		assert_noop!(
			EncointerReputationRings::initiate_rings(RuntimeOrigin::signed(alice), cid, 6,),
			Error::<TestRuntime>::WrongPhase
		);
	});
}

// -- Multi-block computation tests --

#[test]
fn full_ring_computation_produces_all_5_rings() {
	new_test_ext().execute_with(|| {
		let bs = bootstrappers();
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);

		// Setup: 3 accounts with different attendance patterns over ceremonies 2-6.
		// Alice: attended ceremonies 2,3,4,5,6 → 5/5
		// Bob: attended ceremonies 3,5,6 → 3/5
		// Charlie: attended ceremony 6 → 1/5
		let alice_idx = 0usize;
		let bob_idx = 1usize;
		let charlie_idx = 2usize;

		let verified: Vec<(usize, u32)> = vec![
			(alice_idx, 2),
			(alice_idx, 3),
			(alice_idx, 4),
			(alice_idx, 5),
			(alice_idx, 6),
			(bob_idx, 3),
			(bob_idx, 5),
			(bob_idx, 6),
			(charlie_idx, 6),
		];
		setup_community_with_reputations(cid, &bs[..3], &verified);

		// Initiate ring computation for ceremony 6.
		advance_to_assigning();
		let caller = account_id(&bs[0]);
		assert_ok!(EncointerReputationRings::initiate_rings(
			RuntimeOrigin::signed(caller.clone()),
			cid,
			6,
		));

		// Run member collection: 5 scan steps + 1 transition step = 6.
		for _ in 0..(MAX_REPUTATION_LEVELS as u32 + 1) {
			assert_ok!(EncointerReputationRings::continue_ring_computation(RuntimeOrigin::signed(
				caller.clone()
			),));
		}

		// Check state: should now be in BuildingRing phase.
		let state = EncointerReputationRings::pending_ring_computation().unwrap();
		assert_eq!(
			state.phase,
			RingComputationPhase::BuildingRing { current_level: MAX_REPUTATION_LEVELS }
		);

		// Verify attendance counts.
		let alice_acc = account_id(&bs[alice_idx]);
		let bob_acc = account_id(&bs[bob_idx]);
		let charlie_acc = account_id(&bs[charlie_idx]);
		let alice_att = state.attendance.iter().find(|(a, _)| *a == alice_acc).unwrap().1;
		let bob_att = state.attendance.iter().find(|(a, _)| *a == bob_acc).unwrap().1;
		let charlie_att = state.attendance.iter().find(|(a, _)| *a == charlie_acc).unwrap().1;
		assert_eq!(alice_att, 5);
		assert_eq!(bob_att, 3);
		assert_eq!(charlie_att, 1);

		// Run ring building: 5 steps (one per level, 5/5 down to 1/5).
		for _ in 0..MAX_REPUTATION_LEVELS {
			assert_ok!(EncointerReputationRings::continue_ring_computation(RuntimeOrigin::signed(
				caller.clone()
			),));
		}

		// Computation should be done (storage cleared).
		assert!(EncointerReputationRings::pending_ring_computation().is_none());

		// Verify rings exist for all 5 levels (sub_ring_index=0 for small communities).
		let alice_key = fake_bandersnatch_key(1);
		let bob_key = fake_bandersnatch_key(2);
		let charlie_key = fake_bandersnatch_key(3);

		// 5/5 ring: only Alice (attended all 5).
		let ring5 = EncointerReputationRings::ring_members((cid, 6, 5, 0)).unwrap();
		assert_eq!(ring5.len(), 1);
		assert!(ring5.contains(&alice_key));

		// 4/5 ring: only Alice (Bob has 3, Charlie has 1).
		let ring4 = EncointerReputationRings::ring_members((cid, 6, 4, 0)).unwrap();
		assert_eq!(ring4.len(), 1);
		assert!(ring4.contains(&alice_key));

		// 3/5 ring: Alice + Bob.
		let ring3 = EncointerReputationRings::ring_members((cid, 6, 3, 0)).unwrap();
		assert_eq!(ring3.len(), 2);
		assert!(ring3.contains(&alice_key));
		assert!(ring3.contains(&bob_key));

		// 2/5 ring: Alice + Bob (Charlie has only 1).
		let ring2 = EncointerReputationRings::ring_members((cid, 6, 2, 0)).unwrap();
		assert_eq!(ring2.len(), 2);
		assert!(ring2.contains(&alice_key));
		assert!(ring2.contains(&bob_key));

		// 1/5 ring: Alice + Bob + Charlie.
		let ring1 = EncointerReputationRings::ring_members((cid, 6, 1, 0)).unwrap();
		assert_eq!(ring1.len(), 3);
		assert!(ring1.contains(&alice_key));
		assert!(ring1.contains(&bob_key));
		assert!(ring1.contains(&charlie_key));

		// SubRingCount should be 1 for all levels.
		for level in 1..=5u8 {
			assert_eq!(EncointerReputationRings::sub_ring_count((cid, 6, level)), 1);
		}
	});
}

#[test]
fn ring_nesting_is_strict_subset() {
	new_test_ext().execute_with(|| {
		let bs = bootstrappers();
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);

		// 4 accounts with attendance 5,4,2,1 respectively.
		let verified: Vec<(usize, u32)> = vec![
			// Account 0: 5 ceremonies
			(0, 2),
			(0, 3),
			(0, 4),
			(0, 5),
			(0, 6),
			// Account 1: 4 ceremonies
			(1, 3),
			(1, 4),
			(1, 5),
			(1, 6),
			// Account 2: 2 ceremonies
			(2, 5),
			(2, 6),
			// Account 3: 1 ceremony
			(3, 6),
		];
		setup_community_with_reputations(cid, &bs[..4], &verified);

		advance_to_assigning();
		let caller = account_id(&bs[0]);
		assert_ok!(EncointerReputationRings::initiate_rings(
			RuntimeOrigin::signed(caller.clone()),
			cid,
			6,
		));

		// Run all steps to completion.
		run_computation_to_completion(&caller);

		// Verify strict nesting: ring[n+1] ⊂ ring[n] (single sub-ring).
		for level in 1..MAX_REPUTATION_LEVELS {
			let ring_lower = EncointerReputationRings::ring_members((cid, 6, level, 0)).unwrap();
			let ring_higher =
				EncointerReputationRings::ring_members((cid, 6, level + 1, 0)).unwrap();
			// Every member of the stricter ring must be in the looser ring.
			for member in ring_higher.iter() {
				assert!(
					ring_lower.contains(member),
					"Member in {}/{} ring not found in {}/{} ring",
					level + 1,
					MAX_REPUTATION_LEVELS,
					level,
					MAX_REPUTATION_LEVELS,
				);
			}
			// The stricter ring must be <= the looser ring.
			assert!(ring_higher.len() <= ring_lower.len());
		}
	});
}

#[test]
fn accounts_without_bandersnatch_key_are_excluded() {
	new_test_ext().execute_with(|| {
		let bs = bootstrappers();
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);

		// Alice has a key, Bob does not.
		let alice = account_id(&bs[0]);
		let bob = account_id(&bs[1]);

		assert_ok!(EncointerReputationRings::register_bandersnatch_key(
			RuntimeOrigin::signed(alice.clone()),
			fake_bandersnatch_key(1),
		));
		// Bob: no key registration.

		// Both have verified reputation.
		pallet_encointer_ceremonies::Pallet::<TestRuntime>::fake_reputation(
			(cid, 6),
			&alice,
			Reputation::VerifiedLinked(6),
		);
		pallet_encointer_ceremonies::Pallet::<TestRuntime>::fake_reputation(
			(cid, 6),
			&bob,
			Reputation::VerifiedLinked(6),
		);

		advance_to_assigning();
		let caller = alice.clone();
		assert_ok!(EncointerReputationRings::initiate_rings(
			RuntimeOrigin::signed(caller.clone()),
			cid,
			6,
		));

		// Run all steps to completion.
		run_computation_to_completion(&caller);

		// Only Alice should be in the ring.
		let ring1 = EncointerReputationRings::ring_members((cid, 6, 1, 0)).unwrap();
		assert_eq!(ring1.len(), 1);
		assert!(ring1.contains(&fake_bandersnatch_key(1)));
	});
}

#[test]
fn ring_members_are_deterministically_sorted() {
	new_test_ext().execute_with(|| {
		let bs = bootstrappers();
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);

		// Register 4 accounts with keys.
		for (i, pair) in bs.iter().enumerate().take(4) {
			let acc = account_id(pair);
			assert_ok!(EncointerReputationRings::register_bandersnatch_key(
				RuntimeOrigin::signed(acc.clone()),
				fake_bandersnatch_key(i as u8 + 1),
			));
			pallet_encointer_ceremonies::Pallet::<TestRuntime>::fake_reputation(
				(cid, 6),
				&acc,
				Reputation::VerifiedLinked(6),
			);
		}

		advance_to_assigning();
		let caller = account_id(&bs[0]);
		assert_ok!(EncointerReputationRings::initiate_rings(
			RuntimeOrigin::signed(caller.clone()),
			cid,
			6,
		));

		run_computation_to_completion(&caller);

		let ring1 = EncointerReputationRings::ring_members((cid, 6, 1, 0)).unwrap();

		// Members should be sorted by pubkey.
		let mut sorted = ring1.clone().into_inner();
		sorted.sort();
		assert_eq!(ring1.into_inner(), sorted);
	});
}

#[test]
fn continue_ring_computation_fails_when_no_computation() {
	new_test_ext().execute_with(|| {
		let alice = account_id(&bootstrappers()[0]);
		assert_noop!(
			EncointerReputationRings::continue_ring_computation(RuntimeOrigin::signed(alice),),
			Error::<TestRuntime>::NoComputationPending
		);
	});
}

#[test]
fn account_with_3_attendances_in_correct_rings() {
	new_test_ext().execute_with(|| {
		let bs = bootstrappers();
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);

		// Single account with exactly 3 attendances.
		let acc = account_id(&bs[0]);
		assert_ok!(EncointerReputationRings::register_bandersnatch_key(
			RuntimeOrigin::signed(acc.clone()),
			fake_bandersnatch_key(1),
		));

		for cindex in [4, 5, 6] {
			pallet_encointer_ceremonies::Pallet::<TestRuntime>::fake_reputation(
				(cid, cindex),
				&acc,
				Reputation::VerifiedLinked(cindex),
			);
		}

		advance_to_assigning();
		assert_ok!(EncointerReputationRings::initiate_rings(
			RuntimeOrigin::signed(acc.clone()),
			cid,
			6,
		));

		run_computation_to_completion(&acc);

		let key = fake_bandersnatch_key(1);

		// Should be in 1/5, 2/5, 3/5.
		assert!(EncointerReputationRings::ring_members((cid, 6, 1, 0)).unwrap().contains(&key));
		assert!(EncointerReputationRings::ring_members((cid, 6, 2, 0)).unwrap().contains(&key));
		assert!(EncointerReputationRings::ring_members((cid, 6, 3, 0)).unwrap().contains(&key));

		// Should NOT be in 4/5 or 5/5.
		let ring4 = EncointerReputationRings::ring_members((cid, 6, 4, 0)).unwrap();
		assert!(!ring4.contains(&key));
		let ring5 = EncointerReputationRings::ring_members((cid, 6, 5, 0)).unwrap();
		assert!(!ring5.contains(&key));
	});
}

#[test]
fn rings_are_per_community() {
	new_test_ext().execute_with(|| {
		let bs = bootstrappers();
		let cid_a = register_test_community::<TestRuntime>(None, 1.0, 1.0);
		let cid_b = register_test_community::<TestRuntime>(None, 2.0, 2.0);

		let acc = account_id(&bs[0]);
		assert_ok!(EncointerReputationRings::register_bandersnatch_key(
			RuntimeOrigin::signed(acc.clone()),
			fake_bandersnatch_key(1),
		));

		// Reputation in community A only.
		pallet_encointer_ceremonies::Pallet::<TestRuntime>::fake_reputation(
			(cid_a, 6),
			&acc,
			Reputation::VerifiedLinked(6),
		);

		// Compute rings for community A.
		advance_to_assigning();
		assert_ok!(EncointerReputationRings::initiate_rings(
			RuntimeOrigin::signed(acc.clone()),
			cid_a,
			6,
		));
		run_computation_to_completion(&acc);

		// Compute rings for community B.
		assert_ok!(EncointerReputationRings::initiate_rings(
			RuntimeOrigin::signed(acc.clone()),
			cid_b,
			6,
		));
		run_computation_to_completion(&acc);

		// Community A should have the member.
		let ring_a = EncointerReputationRings::ring_members((cid_a, 6, 1, 0)).unwrap();
		assert_eq!(ring_a.len(), 1);

		// Community B should have empty ring (or not exist).
		let ring_b = EncointerReputationRings::ring_members((cid_b, 6, 1, 0));
		assert!(ring_b.is_none() || ring_b.unwrap().is_empty());
	});
}

// -- Large community tests (realistic 500-member scenario) --

/// Register `n` generated accounts with bandersnatch keys and return their account IDs.
fn setup_large_population(n: usize) -> Vec<test_utils::AccountId> {
	let pairs = add_population(n, 0);
	pairs
		.iter()
		.enumerate()
		.map(|(i, pair)| {
			let acc = account_id(pair);
			assert_ok!(EncointerReputationRings::register_bandersnatch_key(
				RuntimeOrigin::signed(acc.clone()),
				fake_bandersnatch_key_u32(i as u32 + 1),
			));
			acc
		})
		.collect()
}

#[test]
fn large_community_500_members_full_computation() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);
		let accounts = setup_large_population(500);

		// All 500 attended ceremony 6 → 1/5 reputation.
		for acc in &accounts {
			pallet_encointer_ceremonies::Pallet::<TestRuntime>::fake_reputation(
				(cid, 6),
				acc,
				Reputation::VerifiedLinked(6),
			);
		}

		let caller = accounts[0].clone();
		advance_to_assigning();
		assert_ok!(EncointerReputationRings::initiate_rings(
			RuntimeOrigin::signed(caller.clone()),
			cid,
			6,
		));

		let steps = run_computation_to_completion(&caller);

		// 6 collection steps (5 scans + 1 transition) + 5 building steps = 11.
		assert_eq!(steps, 11);

		// 1/5 ring: MaxRingSize=2048, so 500 fits in 1 sub-ring.
		let ring1 = EncointerReputationRings::ring_members((cid, 6, 1, 0)).unwrap();
		assert_eq!(ring1.len(), 500);
		assert_eq!(EncointerReputationRings::sub_ring_count((cid, 6, 1)), 1);

		// 2/5 through 5/5 should be empty (only 1 ceremony attended).
		for level in 2..=5u8 {
			let ring = EncointerReputationRings::ring_members((cid, 6, level, 0)).unwrap();
			assert_eq!(ring.len(), 0, "Ring {level}/5 should be empty");
		}
	});
}

#[test]
fn large_community_500_members_varied_attendance() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);
		let accounts = setup_large_population(500);

		// Distribute attendance realistically:
		// Accounts 0..100:   attended all 5 ceremonies (2-6) → 5/5
		// Accounts 100..200: attended 4 ceremonies (3-6)     → 4/5
		// Accounts 200..350: attended 3 ceremonies (4-6)     → 3/5
		// Accounts 350..450: attended 2 ceremonies (5-6)     → 2/5
		// Accounts 450..500: attended 1 ceremony (6)         → 1/5
		for (i, acc) in accounts.iter().enumerate() {
			let ceremonies: Vec<u32> = match i {
				0..100 => vec![2, 3, 4, 5, 6],
				100..200 => vec![3, 4, 5, 6],
				200..350 => vec![4, 5, 6],
				350..450 => vec![5, 6],
				_ => vec![6],
			};
			for cindex in ceremonies {
				pallet_encointer_ceremonies::Pallet::<TestRuntime>::fake_reputation(
					(cid, cindex),
					acc,
					Reputation::VerifiedLinked(cindex),
				);
			}
		}

		let caller = accounts[0].clone();
		advance_to_assigning();
		assert_ok!(EncointerReputationRings::initiate_rings(
			RuntimeOrigin::signed(caller.clone()),
			cid,
			6,
		));

		let steps = run_computation_to_completion(&caller);
		assert_eq!(steps, 11);

		// Verify ring sizes match expected distribution (all fit in single sub-rings).
		let ring1 = EncointerReputationRings::ring_members((cid, 6, 1, 0)).unwrap();
		let ring2 = EncointerReputationRings::ring_members((cid, 6, 2, 0)).unwrap();
		let ring3 = EncointerReputationRings::ring_members((cid, 6, 3, 0)).unwrap();
		let ring4 = EncointerReputationRings::ring_members((cid, 6, 4, 0)).unwrap();
		let ring5 = EncointerReputationRings::ring_members((cid, 6, 5, 0)).unwrap();

		assert_eq!(ring1.len(), 500); // all 500
		assert_eq!(ring2.len(), 450); // >= 2 attendances
		assert_eq!(ring3.len(), 350); // >= 3 attendances
		assert_eq!(ring4.len(), 200); // >= 4 attendances
		assert_eq!(ring5.len(), 100); // >= 5 attendances

		// Strict nesting holds.
		assert!(ring5.len() < ring4.len());
		assert!(ring4.len() < ring3.len());
		assert!(ring3.len() < ring2.len());
		assert!(ring2.len() < ring1.len());

		// All ring5 members should be in ring4, etc.
		for key in ring5.iter() {
			assert!(ring4.contains(key));
		}
		for key in ring4.iter() {
			assert!(ring3.contains(key));
		}
	});
}

#[test]
fn large_community_step_count_is_predictable() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);
		let accounts = setup_large_population(500);

		// 5/5 attendance for all → worst case for collection (all match every scan).
		for acc in &accounts {
			for cindex in 2..=6u32 {
				pallet_encointer_ceremonies::Pallet::<TestRuntime>::fake_reputation(
					(cid, cindex),
					acc,
					Reputation::VerifiedLinked(cindex),
				);
			}
		}

		let caller = accounts[0].clone();
		advance_to_assigning();
		assert_ok!(EncointerReputationRings::initiate_rings(
			RuntimeOrigin::signed(caller.clone()),
			cid,
			6,
		));

		// Track phase transitions.
		let mut collection_steps = 0u32;
		let mut building_steps = 0u32;

		loop {
			let state = EncointerReputationRings::pending_ring_computation();
			if state.is_none() {
				break;
			}
			let phase = &state.unwrap().phase;
			match phase {
				RingComputationPhase::CollectingMembers { .. } => collection_steps += 1,
				RingComputationPhase::BuildingRing { .. } => building_steps += 1,
				RingComputationPhase::Done => break,
			}
			assert_ok!(EncointerReputationRings::continue_ring_computation(RuntimeOrigin::signed(
				caller.clone()
			),));
		}

		// 5 scans + 1 transition = 6 collection steps.
		assert_eq!(collection_steps, 6);
		// 5 ring levels = 5 building steps.
		assert_eq!(building_steps, 5);

		// All 500 members should be in every ring (all have 5/5).
		for level in 1..=5u8 {
			let ring = EncointerReputationRings::ring_members((cid, 6, level, 0)).unwrap();
			assert_eq!(ring.len(), 500, "Ring {level}/5 should have 500 members");
		}
	});
}

// -- Sub-ring splitting tests --

mod small_ring {
	use super::*;
	use crate as dut;
	use encointer_primitives::{balances::BalanceType, scheduler::CeremonyPhaseType};
	use sp_runtime::BuildStorage;
	use test_utils::*;

	type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<SmallRingRuntime>;

	frame_support::construct_runtime!(
		pub enum SmallRingRuntime
		{
			System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
			Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
			Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
			EncointerScheduler: pallet_encointer_scheduler::{Pallet, Call, Storage, Config<T>, Event},
			EncointerCommunities: pallet_encointer_communities::{Pallet, Call, Storage, Event<T>},
			EncointerCeremonies: pallet_encointer_ceremonies::{Pallet, Call, Storage, Event<T>},
			EncointerBalances: pallet_encointer_balances::{Pallet, Call, Storage, Event<T>},
			EncointerReputationRings: dut::{Pallet, Call, Storage, Event<T>},
		}
	);

	impl dut::Config for SmallRingRuntime {
		type RuntimeEvent = RuntimeEvent;
		type WeightInfo = ();
		type MaxRingSize = ConstU32<16>;
		type ChunkSize = ConstU32<100>;
	}

	impl_frame_system!(SmallRingRuntime);
	impl_timestamp!(SmallRingRuntime, EncointerScheduler);
	impl_balances!(SmallRingRuntime, System);
	impl_encointer_balances!(SmallRingRuntime);
	impl_encointer_communities!(SmallRingRuntime);
	impl_encointer_scheduler!(SmallRingRuntime, EncointerReputationRings);
	impl_encointer_ceremonies!(SmallRingRuntime);

	pub fn new_test_ext() -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<SmallRingRuntime>::default()
			.build_storage()
			.unwrap();

		pallet_encointer_scheduler::GenesisConfig::<SmallRingRuntime> {
			current_ceremony_index: 7,
			phase_durations: vec![
				(CeremonyPhaseType::Registering, ONE_DAY),
				(CeremonyPhaseType::Assigning, ONE_DAY),
				(CeremonyPhaseType::Attesting, ONE_DAY),
			],
			..Default::default()
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_encointer_ceremonies::GenesisConfig::<SmallRingRuntime> {
			ceremony_reward: BalanceType::from_num(1),
			location_tolerance: LOCATION_TOLERANCE,
			time_tolerance: TIME_TOLERANCE,
			inactivity_timeout: 12,
			endorsement_tickets_per_bootstrapper: 50,
			endorsement_tickets_per_reputable: 2,
			reputation_lifetime: 5,
			meetup_time_offset: 0,
			..Default::default()
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_encointer_communities::GenesisConfig::<SmallRingRuntime> {
			min_solar_trip_time_s: 1,
			max_speed_mps: 83,
			..Default::default()
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}

	fn setup_population(n: usize) -> Vec<test_utils::AccountId> {
		let pairs = add_population(n, 0);
		pairs
			.iter()
			.enumerate()
			.map(|(i, pair)| {
				let acc = account_id(pair);
				assert_ok!(EncointerReputationRings::register_bandersnatch_key(
					RuntimeOrigin::signed(acc.clone()),
					fake_bandersnatch_key_u32(i as u32 + 1),
				));
				acc
			})
			.collect()
	}

	fn advance_to_assigning() {
		assert_eq!(EncointerScheduler::current_phase(), CeremonyPhaseType::Registering);
		assert_ok!(EncointerScheduler::next_phase(RuntimeOrigin::root()));
		assert_eq!(EncointerScheduler::current_phase(), CeremonyPhaseType::Assigning);
	}

	fn run_to_completion(caller: &test_utils::AccountId) -> u32 {
		let mut steps = 0u32;
		while EncointerReputationRings::continue_ring_computation(RuntimeOrigin::signed(
			caller.clone(),
		))
		.is_ok()
		{
			steps += 1;
			assert!(steps < 100, "Ring computation didn't complete in 100 steps");
		}
		steps
	}

	#[test]
	fn sub_ring_splitting_with_small_max_ring_size() {
		new_test_ext().execute_with(|| {
			let cid = register_test_community::<SmallRingRuntime>(None, 1.0, 1.0);
			let accounts = setup_population(50);

			// All 50 attended ceremony 6 → 1/5 reputation.
			for acc in &accounts {
				pallet_encointer_ceremonies::Pallet::<SmallRingRuntime>::fake_reputation(
					(cid, 6),
					acc,
					Reputation::VerifiedLinked(6),
				);
			}

			let caller = accounts[0].clone();
			advance_to_assigning();
			assert_ok!(EncointerReputationRings::initiate_rings(
				RuntimeOrigin::signed(caller.clone()),
				cid,
				6,
			));

			run_to_completion(&caller);

			// ceil(50/16) = 4 sub-rings.
			let count = EncointerReputationRings::sub_ring_count((cid, 6, 1));
			assert_eq!(count, 4);

			// Collect all members across sub-rings.
			let mut all_members = Vec::new();
			for i in 0..count {
				let sub_ring = EncointerReputationRings::ring_members((cid, 6, 1, i)).unwrap();
				assert!(sub_ring.len() <= 16, "Sub-ring {} has {} > 16 members", i, sub_ring.len());
				// 50/4 = 12 or 13.
				assert!(
					sub_ring.len() == 12 || sub_ring.len() == 13,
					"Sub-ring {} has {} members, expected 12 or 13",
					i,
					sub_ring.len()
				);
				all_members.extend(sub_ring.into_inner());
			}

			// Union equals full sorted member list.
			assert_eq!(all_members.len(), 50);

			// No duplicates.
			let mut deduped = all_members.clone();
			deduped.sort();
			deduped.dedup();
			assert_eq!(deduped.len(), 50);
		});
	}

	#[test]
	fn sub_ring_sizes_are_balanced() {
		new_test_ext().execute_with(|| {
			let cid = register_test_community::<SmallRingRuntime>(None, 1.0, 1.0);
			let accounts = setup_population(35);

			for acc in &accounts {
				pallet_encointer_ceremonies::Pallet::<SmallRingRuntime>::fake_reputation(
					(cid, 6),
					acc,
					Reputation::VerifiedLinked(6),
				);
			}

			let caller = accounts[0].clone();
			advance_to_assigning();
			assert_ok!(EncointerReputationRings::initiate_rings(
				RuntimeOrigin::signed(caller.clone()),
				cid,
				6,
			));

			run_to_completion(&caller);

			// ceil(35/16) = 3 sub-rings.
			let count = EncointerReputationRings::sub_ring_count((cid, 6, 1));
			assert_eq!(count, 3);

			// 35/3: chunks of 11 or 12.
			let mut total = 0usize;
			for i in 0..count {
				let sub_ring = EncointerReputationRings::ring_members((cid, 6, 1, i)).unwrap();
				assert!(sub_ring.len() <= 16);
				assert!(
					sub_ring.len() == 11 || sub_ring.len() == 12,
					"Sub-ring {} has {} members",
					i,
					sub_ring.len()
				);
				total += sub_ring.len();
			}
			assert_eq!(total, 35);
		});
	}

	#[test]
	fn single_sub_ring_for_small_community() {
		new_test_ext().execute_with(|| {
			let cid = register_test_community::<SmallRingRuntime>(None, 1.0, 1.0);
			let accounts = setup_population(10);

			for acc in &accounts {
				pallet_encointer_ceremonies::Pallet::<SmallRingRuntime>::fake_reputation(
					(cid, 6),
					acc,
					Reputation::VerifiedLinked(6),
				);
			}

			let caller = accounts[0].clone();
			advance_to_assigning();
			assert_ok!(EncointerReputationRings::initiate_rings(
				RuntimeOrigin::signed(caller.clone()),
				cid,
				6,
			));

			run_to_completion(&caller);

			// 10 <= 16, so 1 sub-ring.
			assert_eq!(EncointerReputationRings::sub_ring_count((cid, 6, 1)), 1);

			let ring = EncointerReputationRings::ring_members((cid, 6, 1, 0)).unwrap();
			assert_eq!(ring.len(), 10);
		});
	}

	#[test]
	fn sub_rings_are_contiguous_slices_of_sorted_list() {
		new_test_ext().execute_with(|| {
			let cid = register_test_community::<SmallRingRuntime>(None, 1.0, 1.0);
			let accounts = setup_population(50);

			for acc in &accounts {
				pallet_encointer_ceremonies::Pallet::<SmallRingRuntime>::fake_reputation(
					(cid, 6),
					acc,
					Reputation::VerifiedLinked(6),
				);
			}

			let caller = accounts[0].clone();
			advance_to_assigning();
			assert_ok!(EncointerReputationRings::initiate_rings(
				RuntimeOrigin::signed(caller.clone()),
				cid,
				6,
			));

			run_to_completion(&caller);

			let count = EncointerReputationRings::sub_ring_count((cid, 6, 1));

			// Concatenating all sub-rings in order should produce a sorted list.
			let mut concatenated = Vec::new();
			for i in 0..count {
				let sub_ring = EncointerReputationRings::ring_members((cid, 6, 1, i)).unwrap();
				concatenated.extend(sub_ring.into_inner());
			}

			let mut sorted = concatenated.clone();
			sorted.sort();
			assert_eq!(concatenated, sorted, "Sub-rings should be contiguous sorted slices");
		});
	}
}

// -- Automatic ring computation (on_idle + OnCeremonyPhaseChange) tests --

mod automatic {
	use super::*;
	use crate::pallet::{PendingCeremonyIndex, PendingCommunities};
	use encointer_primitives::scheduler::CeremonyPhaseType;
	use frame_support::{traits::Hooks, weights::Weight};
	use pallet_encointer_scheduler::OnCeremonyPhaseChange;

	#[test]
	fn on_ceremony_phase_change_assigning_populates_queue() {
		new_test_ext().execute_with(|| {
			let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);

			// Current ceremony_index is 7 (from genesis).
			<EncointerReputationRings as OnCeremonyPhaseChange>::on_ceremony_phase_change(
				CeremonyPhaseType::Assigning,
			);

			let queue = PendingCommunities::<TestRuntime>::get();
			assert_eq!(queue.len(), 1);
			assert_eq!(queue[0], cid);
			assert_eq!(PendingCeremonyIndex::<TestRuntime>::get(), 6); // 7 - 1
		});
	}

	#[test]
	fn on_ceremony_phase_change_ignores_non_assigning() {
		new_test_ext().execute_with(|| {
			let _cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);

			<EncointerReputationRings as OnCeremonyPhaseChange>::on_ceremony_phase_change(
				CeremonyPhaseType::Registering,
			);
			assert!(PendingCommunities::<TestRuntime>::get().is_empty());

			<EncointerReputationRings as OnCeremonyPhaseChange>::on_ceremony_phase_change(
				CeremonyPhaseType::Attesting,
			);
			assert!(PendingCommunities::<TestRuntime>::get().is_empty());
		});
	}

	#[test]
	fn on_ceremony_phase_change_skips_if_queue_nonempty() {
		new_test_ext().execute_with(|| {
			let _cid_a = register_test_community::<TestRuntime>(None, 1.0, 1.0);
			let _cid_b = register_test_community::<TestRuntime>(None, 2.0, 2.0);

			// Trigger first phase change — populates with both communities.
			<EncointerReputationRings as OnCeremonyPhaseChange>::on_ceremony_phase_change(
				CeremonyPhaseType::Assigning,
			);
			let queue = PendingCommunities::<TestRuntime>::get();
			assert_eq!(queue.len(), 2);

			// Remove one to simulate partial processing but leave queue non-empty.
			let mut q = queue;
			q.remove(0);
			PendingCommunities::<TestRuntime>::put(q);

			// Second phase change should NOT overwrite.
			<EncointerReputationRings as OnCeremonyPhaseChange>::on_ceremony_phase_change(
				CeremonyPhaseType::Assigning,
			);
			let queue2 = PendingCommunities::<TestRuntime>::get();
			// Should still contain only 1 remaining community, not a fresh 2-entry queue.
			assert_eq!(queue2.len(), 1);
		});
	}

	#[test]
	fn on_idle_processes_single_community() {
		new_test_ext().execute_with(|| {
			let bs = bootstrappers();
			let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);

			// Setup: single account with 1/5 reputation.
			let acc = account_id(&bs[0]);
			assert_ok!(EncointerReputationRings::register_bandersnatch_key(
				RuntimeOrigin::signed(acc.clone()),
				fake_bandersnatch_key(1),
			));
			pallet_encointer_ceremonies::Pallet::<TestRuntime>::fake_reputation(
				(cid, 6),
				&acc,
				Reputation::VerifiedLinked(6),
			);

			// Trigger phase change to queue automatic computation.
			<EncointerReputationRings as OnCeremonyPhaseChange>::on_ceremony_phase_change(
				CeremonyPhaseType::Assigning,
			);
			assert_eq!(PendingCommunities::<TestRuntime>::get().len(), 1);

			// Run on_idle with large weight budget until queue is drained.
			let big_weight = Weight::from_parts(u64::MAX, u64::MAX);
			for _ in 0..20 {
				if PendingCommunities::<TestRuntime>::get().is_empty() &&
					EncointerReputationRings::pending_ring_computation().is_none()
				{
					break;
				}
				EncointerReputationRings::on_idle(1u32.into(), big_weight);
			}

			// Queue should be drained and computation complete.
			assert!(PendingCommunities::<TestRuntime>::get().is_empty());
			assert!(EncointerReputationRings::pending_ring_computation().is_none());

			// Ring should be published.
			let ring1 = EncointerReputationRings::ring_members((cid, 6, 1, 0)).unwrap();
			assert_eq!(ring1.len(), 1);
			assert!(ring1.contains(&fake_bandersnatch_key(1)));
		});
	}

	#[test]
	fn on_idle_processes_multiple_communities() {
		new_test_ext().execute_with(|| {
			let bs = bootstrappers();
			let cid_a = register_test_community::<TestRuntime>(None, 1.0, 1.0);
			let cid_b = register_test_community::<TestRuntime>(None, 2.0, 2.0);

			// Setup: one account with reputation in both communities.
			let acc = account_id(&bs[0]);
			assert_ok!(EncointerReputationRings::register_bandersnatch_key(
				RuntimeOrigin::signed(acc.clone()),
				fake_bandersnatch_key(1),
			));
			for &cid in &[cid_a, cid_b] {
				pallet_encointer_ceremonies::Pallet::<TestRuntime>::fake_reputation(
					(cid, 6),
					&acc,
					Reputation::VerifiedLinked(6),
				);
			}

			// Trigger queue population.
			<EncointerReputationRings as OnCeremonyPhaseChange>::on_ceremony_phase_change(
				CeremonyPhaseType::Assigning,
			);
			assert_eq!(PendingCommunities::<TestRuntime>::get().len(), 2);

			// Run on_idle repeatedly until done.
			let big_weight = Weight::from_parts(u64::MAX, u64::MAX);
			for _ in 0..50 {
				if PendingCommunities::<TestRuntime>::get().is_empty() &&
					EncointerReputationRings::pending_ring_computation().is_none()
				{
					break;
				}
				EncointerReputationRings::on_idle(1u32.into(), big_weight);
			}

			assert!(PendingCommunities::<TestRuntime>::get().is_empty());
			assert!(EncointerReputationRings::pending_ring_computation().is_none());

			// Both communities should have rings.
			let ring_a = EncointerReputationRings::ring_members((cid_a, 6, 1, 0)).unwrap();
			assert_eq!(ring_a.len(), 1);
			let ring_b = EncointerReputationRings::ring_members((cid_b, 6, 1, 0)).unwrap();
			assert_eq!(ring_b.len(), 1);
		});
	}

	#[test]
	fn on_idle_respects_weight_limit() {
		new_test_ext().execute_with(|| {
			let _cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);

			// Queue a community.
			<EncointerReputationRings as OnCeremonyPhaseChange>::on_ceremony_phase_change(
				CeremonyPhaseType::Assigning,
			);
			assert_eq!(PendingCommunities::<TestRuntime>::get().len(), 1);

			// Call on_idle with zero weight — should do nothing.
			let consumed = EncointerReputationRings::on_idle(1u32.into(), Weight::zero());
			assert_eq!(consumed, Weight::zero());

			// Queue should still be full.
			assert_eq!(PendingCommunities::<TestRuntime>::get().len(), 1);
		});
	}

	#[test]
	fn on_idle_handles_step_error_gracefully() {
		new_test_ext().execute_with(|| {
			let bs = bootstrappers();
			let _cid_a = register_test_community::<TestRuntime>(None, 1.0, 1.0);
			let cid_b = register_test_community::<TestRuntime>(None, 2.0, 2.0);

			// Setup: account with reputation in community B only.
			let acc = account_id(&bs[0]);
			assert_ok!(EncointerReputationRings::register_bandersnatch_key(
				RuntimeOrigin::signed(acc.clone()),
				fake_bandersnatch_key(1),
			));
			pallet_encointer_ceremonies::Pallet::<TestRuntime>::fake_reputation(
				(cid_b, 6),
				&acc,
				Reputation::VerifiedLinked(6),
			);

			// Queue both communities.
			<EncointerReputationRings as OnCeremonyPhaseChange>::on_ceremony_phase_change(
				CeremonyPhaseType::Assigning,
			);

			// Run on_idle until done — even if community A produces empty rings,
			// the computation should complete and move on to community B.
			let big_weight = Weight::from_parts(u64::MAX, u64::MAX);
			for _ in 0..50 {
				if PendingCommunities::<TestRuntime>::get().is_empty() &&
					EncointerReputationRings::pending_ring_computation().is_none()
				{
					break;
				}
				EncointerReputationRings::on_idle(1u32.into(), big_weight);
			}

			assert!(PendingCommunities::<TestRuntime>::get().is_empty());

			// Community B should have its ring.
			let ring_b = EncointerReputationRings::ring_members((cid_b, 6, 1, 0)).unwrap();
			assert_eq!(ring_b.len(), 1);
		});
	}

	#[test]
	fn on_ceremony_phase_change_registering_purges_old_rings() {
		new_test_ext().execute_with(|| {
			let bs = bootstrappers();
			let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);

			// Setup: account with reputation at cindex=1.
			let acc = account_id(&bs[0]);
			assert_ok!(EncointerReputationRings::register_bandersnatch_key(
				RuntimeOrigin::signed(acc.clone()),
				fake_bandersnatch_key(1),
			));
			pallet_encointer_ceremonies::Pallet::<TestRuntime>::fake_reputation(
				(cid, 1),
				&acc,
				Reputation::VerifiedLinked(1),
			);

			// Compute rings at cindex=1.
			// Need cindex=1 < current(7), so it's valid.
			advance_to_assigning();
			assert_ok!(EncointerReputationRings::initiate_rings(
				RuntimeOrigin::signed(acc.clone()),
				cid,
				1,
			));
			run_computation_to_completion(&acc);

			// Verify ring exists at cindex=1.
			assert!(EncointerReputationRings::ring_members((cid, 1, 1, 0)).is_some());
			assert_eq!(EncointerReputationRings::sub_ring_count((cid, 1, 1)), 1);

			// reputation_lifetime=5, current cindex=7.
			// Registering phase: purge target = 7 - 5 - 1 = 1. Rings at cindex=1 purged.
			<EncointerReputationRings as OnCeremonyPhaseChange>::on_ceremony_phase_change(
				CeremonyPhaseType::Registering,
			);

			assert!(EncointerReputationRings::ring_members((cid, 1, 1, 0)).is_none());
			assert_eq!(EncointerReputationRings::sub_ring_count((cid, 1, 1)), 0);
		});
	}

	#[test]
	fn manual_extrinsics_still_work() {
		new_test_ext().execute_with(|| {
			let bs = bootstrappers();
			let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);

			let acc = account_id(&bs[0]);
			assert_ok!(EncointerReputationRings::register_bandersnatch_key(
				RuntimeOrigin::signed(acc.clone()),
				fake_bandersnatch_key(1),
			));
			pallet_encointer_ceremonies::Pallet::<TestRuntime>::fake_reputation(
				(cid, 6),
				&acc,
				Reputation::VerifiedLinked(6),
			);

			// Advance to Assigning — also triggers on_ceremony_phase_change which queues
			// automatic computation.
			advance_to_assigning();

			// But use manual extrinsic instead — should work even with queue pending.
			assert_ok!(EncointerReputationRings::initiate_rings(
				RuntimeOrigin::signed(acc.clone()),
				cid,
				6,
			));
			run_computation_to_completion(&acc);

			// Ring should be published via manual path.
			let ring1 = EncointerReputationRings::ring_members((cid, 6, 1, 0)).unwrap();
			assert_eq!(ring1.len(), 1);

			// Queue is still pending (manual doesn't drain it).
			assert!(!PendingCommunities::<TestRuntime>::get().is_empty());
		});
	}
}

// -- Purge tests --

#[test]
fn purge_community_ceremony_rings_clears_data() {
	new_test_ext().execute_with(|| {
		let bs = bootstrappers();
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);

		let acc = account_id(&bs[0]);
		assert_ok!(EncointerReputationRings::register_bandersnatch_key(
			RuntimeOrigin::signed(acc.clone()),
			fake_bandersnatch_key(1),
		));
		pallet_encointer_ceremonies::Pallet::<TestRuntime>::fake_reputation(
			(cid, 6),
			&acc,
			Reputation::VerifiedLinked(6),
		);

		advance_to_assigning();
		assert_ok!(EncointerReputationRings::initiate_rings(
			RuntimeOrigin::signed(acc.clone()),
			cid,
			6,
		));
		run_computation_to_completion(&acc);

		// Verify data exists.
		assert!(EncointerReputationRings::ring_members((cid, 6, 1, 0)).is_some());
		assert_eq!(EncointerReputationRings::sub_ring_count((cid, 6, 1)), 1);

		// Purge.
		EncointerReputationRings::purge_community_ceremony_rings(cid, 6);

		// Verify data is gone.
		for level in 1..=5u8 {
			assert!(EncointerReputationRings::ring_members((cid, 6, level, 0)).is_none());
			assert_eq!(EncointerReputationRings::sub_ring_count((cid, 6, level)), 0);
		}
	});
}

#[test]
fn purge_rings_clears_all_communities() {
	new_test_ext().execute_with(|| {
		let bs = bootstrappers();
		let cid_a = register_test_community::<TestRuntime>(None, 1.0, 1.0);
		let cid_b = register_test_community::<TestRuntime>(None, 2.0, 2.0);

		let acc = account_id(&bs[0]);
		assert_ok!(EncointerReputationRings::register_bandersnatch_key(
			RuntimeOrigin::signed(acc.clone()),
			fake_bandersnatch_key(1),
		));
		for &cid in &[cid_a, cid_b] {
			pallet_encointer_ceremonies::Pallet::<TestRuntime>::fake_reputation(
				(cid, 6),
				&acc,
				Reputation::VerifiedLinked(6),
			);
		}

		// Compute rings for both communities.
		advance_to_assigning();
		for &cid in &[cid_a, cid_b] {
			assert_ok!(EncointerReputationRings::initiate_rings(
				RuntimeOrigin::signed(acc.clone()),
				cid,
				6,
			));
			run_computation_to_completion(&acc);
		}

		// Verify data exists for both.
		assert!(EncointerReputationRings::ring_members((cid_a, 6, 1, 0)).is_some());
		assert!(EncointerReputationRings::ring_members((cid_b, 6, 1, 0)).is_some());

		// Purge all at cindex=6.
		EncointerReputationRings::purge_rings(6);

		// Verify both are gone.
		assert!(EncointerReputationRings::ring_members((cid_a, 6, 1, 0)).is_none());
		assert!(EncointerReputationRings::ring_members((cid_b, 6, 1, 0)).is_none());
	});
}

#[test]
fn purge_does_not_affect_other_cindex() {
	new_test_ext().execute_with(|| {
		let bs = bootstrappers();
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);

		let acc = account_id(&bs[0]);
		assert_ok!(EncointerReputationRings::register_bandersnatch_key(
			RuntimeOrigin::signed(acc.clone()),
			fake_bandersnatch_key(1),
		));

		// Reputation at both cindex=5 and cindex=6.
		for cindex in [5, 6] {
			pallet_encointer_ceremonies::Pallet::<TestRuntime>::fake_reputation(
				(cid, cindex),
				&acc,
				Reputation::VerifiedLinked(cindex),
			);
		}

		// Compute rings at both ceremony indices.
		advance_to_assigning();
		for cindex in [5, 6] {
			assert_ok!(EncointerReputationRings::initiate_rings(
				RuntimeOrigin::signed(acc.clone()),
				cid,
				cindex,
			));
			run_computation_to_completion(&acc);
		}

		// Verify both exist.
		assert!(EncointerReputationRings::ring_members((cid, 5, 1, 0)).is_some());
		assert!(EncointerReputationRings::ring_members((cid, 6, 1, 0)).is_some());

		// Purge only cindex=5.
		EncointerReputationRings::purge_community_ceremony_rings(cid, 5);

		// cindex=5 gone, cindex=6 survives.
		assert!(EncointerReputationRings::ring_members((cid, 5, 1, 0)).is_none());
		assert!(EncointerReputationRings::ring_members((cid, 6, 1, 0)).is_some());
	});
}
