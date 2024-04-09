use crate::*;
use encointer_primitives::communities::{CommunityIdentifier, CommunityMetadata, Degree, Location};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::assert_ok;
use frame_system::RawOrigin;
use sp_core::{crypto::ByteArray, sr25519};
use sp_runtime::RuntimeAppPublic;

/// Our own little test-crypto module because we can't use the `sp-core::sr25519` signing methods
/// in the runtime.
mod app_sr25519 {
	use sp_application_crypto::KeyTypeId;
	pub const TEST_KEY_TYPE_ID: KeyTypeId = KeyTypeId(*b"test");

	use sp_application_crypto::{app_crypto, sr25519};
	app_crypto!(sr25519, TEST_KEY_TYPE_ID);
}

type TestPublic = app_sr25519::Public;

/// Generates a pair in the externalities' `KeyStoreExt`.
///
/// Returns the public key of the generated pair.
fn generate_pair() -> TestPublic {
	// passing a seed gives an error for some reason
	TestPublic::generate_pair(None)
}

fn sign(signer: &TestPublic, data: &Vec<u8>) -> sr25519::Signature {
	signer.sign(data).unwrap().into()
}

fn bootstrappers<T: frame_system::Config>() -> Vec<T::AccountId> {
	let alice: T::AccountId = account("alice", 1, 1);
	let bob: T::AccountId = account("bob", 2, 2);
	let charlie: T::AccountId = account("charlie", 3, 3);

	vec![alice.clone(), bob.clone(), charlie.clone()]
}

fn test_location() -> Location {
	Location { lat: Degree::from_num(1i32), lon: Degree::from_num(1i32) }
}

fn create_community<T: Config>() -> CommunityIdentifier {
	let location = test_location();
	let bs = bootstrappers::<T>();
	pallet_encointer_scheduler::Pallet::<T>::set_phase_duration(
		RawOrigin::Root.into(),
		CeremonyPhaseType::Assigning,
		10u32.into(),
	)
	.ok();
	pallet_encointer_scheduler::Pallet::<T>::set_phase_duration(
		RawOrigin::Root.into(),
		CeremonyPhaseType::Attesting,
		10u32.into(),
	)
	.ok();
	pallet_encointer_scheduler::Pallet::<T>::set_phase_duration(
		RawOrigin::Root.into(),
		CeremonyPhaseType::Registering,
		10u32.into(),
	)
	.ok();
	next_phase::<T>();
	next_phase::<T>();
	next_phase::<T>();
	Pallet::<T>::set_inactivity_timeout(RawOrigin::Root.into(), 5).ok();
	Pallet::<T>::set_reputation_lifetime(RawOrigin::Root.into(), 5).ok();
	Pallet::<T>::set_endorsement_tickets_per_bootstrapper(RawOrigin::Root.into(), 1).ok();
	Pallet::<T>::set_endorsement_tickets_per_reputable(RawOrigin::Root.into(), 1).ok();
	Pallet::<T>::set_location_tolerance(RawOrigin::Root.into(), 1000).ok();
	Pallet::<T>::set_time_tolerance(RawOrigin::Root.into(), 1_000_000u32.into()).ok();

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
	let cid = CommunityIdentifier::new(location, bs).unwrap();
	cid
}

fn create_proof_of_attendance<T: Config>(
	prover: T::AccountId,
	cid: CommunityIdentifier,
	cindex: CeremonyIndexType,
	attendee: &TestPublic,
) -> ProofOfAttendance<T::Signature, T::AccountId>
where
	<T as frame_system::Config>::AccountId: ByteArray,
	<T as Config>::Signature: From<sr25519::Signature>,
{
	let msg = (prover.clone(), cindex);
	ProofOfAttendance {
		prover_public: prover,
		community_identifier: cid,
		ceremony_index: cindex,
		attendee_public: account_id::<T>(&attendee),
		attendee_signature: T::Signature::from(sign(attendee, &msg.encode())),
	}
}

/// Goes to next ceremony phase.
///
/// We purposely don't use `run_to_next_phase` because in the actual node aura complained
/// when the timestamps were manipulated.
fn next_phase<T: Config>() {
	pallet_encointer_scheduler::Pallet::<T>::next_phase(RawOrigin::Root.into()).unwrap();
}

pub fn account_id<T: Config>(account: &TestPublic) -> T::AccountId
where
	<T as frame_system::Config>::AccountId: ByteArray,
{
	T::AccountId::from_slice(account.as_slice()).unwrap()
}

fn fake_last_attendance_and_get_proof<T: Config>(
	prover: &TestPublic,
	cid: CommunityIdentifier,
) -> ProofOfAttendance<T::Signature, T::AccountId>
where
	<T as frame_system::Config>::AccountId: ByteArray,
	<T as Config>::Signature: From<sr25519::Signature>,
{
	let cindex = pallet_encointer_scheduler::Pallet::<T>::current_ceremony_index();
	let prover_account: T::AccountId = account_id::<T>(prover);

	assert_ok!(pallet_encointer_balances::Pallet::<T>::issue(
		cid,
		&prover_account,
		NominalIncome::from_num(1)
	));
	Pallet::<T>::fake_reputation((cid, cindex - 1), &prover_account, Reputation::VerifiedUnlinked);
	assert_eq!(
		Pallet::<T>::participant_reputation((cid, cindex - 1), &prover_account),
		Reputation::VerifiedUnlinked
	);

	let cindex = pallet_encointer_scheduler::Pallet::<T>::current_ceremony_index();
	IssuedRewards::<T>::insert((cid, cindex - 1), 0, MeetupResult::Ok);
	let proof = create_proof_of_attendance::<T>(prover_account, cid, cindex - 1, prover);
	proof
}

pub fn last_event<T: Config>() -> Option<<T as frame_system::Config>::RuntimeEvent> {
	let events = frame_system::Pallet::<T>::events();
	if events.len() < 1 {
		return None
	}
	let frame_system::EventRecord { event, .. } = &events[events.len() - 1];
	Some(event.clone())
}

fn register_users<T: Config>(
	cid: CommunityIdentifier,
	num_newbies: u32,
	num_reputables: u32,
) -> Vec<TestPublic>
where
	<T as frame_system::Config>::AccountId: ByteArray,
	<T as Config>::Signature: From<sr25519::Signature>,
{
	let mut users: Vec<TestPublic> = vec![];
	let mut proofs: Vec<ProofOfAttendance<T::Signature, T::AccountId>> = vec![];

	let num_users_total = num_newbies + num_reputables;
	// create users and fake reputation
	for i in 0..num_users_total {
		let p = generate_pair();
		users.push(p.clone());
		if i < num_reputables {
			proofs.push(fake_last_attendance_and_get_proof::<T>(&p, cid));
		}
	}

	// register users
	for (i, p) in users.iter().enumerate() {
		let mut maybe_proof = None;
		if i < num_reputables as usize {
			maybe_proof = Some(proofs[i].clone())
		}

		assert_ok!(Pallet::<T>::register_participant(
			RawOrigin::Signed(account_id::<T>(p)).into(),
			cid,
			maybe_proof
		));
	}
	users
}

benchmarks! {
	where_clause {
		where
		<T as frame_system::Config>::AccountId: ByteArray,
		<T as Config>::Signature: From<sr25519::Signature>,
		<T as frame_system::Config>::RuntimeEvent: From<pallet::Event<T>>
	}

	register_participant {
		let cid = create_community::<T>();

		let zoran = generate_pair();
		let zoran_account= account_id::<T>(&zoran);
		let proof = fake_last_attendance_and_get_proof::<T>(&zoran, cid);
		let cindex = pallet_encointer_scheduler::Pallet::<T>::current_ceremony_index();

		assert_eq!(ReputableCount::<T>::get((cid, cindex)), 0);
	}: _(RawOrigin::Signed(zoran_account.clone()), cid, Some(proof))
	verify {
		assert_eq!(ReputableCount::<T>::get((cid, cindex)), 1);
		assert_eq!(
			Pallet::<T>::participant_reputation((cid, cindex - 1), zoran_account),
			Reputation::VerifiedLinked(cindex)
		);
	}

	upgrade_registration {
		let cid = create_community::<T>();

		let zoran = generate_pair();
		let zoran_account= account_id::<T>(&zoran);
		let proof = fake_last_attendance_and_get_proof::<T>(&zoran, cid);
		let cindex = pallet_encointer_scheduler::Pallet::<T>::current_ceremony_index();

		assert_ok!(Pallet::<T>::register_participant(
			RawOrigin::Signed(zoran_account.clone()).into(),
			cid,
			None
		));

		assert_eq!(NewbieCount::<T>::get((cid, cindex)), 1);
	}: _(RawOrigin::Signed(zoran_account.clone()), cid, proof)
	verify {
		assert_eq!(ReputableCount::<T>::get((cid, cindex)), 1);
		assert_eq!(NewbieCount::<T>::get((cid, cindex)), 0);
		assert_eq!(
			Pallet::<T>::participant_reputation((cid, cindex - 1), zoran_account),
			Reputation::VerifiedLinked(cindex)
		);
	}

	unregister_participant {
		let cid = create_community::<T>();

		let zoran = generate_pair();
		let zoran_account= account_id::<T>(&zoran);
		let proof = fake_last_attendance_and_get_proof::<T>(&zoran, cid);
		let cindex = pallet_encointer_scheduler::Pallet::<T>::current_ceremony_index();

		assert_ok!(Pallet::<T>::register_participant(
			RawOrigin::Signed(zoran_account.clone()).into(),
			cid,
			Some(proof)
		));

		assert_eq!(ReputableCount::<T>::get((cid, cindex)), 1);
		assert_eq!(
			Pallet::<T>::participant_reputation((cid, cindex - 1), &zoran_account),
			Reputation::VerifiedLinked(cindex)
		);
	}: _(RawOrigin::Signed(zoran_account.clone()), cid, Some((cid, cindex - 1)))
	verify {
		assert_eq!(ReputableCount::<T>::get((cid, cindex)), 0);
		assert_eq!(
			Pallet::<T>::participant_reputation((cid, cindex - 1), zoran_account),
			Reputation::VerifiedUnlinked
		);
	}

	attest_attendees {
		let cid = create_community::<T>();

		let attestor = generate_pair();
		let attestor_account = account_id::<T>(&attestor);

		assert_ok!(Pallet::<T>::register_participant(
			RawOrigin::Signed(attestor_account.clone()).into(),
			cid,
			Some(fake_last_attendance_and_get_proof::<T>(&attestor, cid)))
		);

		let attestees =  BoundedVec::try_from(register_users::<T>(cid, 2, 7).into_iter().map(|u| account_id::<T>(&u)).collect::<Vec<T::AccountId>>()).unwrap();

		next_phase::<T>();
		next_phase::<T>();

		let cindex = pallet_encointer_scheduler::Pallet::<T>::current_ceremony_index();
		let mindex = 1;

	}: _(RawOrigin::Signed(attestor_account.clone()), cid, 3, attestees)
	verify {
		assert_eq!(AttestationCount::<T>::get((cid, cindex)), 1);
		assert_eq!(MeetupParticipantCountVote::<T>::get((cid, cindex), &attestor_account), 3);
	}


	endorse_newcomer {
		let cid = create_community::<T>();
		let cindex = pallet_encointer_scheduler::Pallet::<T>::current_ceremony_index();

		// we let the newbie be endorsed by a reputable as this is the worst case scenario
		let zoran = account_id::<T>(&generate_pair());
		Pallet::<T>::fake_reputation((cid, cindex - 1), &zoran, Reputation::VerifiedUnlinked);

		// issue some income such that newbies are allowed to register
		assert_ok!(pallet_encointer_balances::Pallet::<T>::issue(
			cid,
			&zoran,
			NominalIncome::from_num(1)
		));

		let newbie = generate_pair();
		assert_ok!(Pallet::<T>::register_participant(
			RawOrigin::Signed(account_id::<T>(&newbie)).into(),
			cid, None
		));


		assert_eq!(<EndorseesCount<T>>::get((cid, cindex)), 0);
	}: _(RawOrigin::Signed(zoran), cid, account_id::<T>(&newbie))
	verify {
		assert_eq!(<EndorseesCount<T>>::get((cid, cindex)), 1);
	}

	claim_rewards {
		frame_system::Pallet::<T>::set_block_number(frame_system::Pallet::<T>::block_number() + 1u32.into()); // this is needed to assert events
		let cid = create_community::<T>();
		let users: Vec<_> = register_users::<T>(cid, 2, 8).into_iter().map(|u| account_id::<T>(&u)).collect();

		next_phase::<T>();
		next_phase::<T>();

		let cindex = pallet_encointer_scheduler::Pallet::<T>::current_ceremony_index();
		let loc = test_location();
		let time = crate::Pallet::<T>::get_meetup_time(loc).expect("Could not get meetup time");
		let mindex = 1;

		// attest_attendees
		for attestor in users.iter() {
			assert_ok!(Pallet::<T>::attest_attendees(
				RawOrigin::Signed(attestor.clone()).into(),
				cid, 10,
				BoundedVec::try_from(users.clone().into_iter().filter(|u| u!= attestor).collect::<Vec<T::AccountId>>()).unwrap()
			));
		}

		next_phase::<T>();
		assert!(!IssuedRewards::<T>::contains_key((cid, cindex), mindex));

	}: _(RawOrigin::Signed(users[0].clone()), cid, None)
	verify {
		assert_eq!(last_event::<T>(), Some(Event::RewardsIssued(cid, 1, 10).into()));
		assert!(IssuedRewards::<T>::contains_key((cid, cindex), mindex));
	}

	set_inactivity_timeout {
	}: _(RawOrigin::Root, 13)
	verify {
		assert_eq!(InactivityTimeout::<T>::get(), 13)
	}

	set_meetup_time_offset {
	}: _(RawOrigin::Root, 12i32)
	verify {
		assert_eq!(MeetupTimeOffset::<T>::get(), 12i32)
	}

	set_reputation_lifetime {
	}: _(RawOrigin::Root, 11)
	verify {
		assert_eq!(ReputationLifetime::<T>::get(), 11)
	}

	set_endorsement_tickets_per_bootstrapper {
	}: _(RawOrigin::Root, 10)
	verify {
		assert_eq!(EndorsementTicketsPerBootstrapper::<T>::get(), 10)
	}

	set_endorsement_tickets_per_reputable {
	}: _(RawOrigin::Root, 10)
	verify {
		assert_eq!(EndorsementTicketsPerReputable::<T>::get(), 10)
	}

	set_time_tolerance {
		let tolerance: T::Moment = 600_000u32.into();
	}: _(RawOrigin::Root, tolerance)
	verify {
		assert_eq!(TimeTolerance::<T>::get(), tolerance)
	}

	set_location_tolerance {
	}: _(RawOrigin::Root, 1000u32)
	verify {
		assert_eq!(LocationTolerance::<T>::get(), 1000u32)
	}

	purge_community_ceremony {
		let cid = create_community::<T>();
		let cindex = pallet_encointer_scheduler::Pallet::<T>::current_ceremony_index();
		let user = generate_pair();
		assert_ok!(Pallet::<T>::register_participant(RawOrigin::Signed(account_id::<T>(&user.clone())).into(), cid, Some(fake_last_attendance_and_get_proof::<T>(&user.clone(), cid))));
		assert_eq!(ReputableCount::<T>::get((cid, cindex)), 1);
	}: _(RawOrigin::Root, (cid, cindex))
	verify {
		assert_eq!(ReputableCount::<T>::get((cid, cindex)), 0);
	}

}

impl_benchmark_test_suite!(Pallet, crate::benchmarking::new_test_ext(), crate::mock::TestRuntime);

#[cfg(test)]
fn new_test_ext() -> sp_io::TestExternalities {
	use sp_keystore::{testing::MemoryKeystore, KeystoreExt, KeystorePtr};
	use sp_std::sync::Arc;

	let mut ext = crate::mock::new_test_ext();

	ext.register_extension(KeystoreExt(Arc::new(MemoryKeystore::new()) as KeystorePtr));

	ext
}
