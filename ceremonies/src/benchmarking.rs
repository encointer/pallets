use crate::*;
use encointer_primitives::communities::{CommunityIdentifier, CommunityMetadata, Degree, Location};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::assert_ok;
use frame_system::RawOrigin;
use sp_application_crypto::KeyTypeId;
use sp_core::{crypto::ByteArray, sr25519};
use sp_runtime::RuntimeAppPublic;

pub const TEST_KEY_TYPE_ID: KeyTypeId = KeyTypeId(*b"test");

mod app_sr25519 {
	use super::TEST_KEY_TYPE_ID;
	use sp_application_crypto::{app_crypto, sr25519};
	app_crypto!(sr25519, TEST_KEY_TYPE_ID);
}

type TestPublic = app_sr25519::Public;

/// Generates a pair in the test externalities' `KeyStoreExt`.
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

	encointer_communities::Pallet::<T>::new_community(
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

fn get_all_claims<T: Config>(
	attestees: Vec<TestPublic>,
	cid: CommunityIdentifier,
	cindex: CeremonyIndexType,
	mindex: MeetupIndexType,
	location: Location,
	timestamp: T::Moment,
	n_participants: u32,
) -> Vec<ClaimOfAttendance<T::Signature, T::AccountId, T::Moment>>
where
	<T as frame_system::Config>::AccountId: ByteArray,
	<T as Config>::Signature: From<sr25519::Signature>,
{
	let mut claims: Vec<ClaimOfAttendance<T::Signature, T::AccountId, T::Moment>> = vec![];
	for a in attestees.into_iter() {
		let mut claim = ClaimOfAttendance::<T::Signature, T::AccountId, T::Moment>::new_unsigned(
			account_id::<T>(&a),
			cindex,
			cid,
			mindex,
			location,
			timestamp,
			n_participants,
		);

		claim.claimant_signature = Some(sign(&a, &claim.payload_encoded()).into());

		claims.push(claim);
	}
	claims
}

/// Goes to next ceremony phase.
///
/// We purposely don't use `run_to_next_phase` because in the actual node aura complained
/// when the timestamps were manipulated.
fn next_phase<T: Config>() {
	encointer_scheduler::Pallet::<T>::next_phase(RawOrigin::Root.into()).unwrap();
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
	let cindex = encointer_scheduler::Pallet::<T>::current_ceremony_index();
	let prover_account: T::AccountId = account_id::<T>(prover);

	assert_ok!(encointer_balances::Pallet::<T>::issue(
		cid,
		&prover_account,
		NominalIncome::from_num(1)
	));
	Pallet::<T>::fake_reputation((cid, cindex - 1), &prover_account, Reputation::VerifiedUnlinked);
	assert_eq!(
		Pallet::<T>::participant_reputation((cid, cindex - 1), &prover_account),
		Reputation::VerifiedUnlinked
	);

	let cindex = encointer_scheduler::Pallet::<T>::current_ceremony_index();
	IssuedRewards::<T>::insert((cid, cindex - 1), 0, ());
	let proof = create_proof_of_attendance::<T>(prover_account, cid, cindex - 1, prover);
	proof
}

pub fn last_event<T: Config>() -> Option<<T as frame_system::Config>::Event> {
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
		<T as frame_system::Config>::Event: From<pallet::Event<T>>
	}

	register_participant {
		let cid = create_community::<T>();

		let zoran = generate_pair();
		let zoran_account= account_id::<T>(&zoran);
		let proof = fake_last_attendance_and_get_proof::<T>(&zoran, cid);
		let cindex = encointer_scheduler::Pallet::<T>::current_ceremony_index();

		assert_eq!(ReputableCount::<T>::get((cid, cindex)), 0);
	}: _(RawOrigin::Signed(zoran_account.clone()), cid, Some(proof))
	verify {
		assert_eq!(ReputableCount::<T>::get((cid, cindex)), 1);
		assert_eq!(
			Pallet::<T>::participant_reputation((cid, cindex - 1), zoran_account),
			Reputation::VerifiedLinked
		);
	}

	attest_claims {
		let cid = create_community::<T>();

		let attestor = generate_pair();
		let attestor_account = account_id::<T>(&attestor);

		assert_ok!(Pallet::<T>::register_participant(
			RawOrigin::Signed(attestor_account.clone()).into(),
			cid,
			Some(fake_last_attendance_and_get_proof::<T>(&attestor, cid)))
		);

		let attestees =  register_users::<T>(cid, 2, 7);

		next_phase::<T>();
		next_phase::<T>();

		let cindex = encointer_scheduler::Pallet::<T>::current_ceremony_index();
		let loc = test_location();
		let time = crate::Pallet::<T>::get_meetup_time(loc).expect("Could not get meetup time");
		let mindex = 1;

		let claims = get_all_claims::<T>(attestees, cid, cindex, mindex, loc, time, 10);
		assert_eq!(AttestationCount::<T>::get((cid, cindex)), 0);

	}: _(RawOrigin::Signed(attestor_account), claims)
	verify {
		assert_eq!(AttestationCount::<T>::get((cid, cindex)), 1);
	}

	endorse_newcomer {
		let cid = create_community::<T>();
		let alice: T::AccountId = account("alice", 1, 1);

		// issue some income such that newbies are allowed to register
		assert_ok!(encointer_balances::Pallet::<T>::issue(
			cid,
			&alice,
			NominalIncome::from_num(1)
		));

		let newbie = generate_pair();
		assert_ok!(Pallet::<T>::register_participant(
			RawOrigin::Signed(account_id::<T>(&newbie)).into(),
			cid, None
		));

		let cindex = encointer_scheduler::Pallet::<T>::current_ceremony_index();

		assert_eq!(<EndorseesCount<T>>::get((cid, cindex)), 0);
	}: _(RawOrigin::Signed(alice), cid, account_id::<T>(&newbie))
	verify {
		assert_eq!(<EndorseesCount<T>>::get((cid, cindex)), 1);
	}

	claim_rewards {
		frame_system::Pallet::<T>::set_block_number(frame_system::Pallet::<T>::block_number() + 1u32.into()); // this is needed to assert events
		let cid = create_community::<T>();
		let users = register_users::<T>(cid, 2, 8);

		next_phase::<T>();
		next_phase::<T>();

		let cindex = encointer_scheduler::Pallet::<T>::current_ceremony_index();
		let loc = test_location();
		let time = crate::Pallet::<T>::get_meetup_time(loc).expect("Could not get meetup time");
		let mindex = 1;

		// attest_claims
		for i in 0..10 {
			let attestor = users[i as usize].clone();
			let mut attestees = users.clone();
			attestees.remove(i as usize);
			let claims = get_all_claims::<T>(attestees, cid, cindex, mindex, loc, time, 10);
			assert_ok!(Pallet::<T>::attest_claims(RawOrigin::Signed(account_id::<T>(&attestor)).into(), claims));
		}

		next_phase::<T>();
		assert!(!IssuedRewards::<T>::contains_key((cid, cindex), mindex));

	}: _(RawOrigin::Signed(account_id::<T>(&users[0])), cid)
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

	purge_community_ceremony {
		let cid = create_community::<T>();
		let cindex = encointer_scheduler::Pallet::<T>::current_ceremony_index();
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
	use sp_keystore::{testing::KeyStore, KeystoreExt, SyncCryptoStorePtr};
	use sp_std::sync::Arc;

	let mut ext = crate::mock::new_test_ext();

	ext.register_extension(KeystoreExt(Arc::new(KeyStore::new()) as SyncCryptoStorePtr));

	ext
}
