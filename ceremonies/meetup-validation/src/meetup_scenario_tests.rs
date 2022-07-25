use super::*;
use rstest::*;
use sp_std::cmp::max;
#[rstest(
	meetup_size,
	num_attendees,
	n,
	case(16, 16, 1),
	case(16, 16, 5),
	case(16, 16, 9),
	case(16, 9, 0),
	case(16, 9, 1),
	case(16, 9, 5),
	case(16, 3, 0),
	case(16, 3, 1),
	case(12, 7, 0),
	case(12, 7, 3),
	case(12, 5, 1),
	case(12, 5, 2),
	case(8, 8, 0),
	case(8, 8, 1),
	case(8, 8, 3),
	case(8, 8, 5),
	case(8, 6, 0),
	case(8, 7, 1),
	case(8, 5, 3),
	case(3, 3, 0),
	case(3, 3, 1),
	case(3, 3, 2)
)]
fn n_attendees_do_not_attest_anyone(meetup_size: usize, num_attendees: usize, n: usize) {
	let participants: Participants = (0..meetup_size).collect();
	let mut participant_votes = vec![num_attendees as u32; num_attendees];
	participant_votes.append(&mut vec![0u32; meetup_size - num_attendees]);
	let mut participant_attestations: Attestations = vec![];
	for i in 0..(num_attendees - n) {
		participant_attestations.push((0..num_attendees).filter(|&j| j != i).collect());
	}
	for _ in 0..(meetup_size - num_attendees + n) {
		participant_attestations.push(vec![]);
	}
	assert_eq!(
		get_participant_judgements(
			&participants,
			&participant_votes,
			&participant_attestations,
			|n| n - 1,
		)
		.unwrap()
		.legit,
		(0..(num_attendees - n)).collect::<Vec<usize>>()
	);
}

#[rstest(
	meetup_size,
	num_attendees,
	n,
	case(16, 16, 1),
	case(16, 16, 5),
	case(16, 16, 9),
	case(16, 9, 0),
	case(16, 9, 1),
	case(16, 9, 5),
	case(16, 3, 0),
	case(16, 3, 1),
	case(12, 7, 0),
	case(12, 7, 3),
	case(12, 5, 1),
	case(12, 5, 2),
	case(8, 8, 0),
	case(8, 8, 1),
	case(8, 8, 3),
	case(8, 8, 5),
	case(8, 6, 0),
	case(8, 7, 1),
	case(8, 5, 3),
	case(3, 3, 0),
	case(3, 3, 1),
	case(3, 3, 2)
)]
fn attendee_is_not_attested_by_n_others(meetup_size: usize, num_attendees: usize, n: usize) {
	let participants: Participants = (0..meetup_size).collect();
	let mut participant_votes = vec![num_attendees as u32; num_attendees];
	participant_votes.append(&mut vec![0u32; meetup_size - num_attendees]);
	let mut participant_attestations: Attestations = vec![];

	for i in 0..num_attendees {
		let first_index = if i <= n { 1 } else { 0 };
		participant_attestations.push((first_index..num_attendees).filter(|&j| j != i).collect());
	}

	for _ in 0..(meetup_size - num_attendees) {
		participant_attestations.push(vec![]);
	}
	let mut expected = (0..num_attendees).collect::<Vec<usize>>();
	if n > 0 {
		if n == 1 {
			expected = vec![0];
			expected.append(&mut (2..num_attendees).collect());
		} else {
			expected = (1..num_attendees).collect();
		}
	}
	assert_eq!(
		get_participant_judgements(
			&participants,
			&participant_votes,
			&participant_attestations,
			|n| n - 1,
		)
		.unwrap()
		.legit,
		expected
	);
}

#[rstest(
	meetup_size,
	num_attendees,
	n,
	case(16, 16, 0),
	case(16, 16, 1),
	case(16, 16, 5),
	case(16, 16, 9),
	case(16, 9, 0),
	case(16, 9, 1),
	case(16, 9, 5),
	case(16, 3, 0),
	case(12, 7, 0),
	case(12, 7, 3),
	case(12, 5, 1),
	case(12, 5, 2),
	case(8, 8, 0),
	case(8, 8, 1),
	case(8, 8, 3),
	case(8, 8, 5),
	case(8, 6, 0),
	case(8, 7, 1),
	case(8, 5, 3),
	case(3, 3, 0),
    // 50 / 50 meetup split
    case(12, 12, 6),
    case(14, 14, 7),
    case(12, 12, 6),
    case(10, 10, 5),
    case(8, 8, 4),
    case(6, 6, 3),
)]
fn adversary_holds_n_assignee_keys_and_self_attests(
	meetup_size: usize,
	num_attendees: usize,
	n: usize,
) {
	let participants: Participants = (0..meetup_size).collect();
	// attacker votes
	let num_attackers = n;
	let mut participant_votes = vec![num_attackers as u32; num_attackers];
	// honest votes
	let num_honest_participants = num_attendees - n;
	participant_votes.append(&mut vec![num_honest_participants as u32; num_honest_participants]);

	participant_votes.append(&mut vec![0u32; meetup_size - num_attendees]);
	let mut participant_attestations: Attestations = vec![];

	for i in 0..num_attackers {
		participant_attestations.push((0..num_attackers).filter(|&j| j != i).collect());
	}
	for i in num_attackers..num_attendees {
		participant_attestations.push((num_attackers..num_attendees).filter(|&j| j != i).collect());
	}

	for _ in num_attendees..meetup_size {
		participant_attestations.push(vec![]);
	}

	let expected;
	if num_attackers as f64 > (num_attendees as f64 / 2.0) {
		// attacker wins
		expected = (0..num_attackers).collect::<Vec<usize>>();
	} else if num_attackers as f64 == (num_attendees as f64 / 2.0) {
		expected = vec![];
	} else {
		expected = (num_attackers..num_attendees).collect::<Vec<usize>>();
	}
	assert_eq!(
		get_participant_judgements(
			&participants,
			&participant_votes,
			&participant_attestations,
			|n| n - 1,
		)
		.unwrap()
		.legit,
		expected
	);
}

#[rstest(
	meetup_size,
	num_attendees,
	n,
	case(16, 16, 1),
	case(16, 16, 5),
	case(16, 16, 9),
	case(16, 9, 0),
	case(16, 9, 1),
	case(16, 9, 5),
	case(16, 4, 0),
	case(12, 7, 0),
	case(12, 7, 3),
	case(12, 5, 1),
	case(8, 8, 0),
	case(8, 8, 1),
	case(8, 8, 3),
	case(8, 8, 5),
	case(8, 6, 0),
	case(8, 7, 1),
	case(8, 5, 3),
	case(4, 4, 0)
)]
fn n_attendees_vote_plus_one_and_attest_absent_assignee(
	meetup_size: usize,
	num_attendees: usize,
	n: usize,
) {
	let participants: Participants = (0..meetup_size).collect();
	// attacker votes
	let num_attackers = n;
	let mut participant_votes = vec![(num_attackers + 1) as u32; num_attackers];
	// honest votes
	let num_honest_participants = num_attendees - n - 1;
	participant_votes.append(&mut vec![num_honest_participants as u32; num_honest_participants]);

	participant_votes.append(&mut vec![0u32; meetup_size - num_attendees + 1]);
	let mut participant_attestations: Attestations = vec![];

	for i in 0..num_attackers {
		let mut attestations: Vec<usize> = (0..num_attackers).filter(|&j| j != i).collect();
		// attest the non present attacker
		attestations.append(&mut vec![num_attendees - 1]);
		participant_attestations.push(attestations);
	}
	for i in num_attackers..(num_attendees - 1) {
		participant_attestations
			.push((num_attackers..(num_attendees - 1)).filter(|&j| j != i).collect());
	}

	for _ in (num_attendees - 1)..meetup_size {
		participant_attestations.push(vec![]);
	}

	let expected;
	if num_attackers as f64 > (num_attendees as f64 / 2.0) {
		expected = (0..num_attackers).collect::<Vec<usize>>();
	} else if num_attackers as f64 == (num_attendees as f64 / 2.0) {
		expected = vec![];
	} else {
		expected = (num_attackers..(num_attendees - 1)).collect::<Vec<usize>>();
	}
	assert_eq!(
		get_participant_judgements(
			&participants,
			&participant_votes,
			&participant_attestations,
			|n| n - 1,
		)
		.unwrap()
		.legit,
		expected
	);
}

#[rstest(
	meetup_size,
	n,
	honest_participants_receive_rewards,
	case(13, 6, true),
	case(11, 5, true),
	case(10, 4, true),
	case(9, 4, true),
	case(8, 3, true),
	case(7, 3, true),
	// if attacker controls half or more, he wins
	case(13, 7, false),
	case(11, 6, false),
	case(10, 5, false),
	case(9, 5, false),
	case(8, 4, false),
	case(7, 4, false),
)]
fn adversary_holds_n_assignee_keys_and_self_attests_and_votes_like_honest_participants(
	meetup_size: usize,
	n: usize,
	honest_participants_receive_rewards: bool,
) {
	let participants: Participants = (0..meetup_size).collect();
	let num_attackers = n;
	let num_honest_participants = meetup_size - n;
	let participant_votes = vec![num_honest_participants as u32; meetup_size];

	let mut participant_attestations: Attestations = vec![];

	for i in 0..num_attackers {
		participant_attestations.push((0..num_attackers).filter(|&j| j != i).collect());
	}
	for i in num_attackers..meetup_size {
		participant_attestations.push((num_attackers..meetup_size).filter(|&j| j != i).collect());
	}

	let legit_participants = get_participant_judgements(
		&participants,
		&participant_votes,
		&participant_attestations,
		|i: usize| max(if i > 5 { i.saturating_sub(2) } else { i.saturating_sub(1) }, 1),
	)
	.unwrap()
	.legit;
	for i in num_attackers..meetup_size {
		assert!(legit_participants.contains(&i) == honest_participants_receive_rewards);
	}
}

#[rstest(
	meetup_size,
	n,
	case(15, 4),
	case(15, 5),
	case(15, 7),
	case(13, 6),
	case(10, 4),
	case(9, 4),
	case(8, 3),
	case(7, 3)
)]
fn n_honest_participants_dont_have_internet_and_n_attackers_try_to_provoke_early_payout(
	meetup_size: usize,
	n: usize,
) {
	let participants: Participants = (0..meetup_size).collect();
	let num_attackers = n;
	let num_honest_participants = meetup_size - n;
	// attackers vote like honest participants
	let mut participant_votes = vec![num_honest_participants as u32; meetup_size - n];

	// n participants do not have internet
	participant_votes.append(&mut vec![0 as u32; n]);

	let mut participant_attestations: Attestations = vec![];

	for i in 0..num_attackers {
		// push enough attestations such that number of attestations matches vote
		participant_attestations.push((0..num_honest_participants).filter(|&j| j != i).collect());
	}
	for i in num_attackers..(meetup_size - n) {
		participant_attestations.push((num_attackers..meetup_size).filter(|&j| j != i).collect());
	}

	// n participants do not have internet
	for _i in (meetup_size - n)..meetup_size {
		participant_attestations.push(vec![]);
	}

	let early_rewards_possible = get_participant_judgements(
		&participants,
		&participant_votes,
		&participant_attestations,
		|i: usize| max(if i > 5 { i.saturating_sub(2) } else { i.saturating_sub(1) }, 1),
	)
	.unwrap()
	.early_rewards_possible;

	assert!(!early_rewards_possible);
}
