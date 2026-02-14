use crate::{Pallet as ReputationRing, *};
use encointer_primitives::{
	ceremonies::Reputation,
	communities::{CommunityIdentifier, Degree, Location},
	storage::{current_ceremony_index_key, participant_reputation},
};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::{assert_ok, traits::OriginTrait};
use frame_system::RawOrigin;
use parity_scale_codec::Encode;

/// Realistic community size for benchmarking.
const COMMUNITY_SIZE: u32 = 500;

fn fake_key(seed: u32) -> BandersnatchPublicKey {
	let bytes = seed.to_le_bytes();
	let mut key = [0u8; 32];
	key[..4].copy_from_slice(&bytes);
	key
}

/// Register `n` accounts with Bandersnatch keys.
fn setup_accounts<T: Config>(n: u32) -> Vec<T::AccountId> {
	let mut accounts = Vec::with_capacity(n as usize);
	for i in 0..n {
		let acc: T::AccountId = account("member", i, i);
		BandersnatchKeys::<T>::insert(&acc, fake_key(i));
		accounts.push(acc);
	}
	accounts
}

/// Write reputation directly to storage for `n` accounts at a given community+ceremony.
/// Uses raw storage writes (same pattern as democracy benchmarks) since
/// `ParticipantReputation` is `pub(super)`.
fn fake_reputations<T: Config>(
	accounts: &[T::AccountId],
	cid: CommunityIdentifier,
	cindex: CeremonyIndexType,
) where
	T::AccountId: AsRef<[u8; 32]>,
{
	for acc in accounts {
		frame_support::storage::unhashed::put_raw(
			&participant_reputation((cid, cindex), acc),
			&Reputation::VerifiedLinked(cindex).encode(),
		);
	}
}

/// Register a community via the communities pallet extrinsic and return its CID.
fn register_community<T: Config>() -> CommunityIdentifier
where
	T::AccountId: AsRef<[u8; 32]>,
{
	// Set required communities pallet config via extrinsics.
	assert_ok!(pallet_encointer_communities::Pallet::<T>::set_max_speed_mps(
		T::RuntimeOrigin::root(),
		83,
	));
	assert_ok!(pallet_encointer_communities::Pallet::<T>::set_min_solar_trip_time_s(
		T::RuntimeOrigin::root(),
		1,
	));

	let bootstrappers: Vec<T::AccountId> = (0..6).map(|n| account("bs", n, n)).collect();
	let location = Location { lat: Degree::from_num(1.0), lon: Degree::from_num(1.0) };

	assert_ok!(pallet_encointer_communities::Pallet::<T>::new_community(
		T::RuntimeOrigin::root(),
		location,
		bootstrappers.clone(),
		Default::default(),
		None,
		None,
	));

	CommunityIdentifier::new(location, bootstrappers).unwrap()
}

/// Setup a community with `n` members who all have 5/5 reputation (worst case).
/// Ceremony index 6 is used (genesis sets current to 7).
fn setup_full_community<T: Config>(n: u32) -> (CommunityIdentifier, Vec<T::AccountId>)
where
	T::AccountId: AsRef<[u8; 32]>,
{
	// Set ceremony index to 7 so ceremony 6 is valid.
	frame_support::storage::unhashed::put_raw(&current_ceremony_index_key(), &7u32.encode());

	let cid = register_community::<T>();
	let accounts = setup_accounts::<T>(n);

	// Give everyone 5/5 reputation (worst case: all 5 ceremonies scanned, all match).
	for cindex in 2..=6u32 {
		fake_reputations::<T>(&accounts, cid, cindex);
	}

	(cid, accounts)
}

/// Initiate and advance computation to the building phase.
fn advance_to_building_phase<T: Config>(caller: &T::AccountId, cid: CommunityIdentifier)
where
	T::AccountId: AsRef<[u8; 32]>,
{
	assert_ok!(ReputationRing::<T>::initiate_rings(
		RawOrigin::Signed(caller.clone()).into(),
		cid,
		6,
	));

	// 5 scan steps + 1 transition = 6 collection steps.
	for _ in 0..((MAX_REPUTATION_LEVELS as u32) + 1) {
		assert_ok!(ReputationRing::<T>::continue_ring_computation(
			RawOrigin::Signed(caller.clone()).into(),
		));
	}

	// Verify we're in building phase.
	let state = PendingRingComputation::<T>::get().unwrap();
	assert_eq!(
		state.phase,
		RingComputationPhase::BuildingRing { current_level: MAX_REPUTATION_LEVELS }
	);
}

benchmarks! {
	where_clause {
		where
		T::AccountId: AsRef<[u8; 32]>,
	}

	// Benchmark: register a Bandersnatch key (simple storage write).
	register_bandersnatch_key {
		let caller: T::AccountId = account("caller", 0, 0);
		let key = fake_key(42);
	}: _(RawOrigin::Signed(caller.clone()), key)
	verify {
		assert_eq!(BandersnatchKeys::<T>::get(&caller), Some(key));
	}

	// Benchmark: initiate ring computation.
	// Worst case: community exists and ceremony is valid.
	initiate_rings {
		let (cid, accounts) = setup_full_community::<T>(COMMUNITY_SIZE);
		let caller = accounts[0].clone();
	}: _(RawOrigin::Signed(caller), cid, 6)
	verify {
		assert!(PendingRingComputation::<T>::get().is_some());
	}

	// Benchmark: continue_ring_computation during member COLLECTION phase.
	// Worst case: `n` registered keys, all have verified reputation for the scanned ceremony.
	// This is the heaviest step: iterates over all BandersnatchKeys and checks reputation.
	continue_ring_computation_collect {
		let n in 10 .. COMMUNITY_SIZE;

		frame_support::storage::unhashed::put_raw(
			&current_ceremony_index_key(),
			&7u32.encode(),
		);
		let cid = register_community::<T>();
		let accounts = setup_accounts::<T>(n);
		// Reputation for ceremony 6 (offset 0 = first scan).
		fake_reputations::<T>(&accounts, cid, 6);

		let caller = accounts[0].clone();
		assert_ok!(ReputationRing::<T>::initiate_rings(
			RawOrigin::Signed(caller.clone()).into(),
			cid,
			6,
		));
		// State is now CollectingMembers { next_ceremony_offset: 0 }.
	}: continue_ring_computation(RawOrigin::Signed(caller))
	verify {
		let state = PendingRingComputation::<T>::get().unwrap();
		assert_eq!(
			state.phase,
			RingComputationPhase::CollectingMembers { next_ceremony_offset: 1 }
		);
		// All n accounts should have been collected.
		assert_eq!(state.attendance.len(), n as usize);
	}

	// Benchmark: continue_ring_computation during ring BUILDING phase.
	// Worst case: `n` members all qualify for the ring (5/5, all included).
	// Iterates attendance list + reads n keys + sorts + writes bounded vec.
	continue_ring_computation_build {
		let n in 10 .. COMMUNITY_SIZE;

		let (cid, accounts) = setup_full_community::<T>(n);
		let caller = accounts[0].clone();
		advance_to_building_phase::<T>(&caller, cid);
	}: continue_ring_computation(RawOrigin::Signed(caller))
	verify {
		// First build step produces the 5/5 ring.
		let ring = RingMembers::<T>::get((cid, 6u32, 5u8, 0u32)).unwrap();
		assert_eq!(ring.len(), n as usize);
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::TestRuntime);
