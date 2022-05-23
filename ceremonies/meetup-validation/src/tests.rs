use super::*;
use crate::get_excluded_participants_no_vote;

#[test]
fn get_excluded_participants_no_vote_works() {
	let participants: Vec<usize> = vec![0, 1, 2, 3, 4];
	let participant_votes: Vec<u32> = vec![0, 1, 2, 2, 0];
	let excluded_participants: Vec<usize> = vec![0, 4];
	assert_eq!(
		get_excluded_participants_no_vote(&participants, &participant_votes),
		excluded_participants
	);
}

#[test]
fn get_excluded_participants_wrong_vote_works() {
	let participants: Vec<usize> = vec![0, 1, 2, 3, 4];
	let participant_votes: Vec<u32> = vec![3, 1, 2, 2, 5];
	let excluded_participants: Vec<usize> = vec![0, 1, 4];
	assert_eq!(
		get_excluded_participants_wrong_vote(&participants, &participant_votes, 2),
		excluded_participants
	);
}

#[test]
fn get_excluded_participants_outgoing_attestations_works() {
	let participants: Vec<usize> = vec![0, 1, 2, 3, 4];
	let participant_attestations: Vec<Vec<usize>> =
		vec![vec![1, 2, 3], vec![3], vec![0, 1, 3], vec![1, 2], vec![0, 1, 2]];
	let excluded_participants: Vec<usize> = vec![1, 3];
	assert_eq!(
		get_excluded_participants_outgoing_attestations(
			&participants,
			&participant_attestations,
			3
		),
		excluded_participants
	);
}

#[test]
fn get_excluded_participants_incoming_attestations_works() {
	let participants: Vec<usize> = vec![0, 1, 2, 3, 4];
	let participant_attestations: Vec<Vec<usize>> =
		vec![vec![1, 2, 3], vec![4], vec![0, 1, 2], vec![1, 2], vec![0, 1, 2]];
	let excluded_participants: Vec<usize> = vec![3, 4];
	assert_eq!(
		get_excluded_participants_incoming_attestations(
			&participants,
			&participant_attestations,
			2
		),
		excluded_participants
	);
}

#[test]
fn find_majority_vote_works() {
	let participants: Vec<usize> = vec![0, 1, 2, 3, 4];
	let participant_votes: Vec<u32> = vec![1, 1, 2, 3, 1];
	assert_eq!(find_majority_vote(&participants, &participant_votes), Ok((1u32, 3u32)));
}

#[test]
fn get_updated_participants_works_case_0() {
	let participants: Vec<usize> = vec![0, 1, 2];
	let participant_votes: Vec<u32> = vec![3, 3, 3];
	let participant_attestations: Vec<Vec<usize>> = vec![vec![1, 2], vec![0, 2], vec![0, 1]];

	let updated_participants =
		UpdatedParticipants { included: participants.clone(), excluded: vec![] };
	let outgoing_attestation_threshold_fn = |i| i - 1;
	let incoming_attestation_threshold_fn = |i| i - 1;

	assert_eq!(
		get_updated_participants(
			&participants,
			&participant_votes,
			&participant_attestations,
			outgoing_attestation_threshold_fn,
			incoming_attestation_threshold_fn
		)
		.unwrap(),
		updated_participants
	);
}

#[test]
fn get_updated_participants_works_case_1() {
	// Scenario: One participant did not vote
	let participants: Vec<usize> = vec![0, 1, 2, 3];
	let participant_votes: Vec<u32> = vec![0, 3, 3, 3];
	let participant_attestations: Vec<Vec<usize>> =
		vec![vec![1, 2, 3], vec![0, 2, 3], vec![0, 1, 3], vec![0, 1, 2]];

	let updated_participants = UpdatedParticipants {
		included: vec![1, 2, 3],
		excluded: vec![ExcludedParticipant { index: 0, reason: ExclusionReason::NoVote }],
	};
	let outgoing_attestation_threshold_fn = |i| i - 1;
	let incoming_attestation_threshold_fn = |i| i - 1;

	assert_eq!(
		get_updated_participants(
			&participants,
			&participant_votes,
			&participant_attestations,
			outgoing_attestation_threshold_fn,
			incoming_attestation_threshold_fn
		)
		.unwrap(),
		updated_participants
	);
}

#[test]
fn get_updated_participants_works_case_2() {
	// Scenario: One participant did not vote like majority
	let participants: Vec<usize> = vec![0, 1, 2, 3];
	let participant_votes: Vec<u32> = vec![1, 3, 3, 3];
	let participant_attestations: Vec<Vec<usize>> =
		vec![vec![1, 2, 3], vec![0, 2, 3], vec![0, 1, 3], vec![0, 1, 2]];

	let updated_participants = UpdatedParticipants {
		included: vec![1, 2, 3],
		excluded: vec![ExcludedParticipant { index: 0, reason: ExclusionReason::WrongVote }],
	};
	let outgoing_attestation_threshold_fn = |i| i - 1;
	let incoming_attestation_threshold_fn = |i| i - 1;

	assert_eq!(
		get_updated_participants(
			&participants,
			&participant_votes,
			&participant_attestations,
			outgoing_attestation_threshold_fn,
			incoming_attestation_threshold_fn
		)
		.unwrap(),
		updated_participants
	);
}

#[test]
fn get_updated_participants_works_case_3() {
	// Scenario: One participant has too few outgoing attestations
	let participants: Vec<usize> = vec![0, 1, 2, 3];
	let participant_votes: Vec<u32> = vec![3, 3, 3, 3];
	let participant_attestations: Vec<Vec<usize>> =
		vec![vec![1, 2], vec![0, 2, 3], vec![0, 1, 3], vec![0, 1, 2]];

	let updated_participants = UpdatedParticipants {
		included: vec![1, 2, 3],
		excluded: vec![ExcludedParticipant {
			index: 0,
			reason: ExclusionReason::TooFewOutgoingAttestations,
		}],
	};
	let outgoing_attestation_threshold_fn = |i| i - 1;
	// It works without loosening the requirements on incoming attestations
	// because no more incoming votes are expected from the malicious user
	let incoming_attestation_threshold_fn = |i| i - 1;

	assert_eq!(
		get_updated_participants(
			&participants,
			&participant_votes,
			&participant_attestations,
			outgoing_attestation_threshold_fn,
			incoming_attestation_threshold_fn
		)
		.unwrap(),
		updated_participants
	);
}

#[test]
fn get_updated_participants_works_case_4() {
	// Scenario: One participant has too few incoming attestations
	let participants: Vec<usize> = vec![0, 1, 2, 3];
	let participant_votes: Vec<u32> = vec![3, 3, 3, 3];
	let participant_attestations: Vec<Vec<usize>> =
		vec![vec![1, 2, 3], vec![0, 2, 3], vec![0, 1, 3], vec![1, 2]];

	let updated_participants = UpdatedParticipants {
		included: vec![1, 2, 3],
		excluded: vec![ExcludedParticipant {
			index: 0,
			reason: ExclusionReason::TooFewIncomingAttestations,
		}],
	};
	// loosening the threshold for outgoing attestations
	let outgoing_attestation_threshold_fn = |i| i - 2;
	let incoming_attestation_threshold_fn = |i| i - 1;

	assert_eq!(
		get_updated_participants(
			&participants,
			&participant_votes,
			&participant_attestations,
			outgoing_attestation_threshold_fn,
			incoming_attestation_threshold_fn
		)
		.unwrap(),
		updated_participants
	);
}
