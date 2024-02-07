use super::*;

#[test]
fn group_indices_by_value_works() {
	let participants: Participants = vec![0, 1, 2, 3, 4];
	let num_attestations: Vec<usize> = vec![2, 0, 3, 2, 0];
	assert_eq!(
		group_indices_by_value(participants, &num_attestations).unwrap(),
		vec![(0, vec![1, 4]), (2, vec![0, 3]), (3, vec![2])]
	);
}
#[test]
fn group_participants_by_num_outgoing_attestations_works() {
	let participants: Participants = vec![0, 1, 2, 3, 4];
	let participant_attestations: Vec<Vec<usize>> =
		vec![vec![1, 2, 3], vec![3], vec![0, 1, 3], vec![1, 2], vec![0, 1, 2]];
	let expected: Vec<ParticipantGroup> = vec![(1, vec![1]), (2, vec![3]), (3, vec![0, 2, 4])];
	assert_eq!(
		group_participants_by_num_outgoing_attestations(participants, &participant_attestations)
			.unwrap(),
		expected
	);
}

#[test]
fn group_participants_by_num_incoming_attestations_works() {
	let participants: Participants = vec![0, 1, 2, 3, 4];
	let participant_attestations: Attestations =
		vec![vec![1, 2, 3], vec![3], vec![0, 1, 3], vec![1, 2], vec![0, 1, 2]];
	let expected: Vec<ParticipantGroup> =
		vec![(0, vec![4]), (2, vec![0]), (3, vec![2, 3]), (4, vec![1])];
	assert_eq!(
		group_participants_by_num_incoming_attestations(participants, &participant_attestations)
			.unwrap(),
		expected
	);
}

#[test]
fn filter_attestations_works() {
	let participants: Participants = vec![0, 2, 4];
	let participant_attestations: Attestations =
		vec![vec![2, 3], vec![3, 4], vec![0, 1], vec![2, 1, 3], vec![0, 2, 3]];
	let expected: Attestations = vec![vec![2], vec![4], vec![0], vec![2], vec![0, 2]];
	assert_eq!(filter_attestations(&participants, participant_attestations), expected);
}

#[test]
fn get_excluded_participants_no_vote_works() {
	let participants: Participants = vec![0, 1, 2, 3, 4];
	let participant_votes: Vec<u32> = vec![0, 1, 2, 2, 0];
	let excluded_participants: Participants = vec![0, 4];
	assert_eq!(
		get_excluded_participants_no_vote(&participants, &participant_votes)
			.unwrap()
			.iter()
			.map(|p| p.0)
			.collect::<Participants>(),
		excluded_participants
	);
}

#[test]
fn get_excluded_participants_wrong_vote_works() {
	let participants: Participants = vec![0, 1, 2, 3, 4];
	let participant_votes: Vec<u32> = vec![3, 1, 2, 2, 5];
	let excluded_participants: Participants = vec![0, 1, 4];
	assert_eq!(
		get_excluded_participants_wrong_vote(&participants, &participant_votes, 2)
			.unwrap()
			.iter()
			.map(|p| p.0)
			.collect::<Participants>(),
		excluded_participants
	);
}

#[test]
fn get_excluded_participants_num_attestations_works() {
	let participants: Participants = vec![0, 1, 2, 3, 4];
	let participant_attestations: Attestations =
		vec![vec![1, 2, 4], vec![1], vec![0, 1, 4], vec![0, 1, 2, 4], vec![0, 1, 2]];
	let excluded_participants: Vec<(usize, ExclusionReason)> = vec![
		(3, ExclusionReason::TooFewIncomingAttestations),
		(1, ExclusionReason::TooFewOutgoingAttestations),
	];
	assert_eq!(
		get_excluded_participants_num_attestations(&participants, participant_attestations, |n| n
			- 1)
		.unwrap(),
		excluded_participants
	);
}

#[test]
fn find_majority_vote_works() {
	let participants: Participants = vec![0, 1, 2, 3, 4];
	let participant_votes: Vec<u32> = vec![1, 1, 2, 3, 1];
	assert_eq!(find_majority_vote(&participants, &participant_votes), Ok((1u32, 3u32, false)));
}

#[test]
fn find_majority_vote_works_with_unanimous_vote() {
	let participants: Participants = vec![0, 1, 2, 3, 4];
	let participant_votes: Vec<u32> = vec![1, 1, 1, 1, 1];
	assert_eq!(find_majority_vote(&participants, &participant_votes), Ok((1u32, 5u32, true)));
}

#[test]
fn get_participant_judgements_works_case_0() {
	// Scenario: Everyone attests everyone
	validate_participant_judgements(
		vec![0, 1, 2],
		vec![3, 3, 3],
		vec![vec![1, 2], vec![0, 2], vec![0, 1]],
		ParticipantJudgements {
			legit: vec![0, 1, 2],
			excluded: vec![],
			early_rewards_possible: true,
		},
	);
}

#[test]
fn get_participant_judgements_works_case_1() {
	// Scenario: One participant did not receive any attestations and therefore has a vote of 0
	validate_participant_judgements(
		vec![0, 1, 2, 3],
		vec![0, 3, 3, 3],
		vec![vec![1, 2, 3], vec![2, 3], vec![1, 3], vec![1, 2]],
		ParticipantJudgements {
			legit: vec![1, 2, 3],
			excluded: vec![ExcludedParticipant { index: 0, reason: ExclusionReason::NoVote }],
			early_rewards_possible: true,
		},
	);
}

#[test]
fn get_participant_judgements_works_case_2() {
	// Scenario: One participant did not vote like majority
	validate_participant_judgements(
		vec![0, 1, 2, 3],
		vec![1, 3, 3, 3],
		vec![vec![1, 2, 3], vec![0, 2, 3], vec![0, 1, 3], vec![0, 1, 2]],
		ParticipantJudgements {
			legit: vec![1, 2, 3],
			excluded: vec![ExcludedParticipant { index: 0, reason: ExclusionReason::WrongVote }],
			early_rewards_possible: false,
		},
	);
}

#[test]
fn get_participant_judgements_works_case_3() {
	// Scenario: 5 participants
	// 0 has a broken phone and can only be attested by 1 and 2 and attests nobody
	// we expect that everybody except for 0 gets a reward
	validate_participant_judgements(
		vec![0, 1, 2, 3, 4],
		vec![5, 5, 5, 5, 5],
		vec![vec![], vec![0, 2, 3, 4], vec![0, 1, 3, 4], vec![1, 2, 4], vec![1, 2, 3]],
		ParticipantJudgements {
			legit: vec![1, 2, 3, 4],
			excluded: vec![ExcludedParticipant {
				index: 0,
				reason: ExclusionReason::TooFewOutgoingAttestations,
			}],
			early_rewards_possible: false,
		},
	);
}

#[test]
fn get_participant_judgements_works_case_4() {
	// Scenario: 5 participants
	// 0 has a broken screen and can only be attested by 1 and 2 but attests everybody else
	// we expect that everybody except for 0 gets a reward
	validate_participant_judgements(
		vec![0, 1, 2, 3, 4],
		vec![5, 5, 5, 5, 5],
		vec![vec![1, 2, 3, 4], vec![0, 2, 3, 4], vec![0, 1, 3, 4], vec![1, 2, 4], vec![1, 2, 3]],
		ParticipantJudgements {
			legit: vec![1, 2, 3, 4],
			excluded: vec![ExcludedParticipant {
				index: 0,
				reason: ExclusionReason::TooFewIncomingAttestations,
			}],
			early_rewards_possible: false,
		},
	);
}

fn validate_participant_judgements(
	participants: Participants,
	participant_votes: Vec<u32>,
	participant_attestations: Attestations,
	participant_judgements: ParticipantJudgements,
) {
	let attestation_threshold_fn = |i| i - 1;

	assert_eq!(
		get_participant_judgements(
			&participants,
			&participant_votes,
			&participant_attestations,
			attestation_threshold_fn,
		)
		.unwrap(),
		participant_judgements
	);
}

#[test]
fn vote_yields_majority_works() {
	assert_eq!(vote_yields_majority(5, 3), true);
	assert_eq!(vote_yields_majority(5, 2), false);
	assert_eq!(vote_yields_majority(4, 2), false);
}

#[test]
fn num_attestations_matches_vote_works() {
	assert_eq!(
		num_attestations_matches_vote(
			&vec![0, 1, 2, 3],
			&vec![
				vec![1, 2, 3],
				vec![0, 2, 3],
				vec![0, 1, 3],
				vec![0, 1, 2],
				vec![0, 1, 2, 3, 4, 5]
			],
			4
		),
		true
	);

	assert_eq!(
		num_attestations_matches_vote(
			&vec![0, 1, 2, 3],
			&vec![vec![1, 2, 3], vec![0, 2], vec![0, 1, 3], vec![0, 1, 2], vec![0, 1, 2, 3, 4, 5]],
			4
		),
		false
	);
}

#[test]
fn attestation_graph_is_fully_connected_works() {
	assert_eq!(
		attestation_graph_is_fully_connected(
			vec![0, 1, 2, 3],
			vec![
				vec![1, 2, 3],
				vec![0, 2, 3],
				vec![0, 1, 3],
				vec![0, 1, 2],
				vec![0, 1, 2, 3, 4, 5]
			],
		),
		true
	);

	assert_eq!(
		attestation_graph_is_fully_connected(
			vec![0, 1, 2, 3],
			vec![
				vec![1, 2, 3],
				vec![0, 2, 4],
				vec![0, 1, 3],
				vec![0, 1, 2],
				vec![0, 1, 2, 3, 4, 5]
			],
		),
		false
	);
}

#[test]
fn early_rewards_possible_works() {
	assert_eq!(
		early_rewards_possible(
			vec![0, 1, 2, 3],
			vec![
				vec![1, 2, 3],
				vec![0, 2, 3],
				vec![0, 1, 3],
				vec![0, 1, 2],
				vec![0, 1, 2, 3, 4, 5]
			],
			5,
			4,
			true
		),
		true
	);
}
#[test]
fn early_rewards_possible_works_unanimous_vote() {
	assert_eq!(
		early_rewards_possible(
			vec![0, 1, 2, 3],
			vec![
				vec![1, 2, 3],
				vec![0, 2, 3],
				vec![0, 1, 3],
				vec![0, 1, 2],
				vec![0, 1, 2, 3, 4, 5]
			],
			5,
			4,
			false
		),
		false
	);
}
#[test]
fn early_rewards_possible_works_vote_is_not_majority() {
	assert_eq!(
		early_rewards_possible(
			vec![0, 1, 2, 3],
			vec![
				vec![1, 2, 3],
				vec![0, 2, 3],
				vec![0, 1, 3],
				vec![0, 1, 2],
				vec![0, 1, 2, 3, 4, 5]
			],
			5,
			2,
			true
		),
		false
	);
}
#[test]
fn early_rewards_possible_works_attesttations_do_not_match_vote() {
	assert_eq!(
		early_rewards_possible(
			vec![0, 1, 2, 3],
			vec![
				vec![1, 2, 3],
				vec![0, 2, 3, 4],
				vec![0, 1, 3],
				vec![0, 1, 2],
				vec![0, 1, 2, 3, 4, 5]
			],
			5,
			4,
			true
		),
		false
	);
}
#[test]
fn early_rewards_possible_works_attestation_graph_is_not_fully_connected() {
	assert_eq!(
		early_rewards_possible(
			vec![0, 1, 2, 3],
			vec![
				vec![1, 2, 3],
				vec![0, 2, 4],
				vec![0, 1, 3],
				vec![0, 1, 2],
				vec![0, 1, 2, 3, 4, 5]
			],
			5,
			4,
			true
		),
		false
	);
}
