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
use approx::assert_abs_diff_eq;
use encointer_balances::Event as BalancesEvent;
use encointer_primitives::{
	communities::{CommunityIdentifier, Degree, Location, LossyInto},
	scheduler::{CeremonyIndexType, CeremonyPhaseType},
};
use frame_support::{
	assert_err, assert_ok,
	traits::{OnFinalize, OnInitialize},
};
use itertools::Itertools;
use mock::{
	master, new_test_ext, EncointerBalances, EncointerCeremonies, EncointerCommunities,
	EncointerScheduler, Origin, System, TestProofOfAttendance, TestRuntime, Timestamp,
};
use rstest::*;
use sp_core::{sr25519, Pair, H256, U256};
use sp_runtime::{traits::BlakeTwo256, DispatchError};
use std::{ops::Rem, str::FromStr};
use test_utils::{
	helpers::{
		account_id, assert_dispatch_err, bootstrappers, event_at_index, event_deposited,
		get_num_events, last_event, register_test_community,
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

fn make_reputable_and_get_proof(
	p: &sr25519::Pair,
	cid: CommunityIdentifier,
	cindex: CeremonyIndexType,
) -> TestProofOfAttendance {
	EncointerBalances::issue(cid, &account_id(p), NominalIncome::from_num(1)).unwrap();
	EncointerCeremonies::fake_reputation(
		(cid, cindex),
		&account_id(&p),
		Reputation::VerifiedUnlinked,
	);
	let proof = prove_attendance(account_id(p), cid, cindex, p);
	proof
}

fn register_as_reputable(
	p: &sr25519::Pair,
	cid: CommunityIdentifier,
) -> DispatchResultWithPostInfo {
	let proof =
		make_reputable_and_get_proof(p, cid, EncointerScheduler::current_ceremony_index() - 1);
	register(account_id(p), cid, Some(proof))
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

/// Shorthand for attesting all attendees.
fn attest_all(
	attestor: AccountId,
	attestees: Vec<AccountId>,
	cid: CommunityIdentifier,
	n_participants: u32,
) {
	assert_ok!(EncointerCeremonies::attest_attendees(
		Origin::signed(attestor),
		cid,
		n_participants,
		attestees
	));
}

/// Fully attest all attendees with the new `attest_attendees` extrinsic.
fn fully_attest_attendees(
	attendees: Vec<AccountId>,
	cid: CommunityIdentifier,
	n_participants: u32,
) {
	for attestor in attendees.iter() {
		assert_ok!(EncointerCeremonies::attest_attendees(
			Origin::signed(attestor.clone()),
			cid,
			n_participants,
			attendees.clone().into_iter().filter(|a| a != attestor).collect()
		));
	}
}

/// Perform full attestation of all participants for a given meetup.
fn fully_attest_meetup(cid: CommunityIdentifier, mindex: MeetupIndexType) {
	let cindex = EncointerScheduler::current_ceremony_index();
	let meetup_participants =
		EncointerCeremonies::get_meetup_participants((cid, cindex), mindex).unwrap();
	let n_participants = meetup_participants.len() as u32;

	fully_attest_attendees(meetup_participants, cid, n_participants);
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
	custom_bootstrappers: Option<Vec<AccountId>>,
	n_locations: u32,
) -> CommunityIdentifier {
	let bootstrappers: Vec<_> = custom_bootstrappers
		.unwrap_or_else(|| bootstrappers().into_iter().map(|b| account_id(&b)).collect());
	let cid = register_test_community::<TestRuntime>(Some(bootstrappers.clone()), 0.0, 0.0);
	if n_locations > 70 {
		panic!("Too many locations.")
	}

	create_locations(n_locations).into_iter().for_each(|location| {
		assert_ok!(EncointerCommunities::add_location(
			Origin::signed(bootstrappers[0].clone()),
			cid,
			location
		));
	});

	bootstrappers.iter().cloned().for_each(|b| {
		assert_ok!(register(b, cid, None));
	});

	run_to_next_phase();
	// Assigning
	run_to_next_phase();
	// Attesting

	fully_attest_attendees(bootstrappers, cid, 6);

	run_to_next_phase();
	// Registering
	cid
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

		assert!(event_deposited::<TestRuntime>(
			Event::ParticipantRegistered(cid, ParticipantType::Bootstrapper, alice.clone()).into()
		));

		assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 1);
		assert_ok!(register(bob.clone(), cid, None));

		assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 2);

		assert_eq!(EncointerCeremonies::bootstrapper_index((cid, cindex), &bob), 2);
		assert_eq!(EncointerCeremonies::bootstrapper_registry((cid, cindex), &1).unwrap(), alice);
		assert_eq!(EncointerCeremonies::bootstrapper_registry((cid, cindex), &2).unwrap(), bob);

		let newbies = add_population(2, 2);
		let newbie_1 = account_id(&newbies[0]);
		let newbie_2 = account_id(&newbies[01]);
		IssuedRewards::<TestRuntime>::insert((cid, cindex - 1), 0, MeetupResult::Ok);
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
		assert_err!(
			register(alice.clone(), cid, None),
			Error::<TestRuntime>::ParticipantAlreadyRegistered
		);
	});
}

#[test]
fn registering_participant_in_wrong_phase_fails() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountId::from(AccountKeyring::Alice);
		run_to_next_phase();
		assert_eq!(EncointerScheduler::current_phase(), CeremonyPhaseType::Assigning);
		assert_err!(
			register(alice.clone(), cid, None),
			Error::<TestRuntime>::RegisteringOrAttestationPhaseRequired
		);
	});
}

#[test]
fn attest_attendees_works2() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.to_account_id();
		let bob = AccountKeyring::Bob.to_account_id();
		let ferdie = AccountKeyring::Ferdie.to_account_id();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		run_to_next_phase();
		run_to_next_phase();
		// Attesting
		assert_eq!(EncointerCeremonies::get_meetup_index((cid, cindex), &alice).unwrap(), 1);

		attest_all(alice.clone(), vec![bob.clone(), ferdie.clone()], cid, 3);
		assert_eq!(
			last_event::<TestRuntime>(),
			Some(Event::AttestationsRegistered(cid, 1, 2, alice.clone()).into())
		);
		attest_all(bob.clone(), vec![alice.clone(), ferdie.clone()], cid, 3);

		assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 2);
		assert_eq!(EncointerCeremonies::attestation_index((cid, cindex), &bob), 2);
		let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &2).unwrap();
		assert!(wit_vec.len() == 2);
		assert!(wit_vec.contains(&alice));
		assert!(wit_vec.contains(&ferdie));

		// TEST: re-registering must overwrite previous entry
		attest_all(alice, vec![bob, ferdie], cid, 3);
		assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 2);
	});
}

#[test]
fn attest_attendees_for_non_participant_fails_silently() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.to_account_id();
		let bob = AccountKeyring::Bob.to_account_id();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		run_to_next_phase();
		run_to_next_phase();
		// Attesting

		attest_all(alice.clone(), vec![bob, alice.clone()], cid, 3);
		assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 1);
		let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1).unwrap();
		assert!(wit_vec.contains(&alice) == false);
		assert!(wit_vec.len() == 1);
	});
}

#[test]
fn attest_attendee_from_non_registered_participant_fails() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.to_account_id();
		let ferdie = AccountKeyring::Ferdie.to_account_id();
		let eve = AccountKeyring::Eve.to_account_id();
		register_alice_bob_ferdie(cid);
		run_to_next_phase();
		run_to_next_phase();
		// Attesting
		assert_err!(
			EncointerCeremonies::attest_attendees(Origin::signed(eve), cid, 3, vec![alice, ferdie],),
			Error::<TestRuntime>::ParticipantIsNotRegistered
		);
	});
}

#[test]
fn attest_attendee_for_alien_participant_fails() {
	new_test_ext().execute_with(|| {
		let alice = AccountKeyring::Alice.to_account_id();
		let bob = AccountKeyring::Bob.to_account_id();
		let charlie = AccountKeyring::Charlie.to_account_id();
		let bootstrappers = vec![alice.clone(), bob.clone(), charlie.clone()];
		let cid = perform_bootstrapping_ceremony(Some(bootstrappers), 3);

		EncointerCeremonies::claim_rewards(Origin::signed(alice.clone()), cid, None).unwrap();

		register_alice_bob_ferdie(cid);
		register_charlie_dave_eve(cid);

		let participants: Vec<AccountId> =
			add_population(99, 0).iter().map(|b| account_id(&b)).collect();
		assert_ok!(EncointerCeremonies::set_endorsement_tickets_per_bootstrapper(
			Origin::signed(master()),
			100u8
		));
		for p in participants.iter() {
			assert_ok!(EncointerCeremonies::endorse_newcomer(
				Origin::signed(alice.clone()),
				cid,
				p.clone()
			));
			assert_ok!(register(p.clone(), cid, None));
		}

		run_to_next_phase();
		run_to_next_phase();
		let cindex = EncointerScheduler::current_ceremony_index();
		let alices_meetup_index =
			EncointerCeremonies::get_meetup_index((cid, cindex), &alice).unwrap();
		let bobs_meetup_index = EncointerCeremonies::get_meetup_index((cid, cindex), &bob).unwrap();
		assert_ne!(alices_meetup_index, bobs_meetup_index);

		let mut bobs_peers =
			EncointerCeremonies::get_meetup_participants((cid, cindex), bobs_meetup_index).unwrap();
		// remove self
		let i = bobs_peers.iter().position(|a| a == &bob).unwrap();
		bobs_peers.remove(i);

		// Attesting
		assert_err!(
			EncointerCeremonies::attest_attendees(
				Origin::signed(alice),
				cid,
				bobs_peers.len() as u32 + 1,
				bobs_peers,
			),
			Error::<TestRuntime>::NoValidAttestations
		);
	});
}

#[test]
fn attest_attendees_with_non_participant_fails_silently() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.to_account_id();
		let bob = AccountKeyring::Bob.to_account_id();
		let eve = AccountKeyring::Eve.to_account_id();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		run_to_next_phase();
		run_to_next_phase();
		// Attesting
		attest_all(alice, vec![bob, eve.clone()], cid, 3);
		assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 1);
		let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1).unwrap();
		assert!(wit_vec.contains(&eve) == false);
		assert!(wit_vec.len() == 1);
	});
}

#[test]
fn claim_rewards_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.to_account_id();
		let bob = AccountKeyring::Bob.to_account_id();
		let charlie = AccountKeyring::Charlie.to_account_id();
		let dave = AccountKeyring::Dave.to_account_id();
		let eve = AccountKeyring::Eve.to_account_id();
		let ferdie = AccountKeyring::Ferdie.to_account_id();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		register_charlie_dave_eve(cid);

		Assignments::<TestRuntime>::insert(
			(cid, cindex),
			Assignment {
				bootstrappers_reputables: Default::default(),
				endorsees: Default::default(),
				newbies: Default::default(),
				locations: AssignmentParams { m: 7, s1: 8, s2: 9 },
			},
		);

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
		attest_all(
			alice.clone(),
			vec![bob.clone(), charlie.clone(), dave.clone(), eve.clone()],
			cid,
			5,
		);
		// bob attests all others except for ferdie, who doesn't show up
		attest_all(
			bob.clone(),
			vec![alice.clone(), charlie.clone(), dave.clone(), eve.clone()],
			cid,
			5,
		);
		// charlie attests all others except for ferdie, who doesn't show up
		attest_all(
			charlie.clone(),
			vec![alice.clone(), bob.clone(), dave.clone(), eve.clone()],
			cid,
			5,
		);
		// dave attests all others plus nonexistent ferdie and reports wrong number
		attest_all(
			dave.clone(),
			vec![alice.clone(), bob.clone(), charlie.clone(), eve.clone(), ferdie.clone()],
			cid,
			6,
		);
		// eve does not attest anybody...
		// ferdie is not here...

		assert_eq!(EncointerBalances::balance(cid, &alice), ZERO);

		run_to_next_phase();
		// Registering
		EncointerCeremonies::claim_rewards(Origin::signed(alice.clone()), cid, None).unwrap();
		let meetup_result = IssuedRewards::<TestRuntime>::get((cid, cindex), 1);
		assert_eq!(meetup_result, Some(MeetupResult::Ok));

		assert!(event_deposited::<TestRuntime>(Event::RewardsIssued(cid, 1, 3).into()));

		for sender in vec![alice.clone(), bob.clone(), charlie.clone()].iter() {
			let result: f64 = EncointerBalances::balance(cid, sender).lossy_into();
			assert_abs_diff_eq!(
				result,
				EncointerCeremonies::ceremony_reward().lossy_into(),
				epsilon = 1.0e-6
			);
			assert_eq!(
				EncointerCeremonies::participant_reputation((cid, cindex), sender),
				Reputation::VerifiedUnlinked
			);
			assert!(event_deposited::<TestRuntime>(
				BalancesEvent::Issued(
					cid,
					sender.clone(),
					EncointerCeremonies::ceremony_reward().lossy_into()
				)
				.into()
			));
		}

		for sender in vec![eve.clone(), ferdie.clone()].iter() {
			assert_eq!(EncointerBalances::balance(cid, sender), ZERO);
			assert_eq!(
				EncointerCeremonies::participant_reputation((cid, cindex), sender),
				Reputation::Unverified
			);
			assert!(event_deposited::<TestRuntime>(
				Event::NoReward {
					cid,
					cindex,
					meetup_index: 1,
					account: sender.clone(),
					reason: ExclusionReason::NoVote,
				}
				.into()
			));
		}
		assert_eq!(EncointerBalances::balance(cid, &dave), ZERO);
		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex), &dave),
			Reputation::Unverified
		);
		assert!(event_deposited::<TestRuntime>(
			Event::NoReward {
				cid,
				cindex,
				meetup_index: 1,
				account: dave.clone(),
				reason: ExclusionReason::WrongVote,
			}
			.into()
		));

		// Claiming twice does not work for any of the meetup participants
		for sender in vec![alice, bob, charlie, dave, ferdie].iter() {
			assert_err!(
				EncointerCeremonies::claim_rewards(Origin::signed(sender.clone()), cid, None),
				Error::<TestRuntime>::RewardsAlreadyIssued
			);
		}
	});
}

#[test]
fn claim_rewards_works_with_one_missing_attestation() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.to_account_id();
		let bob = AccountKeyring::Bob.to_account_id();
		let charlie = AccountKeyring::Charlie.to_account_id();
		let dave = AccountKeyring::Dave.to_account_id();
		let eve = AccountKeyring::Eve.to_account_id();
		let ferdie = AccountKeyring::Ferdie.to_account_id();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		register_charlie_dave_eve(cid);

		Assignments::<TestRuntime>::insert(
			(cid, cindex),
			Assignment {
				bootstrappers_reputables: Default::default(),
				endorsees: Default::default(),
				newbies: Default::default(),
				locations: AssignmentParams { m: 7, s1: 8, s2: 9 },
			},
		);

		run_to_next_phase();
		// Assigning
		run_to_next_phase();
		// Attesting
		let all_participants = vec![alice.clone(), bob, charlie, dave, eve, ferdie];

		for p in all_participants.clone().into_iter() {
			let mut attestees = all_participants.clone();
			// remove self
			let i = attestees.iter().position(|a| a == &p).unwrap();
			attestees.remove(i);
			// remove one more participant
			attestees.remove(i % 5);
			attest_all(p, attestees, cid, 6);
		}

		run_to_next_phase();
		// Registering
		EncointerCeremonies::claim_rewards(Origin::signed(alice), cid, None).ok();

		// everybody should receive their reward
		assert!(event_deposited::<TestRuntime>(Event::RewardsIssued(cid, 1, 6).into()));
	});
}

#[test]
fn claim_rewards_can_only_be_called_for_valid_meetup_indices() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = perform_bootstrapping_ceremony(None, 10);
		let alice = AccountKeyring::Alice.pair();
		let bob = AccountKeyring::Bob.pair();
		let charlie = AccountKeyring::Charlie.pair();
		let dave = AccountKeyring::Dave.pair();
		let eve = AccountKeyring::Eve.pair();
		let ferdie = AccountKeyring::Ferdie.pair();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		register_charlie_dave_eve(cid);

		let mut all_participants = vec![alice, bob, charlie, dave, eve, ferdie];

		for i in 0..50 {
			let n: u8 = i + 13;
			let pair = sr25519::Pair::from_entropy(&[n; 32], None).0;
			register_as_reputable(&pair.clone(), cid).ok();
			all_participants.push(pair);
		}

		Assignments::<TestRuntime>::insert(
			(cid, cindex),
			Assignment {
				bootstrappers_reputables: Default::default(),
				endorsees: Default::default(),
				newbies: Default::default(),
				locations: AssignmentParams { m: 7, s1: 8, s2: 9 },
			},
		);

		run_to_next_phase();
		// Assigning
		run_to_next_phase();
		// Attesting

		let meetup_count = EncointerCeremonies::meetup_count((cid, cindex));
		for i in 1..=meetup_count {
			fully_attest_meetup(cid, i);
		}

		run_to_next_phase();
		// Registering

		for i in 1..=meetup_count {
			assert_ok!(EncointerCeremonies::claim_rewards(
				Origin::signed(account_id(&all_participants[0].clone())),
				cid,
				Some(i),
			));
		}

		for index in
			vec![0, 1 + meetup_count, 2 + meetup_count, 2 * meetup_count - 1, 2 * meetup_count + 1]
		{
			assert_err!(
				EncointerCeremonies::claim_rewards(
					Origin::signed(account_id(&all_participants[0].clone())),
					cid,
					Some(index)
				),
				Error::<TestRuntime>::InvalidMeetupIndex,
			);
		}
	});
}

#[test]
fn claim_rewards_fails_with_two_missing_attestations() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.to_account_id();
		let bob = AccountKeyring::Bob.to_account_id();
		let charlie = AccountKeyring::Charlie.to_account_id();
		let dave = AccountKeyring::Dave.to_account_id();
		let eve = AccountKeyring::Eve.to_account_id();
		let ferdie = AccountKeyring::Ferdie.to_account_id();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		register_charlie_dave_eve(cid);

		Assignments::<TestRuntime>::insert(
			(cid, cindex),
			Assignment {
				bootstrappers_reputables: Default::default(),
				endorsees: Default::default(),
				newbies: Default::default(),
				locations: AssignmentParams { m: 7, s1: 8, s2: 9 },
			},
		);

		run_to_next_phase();
		// Assigning
		run_to_next_phase();
		// Attesting
		let all_participants = vec![alice.clone(), bob, charlie, dave, eve, ferdie];

		for p in all_participants.clone().into_iter() {
			let mut attestees = all_participants.clone();
			// remove self
			let i = attestees.iter().position(|a| a == &p).unwrap();
			attestees.remove(i);
			// remove two more participants
			attestees.remove(i % 5);
			attestees.remove(i % 4);
			attest_all(p, attestees, cid, 6);
		}

		run_to_next_phase();
		// Registering
		EncointerCeremonies::claim_rewards(Origin::signed(alice), cid, None).ok();

		// nobody receives their reward
		assert!(event_deposited::<TestRuntime>(Event::RewardsIssued(cid, 1, 0).into()));
	});
}

#[test]
fn meetup_marked_as_completed_in_registration_when_claim_rewards_validation_error() {
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

		Assignments::<TestRuntime>::insert(
			(cid, cindex),
			Assignment {
				bootstrappers_reputables: Default::default(),
				endorsees: Default::default(),
				newbies: Default::default(),
				locations: AssignmentParams { m: 7, s1: 8, s2: 9 },
			},
		);

		run_to_next_phase();
		// Assigning
		run_to_next_phase();
		// Attesting
		let all_participants = vec![&alice, &bob, &charlie, &dave, &eve, &ferdie];

		for (i, p) in all_participants.clone().into_iter().enumerate() {
			let mut attestees = all_participants.clone();
			// remove self
			attestees.retain(|&a| account_id(a) != account_id(p));
			// this will lead to an error beacuse there is no depandable vote
			EncointerCeremonies::attest_attendees(
				Origin::signed(account_id(&p)),
				cid,
				i as u32,
				attestees.into_iter().map(|pa| account_id(&pa)).collect(),
			)
			.unwrap();
		}

		// no early claim possible
		assert!(EncointerCeremonies::claim_rewards(Origin::signed(account_id(&alice)), cid, None)
			.is_err());
		// nothing happens in attesting phase
		assert!(!IssuedRewards::<TestRuntime>::contains_key((cid, cindex), 1));
		run_to_next_phase();
		// Registering phase
		assert!(EncointerCeremonies::claim_rewards(Origin::signed(account_id(&alice)), cid, None)
			.is_ok());
		// in registering phase, the meetup is marked as completed
		assert!(IssuedRewards::<TestRuntime>::contains_key((cid, cindex), 1));
		let meetup_result = IssuedRewards::<TestRuntime>::get((cid, cindex), 1);
		assert_eq!(meetup_result, Some(MeetupResult::VotesNotDependable));

		assert!(event_deposited::<TestRuntime>(
			Event::MeetupEvaluated(cid, 1, MeetupResult::VotesNotDependable).into()
		));
	});
}

#[test]
fn claim_rewards_can_be_called_by_non_participant() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.to_account_id();
		let bob = AccountKeyring::Bob.to_account_id();
		let charlie = AccountKeyring::Charlie.to_account_id();
		let dave = AccountKeyring::Dave.to_account_id();
		let eve = AccountKeyring::Eve.to_account_id();
		let ferdie = AccountKeyring::Ferdie.to_account_id();

		let yran = sr25519::Pair::from_entropy(&[8u8; 32], None).0;

		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		register_charlie_dave_eve(cid);

		Assignments::<TestRuntime>::insert(
			(cid, cindex),
			Assignment {
				bootstrappers_reputables: Default::default(),
				endorsees: Default::default(),
				newbies: Default::default(),
				locations: AssignmentParams { m: 7, s1: 8, s2: 9 },
			},
		);

		run_to_next_phase();
		// Assigning
		run_to_next_phase();
		// Attesting
		let all_participants = vec![alice, bob, charlie, dave, eve, ferdie];

		for p in all_participants.clone().into_iter() {
			let mut attestees = all_participants.clone();
			// remove self
			let i = attestees.iter().position(|a| a == &p).unwrap();
			attestees.remove(i);
			// remove one more participant
			attestees.remove(i % 5);
			attest_all(p, attestees, cid, 6);
		}

		run_to_next_phase();
		// Registering
		EncointerCeremonies::claim_rewards(Origin::signed(account_id(&yran)), cid, Some(1)).ok();

		// everybody should receive their reward
		assert!(event_deposited::<TestRuntime>(Event::RewardsIssued(cid, 1, 6).into()));
	});
}

#[test]
fn early_rewards_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.to_account_id();
		let bob = AccountKeyring::Bob.to_account_id();
		let charlie = AccountKeyring::Charlie.to_account_id();
		let dave = AccountKeyring::Dave.to_account_id();
		let eve = AccountKeyring::Eve.to_account_id();
		let ferdie = AccountKeyring::Ferdie.to_account_id();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		register_charlie_dave_eve(cid);

		Assignments::<TestRuntime>::insert(
			(cid, cindex),
			Assignment {
				bootstrappers_reputables: Default::default(),
				endorsees: Default::default(),
				newbies: Default::default(),
				locations: AssignmentParams { m: 7, s1: 8, s2: 9 },
			},
		);

		run_to_next_phase();
		// Assigning
		run_to_next_phase();
		// Attesting

		let all_participants = vec![alice.clone(), bob, charlie, dave, eve, ferdie];
		fully_attest_attendees(all_participants, cid, 6);

		// Still attesting phase
		EncointerCeremonies::claim_rewards(Origin::signed(alice), cid, None).ok();

		// everybody should receive their reward
		assert_eq!(last_event::<TestRuntime>(), Some(Event::RewardsIssued(cid, 1, 6).into()));
	})
}

#[test]
fn early_rewards_with_one_noshow_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.to_account_id();
		let bob = AccountKeyring::Bob.to_account_id();
		let charlie = AccountKeyring::Charlie.to_account_id();
		let dave = AccountKeyring::Dave.to_account_id();
		let eve = AccountKeyring::Eve.to_account_id();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		register_charlie_dave_eve(cid);

		Assignments::<TestRuntime>::insert(
			(cid, cindex),
			Assignment {
				bootstrappers_reputables: Default::default(),
				endorsees: Default::default(),
				newbies: Default::default(),
				locations: AssignmentParams { m: 7, s1: 8, s2: 9 },
			},
		);

		run_to_next_phase();
		// Assigning
		run_to_next_phase();
		// Attesting

		// Ferdie is missing
		let all_participants = vec![alice.clone(), bob, charlie, dave, eve];

		fully_attest_attendees(all_participants, cid, 5);

		// Still attesting phase
		EncointerCeremonies::claim_rewards(Origin::signed(alice), cid, None).ok();

		// everybody should receive their reward
		assert_eq!(last_event::<TestRuntime>(), Some(Event::RewardsIssued(cid, 1, 5).into()));
	})
}

#[test]
fn early_rewards_does_not_work_with_one_missing_submission_of_attestations() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.to_account_id();
		let bob = AccountKeyring::Bob.to_account_id();
		let charlie = AccountKeyring::Charlie.to_account_id();
		let dave = AccountKeyring::Dave.to_account_id();
		let eve = AccountKeyring::Eve.to_account_id();
		let ferdie = AccountKeyring::Ferdie.to_account_id();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		register_charlie_dave_eve(cid);

		Assignments::<TestRuntime>::insert(
			(cid, cindex),
			Assignment {
				bootstrappers_reputables: Default::default(),
				endorsees: Default::default(),
				newbies: Default::default(),
				locations: AssignmentParams { m: 7, s1: 8, s2: 9 },
			},
		);

		run_to_next_phase();
		// Assigning
		run_to_next_phase();
		// Attesting
		let all_participants = vec![alice.clone(), bob, charlie, dave, eve, ferdie];
		let mut submitters = all_participants.clone();
		submitters.remove(0);

		for p in submitters.into_iter() {
			let mut attestees = all_participants.clone();
			// remove self
			let i = attestees.iter().position(|a| a == &p).unwrap();
			attestees.remove(i);
			attest_all(p, attestees, cid, 6);
		}

		// Still attesting phase
		assert_err!(
			EncointerCeremonies::claim_rewards(Origin::signed(alice), cid, None),
			Error::<TestRuntime>::EarlyRewardsNotPossible
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

		EncointerCeremonies::claim_rewards(Origin::signed(account_id(&alice)), cid, None).ok();
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
		assert_err!(
			register(account_id(&yuri), cid, Some(proof)),
			Error::<TestRuntime>::WrongProofSubject
		);

		// see if Zoran can register with his fresh key
		// for the next ceremony claiming his former attendance
		IssuedRewards::<TestRuntime>::insert((cid, cindex - 1), 0, MeetupResult::Ok);
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
		assert_err!(
			register(account_id(&yuri), cid, Some(proof_second)),
			Error::<TestRuntime>::AttendanceUnverifiedOrAlreadyUsed
		);

		// signer without reputation fails
		let proof = prove_attendance(account_id(&yuri), cid, cindex - 1, &yuri);
		assert_err!(
			register(account_id(&yuri), cid, Some(proof)),
			Error::<TestRuntime>::AttendanceUnverifiedOrAlreadyUsed
		);
	});
}

#[test]
fn endorsement_by_bootstrapper_for_newbie_works_until_no_more_tickets() {
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
fn endorsing_newbie_for_next_ceremony_works_after_registering_phase() {
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

		run_to_next_phase();

		let bogdan = sr25519::Pair::from_entropy(&[99u8; 32], None).0;
		assert_ok!(EncointerCeremonies::endorse_newcomer(
			Origin::signed(alice.clone()),
			cid,
			account_id(&bogdan)
		));
		assert!(Endorsees::<TestRuntime>::contains_key((cid, cindex + 1), &account_id(&bogdan)));
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
fn endorsement_survives_idle_cycle() {
	new_test_ext().execute_with(|| {
		let cid = perform_bootstrapping_ceremony(None, 1);
		let alice = AccountId::from(AccountKeyring::Alice);

		// a newbie
		let zoran = account_id(&sr25519::Pair::from_entropy(&[9u8; 32], None).0);
		assert_ok!(EncointerCeremonies::endorse_newcomer(
			Origin::signed(alice.clone()),
			cid,
			zoran.clone()
		));
		assert!(EncointerCeremonies::is_endorsed(&zoran, &(cid, 4)).is_some());
		run_to_next_phase();
		run_to_next_phase();
		run_to_next_phase();
		run_to_next_phase();
		run_to_next_phase();
		run_to_next_phase();
		assert!(EncointerCeremonies::is_endorsed(&zoran, &(cid, 4)).is_some());
	});
}

#[test]
fn endorsing_works_after_subject_has_already_registered() {
	new_test_ext().execute_with(|| {
		let cid = perform_bootstrapping_ceremony(None, 1);
		let alice = AccountId::from(AccountKeyring::Alice);
		let cindex = EncointerScheduler::current_ceremony_index();

		// a newbie
		let yran = account_id(&sr25519::Pair::from_entropy(&[8u8; 32], None).0);

		assert!(EncointerBalances::issue(cid, &alice, NominalIncome::from_num(1)).is_ok());
		assert_ok!(register(yran.clone(), cid, None));

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
fn endorse_newbie_works_for_reputables() {
	new_test_ext().execute_with(|| {
		let cid = perform_bootstrapping_ceremony(None, 1);
		let reputable = account_id(&sr25519::Pair::from_entropy(&[10u8; 32], None).0);

		let cindex = EncointerScheduler::current_ceremony_index();

		EncointerCeremonies::fake_reputation(
			(cid, cindex - 1),
			&reputable,
			Reputation::VerifiedUnlinked,
		);

		// a newbie
		let yran = sr25519::Pair::from_entropy(&[8u8; 32], None).0;
		let zoran = sr25519::Pair::from_entropy(&[9u8; 32], None).0;
		let bob = sr25519::Pair::from_entropy(&[10u8; 32], None).0;
		assert_ok!(EncointerCeremonies::endorse_newcomer(
			Origin::signed(reputable.clone()),
			cid,
			account_id(&zoran)
		));
		assert!(Endorsees::<TestRuntime>::contains_key((cid, cindex), &account_id(&zoran)));
		assert_eq!(BurnedReputableNewbieTickets::<TestRuntime>::get((cid, cindex), &reputable), 1);
		assert_eq!(BurnedBootstrapperNewbieTickets::<TestRuntime>::get(cid, &reputable), 0);
		assert_ok!(EncointerCeremonies::endorse_newcomer(
			Origin::signed(reputable.clone()),
			cid,
			account_id(&yran)
		));
		assert!(Endorsees::<TestRuntime>::contains_key((cid, cindex), &account_id(&yran)));
		assert_eq!(BurnedReputableNewbieTickets::<TestRuntime>::get((cid, cindex), &reputable), 2);
		assert_eq!(BurnedBootstrapperNewbieTickets::<TestRuntime>::get(cid, &reputable), 0);

		assert_err!(
			EncointerCeremonies::endorse_newcomer(
				Origin::signed(reputable.clone()),
				cid,
				account_id(&bob)
			),
			Error::<TestRuntime>::NoMoreNewbieTickets,
		);
	});
}

#[test]
fn endorse_newbie_fails_if_already_endorsed_in_previous_ceremony() {
	new_test_ext().execute_with(|| {
		let cid = perform_bootstrapping_ceremony(None, 1);
		let alice = AccountId::from(AccountKeyring::Alice);

		// a newbie
		let yran = account_id(&sr25519::Pair::from_entropy(&[8u8; 32], None).0);
		assert_ok!(EncointerCeremonies::endorse_newcomer(
			Origin::signed(alice.clone()),
			cid,
			yran.clone()
		));

		run_to_next_phase();
		run_to_next_phase();
		run_to_next_phase();

		assert_err!(
			EncointerCeremonies::endorse_newcomer(Origin::signed(alice.clone()), cid, yran),
			Error::<TestRuntime>::AlreadyEndorsed
		);
	});
}

#[test]
fn endorse_newbie_fails_if_sender_has_no_reputation_and_is_not_bootstrapper() {
	new_test_ext().execute_with(|| {
		let cid = perform_bootstrapping_ceremony(None, 1);

		let yran = account_id(&sr25519::Pair::from_entropy(&[8u8; 32], None).0);
		let zoran = sr25519::Pair::from_entropy(&[9u8; 32], None).0;
		assert_err!(
			EncointerCeremonies::endorse_newcomer(Origin::signed(account_id(&zoran)), cid, yran),
			Error::<TestRuntime>::NoMoreNewbieTickets
		);
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
		register(yran.clone(), cid, None).unwrap();

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

#[test]
fn registering_endorsee_removes_endorsement() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let yran = account_id(&sr25519::Pair::from_entropy(&[8u8; 32], None).0);
		let cindex = EncointerScheduler::current_ceremony_index();
		assert!(EncointerBalances::issue(cid, &yran, NominalIncome::from_num(1)).is_ok());

		Endorsees::<TestRuntime>::insert((cid, cindex), &yran, ());

		assert!(Endorsees::<TestRuntime>::contains_key((cid, cindex), &yran));
		register(yran.clone(), cid, None).unwrap();

		assert!(EndorseeIndex::<TestRuntime>::contains_key((cid, cindex), &yran));
		assert!(!Endorsees::<TestRuntime>::contains_key((cid, cindex), &yran));
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
			// issue some rewards such that the inactivity counter is not increased
			IssuedRewards::<TestRuntime>::insert(
				(cid, EncointerScheduler::current_ceremony_index()),
				0,
				MeetupResult::Ok,
			);

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
fn grow_population_and_removing_community_works() {
	new_test_ext().execute_with(|| {
		let cid = perform_bootstrapping_ceremony(None, 3);
		let mut participants = bootstrappers();

		// generate many keys and register all of them
		// they will use the same keys per participant throughout to following ceremonies
		participants.extend(add_population(14, participants.len()));
		IssuedRewards::<TestRuntime>::insert(
			(cid, EncointerScheduler::current_ceremony_index() - 1),
			0,
			MeetupResult::Ok,
		);
		participants.iter().for_each(|p| {
			assert_ok!(EncointerBalances::issue(cid, &account_id(&p), NominalIncome::from_num(1)));
			assert_ok!(register(account_id(&p), cid, None));
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

		fully_attest_meetup(cid, 1);

		run_to_next_phase();
		// Registering
		for pair in participants.iter() {
			EncointerCeremonies::claim_rewards(Origin::signed(account_id(&pair)), cid, None).ok();
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

		fully_attest_meetup(cid, 1);

		run_to_next_phase();
		// Registering
		for pair in participants.iter() {
			EncointerCeremonies::claim_rewards(Origin::signed(account_id(&pair)), cid, None).ok();
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
		fully_attest_meetup(cid, 1);
		fully_attest_meetup(cid, 2);

		run_to_next_phase();
		// Registering
		for pair in participants.iter() {
			EncointerCeremonies::claim_rewards(Origin::signed(account_id(&pair)), cid, None).ok();
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

		// now we remove the community
		EncointerCeremonies::purge_community(cid);

		let reputation_lifetime = EncointerCeremonies::reputation_lifetime();
		let current_cindex =
			EncointerScheduler::current_ceremony_index().saturating_sub(reputation_lifetime);

		// only sanity check. Community removal is better tested in the communities pallet.
		assert_eq!(EncointerCommunities::community_identifiers().contains(&cid), false);

		assert_eq!(BurnedBootstrapperNewbieTickets::<TestRuntime>::iter_prefix(cid).next(), None);

		for cindex in current_cindex.saturating_sub(reputation_lifetime)..=current_cindex {
			assert_eq!(
				BootstrapperRegistry::<TestRuntime>::iter_prefix((cid, cindex)).next(),
				None
			);
			assert_eq!(BootstrapperIndex::<TestRuntime>::iter_prefix((cid, cindex)).next(), None);
			assert_eq!(BootstrapperCount::<TestRuntime>::contains_key((cid, cindex)), false);

			assert_eq!(ReputableRegistry::<TestRuntime>::iter_prefix((cid, cindex)).next(), None);
			assert_eq!(ReputableIndex::<TestRuntime>::iter_prefix((cid, cindex)).next(), None);
			assert_eq!(ReputableCount::<TestRuntime>::contains_key((cid, cindex)), false);

			assert_eq!(EndorseeRegistry::<TestRuntime>::iter_prefix((cid, cindex)).next(), None);
			assert_eq!(EndorseeIndex::<TestRuntime>::iter_prefix((cid, cindex)).next(), None);
			assert_eq!(EndorseeCount::<TestRuntime>::contains_key((cid, cindex)), false);

			assert_eq!(NewbieRegistry::<TestRuntime>::iter_prefix((cid, cindex)).next(), None);
			assert_eq!(NewbieIndex::<TestRuntime>::iter_prefix((cid, cindex)).next(), None);
			assert_eq!(NewbieCount::<TestRuntime>::contains_key((cid, cindex)), false);

			assert_eq!(AssignmentCounts::<TestRuntime>::contains_key((cid, cindex)), false);
			assert_eq!(Assignments::<TestRuntime>::contains_key((cid, cindex)), false);

			assert_eq!(
				ParticipantReputation::<TestRuntime>::iter_prefix((cid, cindex)).next(),
				None
			);

			assert_eq!(Endorsees::<TestRuntime>::iter_prefix((cid, cindex)).next(), None);
			assert_eq!(EndorseesCount::<TestRuntime>::contains_key((cid, cindex)), false);
			assert_eq!(MeetupCount::<TestRuntime>::contains_key((cid, cindex)), false);

			assert_eq!(AttestationRegistry::<TestRuntime>::iter_prefix((cid, cindex)).next(), None);
			assert_eq!(AttestationIndex::<TestRuntime>::iter_prefix((cid, cindex)).next(), None);
			assert_eq!(AttestationCount::<TestRuntime>::contains_key((cid, cindex)), false);

			assert_eq!(
				MeetupParticipantCountVote::<TestRuntime>::iter_prefix((cid, cindex)).next(),
				None
			);

			assert_eq!(IssuedRewards::<TestRuntime>::iter_prefix((cid, cindex)).next(), None);

			assert_eq!(
				BurnedReputableNewbieTickets::<TestRuntime>::iter_prefix((cid, cindex)).next(),
				None
			);

			assert_eq!(InactivityCounters::<TestRuntime>::contains_key(cid), false);
		}
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
fn update_inactivity_counters_works() {
	new_test_ext().execute_with(|| {
		let cid0 = CommunityIdentifier::default();
		let cid1 = CommunityIdentifier::new(
			Location::new(Degree::from_num(1f64), Degree::from_num(1f64)),
			Vec::<i64>::new(),
		)
		.unwrap();

		let mut cindex = 5;

		IssuedRewards::<TestRuntime>::insert((cid0, cindex), 0, MeetupResult::Ok);
		IssuedRewards::<TestRuntime>::insert((cid1, cindex), 0, MeetupResult::Ok);

		let timeout = 1;
		assert_eq!(
			EncointerCeremonies::update_inactivity_counters(cindex, timeout, vec![cid0, cid1]),
			vec![]
		);

		cindex += 1;
		IssuedRewards::<TestRuntime>::insert((cid0, cindex), 0, MeetupResult::Ok);
		assert_eq!(
			EncointerCeremonies::update_inactivity_counters(cindex, timeout, vec![cid0, cid1]),
			vec![]
		);

		cindex += 1;
		assert_eq!(
			EncointerCeremonies::update_inactivity_counters(cindex, timeout, vec![cid0, cid1]),
			vec![cid1]
		);

		cindex += 1;
		assert_eq!(
			EncointerCeremonies::update_inactivity_counters(cindex, timeout, vec![cid0, cid1]),
			vec![cid0, cid1]
		);
	});
}

#[test]
fn purge_inactive_communities_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = perform_bootstrapping_ceremony(None, 1);

		assert!(
			<encointer_communities::Pallet<TestRuntime>>::community_identifiers().contains(&cid)
		);

		// inactivity counter is 1, beacuse of a full ceremony cycle in the bootstrapping ceremony
		// without any rewards being claimed
		assert_eq!(EncointerCeremonies::inactivity_counters(cid).unwrap(), 1);

		run_to_next_phase();
		// Assigning
		assert_eq!(
			event_at_index::<TestRuntime>(get_num_events::<TestRuntime>() - 2),
			Some(Event::InactivityCounterUpdated(cid, 2).into())
		);

		assert!(
			<encointer_communities::Pallet<TestRuntime>>::community_identifiers().contains(&cid)
		);
		assert_eq!(EncointerCeremonies::inactivity_counters(cid).unwrap(), 2);

		// issued rewards will cause inactivity counter to go to 0 in the next cycle
		IssuedRewards::<TestRuntime>::insert(
			(cid, EncointerScheduler::current_ceremony_index()),
			0,
			MeetupResult::Ok,
		);
		run_to_next_phase();
		run_to_next_phase();
		run_to_next_phase();

		assert_eq!(
			event_at_index::<TestRuntime>(get_num_events::<TestRuntime>() - 2),
			Some(Event::InactivityCounterUpdated(cid, 0).into())
		);

		assert!(
			<encointer_communities::Pallet<TestRuntime>>::community_identifiers().contains(&cid)
		);
		assert_eq!(EncointerCeremonies::inactivity_counters(cid).unwrap(), 0);
		run_to_next_phase();
		run_to_next_phase();
		run_to_next_phase();

		assert!(
			<encointer_communities::Pallet<TestRuntime>>::community_identifiers().contains(&cid)
		);
		assert_eq!(EncointerCeremonies::inactivity_counters(cid).unwrap(), 1);

		run_to_next_phase();
		run_to_next_phase();
		run_to_next_phase();

		assert!(
			<encointer_communities::Pallet<TestRuntime>>::community_identifiers().contains(&cid)
		);
		assert_eq!(EncointerCeremonies::inactivity_counters(cid).unwrap(), 2);

		run_to_next_phase();
		run_to_next_phase();
		run_to_next_phase();

		assert!(
			<encointer_communities::Pallet<TestRuntime>>::community_identifiers().contains(&cid)
		);
		// now the inactivity counter is 3 == inactivity_timeout, so in the next cycle the community will be purged
		assert_eq!(EncointerCeremonies::inactivity_counters(cid).unwrap(), 3);

		run_to_next_phase();
		run_to_next_phase();
		run_to_next_phase();

		// here community gets purged
		assert!(
			!<encointer_communities::Pallet<TestRuntime>>::community_identifiers().contains(&cid)
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

		let reputable_pair = &AccountKeyring::Ferdie.pair();
		let reputable = account_id(reputable_pair);
		let reputable2_pair = &AccountKeyring::Charlie.pair();
		let reputable2 = account_id(reputable2_pair);

		let newbie = account_id(&AccountKeyring::Eve.pair());

		assert!(EncointerBalances::issue(cid, &reputable, NominalIncome::from_num(1)).is_ok());

		assert_ok!(register(bootstrapper.clone(), cid, None));
		assert_ok!(register(bootstrapper2.clone(), cid, None));

		assert_ok!(register_as_reputable(reputable_pair, cid));
		assert_ok!(register_as_reputable(reputable2_pair, cid));
		assert_ok!(register(newbie.clone(), cid, None));

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

		// Error on invalid indices
		assert!(EncointerCeremonies::get_meetup_participants((cid, cindex), 0).is_err());

		assert!(EncointerCeremonies::get_meetup_participants((cid, cindex), 3).is_err());

		assert!(EncointerCeremonies::get_meetup_participants((cid, cindex), 10).is_err());
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
fn remove_participant_from_registry_fails_in_wrong_phase() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 1.0, 1.0);
		let cindex = EncointerScheduler::current_ceremony_index();

		let alice = account_id(&AccountKeyring::Alice.pair());
		run_to_next_phase();
		assert!(EncointerCeremonies::remove_participant_from_registry(cid, cindex, &alice).is_err());
	});
}

#[test]
fn remove_participant_from_registry_works() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cindex = EncointerScheduler::current_ceremony_index();

		IssuedRewards::<TestRuntime>::insert((cid, cindex - 1), 0, MeetupResult::Ok);
		let bootstrapper = account_id(&AccountKeyring::Ferdie.pair());
		EncointerCommunities::insert_bootstrappers(cid, vec![bootstrapper.clone()]);

		let alice = account_id(&AccountKeyring::Alice.pair());
		let bob = account_id(&AccountKeyring::Bob.pair());
		let charlie = account_id(&AccountKeyring::Charlie.pair());
		let eve = account_id(&AccountKeyring::Eve.pair());

		assert!(EncointerBalances::issue(cid, &bootstrapper, NominalIncome::from_num(1)).is_ok());

		assert_ok!(register(alice.clone(), cid, None));
		assert_ok!(register(bob.clone(), cid, None));
		assert_ok!(register(charlie.clone(), cid, None));
		assert_ok!(register(eve.clone(), cid, None));

		assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 4);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 1).unwrap(), alice);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 2).unwrap(), bob);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 3).unwrap(), charlie);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 4).unwrap(), eve);

		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &alice), 1);
		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &bob), 2);
		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &charlie), 3);
		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &eve), 4);

		assert_ok!(EncointerCeremonies::remove_participant_from_registry(cid, cindex, &bob));

		assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 3);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 1).unwrap(), alice);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 2).unwrap(), eve);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 3).unwrap(), charlie);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 4), None);

		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &alice), 1);
		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &eve), 2);
		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &charlie), 3);
		assert_eq!(EncointerCeremonies::newbie_index((cid, cindex), &bob), 0);

		assert_ok!(EncointerCeremonies::remove_participant_from_registry(cid, cindex, &charlie));

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
fn remove_participant_from_registry_works_for_all_participant_types() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cindex = EncointerScheduler::current_ceremony_index();

		IssuedRewards::<TestRuntime>::insert((cid, cindex - 1), 0, MeetupResult::Ok);
		let bootstrapper = account_id(&AccountKeyring::Ferdie.pair());
		EncointerCommunities::insert_bootstrappers(cid, vec![bootstrapper.clone()]);

		let newbie = account_id(&AccountKeyring::Alice.pair());
		let reputable_pair = &AccountKeyring::Bob.pair();
		let reputable = account_id(reputable_pair);
		let endorsee = account_id(&AccountKeyring::Charlie.pair());
		Endorsees::<TestRuntime>::insert((cid, cindex), &endorsee, ());

		assert!(EncointerBalances::issue(cid, &bootstrapper, NominalIncome::from_num(1)).is_ok());

		assert_ok!(register(newbie.clone(), cid, None));
		assert_ok!(register_as_reputable(reputable_pair, cid));
		assert_ok!(register(endorsee.clone(), cid, None));
		assert_ok!(register(bootstrapper.clone(), cid, None));

		assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 1);
		assert_eq!(EncointerCeremonies::reputable_count((cid, cindex)), 1);
		assert_eq!(EncointerCeremonies::endorsee_count((cid, cindex)), 1);
		assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 1);

		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 1).unwrap(), newbie);
		assert_eq!(EncointerCeremonies::reputable_registry((cid, cindex), 1).unwrap(), reputable);
		assert_eq!(EncointerCeremonies::endorsee_registry((cid, cindex), 1).unwrap(), endorsee);
		assert_eq!(
			EncointerCeremonies::bootstrapper_registry((cid, cindex), 1).unwrap(),
			bootstrapper
		);

		assert_ok!(EncointerCeremonies::remove_participant_from_registry(cid, cindex, &newbie));
		assert_ok!(EncointerCeremonies::remove_participant_from_registry(cid, cindex, &reputable));
		assert_ok!(EncointerCeremonies::remove_participant_from_registry(cid, cindex, &endorsee));
		assert_ok!(EncointerCeremonies::remove_participant_from_registry(
			cid,
			cindex,
			&bootstrapper
		));

		assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 0);
		assert_eq!(EncointerCeremonies::reputable_count((cid, cindex)), 0);
		assert_eq!(EncointerCeremonies::endorsee_count((cid, cindex)), 0);
		assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 0);
	});
}

#[test]
fn remove_participant_from_registry_with_no_participants_fails() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cindex = EncointerScheduler::current_ceremony_index();

		IssuedRewards::<TestRuntime>::insert((cid, cindex - 1), 0, MeetupResult::Ok);
		let bootstrapper = account_id(&AccountKeyring::Ferdie.pair());
		EncointerCommunities::insert_bootstrappers(cid, vec![bootstrapper.clone()]);

		let alice = account_id(&AccountKeyring::Alice.pair());

		assert!(EncointerCeremonies::remove_participant_from_registry(cid, cindex, &alice).is_err());
	});
}

#[test]
fn upgrade_registration_works() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cindex = EncointerScheduler::current_ceremony_index();

		IssuedRewards::<TestRuntime>::insert((cid, cindex - 1), 0, MeetupResult::Ok);
		let bootstrapper = account_id(&AccountKeyring::Ferdie.pair());
		EncointerCommunities::insert_bootstrappers(cid, vec![bootstrapper.clone()]);
		assert!(EncointerBalances::issue(cid, &bootstrapper, NominalIncome::from_num(1)).is_ok());

		let a = AccountKeyring::Alice.pair();
		let alice = account_id(&a);

		assert_ok!(register(alice.clone(), cid, None));
		assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 1);
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 1).unwrap(), alice);

		let proof = make_reputable_and_get_proof(&a, cid, cindex - 1);
		assert_ok!(EncointerCeremonies::upgrade_registration(
			Origin::signed(alice.clone()),
			cid,
			proof
		));

		assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 0);
		assert_eq!(EncointerCeremonies::reputable_count((cid, cindex)), 1);
		assert_eq!(EncointerCeremonies::reputable_registry((cid, cindex), 1).unwrap(), alice);
	});
}

#[test]
fn upgrade_registration_fails_if_not_registered_or_not_newbie() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cindex = EncointerScheduler::current_ceremony_index();
		IssuedRewards::<TestRuntime>::insert((cid, cindex - 1), 0, MeetupResult::Ok);
		let bootstrapper = account_id(&AccountKeyring::Ferdie.pair());
		EncointerCommunities::insert_bootstrappers(cid, vec![bootstrapper.clone()]);
		assert!(EncointerBalances::issue(cid, &bootstrapper, NominalIncome::from_num(1)).is_ok());

		let a = AccountKeyring::Alice.pair();
		let alice = account_id(&a);

		let proof = make_reputable_and_get_proof(&a, cid, cindex - 1);
		assert_err!(
			EncointerCeremonies::upgrade_registration(
				Origin::signed(alice.clone()),
				cid,
				proof.clone()
			),
			Error::<TestRuntime>::ParticipantIsNotRegistered
		);

		assert_ok!(register(bootstrapper.clone(), cid, None));
		assert_err!(
			EncointerCeremonies::upgrade_registration(
				Origin::signed(bootstrapper.clone()),
				cid,
				proof
			),
			Error::<TestRuntime>::MustBeNewbieToUpgradeRegistration
		);
	})
}

#[test]
fn upgrade_registration_fails_in_wrong_phase() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cindex = EncointerScheduler::current_ceremony_index();
		IssuedRewards::<TestRuntime>::insert((cid, cindex - 1), 0, MeetupResult::Ok);
		let bootstrapper = account_id(&AccountKeyring::Ferdie.pair());
		EncointerCommunities::insert_bootstrappers(cid, vec![bootstrapper.clone()]);
		assert!(EncointerBalances::issue(cid, &bootstrapper, NominalIncome::from_num(1)).is_ok());

		let a = AccountKeyring::Alice.pair();
		let alice = account_id(&a);

		let proof = make_reputable_and_get_proof(&a, cid, cindex - 1);

		run_to_next_phase();
		assert_err!(
			EncointerCeremonies::upgrade_registration(
				Origin::signed(alice.clone()),
				cid,
				proof.clone()
			),
			Error::<TestRuntime>::RegisteringOrAttestationPhaseRequired
		);
	})
}

#[test]
fn upgrade_registration_fails_with_inexistent_community() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cindex = EncointerScheduler::current_ceremony_index();
		IssuedRewards::<TestRuntime>::insert((cid, cindex - 1), 0, MeetupResult::Ok);
		let bootstrapper = account_id(&AccountKeyring::Ferdie.pair());
		EncointerCommunities::insert_bootstrappers(cid, vec![bootstrapper.clone()]);
		assert!(EncointerBalances::issue(cid, &bootstrapper, NominalIncome::from_num(1)).is_ok());

		let a = AccountKeyring::Alice.pair();
		let alice = account_id(&a);

		let proof = make_reputable_and_get_proof(&a, cid, cindex - 1);

		assert_err!(
			EncointerCeremonies::upgrade_registration(
				Origin::signed(alice.clone()),
				CommunityIdentifier::from_str("aaaaabbbbb").unwrap(),
				proof.clone()
			),
			Error::<TestRuntime>::InexistentCommunity
		);
	})
}

#[test]
fn unregister_participant_works_with_reputables() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cindex = EncointerScheduler::current_ceremony_index();
		IssuedRewards::<TestRuntime>::insert((cid, cindex - 1), 0, MeetupResult::Ok);
		let bootstrapper = account_id(&AccountKeyring::Ferdie.pair());
		EncointerCommunities::insert_bootstrappers(cid, vec![bootstrapper.clone()]);
		assert!(EncointerBalances::issue(cid, &bootstrapper, NominalIncome::from_num(1)).is_ok());

		let a = AccountKeyring::Alice.pair();
		let alice = account_id(&a);

		let proof = make_reputable_and_get_proof(&a, cid, cindex - 1);
		assert_ok!(register(alice.clone(), cid, Some(proof)));
		assert_eq!(EncointerCeremonies::reputable_registry((cid, cindex), 1).unwrap(), alice);
		assert_eq!(EncointerCeremonies::reputable_count((cid, cindex)), 1);
		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex), &alice),
			Reputation::UnverifiedReputable
		);
		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex - 1), &alice),
			Reputation::VerifiedLinked
		);

		assert_ok!(EncointerCeremonies::unregister_participant(
			Origin::signed(alice.clone()),
			cid,
			Some((cid, cindex - 1))
		));

		assert!(!ParticipantReputation::<TestRuntime>::contains_key((cid, cindex), &alice));
		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex - 1), &alice),
			Reputation::VerifiedUnlinked
		);
		assert_eq!(EncointerCeremonies::reputable_count((cid, cindex)), 0);
	})
}

#[test]
fn unregister_participant_fails_with_reputables_and_wrong_reputation() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cindex = EncointerScheduler::current_ceremony_index();
		IssuedRewards::<TestRuntime>::insert((cid, cindex - 1), 0, MeetupResult::Ok);
		let bootstrapper = account_id(&AccountKeyring::Ferdie.pair());
		EncointerCommunities::insert_bootstrappers(cid, vec![bootstrapper.clone()]);
		assert!(EncointerBalances::issue(cid, &bootstrapper, NominalIncome::from_num(1)).is_ok());

		let a = AccountKeyring::Alice.pair();
		let alice = account_id(&a);

		let proof = make_reputable_and_get_proof(&a, cid, cindex - 1);
		assert_ok!(register(alice.clone(), cid, Some(proof)));
		assert_eq!(EncointerCeremonies::reputable_registry((cid, cindex), 1).unwrap(), alice);
		assert_eq!(EncointerCeremonies::reputable_count((cid, cindex)), 1);
		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex), &alice),
			Reputation::UnverifiedReputable
		);
		assert_eq!(
			EncointerCeremonies::participant_reputation((cid, cindex - 1), &alice),
			Reputation::VerifiedLinked
		);

		assert_err!(
			EncointerCeremonies::unregister_participant(Origin::signed(alice.clone()), cid, None),
			Error::<TestRuntime>::ReputationCommunityCeremonyRequired,
		);

		EncointerCeremonies::fake_reputation(
			(cid, cindex - 1),
			&alice,
			Reputation::VerifiedUnlinked,
		);

		assert_err!(
			EncointerCeremonies::unregister_participant(
				Origin::signed(alice.clone()),
				cid,
				Some((cid, cindex - 1))
			),
			Error::<TestRuntime>::ReputationMustBeLinked,
		);
	})
}

#[test]
fn unregister_participant_works_with_newbies() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cindex = EncointerScheduler::current_ceremony_index();
		IssuedRewards::<TestRuntime>::insert((cid, cindex - 1), 0, MeetupResult::Ok);
		let bootstrapper = account_id(&AccountKeyring::Ferdie.pair());
		EncointerCommunities::insert_bootstrappers(cid, vec![bootstrapper.clone()]);
		assert!(EncointerBalances::issue(cid, &bootstrapper, NominalIncome::from_num(1)).is_ok());

		let alice = account_id(&AccountKeyring::Alice.pair());
		assert_ok!(register(alice.clone(), cid, None));
		assert_eq!(EncointerCeremonies::newbie_registry((cid, cindex), 1).unwrap(), alice);
		assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 1);

		assert_ok!(EncointerCeremonies::unregister_participant(
			Origin::signed(alice.clone()),
			cid,
			Some((cid, cindex - 1))
		));
		assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 0);
	})
}

#[test]
fn unregister_participant_fails_in_wrong_phase() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cindex = EncointerScheduler::current_ceremony_index();
		IssuedRewards::<TestRuntime>::insert((cid, cindex - 1), 0, MeetupResult::Ok);
		let bootstrapper = account_id(&AccountKeyring::Ferdie.pair());
		EncointerCommunities::insert_bootstrappers(cid, vec![bootstrapper.clone()]);
		assert!(EncointerBalances::issue(cid, &bootstrapper, NominalIncome::from_num(1)).is_ok());

		let a = AccountKeyring::Alice.pair();
		let alice = account_id(&a);

		run_to_next_phase();
		assert_err!(
			EncointerCeremonies::unregister_participant(
				Origin::signed(alice.clone()),
				cid,
				Some((cid, cindex - 1))
			),
			Error::<TestRuntime>::RegisteringOrAttestationPhaseRequired
		);
	})
}

#[test]
fn unregister_participant_fails_with_inexistent_community() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cindex = EncointerScheduler::current_ceremony_index();
		IssuedRewards::<TestRuntime>::insert((cid, cindex - 1), 0, MeetupResult::Ok);
		let bootstrapper = account_id(&AccountKeyring::Ferdie.pair());
		EncointerCommunities::insert_bootstrappers(cid, vec![bootstrapper.clone()]);
		assert!(EncointerBalances::issue(cid, &bootstrapper, NominalIncome::from_num(1)).is_ok());

		let a = AccountKeyring::Alice.pair();
		let alice = account_id(&a);

		assert_err!(
			EncointerCeremonies::unregister_participant(
				Origin::signed(alice.clone()),
				CommunityIdentifier::from_str("aaaaabbbbb").unwrap(),
				Some((cid, cindex - 1))
			),
			Error::<TestRuntime>::InexistentCommunity
		);
	})
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
fn set_endorsement_tickets_per_reputable_errs_with_bad_origin() {
	new_test_ext().execute_with(|| {
		assert_dispatch_err(
			EncointerCeremonies::set_endorsement_tickets_per_reputable(
				Origin::signed(AccountKeyring::Bob.into()),
				1u8,
			),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn set_endorsement_tickets_per_reputable_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(EncointerCeremonies::set_endorsement_tickets_per_reputable(
			Origin::signed(master()),
			2u8
		));

		assert_eq!(EncointerCeremonies::endorsement_tickets_per_reputable(), 2u8);
		assert_ok!(EncointerCeremonies::set_endorsement_tickets_per_reputable(
			Origin::signed(master()),
			3u8
		));

		assert_eq!(EncointerCeremonies::endorsement_tickets_per_reputable(), 3u8);
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
		let reputable_pair = &AccountKeyring::Bob.pair();
		let reputable = account_id(reputable_pair);
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

		assert_ok!(register(newbie.clone(), cid, None));
		assert_ok!(register_as_reputable(reputable_pair, cid));
		assert_ok!(register(endorsee.clone(), cid, None));
		assert_ok!(register(bootstrapper.clone(), cid, None));

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
		let bootstrapper = account_id(&AccountKeyring::Alice.pair());
		let bootstrapper2 = account_id(&AccountKeyring::Bob.pair());
		EncointerCommunities::insert_bootstrappers(
			cid,
			vec![bootstrapper.clone(), bootstrapper2.clone()],
		);

		let reputable_pair = &AccountKeyring::Ferdie.pair();
		let reputable = account_id(reputable_pair);

		let reputable2_pair = &AccountKeyring::Charlie.pair();
		let reputable2 = account_id(reputable2_pair);

		assert!(EncointerBalances::issue(cid, &reputable, NominalIncome::from_num(1)).is_ok());

		assert_ok!(register(bootstrapper.clone(), cid, None));
		assert_ok!(register(bootstrapper2.clone(), cid, None));

		assert_ok!(register_as_reputable(reputable_pair, cid));

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

		assert_ok!(register_as_reputable(reputable2_pair, cid));
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

#[test]
fn attest_attendees_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = AccountKeyring::Alice.pair();
		let bob = AccountKeyring::Bob.pair();
		let ferdie = AccountKeyring::Ferdie.pair();
		let eve = AccountKeyring::Eve.pair();
		let cindex = EncointerScheduler::current_ceremony_index();
		register_alice_bob_ferdie(cid);
		run_to_next_phase();
		run_to_next_phase();
		// Attesting
		assert_eq!(
			EncointerCeremonies::get_meetup_index((cid, cindex), &account_id(&alice)).unwrap(),
			1
		);

		EncointerCeremonies::attest_attendees(
			Origin::signed(account_id(&alice)),
			cid,
			3,
			vec![account_id(&alice), account_id(&ferdie)],
		)
		.unwrap();

		assert_eq!(
			last_event::<TestRuntime>(),
			Some(Event::AttestationsRegistered(cid, 1, 1, alice.public().into()).into())
		);

		assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 1);
		assert_eq!(EncointerCeremonies::attestation_index((cid, cindex), &account_id(&alice)), 1);
		let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1).unwrap();
		// attestation for self is ignored
		assert!(wit_vec.len() == 1);
		assert!(wit_vec.contains(&account_id(&ferdie)));

		EncointerCeremonies::attest_attendees(
			Origin::signed(account_id(&bob)),
			cid,
			4,
			vec![account_id(&alice), account_id(&ferdie)],
		)
		.unwrap();

		assert_eq!(
			last_event::<TestRuntime>(),
			Some(Event::AttestationsRegistered(cid, 1, 2, bob.public().into()).into())
		);

		assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 2);
		assert_eq!(EncointerCeremonies::attestation_index((cid, cindex), &account_id(&bob)), 2);
		let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &2).unwrap();
		assert!(wit_vec.len() == 2);
		assert!(wit_vec.contains(&account_id(&alice)));
		assert!(wit_vec.contains(&account_id(&ferdie)));

		assert_eq!(
			EncointerCeremonies::meetup_participant_count_vote((cid, cindex), account_id(&alice)),
			3
		);
		assert_eq!(
			EncointerCeremonies::meetup_participant_count_vote((cid, cindex), account_id(&bob)),
			4
		);
		// TEST: re-registering must overwrite previous entry
		EncointerCeremonies::attest_attendees(
			Origin::signed(account_id(&alice)),
			cid,
			3,
			vec![account_id(&bob), account_id(&ferdie)],
		)
		.unwrap();
		assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 2);

		// someone who is not meetup participant will be skipped
		EncointerCeremonies::attest_attendees(
			Origin::signed(account_id(&ferdie)),
			cid,
			4,
			vec![account_id(&bob), account_id(&eve)],
		)
		.unwrap();

		assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 3);
		assert_eq!(EncointerCeremonies::attestation_index((cid, cindex), &account_id(&ferdie)), 3);
		let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &3).unwrap();
		assert!(wit_vec.len() == 1);
		assert!(wit_vec.contains(&account_id(&bob)));
	});
}

#[test]
fn has_reputation_works() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let cid2 = register_test_community::<TestRuntime>(None, 1.0, 1.0);

		let alice = account_id(&AccountKeyring::Alice.pair());

		run_to_next_phase();
		run_to_next_phase();
		run_to_next_phase();

		run_to_next_phase();
		run_to_next_phase();
		run_to_next_phase();

		let cindex = EncointerScheduler::current_ceremony_index();

		assert_eq!(cindex, 3);

		assert_eq!(EncointerCeremonies::has_reputation(&alice, &cid), false);

		// acausal cindex
		EncointerCeremonies::fake_reputation((cid, 4), &alice, Reputation::VerifiedUnlinked);

		assert_eq!(EncointerCeremonies::has_reputation(&alice, &cid), false);

		// reputation of different community doesn't count
		EncointerCeremonies::fake_reputation((cid2, 1), &alice, Reputation::VerifiedUnlinked);

		assert_eq!(EncointerCeremonies::has_reputation(&alice, &cid), false);

		// reputation type does not qualify
		EncointerCeremonies::fake_reputation((cid, 1), &alice, Reputation::Unverified);

		assert_eq!(EncointerCeremonies::has_reputation(&alice, &cid), false);

		EncointerCeremonies::fake_reputation((cid, 1), &alice, Reputation::UnverifiedReputable);

		assert_eq!(EncointerCeremonies::has_reputation(&alice, &cid), false);

		// reputation type qualifies
		EncointerCeremonies::fake_reputation((cid, 1), &alice, Reputation::VerifiedLinked);

		assert_eq!(EncointerCeremonies::has_reputation(&alice, &cid), true);

		EncointerCeremonies::fake_reputation((cid, 1), &alice, Reputation::VerifiedUnlinked);

		assert_eq!(EncointerCeremonies::has_reputation(&alice, &cid), true);
	});
}

#[test]
fn is_endorsed_works() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let alice = account_id(&AccountKeyring::Alice.pair());

		assert_eq!(EncointerCeremonies::is_endorsed(&alice, &(cid, 4)), None);

		Endorsees::<TestRuntime>::insert((cid, 2), &alice, ());

		assert_eq!(EncointerCeremonies::is_endorsed(&alice, &(cid, 4)), Some(2));

		// above reputation lifetime
		assert_eq!(EncointerCeremonies::is_endorsed(&alice, &(cid, 9)), None);
	});
}
