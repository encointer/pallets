use crate::*;
use encointer_primitives::communities::{
	CommunityIdentifier, CommunityMetadata as CommunityMetadataType, Degree, Location, LossyInto,
};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::{
	assert_ok,
	traits::{OnFinalize, OnInitialize},
};
use frame_system::RawOrigin;
use sp_core::{sr25519, Pair};
use sp_runtime::traits::UniqueSaturatedInto;

pub const GENESIS_TIME: u64 = 1_585_058_843_000;
pub const ONE_DAY: u64 = 86_400_000;
pub const BLOCKTIME: u64 = 3_600_000;

fn create_community<T: Config>() -> CommunityIdentifier {
	let alice: T::AccountId = account("alice", 1, 1);
	let bob: T::AccountId = account("bob", 2, 2);
	let charlie: T::AccountId = account("charlie", 3, 3);

	let location = Location { lat: Degree::from_num(1i32), lon: Degree::from_num(1i32) };

	let bs = vec![alice.clone(), bob.clone(), charlie.clone()];
	let community_meta: CommunityMetadataType = CommunityMetadataType {
		name: "Default".into(),
		symbol: "DEF".into(),
		..Default::default()
	};
	encointer_communities::Pallet::<T>::new_community(
		RawOrigin::Root.into(),
		location,
		bs.clone(),
		community_meta.clone(),
		None,
		None,
	)
	.ok();
	let cid = CommunityIdentifier::new(location, bs).unwrap();
	cid
}

fn correct_meetup_time<T: Config>(cindex: CeremonyIndexType, location: Location) -> T::Moment
where
	<T as pallet_timestamp::Config>::Moment: From<u64>,
{
	let mlon: f64 = location.lon.lossy_into();

	let ci = T::Moment::from(cindex.into());
	let genesis_time = T::Moment::from(GENESIS_TIME);
	let one_day = T::Moment::from(ONE_DAY);

	let mut t: T::Moment = genesis_time - (genesis_time % one_day) +
		ci * encointer_scheduler::Pallet::<T>::phase_durations(CeremonyPhaseType::REGISTERING) +
		ci * encointer_scheduler::Pallet::<T>::phase_durations(CeremonyPhaseType::ASSIGNING) +
		(ci - T::Moment::from(1)) *
			encointer_scheduler::Pallet::<T>::phase_durations(CeremonyPhaseType::ATTESTING) +
		one_day / T::Moment::from(2) -
		T::Moment::from((mlon / 360.0 * ONE_DAY as f64) as u64);
	t += Pallet::<T>::meetup_time_offset().into();
	t
}

pub fn account_id<T: Config>(pair: &sr25519::Pair) -> T::AccountId
where
	<T as frame_system::Config>::AccountId: From<sp_core::sr25519::Public>,
{
	pair.public().into()
}

fn create_proof_of_attendance<T: Config>(
	prover: T::AccountId,
	cid: CommunityIdentifier,
	cindex: CeremonyIndexType,
	attendee: &sr25519::Pair,
) -> ProofOfAttendance<T::Signature, T::AccountId>
where
	<T as frame_system::Config>::AccountId: From<sp_core::sr25519::Public>,
	<T as Config>::Signature: From<sp_core::sr25519::Signature>,
{
	let msg = (prover.clone(), cindex);
	ProofOfAttendance {
		prover_public: prover,
		community_identifier: cid,
		ceremony_index: cindex,
		attendee_public: account_id::<T>(&attendee),
		attendee_signature: T::Signature::from(attendee.sign(&msg.encode())),
	}
}

fn get_all_claims<T: Config>(
	attestees: &Vec<sr25519::Pair>,
	cid: CommunityIdentifier,
	cindex: CeremonyIndexType,
	mindex: MeetupIndexType,
	location: Location,
	timestamp: T::Moment,
	n_participants: u32,
) -> Vec<ClaimOfAttendance<T::Signature, T::AccountId, T::Moment>>
where
	<T as frame_system::Config>::AccountId: From<sp_core::sr25519::Public>,
	<T as Config>::Signature: From<sp_core::sr25519::Signature>,
{
	let mut claims: Vec<ClaimOfAttendance<T::Signature, T::AccountId, T::Moment>> = vec![];
	for a in attestees {
		claims.push(
			ClaimOfAttendance::<T::Signature, T::AccountId, T::Moment>::new_unsigned(
				a.public().into(),
				cindex,
				cid,
				mindex,
				location,
				timestamp,
				n_participants,
			)
			.sign(a),
		);
	}
	claims
}

/// Run until a particular block.
fn run_to_block<T: Config>(n: T::BlockNumber)
where
	<T as frame_system::Config>::BlockNumber: From<u64>,
	<T as pallet_timestamp::Config>::Moment: From<u64>,
{
	while frame_system::Pallet::<T>::block_number() < n {
		if frame_system::Pallet::<T>::block_number() > 1.into() {
			frame_system::Pallet::<T>::on_finalize(frame_system::Pallet::<T>::block_number());
		}
		let new_timestamp: u64 = (T::BlockNumber::from(GENESIS_TIME) +
			T::BlockNumber::from(BLOCKTIME) * n)
			.unique_saturated_into();

		set_timestamp::<T>(T::Moment::from(new_timestamp));
		pallet_timestamp::Pallet::<T>::on_finalize(frame_system::Pallet::<T>::block_number());
		frame_system::Pallet::<T>::set_block_number(
			frame_system::Pallet::<T>::block_number() + 1.into(),
		);
		frame_system::Pallet::<T>::on_initialize(frame_system::Pallet::<T>::block_number());
	}
}

/// Progress blocks until the phase changes
fn run_to_next_phase<T: Config>()
where
	<T as frame_system::Config>::BlockNumber: From<u64>,
	<T as pallet_timestamp::Config>::Moment: From<u64>,
{
	let phase = encointer_scheduler::Pallet::<T>::current_phase();
	let mut blocknr = frame_system::Pallet::<T>::block_number();
	while phase == encointer_scheduler::Pallet::<T>::current_phase() {
		blocknr += 1.into();
		run_to_block::<T>(blocknr);
	}
}

pub fn set_timestamp<T: Config>(t: T::Moment) {
	let _ = pallet_timestamp::Pallet::<T>::set(RawOrigin::None.into(), t);
}

fn fake_last_attendance_and_get_proof<T: Config>(
	prover: &sr25519::Pair,
	cid: CommunityIdentifier,
) -> ProofOfAttendance<T::Signature, T::AccountId>
where
	<T as frame_system::Config>::AccountId: From<sp_core::sr25519::Public>,
	<T as Config>::Signature: From<sp_core::sr25519::Signature>,
{
	let cindex = encointer_scheduler::Pallet::<T>::current_ceremony_index();
	let prover_account_id = account_id::<T>(&prover);
	assert_ok!(encointer_balances::Pallet::<T>::issue(
		cid,
		&prover_account_id,
		NominalIncome::from_num(1)
	));
	Pallet::<T>::fake_reputation(
		(cid, cindex - 1),
		&account_id::<T>(&prover),
		Reputation::VerifiedUnlinked,
	);
	assert_eq!(
		Pallet::<T>::participant_reputation((cid, cindex - 1), &prover_account_id),
		Reputation::VerifiedUnlinked
	);
	let cindex = encointer_scheduler::Pallet::<T>::current_ceremony_index();
	IssuedRewards::<T>::insert((cid, cindex - 1), 0, ());
	let proof = create_proof_of_attendance::<T>(prover_account_id, cid, cindex - 1, &prover);
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
) -> Vec<sr25519::Pair>
where
	<T as frame_system::Config>::AccountId: From<sp_core::sr25519::Public>,
	<T as Config>::Signature: From<sp_core::sr25519::Signature>,
{
	let mut users: Vec<sr25519::Pair> = vec![];
	let mut proofs: Vec<ProofOfAttendance<T::Signature, T::AccountId>> = vec![];

	let num_users_total = num_newbies + num_reputables;
	// create users and fake reputation
	for i in 0..num_users_total {
		let p = sr25519::Pair::from_entropy(&[i as u8; 32], None).0;
		users.push(p.clone());
		if i < num_reputables {
			proofs.push(fake_last_attendance_and_get_proof::<T>(&p.clone(), cid));
		}
	}

	// register users
	for i in 0..num_users_total {
		let p = users[i as usize].clone();
		if i < num_reputables {
			assert_ok!(Pallet::<T>::register_participant(
				RawOrigin::Signed(account_id::<T>(&(p.clone()))).into(),
				cid,
				Some(proofs[i as usize].clone())
			));
		} else {
			assert_ok!(Pallet::<T>::register_participant(
				RawOrigin::Signed(account_id::<T>(&(p.clone()))).into(),
				cid,
				None
			));
		}
	}
	users
}

benchmarks! {
	where_clause {
		where
		<T as frame_system::Config>::AccountId: From<sp_core::sr25519::Public>,
		<T as Config>::Signature: From<sp_core::sr25519::Signature>,
		<T as pallet_timestamp::Config>::Moment: From<u64>,
		<T as frame_system::Config>::BlockNumber: From<u64>,
		<T as frame_system::Config>::Event: From<pallet::Event<T>>
	}

	register_participant {
	let cid = create_community::<T>();

	let zoran = sr25519::Pair::from_entropy(&[9u8; 32], None).0;
	let proof = fake_last_attendance_and_get_proof::<T>(&zoran, cid);
	let cindex = encointer_scheduler::Pallet::<T>::current_ceremony_index();

	assert_eq!(ReputableCount::<T>::get((cid, cindex)), 0);
	}: _(RawOrigin::Signed(account_id::<T>(&zoran)), cid, Some(proof))
	verify {
		assert_eq!(ReputableCount::<T>::get((cid, cindex)), 1);
		assert_eq!(
			Pallet::<T>::participant_reputation((cid, cindex - 1), account_id::<T>(&zoran)),
			Reputation::VerifiedLinked
		);
	}

	attest_claims {
		let cid = create_community::<T>();

		let attestor = sr25519::Pair::from_entropy(&[10u8; 32], None).0;
		assert_ok!(Pallet::<T>::register_participant(RawOrigin::Signed(account_id::<T>(&attestor.clone())).into(), cid, Some(fake_last_attendance_and_get_proof::<T>(&attestor.clone(), cid))));

		let attestees =  register_users::<T>(cid, 2, 7);

		run_to_next_phase::<T>();
		run_to_next_phase::<T>();


		let cindex = encointer_scheduler::Pallet::<T>::current_ceremony_index();
		let loc = Location { lat: Degree::from_num(1i32), lon: Degree::from_num(1i32) };
		let time = correct_meetup_time::<T>(cindex, loc);
		let mindex = 1;


		let claims = get_all_claims::<T>(&attestees, cid, cindex, mindex, loc,time, 10);
		assert_eq!(AttestationCount::<T>::get((cid, cindex)), 0);
	}: _(RawOrigin::Signed(account_id::<T>(&attestor)), claims)
	verify {
		assert_eq!(AttestationCount::<T>::get((cid, cindex)), 1);
	}

	endorse_newcomer {
		let cid = create_community::<T>();
		let alice: T::AccountId = account("alice", 1, 1);

		// issue some income such that nebies are allowed to register
		assert_ok!(encointer_balances::Pallet::<T>::issue(
			cid,
			&alice,
			NominalIncome::from_num(1)
		));

		let newbie = sr25519::Pair::from_entropy(&[10u8; 32], None).0;
		assert_ok!(Pallet::<T>::register_participant(RawOrigin::Signed(account_id::<T>(&newbie.clone())).into(), cid, None));
		let cindex = encointer_scheduler::Pallet::<T>::current_ceremony_index();

		assert_eq!(<EndorseesCount<T>>::get((cid, cindex)), 0);
		}: _(RawOrigin::Signed(alice), cid, account_id::<T>(&newbie))
	verify {
		assert_eq!(<EndorseesCount<T>>::get((cid, cindex)), 1);
	}

	claim_rewards {
		frame_system::Pallet::<T>::set_block_number(frame_system::Pallet::<T>::block_number() + T::BlockNumber::from(1u64)); // this is needed to assert events
		let cid = create_community::<T>();


		let users = register_users::<T>(cid, 2, 8);

		run_to_next_phase::<T>();
		run_to_next_phase::<T>();


		let cindex = encointer_scheduler::Pallet::<T>::current_ceremony_index();
		let loc = Location { lat: Degree::from_num(1i32), lon: Degree::from_num(1i32) };
		let time = correct_meetup_time::<T>(cindex, loc);
		let mindex = 1;

		// attest_claims
		for i in 0..10 {
			let attestor = users[i as usize].clone();
			let mut attestees = users.clone();
			attestees.remove(i as usize);
			let claims = get_all_claims::<T>(&attestees, cid, cindex, mindex, loc, time, 10);
			assert_ok!(Pallet::<T>::attest_claims(RawOrigin::Signed(account_id::<T>(&attestor)).into(), claims));
		}
	//
	run_to_next_phase::<T>();
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
	}: _(RawOrigin::Root, 12u64.into())
	verify {
		assert_eq!(MeetupTimeOffset::<T>::get(), 12u64.into())
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

}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::TestRuntime);
