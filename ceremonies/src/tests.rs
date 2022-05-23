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

use super::*;
use mock::{
	master, new_test_ext, EncointerBalances, EncointerCeremonies, EncointerCommunities,
	EncointerScheduler, Origin, System, TestClaim, TestProofOfAttendance, TestRuntime, Timestamp,
};
use sp_runtime::DispatchError;

use approx::assert_abs_diff_eq;
use encointer_primitives::{
	communities::{CommunityIdentifier, Degree, Location, LossyInto},
	scheduler::{CeremonyIndexType, CeremonyPhaseType},
};
use frame_support::{
	assert_err, assert_ok,
	traits::{OnFinalize, OnInitialize},
};
use itertools::Itertools;
use rstest::*;
use sp_core::{sr25519, Pair, H256, U256};
use sp_runtime::traits::BlakeTwo256;
use std::ops::Rem;
use test_utils::{
	helpers::{
		account_id, assert_dispatch_err, bootstrappers, last_event, register_test_community,
	},
	*,
};

/// Run until a particular block.
fn run_to_block(n: u64) {
	while System::block_number() < n {
		if System::block_number() > 1 {
			System::on_finalize(System::block_number());
		}
		set_timestamp(GENESIS_TIME + BLOCKTIME * n);
		Timestamp::on_finalize(System::block_number());
		System::set_block_number(System::block_number() + 1);
		System::on_initialize(System::block_number());
	}
}

/// Progress blocks until the phase changes
fn run_to_next_phase() {
	let phase = EncointerScheduler::current_phase();
	let mut blocknr = System::block_number();
	while phase == EncointerScheduler::current_phase() {
		blocknr += 1;
		run_to_block(blocknr);
	}
}

pub fn set_timestamp(t: u64) {
	let _ = pallet_timestamp::Pallet::<TestRuntime>::set(Origin::none(), t);
}

/// get correct meetup time for a certain cid and meetup
fn correct_meetup_time(cid: &CommunityIdentifier, mindex: MeetupIndexType) -> Moment {
	//assert_eq!(EncointerScheduler::current_phase(), CeremonyPhaseType::Attesting);
	let cindex = EncointerScheduler::current_ceremony_index() as u64;
	let mlon: f64 = EncointerCeremonies::get_meetup_location((*cid, cindex as u32), mindex)
		.unwrap()
		.lon
		.lossy_into();

	let t = GENESIS_TIME - GENESIS_TIME.rem(ONE_DAY) +
		cindex * EncointerScheduler::phase_durations(CeremonyPhaseType::Registering) +
		cindex * EncointerScheduler::phase_durations(CeremonyPhaseType::Assigning) +
		(cindex - 1) * EncointerScheduler::phase_durations(CeremonyPhaseType::Attesting) +
		ONE_DAY / 2 -
		(mlon / 360.0 * ONE_DAY as f64) as u64;

	let time = t as i64 + EncointerCeremonies::meetup_time_offset() as i64;
	Moment::from(time as u64)
}

fn signed_claim(
	claimant: &sr25519::Pair,
	cid: CommunityIdentifier,
	cindex: CeremonyIndexType,
	mindex: MeetupIndexType,
	location: Location,
	timestamp: Moment,
	participant_count: u32,
) -> TestClaim {
	TestClaim::new_unsigned(
		claimant.public().into(),
		cindex,
		cid,
		mindex,
		location,
		timestamp,
		participant_count,
	)
	.sign(claimant)
}

fn get_proof(
	cid: CommunityIdentifier,
	cindex: CeremonyIndexType,
	pair: &sr25519::Pair,
) -> Option<TestProofOfAttendance> {
	match EncointerCeremonies::participant_reputation((cid, cindex), account_id(pair)) {
		Reputation::VerifiedUnlinked =>
			Some(prove_attendance(account_id(&pair), cid, cindex, pair)),
		_ => None,
	}
}

/// generate a proof of attendance based on previous reputation
fn prove_attendance(
	prover: AccountId,
	cid: CommunityIdentifier,
	cindex: CeremonyIndexType,
	attendee: &sr25519::Pair,
) -> TestProofOfAttendance {
	let msg = (prover.clone(), cindex);
	ProofOfAttendance {
		prover_public: prover,
		community_identifier: cid,
		ceremony_index: cindex,
		attendee_public: account_id(&attendee),
		attendee_signature: Signature::from(attendee.sign(&msg.encode())),
	}
}

/// Wrapper for EncointerCeremonies::register_participant that reduces boilerplate code.
fn register(
	account: AccountId,
	cid: CommunityIdentifier,
	proof: Option<TestProofOfAttendance>,
) -> DispatchResultWithPostInfo {
	EncointerCeremonies::register_participant(Origin::signed(account), cid, proof)
}

/// shortcut to register well-known keys for current ceremony
fn register_alice_bob_ferdie(cid: CommunityIdentifier) {
	assert_ok!(register(account_id(&AccountKeyring::Alice.pair()), cid, None));
	assert_ok!(register(account_id(&AccountKeyring::Bob.pair()), cid, None));
	assert_ok!(register(account_id(&AccountKeyring::Ferdie.pair()), cid, None));
}

/// shortcut to register well-known keys for current ceremony
fn register_charlie_dave_eve(cid: CommunityIdentifier) {
	assert_ok!(register(account_id(&AccountKeyring::Charlie.pair()), cid, None));
	assert_ok!(register(account_id(&AccountKeyring::Dave.pair()), cid, None));
	assert_ok!(register(account_id(&AccountKeyring::Eve.pair()), cid, None));
}

/// Creates new key pairs. It implicitly assumes that the i-th key was created with entropy = i.
fn add_population(amount: usize, current_popuplation_size: usize) -> Vec<sr25519::Pair> {
	let mut participants = Vec::with_capacity(amount);
	for population_counter in 1..=amount {
		let entropy = U256::from(current_popuplation_size + population_counter);
		participants.push(sr25519::Pair::from_entropy(&entropy.encode()[..], None).0);
	}
	participants
}

/// shorthand for generating multiple identical signed claims of the attestees
fn attest_all(
	attestor: AccountId,
	attestees: &Vec<&sr25519::Pair>,
	cid: CommunityIdentifier,
	cindex: CeremonyIndexType,
	mindex: MeetupIndexType,
	location: Location,
	timestamp: Moment,
	n_participants: u32,
) {
	let mut claims: Vec<TestClaim> = vec![];
	for a in attestees {
		claims.push(
			TestClaim::new_unsigned(
				a.public().into(),
				cindex,
				cid,
				mindex,
				location,
				timestamp,
				n_participants,
			)
			.sign(*a),
		);
	}
	assert_ok!(EncointerCeremonies::attest_claims(Origin::signed(attestor), claims));
}

fn attest(attestor: AccountId, claims: Vec<TestClaim>) {
	assert_ok!(EncointerCeremonies::attest_claims(Origin::signed(attestor), claims));
}

fn create_locations(n_locations: u32) -> Vec<Location> {
	(1..n_locations)
		.map(|i| i as f64)
		.map(|i| Degree::from_num(i))
		.map(|d| Location::new(d, d))
		.collect()
}

/// perform bootstrapping ceremony for test community with either the supplied bootstrappers or the default bootstrappers
fn perform_bootstrapping_ceremony(
	custom_bootstrappers: Option<Vec<sr25519::Pair>>,
	n_locations: u32,
) -> CommunityIdentifier {
	let bootstrappers: Vec<sr25519::Pair> = custom_bootstrappers.unwrap_or_else(|| bootstrappers());
	let cid = register_test_community::<TestRuntime>(Some(bootstrappers.clone()), 0.0, 0.0);
	if n_locations > 70 {
		panic!("Too many locations.")
	}

	for location in create_locations(n_locations) {
		EncointerCommunities::add_location(
			Origin::signed(bootstrappers[0].public().into()),
			cid,
			location,
		)
		.unwrap();
	}
	bootstrappers.iter().for_each(|b| {
		let _ = register(b.public().into(), cid, None).unwrap();
	});

	let cindex = EncointerScheduler::current_ceremony_index();

	run_to_next_phase();
	// Assigning
	run_to_next_phase();
	// Attesting
	let loc = EncointerCeremonies::get_meetup_location((cid, cindex as u32), 1).unwrap();
	let time = correct_meetup_time(&cid, 1);

	for i in 0..bootstrappers.len() {
		let mut bs = bootstrappers.clone();
		let claimant = bs.remove(i);
		attest_all(account_id(&claimant), &bs.iter().collect(), cid, cindex, 1, loc, time, 6);
	}
	run_to_next_phase();
	// Registering
	cid
}

/// perform full attestation of all participants for a given meetup
fn fully_attest_meetup(
	cid: CommunityIdentifier,
	keys: Vec<sr25519::Pair>,
	mindex: MeetupIndexType,
) {
	let cindex = EncointerScheduler::current_ceremony_index();
	let meetup_participants =
		EncointerCeremonies::get_meetup_participants((cid, cindex), mindex).unwrap();
	for p in meetup_participants.iter() {
		let mut others = Vec::with_capacity(meetup_participants.len() - 1);
		for o in meetup_participants.iter() {
			if o == p {
				continue
			}
			for pair in keys.iter() {
				if account_id(pair) == *o {
					others.push(pair.clone());
				}
			}
		}
		let loc = EncointerCeremonies::get_meetup_location((cid, cindex as u32), mindex).unwrap();
		let time = correct_meetup_time(&cid, mindex);

		attest_all(
			(*p).clone(),
			&others.iter().collect(),
			cid,
			cindex,
			mindex,
			loc,
			time,
			meetup_participants.len() as u32,
		);
	}
}

// unit tests ////////////////////////////////////////

#[test]
fn registering_participant_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountId::from(AccountKeyring::Alice);
		let bob = AccountId::from(AccountKeyring::Bob);
		let cindex = EncointerScheduler::current_ceremony_index();
		assert!(EncointerBalances::issue(cid, &alice, NominalIncome::from_num(1)).is_ok());

		assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 0);

		assert_ok!(register(alice.clone(), cid, None));

		assert_eq!(
			last_event::<TestRuntime>(),
			Some(
				Event::ParticipantRegistered(cid, ParticipantType::Bootstrapper, alice.clone())
					.into()
			)
		);

		assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 1);
		assert_ok!(register(bob.clone(), cid, None));

		assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 2);

		assert_eq!(EncointerCeremonies::bootstrapper_index((cid, cindex), &bob), 2);
		assert_eq!(EncointerCeremonies::bootstrapper_registry((cid, cindex), &1).unwrap(), alice);
		assert_eq!(EncointerCeremonies::bootstrapper_registry((cid, cindex), &2).unwrap(), bob);

		let newbies = add_population(2, 2);
		let newbie_1 = account_id(&newbies[0]);
		let newbie_2 = account_id(&newbies[01]);
		IssuedRewards::<TestRuntime>::insert((cid, cindex - 1), 0, ());
		assert_ok!(register(newbie_1.clone(), cid, None));
		assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 1);

		assert_ok!(register(newbie_2.clone(), cid, None));
		assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 2);
		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &newbie_1), 1);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), &1).unwrap(), newbie_1);

		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &newbie_2), 2);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), &2).unwrap(), newbie_2);

		let newbies = add_population(2, 4);
		let endorsee_1 = account_id(&newbies[0]);
		let endorsee_2 = account_id(&newbies[1]);
		assert_ok!(EncointerCeremonies::endorse_newcomer(
			Origin::signed(alice.clone()),
			cid,
			endorsee_1.clone()
		));

		assert_ok!(EncointerCeremonies::endorse_newcomer(
			Origin::signed(alice.clone()),
			cid,
			endorsee_2.clone()
		));

		assert_ok!(register(endorsee_1.clone(), cid, None));
		assert_eq!(EncointerCeremonies::endorsee_count((cid, cindex)), 1);

		assert_ok!(register(endorsee_2.clone(), cid, None));
		assert_eq!(EncointerCeremonies::endorsee_count((cid, cindex)), 2);

		assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 2);
		assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 2);

		assert_eq!(EncointerCeremonies::endorsee_index((cid, cindex), &endorsee_1), 1);
		assert_eq!(EncointerCeremonies::endorsee_registry((cid, cindex), &1).unwrap(), endorsee_1);

		assert_eq!(EncointerCeremonies::endorsee_index((cid, cindex), &endorsee_2), 2);
		assert_eq!(EncointerCeremonies::endorsee_registry((cid, cindex), &2).unwrap(), endorsee_2);

		// Registering Reputables is tested in grow_population_works.
	});
}

#[test]
fn registering_participant_twice_fails() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountId::from(AccountKeyring::Alice);
		assert_ok!(register(alice.clone(), cid, None));
		assert!(register(alice.clone(), cid, None).is_err());
	});
}

#[test]
fn registering_participant_in_wrong_phase_fails() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountId::from(AccountKeyring::Alice);
		run_to_next_phase();
		assert_eq!(EncointerScheduler::current_phase(), CeremonyPhaseType::Assigning);
		assert!(register(alice.clone(), cid, None).is_err());
	});
}

#[test]
fn attest_claims_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.pair();
		let bob = AccountKeyring::Bob.pair();
		let ferdie = AccountKeyring::Ferdie.pair();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		run_to_next_phase();
		run_to_next_phase();
		// Attesting
		assert_eq!(
			EncointerCeremonies::get_meetup_index((cid, cindex), &account_id(&alice)).unwrap(),
			1
		);
		let loc = Location::default();
		let time = correct_meetup_time(&cid, 1);
		attest_all(account_id(&alice), &vec![&bob, &ferdie], cid, 1, 1, loc, time, 3);
		assert_eq!(
			last_event::<TestRuntime>(),
			Some(Event::AttestationsRegistered(cid, 1, 2, alice.public().into()).into())
		);
		attest_all(account_id(&bob), &vec![&alice, &ferdie], cid, 1, 1, loc, time, 3);

		assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 2);
		assert_eq!(EncointerCeremonies::attestation_index((cid, cindex), &account_id(&bob)), 2);
		let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &2).unwrap();
		assert!(wit_vec.len() == 2);
		assert!(wit_vec.contains(&account_id(&alice)));
		assert!(wit_vec.contains(&account_id(&ferdie)));

		// TEST: re-registering must overwrite previous entry
		attest_all(account_id(&alice), &vec![&bob, &ferdie], cid, 1, 1, loc, time, 3);
		assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 2);
	});
}

#[test]
fn attest_claims_for_non_participant_fails_silently() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.pair();
		let bob = AccountKeyring::Bob.pair();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		run_to_next_phase();
		run_to_next_phase();
		// Attesting

		attest_all(
			account_id(&alice),
			&vec![&bob, &alice],
			cid,
			1,
			1,
			Location::default(),
			correct_meetup_time(&cid, 1),
			3,
		);
		assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 1);
		let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1).unwrap();
		assert!(wit_vec.contains(&account_id(&alice)) == false);
		assert!(wit_vec.len() == 1);
	});
}

#[test]
fn attest_claims_for_non_participant_fails() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.pair();
		let ferdie = AccountKeyring::Ferdie.pair();
		let eve = AccountKeyring::Eve.pair();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		run_to_next_phase();
		run_to_next_phase();
		// Attesting
		let mut eve_claims: Vec<TestClaim> = vec![];
		let loc = Location::default();
		let time = correct_meetup_time(&cid, 1);
		eve_claims.insert(0, signed_claim(&alice, cid, cindex, 1, loc, time, 3));
		eve_claims.insert(1, signed_claim(&ferdie, cid, cindex, 1, loc, time, 3));
		assert!(EncointerCeremonies::attest_claims(
			Origin::signed(account_id(&eve)),
			eve_claims.clone()
		)
		.is_err());
	});
}

#[test]
fn attest_claims_with_non_participant_fails_silently() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.pair();
		let bob = AccountKeyring::Bob.pair();
		let eve = AccountKeyring::Eve.pair();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		run_to_next_phase();
		run_to_next_phase();
		// Attesting
		attest_all(
			account_id(&alice),
			&vec![&bob, &eve],
			cid,
			1,
			1,
			Location::default(),
			correct_meetup_time(&cid, 1),
			3,
		);
		assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 1);
		let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1).unwrap();
		assert!(wit_vec.contains(&account_id(&eve)) == false);
		assert!(wit_vec.len() == 1);
	});
}

#[test]
fn attest_claims_with_wrong_meetup_index_fails() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.pair();
		let bob = AccountKeyring::Bob.pair();
		let ferdie = AccountKeyring::Ferdie.pair();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		run_to_next_phase();
		run_to_next_phase();
		// Attesting
		let loc = Location::default();
		let time = correct_meetup_time(&cid, 1);
		let mut alice_claims: Vec<TestClaim> = vec![];
		alice_claims.push(signed_claim(&bob, cid, 1, 1, loc, time, 3));
		let bogus_claim = signed_claim(
			&ferdie,
			cid,
			1,
			1 + 99,
			// !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
			Location::default(),
			time,
			3,
		);
		alice_claims.push(bogus_claim);
		assert_ok!(EncointerCeremonies::attest_claims(
			Origin::signed(account_id(&alice)),
			alice_claims
		));
		let attestees = EncointerCeremonies::attestation_registry((cid, cindex), &1).unwrap();
		assert!(attestees.contains(&account_id(&ferdie)) == false);
		assert!(attestees.len() == 1);
	});
}

#[test]
fn attest_claims_with_wrong_ceremony_index_fails() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.pair();
		let bob = AccountKeyring::Bob.pair();
		let ferdie = AccountKeyring::Ferdie.pair();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		run_to_next_phase();
		run_to_next_phase();
		// Attesting
		let loc = Location::default();
		let time = correct_meetup_time(&cid, 1);
		let mut alice_attestations: Vec<TestClaim> = vec![];
		alice_attestations.push(signed_claim(&bob, cid, 1, 1, loc, time, 3));
		let bogus_claim = signed_claim(
			&ferdie,
			cid,
			// !!!!!!!!!!!!!!!!!!!!!!!!!!
			99,
			1,
			Location::default(),
			time,
			3,
		);
		alice_attestations.push(bogus_claim);
		assert_ok!(EncointerCeremonies::attest_claims(
			Origin::signed(account_id(&alice)),
			alice_attestations
		));
		let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1).unwrap();
		assert!(wit_vec.contains(&account_id(&ferdie)) == false);
		assert!(wit_vec.len() == 1);
	});
}

#[test]
fn attest_claims_with_wrong_timestamp_fails() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.pair();
		let bob = AccountKeyring::Bob.pair();
		let ferdie = AccountKeyring::Ferdie.pair();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		run_to_next_phase();
		run_to_next_phase();
		// Attesting
		let loc = Location { lon: Degree::from_num(25.9), lat: Degree::from_num(0) };
		// too late!
		let time = correct_meetup_time(&cid, 1) + TIME_TOLERANCE + 1;
		let mut alice_claims: Vec<TestClaim> = vec![];
		alice_claims.push(signed_claim(&bob, cid, 1, 1, loc, time, 3));
		alice_claims.push(signed_claim(&ferdie, cid, 1, 1, loc, time, 3));
		assert!(EncointerCeremonies::attest_claims(
			Origin::signed(account_id(&alice)),
			alice_claims
		)
		.is_err());
		let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1);
		assert!(wit_vec.is_none());
	});
}

#[test]
fn attest_claims_with_wrong_location_fails() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.pair();
		let bob = AccountKeyring::Bob.pair();
		let ferdie = AccountKeyring::Ferdie.pair();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		run_to_next_phase();
		run_to_next_phase();
		// Attesting

		// too far away!
		let mut loc = Location::default();
		loc.lon += Degree::from_num(0.01); // ~1.11km east of meetup location along equator
		let time = correct_meetup_time(&cid, 1);
		let mut alice_claims: Vec<TestClaim> = vec![];
		alice_claims.push(signed_claim(&bob, cid, 1, 1, loc, time, 3));
		alice_claims.push(signed_claim(&ferdie, cid, 1, 1, loc, time, 3));
		assert!(EncointerCeremonies::attest_claims(
			Origin::signed(account_id(&alice)),
			alice_claims
		)
		.is_err());
		let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1);
		assert!(wit_vec.is_none());
	});
}

#[test]
fn claim_rewards_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.pair();
		let bob = AccountKeyring::Bob.pair();
		let charlie = AccountKeyring::Charlie.pair();
		let dave = AccountKeyring::Dave.pair();
		let eve = AccountKeyring::Eve.pair();
		let ferdie = AccountKeyring::Ferdie.pair();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		register_charlie_dave_eve(cid);

		let loc = Location::default();
		Assignments::<TestRuntime>::insert(
			(cid, cindex),
			Assignment {
				bootstrappers_reputables: Default::default(),
				endorsees: Default::default(),
				newbies: Default::default(),
				locations: AssignmentParams { m: 7, s1: 8, s2: 9 },
			},
		);
		let time = correct_meetup_time(&cid, 1);

		let claim_base = TestClaim::new_unsigned(account_id(&alice), cindex, cid, 1, loc, time, 5);

		let claim_alice = claim_base.clone().sign(&alice);
		let claim_bob = claim_base.clone().set_claimant(account_id(&bob)).sign(&bob);
		let claim_charlie = claim_base.clone().set_claimant(account_id(&charlie)).sign(&alice);
		let claim_dave = claim_base
			.clone()
			.set_claimant(account_id(&dave))
			.set_participant_count(6)
			.sign(&dave);
		let claim_eve = claim_base.clone().set_claimant(account_id(&eve)).sign(&eve);
		let claim_ferdie = claim_base.clone().set_claimant(account_id(&ferdie)).sign(&ferdie);

		run_to_next_phase();
		// Assigning
		run_to_next_phase();
		// Attesting
		// Scenario:
		//      ferdie doesn't show up
		//      eve signs no one else
		//      charlie collects bogus signatures
		//      dave signs ferdie and reports wrong number of participants

		// alice attests all others except for ferdie, who doesn't show up
		attest(
			account_id(&alice),
			vec![claim_bob.clone(), claim_charlie.clone(), claim_dave.clone(), claim_eve.clone()],
		);
		// bob attests all others except for ferdie, who doesn't show up
		attest(
			account_id(&bob),
			vec![claim_alice.clone(), claim_charlie.clone(), claim_dave.clone(), claim_eve.clone()],
		);
		// charlie attests all others except for ferdie, who doesn't show up, but he supplies erroneous signatures with the others' claims
		attest(
			account_id(&charlie),
			vec![claim_alice.clone(), claim_bob.clone(), claim_dave.clone(), claim_eve.clone()],
		);
		// dave attests all others plus nonexistent ferdie and reports wrong number
		attest(
			account_id(&dave),
			vec![
				claim_alice.clone(),
				claim_bob.clone(),
				claim_charlie.clone(),
				claim_eve.clone(),
				claim_ferdie.clone(),
			],
		);
		// eve does not attest anybody...
		// ferdie is not here...

		assert_eq!(EncointerBalances::balance(cid, &account_id(&alice)), ZERO);

		run_to_next_phase();
		// Registering
		EncointerCeremonies::claim_rewards(Origin::signed(account_id(&alice)), cid).ok();

		assert_eq!(last_event::<TestRuntime>(), Some(Event::RewardsIssued(cid, 1, 2).into()));

		let result: f64 = EncointerBalances::balance(cid, &account_id(&alice)).lossy_into();
		assert_abs_diff_eq!(
			result,
			EncointerCeremonies::ceremony_reward().lossy_into(),
			epsilon = 1.0e-6
		);

		let result: f64 = EncointerBalances::balance(cid, &account_id(&bob)).lossy_into();
		assert_abs_diff_eq!(
			result,
			EncointerCeremonies::ceremony_reward().lossy_into(),
			epsilon = 1.0e-6
		);

		assert_eq!(EncointerBalances::balance(cid, &account_id(&charlie)), ZERO);
		assert_eq!(EncointerBalances::balance(cid, &account_id(&eve)), ZERO);
		assert_eq!(EncointerBalances::balance(cid, &account_id(&ferdie)), ZERO);

		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex), &account_id(&alice)),
			Reputation::VerifiedUnlinked
		);
		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex), &account_id(&bob)),
			Reputation::VerifiedUnlinked
		);
		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex), &account_id(&charlie)),
			Reputation::Unverified
		);
		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex), &account_id(&eve)),
			Reputation::Unverified
		);
		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex), &account_id(&ferdie)),
			Reputation::Unverified
		);

		// Claiming twice does not work for any of the meetup participants
		assert!(
			EncointerCeremonies::claim_rewards(Origin::signed(account_id(&alice)), cid).is_err()
		);
		assert!(EncointerCeremonies::claim_rewards(Origin::signed(account_id(&bob)), cid).is_err());

		assert!(
			EncointerCeremonies::claim_rewards(Origin::signed(account_id(&charlie)), cid).is_err()
		);

		assert!(EncointerCeremonies::claim_rewards(Origin::signed(account_id(&dave)), cid).is_err());

		assert!(
			EncointerCeremonies::claim_rewards(Origin::signed(account_id(&ferdie)), cid).is_err()
		);
	});
}

#[test]
fn bootstrapping_works() {
	new_test_ext().execute_with(|| {
		let cid = perform_bootstrapping_ceremony(None, 1);
		let alice = AccountKeyring::Alice.pair();
		let bob = AccountKeyring::Bob.pair();
		let charlie = AccountKeyring::Charlie.pair();
		let dave = AccountKeyring::Dave.pair();
		let eve = AccountKeyring::Eve.pair();
		let ferdie = AccountKeyring::Ferdie.pair();

		EncointerCeremonies::claim_rewards(Origin::signed(account_id(&alice)), cid).ok();
		let cindex = EncointerScheduler::current_ceremony_index();

		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex - 1), &account_id(&alice)),
			Reputation::VerifiedUnlinked
		);
		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex - 1), &account_id(&bob)),
			Reputation::VerifiedUnlinked
		);
		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex - 1), &account_id(&charlie)),
			Reputation::VerifiedUnlinked
		);
		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex - 1), &account_id(&dave)),
			Reputation::VerifiedUnlinked
		);
		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex - 1), &account_id(&eve)),
			Reputation::VerifiedUnlinked
		);
		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex - 1), &account_id(&ferdie)),
			Reputation::VerifiedUnlinked
		);
	});
}

#[test]
fn register_with_reputation_works() {
	new_test_ext().execute_with(|| {
		let cid = perform_bootstrapping_ceremony(None, 1);

		// a non-bootstrapper
		let zoran = sr25519::Pair::from_entropy(&[9u8; 32], None).0;
		let zoran_new = sr25519::Pair::from_entropy(&[8u8; 32], None).0;

		// another non-bootstrapper
		let yuri = sr25519::Pair::from_entropy(&[9u8; 32], None).0;

		let cindex = EncointerScheduler::current_ceremony_index();

		// fake reputation registry for first ceremony
		assert!(
			EncointerBalances::issue(cid, &account_id(&zoran), NominalIncome::from_num(1)).is_ok()
		);
		EncointerCeremonies::fake_reputation(
			(cid, cindex - 1),
			&account_id(&zoran),
			Reputation::VerifiedUnlinked,
		);
		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex - 1), account_id(&zoran)),
			Reputation::VerifiedUnlinked
		);

		let cindex = EncointerScheduler::current_ceremony_index();
		println!("cindex {}", cindex);
		// wrong sender of good proof fails
		let proof = prove_attendance(account_id(&zoran_new), cid, cindex - 1, &zoran);
		assert!(register(account_id(&yuri), cid, Some(proof)).is_err());

		// see if Zoran can register with his fresh key
		// for the next ceremony claiming his former attendance
		IssuedRewards::<TestRuntime>::insert((cid, cindex - 1), 0, ());
		let proof = prove_attendance(account_id(&zoran_new), cid, cindex - 1, &zoran);
		assert_ok!(register(account_id(&zoran_new), cid, Some(proof)));
		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex), account_id(&zoran_new)),
			Reputation::UnverifiedReputable
		);
		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex - 1), account_id(&zoran)),
			Reputation::VerifiedLinked
		);

		// double signing (re-using reputation) fails
		let proof_second = prove_attendance(account_id(&yuri), cid, cindex - 1, &zoran);
		assert!(register(account_id(&yuri), cid, Some(proof_second)).is_err());

		// signer without reputation fails
		let proof = prove_attendance(account_id(&yuri), cid, cindex - 1, &yuri);
		assert!(register(account_id(&yuri), cid, Some(proof)).is_err());
	});
}

#[test]
fn endorsing_newbie_works_until_no_more_tickets() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = perform_bootstrapping_ceremony(None, 1);
		let alice = AccountId::from(AccountKeyring::Alice);

		let endorsees = add_population(
			(EncointerCeremonies::endorsement_tickets_per_bootstrapper() + 1) as usize,
			6,
		);
		for i in 0..EncointerCeremonies::endorsement_tickets_per_bootstrapper() {
			assert_ok!(EncointerCeremonies::endorse_newcomer(
				Origin::signed(alice.clone()),
				cid,
				account_id(&endorsees[i as usize])
			));
			assert_eq!(
				last_event::<TestRuntime>(),
				Some(
					Event::EndorsedParticipant(
						cid,
						alice.clone(),
						account_id(&endorsees[i as usize])
					)
					.into()
				)
			);
		}

		assert_err!(
			EncointerCeremonies::endorse_newcomer(
				Origin::signed(alice.clone()),
				cid,
				account_id(
					&endorsees
						[EncointerCeremonies::endorsement_tickets_per_bootstrapper() as usize]
				),
			),
			Error::<TestRuntime>::NoMoreNewbieTickets,
		);
	});
}

#[test]
fn endorsing_newbie_for_second_next_ceremony_works() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountId::from(AccountKeyring::Alice);
		let cindex = EncointerScheduler::current_ceremony_index();
		run_to_next_phase();

		assert_eq!(EncointerScheduler::current_phase(), CeremonyPhaseType::Assigning);
		// a newbie
		let zoran = sr25519::Pair::from_entropy(&[9u8; 32], None).0;
		assert_ok!(EncointerCeremonies::endorse_newcomer(
			Origin::signed(alice.clone()),
			cid,
			account_id(&zoran)
		));
		assert!(Endorsees::<TestRuntime>::contains_key((cid, cindex + 1), &account_id(&zoran)));
	});
}

#[test]
fn endorsing_newbie_twice_fails() {
	new_test_ext().execute_with(|| {
		let cid = perform_bootstrapping_ceremony(None, 1);
		let alice = AccountId::from(AccountKeyring::Alice);
		let cindex = EncointerScheduler::current_ceremony_index();

		// a newbie
		let zoran = sr25519::Pair::from_entropy(&[9u8; 32], None).0;
		assert_ok!(EncointerCeremonies::endorse_newcomer(
			Origin::signed(alice.clone()),
			cid,
			account_id(&zoran)
		));
		assert!(Endorsees::<TestRuntime>::contains_key((cid, cindex), &account_id(&zoran)));
		assert_err!(
			EncointerCeremonies::endorse_newcomer(
				Origin::signed(alice.clone()),
				cid,
				account_id(&zoran),
			),
			Error::<TestRuntime>::AlreadyEndorsed,
		);
	});
}

#[test]
fn endorsing_two_newbies_works() {
	new_test_ext().execute_with(|| {
		let cid = perform_bootstrapping_ceremony(None, 1);
		let alice = AccountId::from(AccountKeyring::Alice);
		let cindex = EncointerScheduler::current_ceremony_index();

		// a newbie
		let yran = sr25519::Pair::from_entropy(&[8u8; 32], None).0;
		let zoran = sr25519::Pair::from_entropy(&[9u8; 32], None).0;
		assert_ok!(EncointerCeremonies::endorse_newcomer(
			Origin::signed(alice.clone()),
			cid,
			account_id(&zoran)
		));
		assert!(Endorsees::<TestRuntime>::contains_key((cid, cindex), &account_id(&zoran)));
		assert_ok!(EncointerCeremonies::endorse_newcomer(
			Origin::signed(alice.clone()),
			cid,
			account_id(&yran)
		));
		assert!(Endorsees::<TestRuntime>::contains_key((cid, cindex), &account_id(&yran)));
	});
}

#[test]
fn endorsing_after_registration_works() {
	new_test_ext().execute_with(|| {
		let cid = perform_bootstrapping_ceremony(None, 1);
		let alice = AccountId::from(AccountKeyring::Alice);
		let cindex = EncointerScheduler::current_ceremony_index();

		// a newbie
		let yran = account_id(&sr25519::Pair::from_entropy(&[8u8; 32], None).0);

		assert!(EncointerBalances::issue(cid, &alice, NominalIncome::from_num(1)).is_ok());
		assert_ok!(EncointerCeremonies::register(cid, cindex, &yran, false));

		assert!(NewbieIndex::<TestRuntime>::contains_key((cid, cindex), &yran));

		assert_ok!(EncointerCeremonies::endorse_newcomer(
			Origin::signed(alice.clone()),
			cid,
			yran.clone()
		));

		assert!(EndorseeIndex::<TestRuntime>::contains_key((cid, cindex), &yran));
		assert!(!NewbieIndex::<TestRuntime>::contains_key((cid, cindex), &yran));
	});
}

#[test]
fn registering_in_attestation_phase_works() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let yran = account_id(&sr25519::Pair::from_entropy(&[8u8; 32], None).0);
		let cindex = EncointerScheduler::current_ceremony_index();
		assert!(EncointerBalances::issue(cid, &yran, NominalIncome::from_num(1)).is_ok());

		run_to_next_phase();
		run_to_next_phase();
		register(yran.clone(), cid, None);

		assert!(NewbieIndex::<TestRuntime>::contains_key((cid, cindex + 1), &yran));
	});
}

#[test]
fn registering_in_assigning_phase_fails() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let yran = account_id(&sr25519::Pair::from_entropy(&[8u8; 32], None).0);
		assert!(EncointerBalances::issue(cid, &yran, NominalIncome::from_num(1)).is_ok());

		run_to_next_phase();

		assert_err!(
			register(yran.clone(), cid, None),
			Error::<TestRuntime>::RegisteringOrAttestationPhaseRequired,
		);
	});
}

// integration tests ////////////////////////////////

#[rstest(lat_micro, lon_micro, meetup_time_offset,
case(0, 0, 0),
case(1_000_000, 1_000_000, 0),
case(0, 2_234_567, 0),
case(2_000_000, 155_000_000, 0),
case(1_000_000, -2_000_000, 0),
case(-31_000_000, -155_000_000, 0),
case(0, 0, 100_000),
case(1_000_000, 1_000_000, 100_000),
case(0, 2_234_567, 100_000),
case(2_000_000, 155_000_000, 100_000),
case(1_000_000, -2_000_000, 100_000),
case(-31_000_000, -155_000_000, 100_000),
case(1_000_000, 1_000_000, -100_000),
case(0, 2_234_567, -100_000),
case(2_000_000, 155_000_000, -100_000),
case(1_000_000, -2_000_000, -100_000),
case(-31_000_000, -155_000_000, -100_000),
)]
fn get_meetup_time_works(lat_micro: i64, lon_micro: i64, meetup_time_offset: i64) {
	new_test_ext().execute_with(|| {
		System::set_block_number(0);
		run_to_block(1);

		let cid = register_test_community::<TestRuntime>(
			None,
			lat_micro as f64 / 1_000_000.0,
			lon_micro as f64 / 1_000_000.0,
		);
		// locations will not generally be returned in the order they were registered
		// and meetups will be at randomized locations after https://github.com/encointer/pallets/issues/65
		// that would break this test if we had more than one location registered

		let cindex = EncointerScheduler::current_ceremony_index();
		assert_eq!(cindex, 1);
		assert_eq!(EncointerScheduler::current_phase(), CeremonyPhaseType::Registering);
		assert_eq!(
			EncointerScheduler::next_phase_timestamp(),
			(GENESIS_TIME - GENESIS_TIME.rem(ONE_DAY)) + ONE_DAY
		);
		register_alice_bob_ferdie(cid);

		EncointerCeremonies::set_meetup_time_offset(
			Origin::signed(master()),
			meetup_time_offset as i32,
		)
		.ok();

		run_to_next_phase();

		assert_eq!(cindex, 1);
		assert_eq!(EncointerScheduler::current_phase(), CeremonyPhaseType::Assigning);

		run_to_next_phase();

		assert_eq!(cindex, 1);
		assert_eq!(EncointerScheduler::current_phase(), CeremonyPhaseType::Attesting);

		let mtime = if lon_micro >= 0 {
			GENESIS_TIME - GENESIS_TIME.rem(ONE_DAY) + 2 * ONE_DAY + ONE_DAY / 2 -
				(lon_micro * ONE_DAY as i64 / 360_000_000) as u64
		} else {
			GENESIS_TIME - GENESIS_TIME.rem(ONE_DAY) +
				2 * ONE_DAY + ONE_DAY / 2 +
				(lon_micro.abs() * ONE_DAY as i64 / 360_000_000) as u64
		};

		let adjusted_mtime = mtime as i64 + meetup_time_offset as i64;

		let location = EncointerCeremonies::get_meetup_location((cid, cindex), 1).unwrap();

		let tol = 60_000; // [ms]

		println!(
			"difference {:?}",
			EncointerCeremonies::get_meetup_time(location).unwrap() as i64 - adjusted_mtime
		);
		println!("lon before {:?}", lon_micro as f64 / 1_000_000.0);
		assert!(
			tol > (EncointerCeremonies::get_meetup_time(location).unwrap() as i64 - adjusted_mtime)
				.abs() as u64
		);
	});
}

#[test]
fn ceremony_index_and_purging_registry_works() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountId::from(AccountKeyring::Alice);
		let cindex = EncointerScheduler::current_ceremony_index();
		let reputation_lifetime = EncointerCeremonies::reputation_lifetime();

		assert_ok!(register(alice.clone(), cid, None));
		assert_eq!(EncointerCeremonies::bootstrapper_registry((cid, cindex), &1).unwrap(), alice);

		for _ in 0..reputation_lifetime {
			run_to_next_phase();
			run_to_next_phase();
			run_to_next_phase();

			// still not purged
			assert_eq!(
				EncointerCeremonies::bootstrapper_registry((cid, cindex), &1).unwrap(),
				alice
			);
		}

		// only after n=ReputationLifetimes cycles everything should be purged

		run_to_next_phase();
		run_to_next_phase();
		run_to_next_phase();
		// now again registering
		let new_cindex = EncointerScheduler::current_ceremony_index();
		assert_eq!(new_cindex, cindex + reputation_lifetime + 1);
		assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 0);
		assert_eq!(EncointerCeremonies::bootstrapper_registry((cid, cindex), &1), None);
		assert_eq!(EncointerCeremonies::bootstrapper_index((cid, cindex), &alice), 0);
	});
}

#[test]
fn after_inactive_cycle_forbid_non_bootstrapper_registration() {
	new_test_ext().execute_with(|| {
		let cid = CommunityIdentifier::default();
		let mut cindex = 1;

		let bootstrapper = account_id(&AccountKeyring::Alice.pair());
		EncointerCommunities::insert_bootstrappers(cid, vec![bootstrapper.clone()]);
		let reputable = account_id(&AccountKeyring::Bob.pair());
		let newbie = account_id(&AccountKeyring::Eve.pair());

		assert!(EncointerCeremonies::register(cid, cindex, &bootstrapper, false).is_ok());
		assert!(EncointerCeremonies::register(cid, cindex, &reputable, false).is_err());
		assert!(EncointerCeremonies::register(cid, cindex, &newbie, false).is_err());

		assert!(EncointerBalances::issue(cid, &reputable, NominalIncome::from_num(1)).is_ok());
		cindex += 1;

		assert!(EncointerCeremonies::register(cid, cindex, &bootstrapper, false).is_ok());
		assert!(EncointerCeremonies::register(cid, cindex, &reputable, false).is_ok());
		assert!(EncointerCeremonies::register(cid, cindex, &newbie, false).is_ok());
	});
}

#[test]
fn grow_population_works() {
	new_test_ext().execute_with(|| {
		let cid = perform_bootstrapping_ceremony(None, 3);
		let mut participants = bootstrappers();

		// generate many keys and register all of them
		// they will use the same keys per participant throughout to following ceremonies
		participants.extend(add_population(14, participants.len()));
		IssuedRewards::<TestRuntime>::insert(
			(cid, EncointerScheduler::current_ceremony_index() - 1),
			0,
			(),
		);
		participants.iter().for_each(|p| {
			assert!(
				EncointerBalances::issue(cid, &account_id(&p), NominalIncome::from_num(1)).is_ok()
			);
			let _ = register(account_id(&p), cid, None).unwrap();
		});

		let cindex = EncointerScheduler::current_ceremony_index();
		run_to_next_phase();
		// Assigning
		assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 6);
		assert_eq!(EncointerCeremonies::reputable_count((cid, cindex)), 0);
		assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 14);
		assert_eq!(EncointerCeremonies::meetup_count((cid, cindex)), 1);

		run_to_next_phase();
		// WITNESSING

		fully_attest_meetup(cid, participants.clone(), 1);

		run_to_next_phase();
		// Registering
		for pair in participants.iter() {
			EncointerCeremonies::claim_rewards(Origin::signed(account_id(&pair)), cid).ok();
		}

		let cindex = EncointerScheduler::current_ceremony_index();
		// register everybody again. also those who didn't have the chance last time
		for pair in participants.iter() {
			let proof = get_proof(cid, cindex - 1, pair);
			register(account_id(&pair), cid, proof).unwrap();
		}
		run_to_next_phase();
		// Assigning

		assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 6);
		assert_eq!(EncointerCeremonies::reputable_count((cid, cindex)), 2);
		assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 12);
		assert_eq!(EncointerCeremonies::meetup_count((cid, cindex)), 1);

		run_to_next_phase();

		fully_attest_meetup(cid, participants.clone(), 1);

		run_to_next_phase();
		// Registering
		for pair in participants.iter() {
			EncointerCeremonies::claim_rewards(Origin::signed(account_id(&pair)), cid).ok();
		}

		let cindex = EncointerScheduler::current_ceremony_index();
		// register everybody again. also those who didn't have the chance last time
		for pair in participants.iter() {
			let proof = get_proof(cid, cindex - 1, pair);
			register(account_id(&pair), cid, proof).unwrap();
		}
		run_to_next_phase();
		// Assigning

		assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 6);
		assert_eq!(EncointerCeremonies::reputable_count((cid, cindex)), 4);
		assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 10);
		assert_eq!(EncointerCeremonies::meetup_count((cid, cindex)), 2);

		run_to_next_phase();
		// WITNESSING
		fully_attest_meetup(cid, participants.clone(), 1);
		fully_attest_meetup(cid, participants.clone(), 2);

		run_to_next_phase();
		// Registering
		for pair in participants.iter() {
			EncointerCeremonies::claim_rewards(Origin::signed(account_id(&pair)), cid).ok();
		}

		let cindex = EncointerScheduler::current_ceremony_index();
		let mut proof_count = 0;
		for pair in participants.iter() {
			let proof = get_proof(cid, cindex - 1, &pair);
			if proof.is_some() {
				proof_count += 1;
			}
			register(account_id(&pair), cid, proof).unwrap();
		}
		run_to_next_phase();
		// Assigning
		assert_eq!(proof_count, 13);
		assert_eq!(EncointerCeremonies::meetup_count((cid, cindex)), 2);
	});
}

#[test]
fn dont_create_assignment_with_less_than_three() {
	new_test_ext().execute_with(|| {
		let cid = perform_bootstrapping_ceremony(None, 1);
		let cindex = EncointerScheduler::current_ceremony_index();

		assert_ok!(register(account_id(&AccountKeyring::Charlie.pair()), cid, None));
		assert_ok!(register(account_id(&AccountKeyring::Dave.pair()), cid, None));
		run_to_next_phase();
		assert_eq!(EncointerCeremonies::assignments((cid, cindex)), Assignment::default());
	});
}

#[test]
fn get_assignment_params_works() {
	new_test_ext().execute_with(|| {
		let cid = perform_bootstrapping_ceremony(None, 1);
		let cindex = EncointerScheduler::current_ceremony_index();
		let assignment = EncointerCeremonies::assignments((cid, cindex));

		assert_eq!(assignment.bootstrappers_reputables.m, 0);
		assert_eq!(assignment.bootstrappers_reputables.s1, 0);
		assert_eq!(assignment.bootstrappers_reputables.s2, 0);
		assert_eq!(assignment.endorsees.m, 0);
		assert_eq!(assignment.endorsees.s1, 0);
		assert_eq!(assignment.endorsees.s2, 0);
		assert_eq!(assignment.newbies.m, 0);
		assert_eq!(assignment.newbies.s1, 0);
		assert_eq!(assignment.newbies.s2, 0);

		register_charlie_dave_eve(cid);
		run_to_next_phase();

		let assignment = EncointerCeremonies::assignments((cid, cindex));

		assert!(assignment.bootstrappers_reputables.m > 0);
		assert!(assignment.bootstrappers_reputables.s1 > 0);
		assert!(assignment.bootstrappers_reputables.s2 > 0);
		assert!(assignment.endorsees.m > 0);
		assert!(assignment.endorsees.s1 > 0);
		assert!(assignment.endorsees.s2 > 0);
		assert!(assignment.newbies.m > 0);
		assert!(assignment.newbies.s1 > 0);
		assert!(assignment.newbies.s2 > 0);
	});
}

#[test]
fn get_inactive_communities_works() {
	new_test_ext().execute_with(|| {
		let cid0 = CommunityIdentifier::default();
		let cid1 = CommunityIdentifier::new(
			Location::new(Degree::from_num(1f64), Degree::from_num(1f64)),
			Vec::<i64>::new(),
		)
		.unwrap();

		let mut cindex = 5;

		IssuedRewards::<TestRuntime>::insert((cid0, cindex), 0, ());
		IssuedRewards::<TestRuntime>::insert((cid1, cindex), 0, ());

		let timeout = 1;
		assert_eq!(
			EncointerCeremonies::get_inactive_communities(cindex, timeout, vec![cid0, cid1]),
			vec![]
		);

		cindex += 1;
		IssuedRewards::<TestRuntime>::insert((cid0, cindex), 0, ());
		assert_eq!(
			EncointerCeremonies::get_inactive_communities(cindex, timeout, vec![cid0, cid1]),
			vec![]
		);

		cindex += 1;
		assert_eq!(
			EncointerCeremonies::get_inactive_communities(cindex, timeout, vec![cid0, cid1]),
			vec![cid1]
		);

		cindex += 1;
		assert_eq!(
			EncointerCeremonies::get_inactive_communities(cindex, timeout, vec![cid0, cid1]),
			vec![cid0, cid1]
		);
	});
}

#[test]
fn get_meetup_index_works() {
	new_test_ext().execute_with(|| {
		let cid = perform_bootstrapping_ceremony(None, 1);
		let cindex = EncointerScheduler::current_ceremony_index();

		let participants = add_population(4, 0);
		let p1 = account_id(&participants[0]);
		let p2 = account_id(&participants[1]);
		let p3 = account_id(&participants[2]);
		let p4 = account_id(&participants[3]);

		MeetupCount::<TestRuntime>::insert((cid, cindex), 10);

		BootstrapperIndex::<TestRuntime>::insert((cid, cindex), p1.clone(), 1);
		AssignmentCounts::<TestRuntime>::insert(
			(cid, cindex),
			AssignmentCount { bootstrappers: 1, reputables: 0, endorsees: 0, newbies: 0 },
		);

		ReputableIndex::<TestRuntime>::insert((cid, cindex), p2.clone(), 1);

		EndorseeIndex::<TestRuntime>::insert((cid, cindex), p3.clone(), 3);
		NewbieIndex::<TestRuntime>::insert((cid, cindex), p4.clone(), 4);

		Assignments::<TestRuntime>::insert(
			(cid, cindex),
			Assignment {
				bootstrappers_reputables: AssignmentParams { m: 2, s1: 1, s2: 1 },
				endorsees: AssignmentParams { m: 5, s1: 2, s2: 3 },
				newbies: AssignmentParams { m: 5, s1: 2, s2: 3 },
				locations: AssignmentParams { m: 9, s1: 8, s2: 7 },
			},
		);

		assert_eq!(EncointerCeremonies::get_meetup_index((cid, cindex), &p1).unwrap(), 2);
		assert_eq!(EncointerCeremonies::get_meetup_index((cid, cindex), &p2), None);
		assert_eq!(EncointerCeremonies::get_meetup_index((cid, cindex), &p3), None);
		assert_eq!(EncointerCeremonies::get_meetup_index((cid, cindex), &p4), None);
	});
}

#[test]
fn get_meetup_location_works() {
	new_test_ext().execute_with(|| {
		let ceremony = (perform_bootstrapping_ceremony(None, 50), 100);

		Assignments::<TestRuntime>::insert(
			ceremony,
			Assignment {
				bootstrappers_reputables: AssignmentParams { m: 5, s1: 2, s2: 3 },
				endorsees: AssignmentParams { m: 3, s1: 2, s2: 1 },
				newbies: AssignmentParams { m: 3, s1: 1, s2: 2 },
				locations: AssignmentParams { m: 9, s1: 8, s2: 7 },
			},
		);

		let locations: Vec<Option<Location>> = (0..9)
			.map(|meetup_index| EncointerCeremonies::get_meetup_location(ceremony, meetup_index))
			.collect();

		assert!(locations.iter().all(|l| l.is_some()));
		assert!(locations
			.iter()
			.map(|o| o.unwrap())
			.combinations(2)
			.all(|v| v.get(0) != v.get(1)));
	});
}

#[test]
fn meetup_with_only_one_newbie_works() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);
		let cindex = EncointerScheduler::current_ceremony_index();

		let bootstrapper = account_id(&AccountKeyring::Alice.pair());
		let bootstrapper2 = account_id(&AccountKeyring::Bob.pair());
		EncointerCommunities::insert_bootstrappers(
			cid,
			vec![bootstrapper.clone(), bootstrapper2.clone()],
		);

		let reputable = account_id(&AccountKeyring::Ferdie.pair());
		let reputable2 = account_id(&AccountKeyring::Charlie.pair());

		let newbie = account_id(&AccountKeyring::Eve.pair());

		assert!(EncointerBalances::issue(cid, &reputable, NominalIncome::from_num(1)).is_ok());

		assert_ok!(EncointerCeremonies::register(cid, cindex, &bootstrapper, false));
		assert_ok!(EncointerCeremonies::register(cid, cindex, &bootstrapper2, false));

		assert_ok!(EncointerCeremonies::register(cid, cindex, &reputable, true));
		assert_ok!(EncointerCeremonies::register(cid, cindex, &reputable2, true));
		assert_ok!(EncointerCeremonies::register(cid, cindex, &newbie, false));

		run_to_next_phase();

		let mut participants =
			EncointerCeremonies::get_meetup_participants((cid, cindex), 1).unwrap();
		let mut expected_participants =
			[bootstrapper, bootstrapper2, reputable, reputable2, newbie];
		expected_participants.sort();
		participants.sort();

		assert_eq!(participants, expected_participants);
	});
}

#[test]
fn get_meetup_participants_works() {
	new_test_ext().execute_with(|| {
		let cid = perform_bootstrapping_ceremony(None, 1);
		let cindex = EncointerScheduler::current_ceremony_index();

		let participants: Vec<AccountId> =
			add_population(12, 0).iter().map(|b| account_id(&b)).collect();

		BootstrapperRegistry::<TestRuntime>::insert((cid, cindex), 1, participants[0].clone());
		BootstrapperRegistry::<TestRuntime>::insert((cid, cindex), 2, participants[1].clone());
		BootstrapperRegistry::<TestRuntime>::insert((cid, cindex), 3, participants[2].clone());

		ReputableRegistry::<TestRuntime>::insert((cid, cindex), 1, participants[3].clone());
		ReputableRegistry::<TestRuntime>::insert((cid, cindex), 2, participants[4].clone());
		ReputableRegistry::<TestRuntime>::insert((cid, cindex), 3, participants[5].clone());

		EndorseeRegistry::<TestRuntime>::insert((cid, cindex), 1, participants[6].clone());
		EndorseeRegistry::<TestRuntime>::insert((cid, cindex), 2, participants[7].clone());
		EndorseeRegistry::<TestRuntime>::insert((cid, cindex), 3, participants[8].clone());

		NewbieRegistry::<TestRuntime>::insert((cid, cindex), 1, participants[9].clone());
		NewbieRegistry::<TestRuntime>::insert((cid, cindex), 2, participants[10].clone());
		NewbieRegistry::<TestRuntime>::insert((cid, cindex), 3, participants[11].clone());
		AssignmentCounts::<TestRuntime>::insert(
			(cid, cindex),
			AssignmentCount { bootstrappers: 3, reputables: 3, endorsees: 3, newbies: 3 },
		);

		MeetupCount::<TestRuntime>::insert((cid, cindex), 2);

		Assignments::<TestRuntime>::insert(
			(cid, cindex),
			Assignment {
				bootstrappers_reputables: AssignmentParams { m: 5, s1: 2, s2: 3 },
				endorsees: AssignmentParams { m: 3, s1: 2, s2: 1 },
				newbies: AssignmentParams { m: 3, s1: 1, s2: 2 },
				locations: AssignmentParams { m: 9, s1: 8, s2: 7 },
			},
		);

		let mut m0_expected_participants = [
			participants[1].clone(),
			participants[2].clone(),
			participants[3].clone(),
			participants[7].clone(),
			participants[8].clone(),
			participants[9].clone(),
			participants[10].clone(),
		];
		let mut m1_expected_participants = [
			participants[0].clone(),
			participants[4].clone(),
			participants[5].clone(),
			participants[6].clone(),
			participants[11].clone(),
		];
		let mut m0_participants =
			EncointerCeremonies::get_meetup_participants((cid, cindex), 1).unwrap();
		let mut m1_participants =
			EncointerCeremonies::get_meetup_participants((cid, cindex), 2).unwrap();

		m0_expected_participants.sort();
		m1_expected_participants.sort();
		m0_participants.sort();
		m1_participants.sort();

		assert_eq!(m0_participants, m0_expected_participants);
		assert_eq!(m1_participants, m1_expected_participants);
	});
}

#[rstest(
	n_locations,
	n_bootstrappers,
	n_reputables,
	n_endorsees,
	n_newbies,
	exp_m_bootstrappers_reputables,
	exp_m_endorsees,
	exp_m_newbies,
	exp_n_assigned_bootstrappers,
	exp_n_assigned_reputables,
	exp_n_assigned_endorsees,
	exp_n_assigned_newbies,
	case(3, 7, 12, 6, 13, 19, 5, 5, 7, 12, 6, 5),
	case(10, 1, 1, 20, 13, 2, 17, 2, 1, 1, 18, 0)
)]
fn generate_meetup_assignment_params_works(
	n_locations: u64,
	n_bootstrappers: u64,
	n_reputables: u64,
	n_endorsees: u64,
	n_newbies: u64,
	exp_m_bootstrappers_reputables: u64,
	exp_m_endorsees: u64,
	exp_m_newbies: u64,
	exp_n_assigned_bootstrappers: u64,
	exp_n_assigned_reputables: u64,
	exp_n_assigned_endorsees: u64,
	exp_n_assigned_newbies: u64,
) {
	new_test_ext().execute_with(|| {
		let cid = perform_bootstrapping_ceremony(None, n_locations as u32);
		let cindex = EncointerScheduler::current_ceremony_index();
		BootstrapperCount::<TestRuntime>::insert((cid, cindex), n_bootstrappers);
		ReputableCount::<TestRuntime>::insert((cid, cindex), n_reputables);
		EndorseeCount::<TestRuntime>::insert((cid, cindex), n_endorsees);
		NewbieCount::<TestRuntime>::insert((cid, cindex), n_newbies);

		let mut random_source = RandomNumberGenerator::<BlakeTwo256>::new(H256::random());

		EncointerCeremonies::generate_meetup_assignment_params((cid, cindex), &mut random_source)
			.ok();
		let assigned_counts = EncointerCeremonies::assignment_counts((cid, cindex));

		assert_eq!(assigned_counts.bootstrappers, exp_n_assigned_bootstrappers);
		assert_eq!(assigned_counts.reputables, exp_n_assigned_reputables);
		assert_eq!(assigned_counts.endorsees, exp_n_assigned_endorsees);
		assert_eq!(assigned_counts.newbies, exp_n_assigned_newbies);

		let assignment = EncointerCeremonies::assignments((cid, cindex));

		assert_eq!(assignment.bootstrappers_reputables.m, exp_m_bootstrappers_reputables);
		assert!(assignment.bootstrappers_reputables.s1 > 0);
		assert!(assignment.bootstrappers_reputables.s1 < exp_m_bootstrappers_reputables);
		assert!(assignment.bootstrappers_reputables.s2 > 0);
		assert!(assignment.bootstrappers_reputables.s2 < exp_m_bootstrappers_reputables);

		assert_eq!(assignment.endorsees.m, exp_m_endorsees);
		assert!(assignment.endorsees.s1 > 0);
		assert!(assignment.endorsees.s1 < exp_m_endorsees);
		assert!(assignment.endorsees.s2 > 0);
		assert!(assignment.endorsees.s2 < exp_m_endorsees);

		assert_eq!(assignment.newbies.m, exp_m_newbies);
		assert!(assignment.newbies.s1 > 0);
		assert!(assignment.newbies.s1 < exp_m_newbies);
		assert!(assignment.newbies.s2 > 0);
		assert!(assignment.newbies.s2 < exp_m_newbies);
	});
}

#[test]
fn generate_meetup_assignment_params_is_random() {
	new_test_ext().execute_with(|| {
		let cid = perform_bootstrapping_ceremony(None, 3);

		let cindex = EncointerScheduler::current_ceremony_index();
		BootstrapperCount::<TestRuntime>::insert((cid, cindex), 7);
		ReputableCount::<TestRuntime>::insert((cid, cindex), 12);
		EndorseeCount::<TestRuntime>::insert((cid, cindex), 6);
		NewbieCount::<TestRuntime>::insert((cid, cindex), 13);

		let mut random_source = RandomNumberGenerator::<BlakeTwo256>::new(H256::random());

		EncointerCeremonies::generate_meetup_assignment_params((cid, cindex), &mut random_source)
			.unwrap();

		let a1 = EncointerCeremonies::assignments((cid, cindex));

		// second time should yield a different result
		EncointerCeremonies::generate_meetup_assignment_params((cid, cindex), &mut random_source)
			.unwrap();

		let a2 = EncointerCeremonies::assignments((cid, cindex));

		assert_ne!(a1, a2)
	});
}

#[test]
fn unregistering_newbie_fails_in_wrong_phase() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);
		let cindex = EncointerScheduler::current_ceremony_index();

		let alice = account_id(&AccountKeyring::Alice.pair());
		run_to_next_phase();
		assert!(EncointerCeremonies::unregister_newbie(cid, cindex, &alice).is_err());
	});
}

#[test]
fn unregistering_newbie_works() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cindex = EncointerScheduler::current_ceremony_index();

		IssuedRewards::<TestRuntime>::insert((cid, cindex - 1), 0, ());
		let bootstrapper = account_id(&AccountKeyring::Ferdie.pair());
		EncointerCommunities::insert_bootstrappers(cid, vec![bootstrapper.clone()]);

		let alice = account_id(&AccountKeyring::Alice.pair());
		let bob = account_id(&AccountKeyring::Bob.pair());
		let charlie = account_id(&AccountKeyring::Charlie.pair());
		let eve = account_id(&AccountKeyring::Eve.pair());

		assert!(EncointerBalances::issue(cid, &bootstrapper, NominalIncome::from_num(1)).is_ok());

		assert_ok!(EncointerCeremonies::register(cid, cindex, &alice, false));
		assert_ok!(EncointerCeremonies::register(cid, cindex, &bob, false));
		assert_ok!(EncointerCeremonies::register(cid, cindex, &charlie, false));
		assert_ok!(EncointerCeremonies::register(cid, cindex, &eve, false));

		assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 4);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 1).unwrap(), alice);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 2).unwrap(), bob);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 3).unwrap(), charlie);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 4).unwrap(), eve);

		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &alice), 1);
		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &bob), 2);
		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &charlie), 3);
		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &eve), 4);

		assert_ok!(EncointerCeremonies::unregister_newbie(cid, cindex, &bob));

		assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 3);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 1).unwrap(), alice);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 2).unwrap(), eve);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 3).unwrap(), charlie);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 4), None);

		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &alice), 1);
		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &eve), 2);
		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &charlie), 3);
		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &bob), 0);

		assert_ok!(EncointerCeremonies::unregister_newbie(cid, cindex, &charlie));

		assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 2);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 1).unwrap(), alice);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 2).unwrap(), eve);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 3), None);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 4), None);

		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &alice), 1);
		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &eve), 2);
		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &charlie), 0);
		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &bob), 0);
	});
}

#[test]
fn unregistering_newbie_with_no_participants_works() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cindex = EncointerScheduler::current_ceremony_index();

		IssuedRewards::<TestRuntime>::insert((cid, cindex - 1), 0, ());
		let bootstrapper = account_id(&AccountKeyring::Ferdie.pair());
		EncointerCommunities::insert_bootstrappers(cid, vec![bootstrapper.clone()]);

		let alice = account_id(&AccountKeyring::Alice.pair());

		assert_ok!(EncointerCeremonies::unregister_newbie(cid, cindex, &alice));
	});
}

#[test]
fn set_inactivity_timeout_errs_with_bad_origin() {
	new_test_ext().execute_with(|| {
		assert_dispatch_err(
			EncointerCeremonies::set_inactivity_timeout(
				Origin::signed(AccountKeyring::Bob.into()),
				1u32,
			),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn set_inactivity_timeout_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(EncointerCeremonies::set_inactivity_timeout(Origin::signed(master()), 2u32));

		assert_eq!(EncointerCeremonies::inactivity_timeout(), 2u32);
		assert_ok!(EncointerCeremonies::set_inactivity_timeout(Origin::signed(master()), 3u32));

		assert_eq!(EncointerCeremonies::inactivity_timeout(), 3u32);
	});
}

#[test]
fn set_endorsement_tickets_per_bootstrapper_errs_with_bad_origin() {
	new_test_ext().execute_with(|| {
		assert_dispatch_err(
			EncointerCeremonies::set_endorsement_tickets_per_bootstrapper(
				Origin::signed(AccountKeyring::Bob.into()),
				1u8,
			),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn set_endorsement_tickets_per_bootstrapper_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(EncointerCeremonies::set_endorsement_tickets_per_bootstrapper(
			Origin::signed(master()),
			2u8
		));

		assert_eq!(EncointerCeremonies::endorsement_tickets_per_bootstrapper(), 2u8);
		assert_ok!(EncointerCeremonies::set_endorsement_tickets_per_bootstrapper(
			Origin::signed(master()),
			3u8
		));

		assert_eq!(EncointerCeremonies::endorsement_tickets_per_bootstrapper(), 3u8);
	});
}

#[test]
fn set_reputation_lifetime_errs_with_bad_origin() {
	new_test_ext().execute_with(|| {
		assert_dispatch_err(
			EncointerCeremonies::set_reputation_lifetime(
				Origin::signed(AccountKeyring::Bob.into()),
				1u32,
			),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn set_reputation_lifetime_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(EncointerCeremonies::set_reputation_lifetime(Origin::signed(master()), 2u32));

		assert_eq!(EncointerCeremonies::reputation_lifetime(), 2u32);
		assert_ok!(EncointerCeremonies::set_reputation_lifetime(Origin::signed(master()), 3u32));

		assert_eq!(EncointerCeremonies::reputation_lifetime(), 3u32);
	});
}

#[test]
fn set_meetup_time_offset_errs_with_bad_origin() {
	new_test_ext().execute_with(|| {
		assert_dispatch_err(
			EncointerCeremonies::set_meetup_time_offset(
				Origin::signed(AccountKeyring::Bob.into()),
				5i32,
			),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn set_meetup_time_offset_fails_with_invalid_value() {
	new_test_ext().execute_with(|| {
		assert_err!(
			EncointerCeremonies::set_meetup_time_offset(
				Origin::signed(master()),
				-8 * 3600 * 1000 - 1,
			),
			Error::<TestRuntime>::InvalidMeetupTimeOffset,
		);

		assert_err!(
			EncointerCeremonies::set_meetup_time_offset(
				Origin::signed(master()),
				8 * 3600 * 1000 + 1,
			),
			Error::<TestRuntime>::InvalidMeetupTimeOffset,
		);
	});
}

#[test]
fn set_meetup_time_offset_fails_with_wrong_phase() {
	new_test_ext().execute_with(|| {
		run_to_next_phase();
		assert_err!(
			EncointerCeremonies::set_meetup_time_offset(Origin::signed(master()), 5i32,),
			Error::<TestRuntime>::WrongPhaseForChangingMeetupTimeOffset,
		);
	});
}

#[test]
fn set_meetup_time_offset_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(EncointerCeremonies::set_meetup_time_offset(Origin::signed(master()), 5i32,));

		assert_eq!(EncointerCeremonies::meetup_time_offset(), 5i32,);
		assert_ok!(EncointerCeremonies::set_meetup_time_offset(Origin::signed(master()), -6i32,));

		assert_eq!(EncointerCeremonies::meetup_time_offset(), -6i32,);
	});
}

#[test]
fn set_time_tolerance_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(EncointerCeremonies::set_time_tolerance(Origin::signed(master()), 600));
		assert_eq!(EncointerCeremonies::time_tolerance(), 600);
	});
}

#[test]
fn set_location_tolerance_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(EncointerCeremonies::set_location_tolerance(Origin::signed(master()), 1234));
		assert_eq!(EncointerCeremonies::location_tolerance(), 1234);
	});
}

#[test]
fn get_participant_type_works() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let mut cindex = 1;

		let bootstrapper = account_id(&AccountKeyring::Alice.pair());
		EncointerCommunities::insert_bootstrappers(cid, vec![bootstrapper.clone()]);
		let reputable = account_id(&AccountKeyring::Bob.pair());
		let newbie = account_id(&AccountKeyring::Eve.pair());
		let endorsee = account_id(&AccountKeyring::Ferdie.pair());
		let unregistered_user = account_id(&AccountKeyring::Charlie.pair());

		assert!(EncointerBalances::issue(cid, &reputable, NominalIncome::from_num(1)).is_ok());
		cindex += 1;

		run_to_next_phase();
		run_to_next_phase();
		run_to_next_phase();

		assert_ok!(EncointerCeremonies::endorse_newcomer(
			Origin::signed(bootstrapper.clone()),
			cid,
			endorsee.clone()
		));

		assert_ok!(EncointerCeremonies::register(cid, cindex, &bootstrapper, false));
		assert_ok!(EncointerCeremonies::register(cid, cindex, &reputable, true));
		assert_ok!(EncointerCeremonies::register(cid, cindex, &newbie, false));
		assert_ok!(EncointerCeremonies::register(cid, cindex, &endorsee, false));

		assert_eq!(
			EncointerCeremonies::get_participant_type((cid, cindex), &bootstrapper),
			Some(ParticipantType::Bootstrapper)
		);

		assert_eq!(
			EncointerCeremonies::get_participant_type((cid, cindex), &newbie),
			Some(ParticipantType::Newbie)
		);

		assert_eq!(
			EncointerCeremonies::get_participant_type((cid, cindex), &endorsee),
			Some(ParticipantType::Endorsee)
		);

		assert_eq!(
			EncointerCeremonies::get_participant_type((cid, cindex), &reputable),
			Some(ParticipantType::Reputable)
		);

		assert_eq!(
			EncointerCeremonies::get_participant_type((cid, cindex), &unregistered_user),
			None
		);
	});
}

#[test]
fn get_aggregated_account_data_works() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);
		let cindex = EncointerScheduler::current_ceremony_index();

		let bootstrapper = account_id(&AccountKeyring::Alice.pair());
		let bootstrapper2 = account_id(&AccountKeyring::Bob.pair());
		EncointerCommunities::insert_bootstrappers(
			cid,
			vec![bootstrapper.clone(), bootstrapper2.clone()],
		);

		let reputable = account_id(&AccountKeyring::Ferdie.pair());
		let reputable2 = account_id(&AccountKeyring::Charlie.pair());

		assert!(EncointerBalances::issue(cid, &reputable, NominalIncome::from_num(1)).is_ok());

		assert_ok!(EncointerCeremonies::register(cid, cindex, &bootstrapper, false));
		assert_ok!(EncointerCeremonies::register(cid, cindex, &bootstrapper2, false));

		assert_ok!(EncointerCeremonies::register(cid, cindex, &reputable, true));

		let mut aggregated_account_data =
			EncointerCeremonies::get_aggregated_account_data(cid, &bootstrapper);

		assert_eq!(aggregated_account_data.global.ceremony_phase, CeremonyPhaseType::Registering);
		assert_eq!(aggregated_account_data.global.ceremony_index, 1);
		let mut personal = aggregated_account_data.personal.unwrap();
		assert_eq!(personal.participant_type, ParticipantType::Bootstrapper);
		assert_eq!(personal.meetup_index, None);
		assert_eq!(personal.meetup_location_index, None);
		assert_eq!(personal.meetup_time, None);
		assert_eq!(personal.meetup_registry, None);

		aggregated_account_data =
			EncointerCeremonies::get_aggregated_account_data(cid, &reputable2);

		assert_eq!(aggregated_account_data.global.ceremony_phase, CeremonyPhaseType::Registering);
		assert_eq!(aggregated_account_data.global.ceremony_index, 1);

		// reputable2 is not yet registered
		assert_eq!(aggregated_account_data.personal, None);

		assert_ok!(EncointerCeremonies::register(cid, cindex, &reputable2, true));
		aggregated_account_data =
			EncointerCeremonies::get_aggregated_account_data(cid, &reputable2);
		personal = aggregated_account_data.personal.unwrap();
		// Now they are
		assert_eq!(personal.participant_type, ParticipantType::Reputable);

		run_to_next_phase();
		run_to_next_phase();

		// Now the assignment is made and the other fields should also be set
		aggregated_account_data = EncointerCeremonies::get_aggregated_account_data(cid, &reputable);
		assert_eq!(aggregated_account_data.global.ceremony_phase, CeremonyPhaseType::Attesting);
		assert_eq!(aggregated_account_data.global.ceremony_index, 1);

		personal = aggregated_account_data.personal.unwrap();
		assert_eq!(personal.participant_type, ParticipantType::Reputable);
		assert_eq!(personal.meetup_index, Some(1));
		assert_eq!(personal.meetup_location_index, Some(0));

		assert_eq!(personal.meetup_time, Some(correct_meetup_time(&cid, 1)));

		let meetup_registry = personal.meetup_registry.unwrap();
		let expected_meetup_registry = vec![bootstrapper, bootstrapper2, reputable, reputable2];
		assert_eq!(meetup_registry.len(), 4);
		assert!(meetup_registry.iter().all(|item| expected_meetup_registry.contains(item)));
	});
}
