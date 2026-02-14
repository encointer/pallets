use crate::{mock::*, BandersnatchPublicKey, Error, RingComputationPhase, MAX_REPUTATION_LEVELS};
use encointer_primitives::ceremonies::Reputation;
use frame_support::{assert_noop, assert_ok};
use test_utils::helpers::{account_id, add_population, bootstrappers, register_test_community};

/// Run ring computation to completion. Returns the number of steps taken.
fn run_computation_to_completion(caller: &test_utils::AccountId) -> u32 {
	let mut steps = 0u32;
	loop {
		match EncointerReputationRing::continue_ring_computation(
			RuntimeOrigin::signed(caller.clone()),
		) {
			Ok(_) => steps += 1,
			Err(_) => {
				// NoComputationPending or ComputationAlreadyDone means we're done.
				break;
			},
		}
		// Safety: avoid infinite loop.
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
		assert_ok!(EncointerReputationRing::register_bandersnatch_key(
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

		assert_ok!(EncointerReputationRing::register_bandersnatch_key(
			RuntimeOrigin::signed(alice.clone()),
			key,
		));

		assert_eq!(EncointerReputationRing::bandersnatch_key(&alice), Some(key));
	});
}

#[test]
fn register_bandersnatch_key_update_works() {
	new_test_ext().execute_with(|| {
		let alice = account_id(&bootstrappers()[0]);
		let key1 = fake_bandersnatch_key(1);
		let key2 = fake_bandersnatch_key(2);

		assert_ok!(EncointerReputationRing::register_bandersnatch_key(
			RuntimeOrigin::signed(alice.clone()),
			key1,
		));
		assert_ok!(EncointerReputationRing::register_bandersnatch_key(
			RuntimeOrigin::signed(alice.clone()),
			key2,
		));

		assert_eq!(EncointerReputationRing::bandersnatch_key(&alice), Some(key2));
	});
}

// -- Ring initiation tests --

#[test]
fn initiate_rings_works() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);
		let alice = account_id(&bootstrappers()[0]);

		// Current ceremony index is 7 (from genesis), so ceremony 6 is valid.
		assert_ok!(EncointerReputationRing::initiate_rings(
			RuntimeOrigin::signed(alice),
			cid,
			6,
		));

		let state = EncointerReputationRing::pending_ring_computation().unwrap();
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

		assert_ok!(EncointerReputationRing::initiate_rings(
			RuntimeOrigin::signed(alice.clone()),
			cid,
			6,
		));
		assert_noop!(
			EncointerReputationRing::initiate_rings(
				RuntimeOrigin::signed(alice),
				cid,
				5,
			),
			Error::<TestRuntime>::ComputationAlreadyInProgress
		);
	});
}

#[test]
fn initiate_rings_fails_for_unknown_community() {
	new_test_ext().execute_with(|| {
		let alice = account_id(&bootstrappers()[0]);
		let fake_cid = encointer_primitives::communities::CommunityIdentifier::default();

		assert_noop!(
			EncointerReputationRing::initiate_rings(
				RuntimeOrigin::signed(alice),
				fake_cid,
				6,
			),
			Error::<TestRuntime>::CommunityNotFound
		);
	});
}

#[test]
fn initiate_rings_fails_for_invalid_ceremony_index() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);
		let alice = account_id(&bootstrappers()[0]);

		// Current index is 7. Index 7 is current (not yet complete).
		assert_noop!(
			EncointerReputationRing::initiate_rings(
				RuntimeOrigin::signed(alice.clone()),
				cid,
				7,
			),
			Error::<TestRuntime>::InvalidCeremonyIndex
		);

		// Index 0 is invalid.
		assert_noop!(
			EncointerReputationRing::initiate_rings(
				RuntimeOrigin::signed(alice),
				cid,
				0,
			),
			Error::<TestRuntime>::InvalidCeremonyIndex
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
		let caller = account_id(&bs[0]);
		assert_ok!(EncointerReputationRing::initiate_rings(
			RuntimeOrigin::signed(caller.clone()),
			cid,
			6,
		));

		// Run member collection: 5 scan steps + 1 transition step = 6.
		for _ in 0..(MAX_REPUTATION_LEVELS as u32 + 1) {
			assert_ok!(EncointerReputationRing::continue_ring_computation(
				RuntimeOrigin::signed(caller.clone()),
			));
		}

		// Check state: should now be in BuildingRing phase.
		let state = EncointerReputationRing::pending_ring_computation().unwrap();
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
			assert_ok!(EncointerReputationRing::continue_ring_computation(
				RuntimeOrigin::signed(caller.clone()),
			));
		}

		// Computation should be done (storage cleared).
		assert!(EncointerReputationRing::pending_ring_computation().is_none());

		// Verify rings exist for all 5 levels.
		let alice_key = fake_bandersnatch_key(1);
		let bob_key = fake_bandersnatch_key(2);
		let charlie_key = fake_bandersnatch_key(3);

		// 5/5 ring: only Alice (attended all 5).
		let ring5 = EncointerReputationRing::ring_members((cid, 6, 5)).unwrap();
		assert_eq!(ring5.len(), 1);
		assert!(ring5.contains(&alice_key));

		// 4/5 ring: only Alice (Bob has 3, Charlie has 1).
		let ring4 = EncointerReputationRing::ring_members((cid, 6, 4)).unwrap();
		assert_eq!(ring4.len(), 1);
		assert!(ring4.contains(&alice_key));

		// 3/5 ring: Alice + Bob.
		let ring3 = EncointerReputationRing::ring_members((cid, 6, 3)).unwrap();
		assert_eq!(ring3.len(), 2);
		assert!(ring3.contains(&alice_key));
		assert!(ring3.contains(&bob_key));

		// 2/5 ring: Alice + Bob (Charlie has only 1).
		let ring2 = EncointerReputationRing::ring_members((cid, 6, 2)).unwrap();
		assert_eq!(ring2.len(), 2);
		assert!(ring2.contains(&alice_key));
		assert!(ring2.contains(&bob_key));

		// 1/5 ring: Alice + Bob + Charlie.
		let ring1 = EncointerReputationRing::ring_members((cid, 6, 1)).unwrap();
		assert_eq!(ring1.len(), 3);
		assert!(ring1.contains(&alice_key));
		assert!(ring1.contains(&bob_key));
		assert!(ring1.contains(&charlie_key));
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
			(0, 2), (0, 3), (0, 4), (0, 5), (0, 6),
			// Account 1: 4 ceremonies
			(1, 3), (1, 4), (1, 5), (1, 6),
			// Account 2: 2 ceremonies
			(2, 5), (2, 6),
			// Account 3: 1 ceremony
			(3, 6),
		];
		setup_community_with_reputations(cid, &bs[..4], &verified);

		let caller = account_id(&bs[0]);
		assert_ok!(EncointerReputationRing::initiate_rings(
			RuntimeOrigin::signed(caller.clone()),
			cid,
			6,
		));

		// Run all steps to completion.
		run_computation_to_completion(&caller);

		// Verify strict nesting: ring[n+1] ⊂ ring[n].
		for level in 1..MAX_REPUTATION_LEVELS {
			let ring_lower =
				EncointerReputationRing::ring_members((cid, 6, level)).unwrap();
			let ring_higher =
				EncointerReputationRing::ring_members((cid, 6, level + 1)).unwrap();
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

		assert_ok!(EncointerReputationRing::register_bandersnatch_key(
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

		let caller = alice.clone();
		assert_ok!(EncointerReputationRing::initiate_rings(
			RuntimeOrigin::signed(caller.clone()),
			cid,
			6,
		));

		// Run all steps to completion.
		run_computation_to_completion(&caller);

		// Only Alice should be in the ring.
		let ring1 = EncointerReputationRing::ring_members((cid, 6, 1)).unwrap();
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
		for i in 0..4 {
			let acc = account_id(&bs[i]);
			assert_ok!(EncointerReputationRing::register_bandersnatch_key(
				RuntimeOrigin::signed(acc.clone()),
				fake_bandersnatch_key(i as u8 + 1),
			));
			pallet_encointer_ceremonies::Pallet::<TestRuntime>::fake_reputation(
				(cid, 6),
				&acc,
				Reputation::VerifiedLinked(6),
			);
		}

		let caller = account_id(&bs[0]);
		assert_ok!(EncointerReputationRing::initiate_rings(
			RuntimeOrigin::signed(caller.clone()),
			cid,
			6,
		));

		run_computation_to_completion(&caller);

		let ring1 = EncointerReputationRing::ring_members((cid, 6, 1)).unwrap();

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
			EncointerReputationRing::continue_ring_computation(
				RuntimeOrigin::signed(alice),
			),
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
		assert_ok!(EncointerReputationRing::register_bandersnatch_key(
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

		assert_ok!(EncointerReputationRing::initiate_rings(
			RuntimeOrigin::signed(acc.clone()),
			cid,
			6,
		));

		run_computation_to_completion(&acc);

		let key = fake_bandersnatch_key(1);

		// Should be in 1/5, 2/5, 3/5.
		assert!(EncointerReputationRing::ring_members((cid, 6, 1)).unwrap().contains(&key));
		assert!(EncointerReputationRing::ring_members((cid, 6, 2)).unwrap().contains(&key));
		assert!(EncointerReputationRing::ring_members((cid, 6, 3)).unwrap().contains(&key));

		// Should NOT be in 4/5 or 5/5.
		let ring4 = EncointerReputationRing::ring_members((cid, 6, 4)).unwrap();
		assert!(!ring4.contains(&key));
		let ring5 = EncointerReputationRing::ring_members((cid, 6, 5)).unwrap();
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
		assert_ok!(EncointerReputationRing::register_bandersnatch_key(
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
		assert_ok!(EncointerReputationRing::initiate_rings(
			RuntimeOrigin::signed(acc.clone()),
			cid_a,
			6,
		));
		run_computation_to_completion(&acc);

		// Compute rings for community B.
		assert_ok!(EncointerReputationRing::initiate_rings(
			RuntimeOrigin::signed(acc.clone()),
			cid_b,
			6,
		));
		run_computation_to_completion(&acc);

		// Community A should have the member.
		let ring_a = EncointerReputationRing::ring_members((cid_a, 6, 1)).unwrap();
		assert_eq!(ring_a.len(), 1);

		// Community B should have empty ring (or not exist).
		let ring_b = EncointerReputationRing::ring_members((cid_b, 6, 1));
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
			assert_ok!(EncointerReputationRing::register_bandersnatch_key(
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
		assert_ok!(EncointerReputationRing::initiate_rings(
			RuntimeOrigin::signed(caller.clone()),
			cid,
			6,
		));

		let steps = run_computation_to_completion(&caller);

		// 6 collection steps (5 scans + 1 transition) + 5 building steps = 11.
		assert_eq!(steps, 11);

		// 1/5 ring should have all 500 members.
		let ring1 = EncointerReputationRing::ring_members((cid, 6, 1)).unwrap();
		assert_eq!(ring1.len(), 500);

		// 2/5 through 5/5 should be empty (only 1 ceremony attended).
		for level in 2..=5u8 {
			let ring = EncointerReputationRing::ring_members((cid, 6, level)).unwrap();
			assert_eq!(ring.len(), 0, "Ring {}/5 should be empty", level);
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
		assert_ok!(EncointerReputationRing::initiate_rings(
			RuntimeOrigin::signed(caller.clone()),
			cid,
			6,
		));

		let steps = run_computation_to_completion(&caller);
		assert_eq!(steps, 11);

		// Verify ring sizes match expected distribution.
		let ring1 = EncointerReputationRing::ring_members((cid, 6, 1)).unwrap();
		let ring2 = EncointerReputationRing::ring_members((cid, 6, 2)).unwrap();
		let ring3 = EncointerReputationRing::ring_members((cid, 6, 3)).unwrap();
		let ring4 = EncointerReputationRing::ring_members((cid, 6, 4)).unwrap();
		let ring5 = EncointerReputationRing::ring_members((cid, 6, 5)).unwrap();

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
		assert_ok!(EncointerReputationRing::initiate_rings(
			RuntimeOrigin::signed(caller.clone()),
			cid,
			6,
		));

		// Track phase transitions.
		let mut collection_steps = 0u32;
		let mut building_steps = 0u32;

		loop {
			let state = EncointerReputationRing::pending_ring_computation();
			if state.is_none() {
				break;
			}
			let phase = &state.unwrap().phase;
			match phase {
				RingComputationPhase::CollectingMembers { .. } => collection_steps += 1,
				RingComputationPhase::BuildingRing { .. } => building_steps += 1,
				RingComputationPhase::Done => break,
			}
			assert_ok!(EncointerReputationRing::continue_ring_computation(
				RuntimeOrigin::signed(caller.clone()),
			));
		}

		// 5 scans + 1 transition = 6 collection steps.
		assert_eq!(collection_steps, 6);
		// 5 ring levels = 5 building steps.
		assert_eq!(building_steps, 5);

		// All 500 members should be in every ring (all have 5/5).
		for level in 1..=5u8 {
			let ring = EncointerReputationRing::ring_members((cid, 6, level)).unwrap();
			assert_eq!(ring.len(), 500, "Ring {}/5 should have 500 members", level);
		}
	});
}
