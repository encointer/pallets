pub fn update_participants_no_vote(
	participants: &Vec<usize>,
	participant_votes: &Vec<u32>,
) -> UpdatedParticipants {
	let mut included_participants: Vec<usize> = vec![];
	for i in participants {
		match participant_votes[*i] {
			v if v > 0 => included_participants.push(*i),
			_ => continue,
		}
	}
	get_updated_participants(participants, &included_participants)
}

pub fn update_participants_wrong_vote(
	participants: &Vec<usize>,
	participant_votes: &Vec<u32>,
	n_confirmed: u32,
) -> UpdatedParticipants {
	let mut included_participants: Vec<usize> = vec![];
	for i in participants {
		if participant_votes[*i] == n_confirmed {
			included_participants.push(*i)
		}
	}
	get_updated_participants(participants, &included_participants)
}

pub fn update_participants_outgoing_attestations(
	participants: &Vec<usize>,
	participant_attestations: &Vec<Vec<usize>>,
	threshold: usize,
) -> UpdatedParticipants {
	let mut included_participants: Vec<usize> = vec![];
	for i in participants {
		if participant_attestations[*i].len() < threshold {
			continue
		}
		included_participants.push(*i);
	}
	get_updated_participants(participants, &included_participants)
}

pub fn update_participants_incoming_attestations(
	participants: &Vec<usize>,
	participant_attestations: &Vec<Vec<usize>>,
	threshold: usize,
) -> UpdatedParticipants {
	let mut included_participants: Vec<usize> = vec![];
	for i in participants {
		let mut num_incoming_attestations = 0;
		for (j, attestations) in participant_attestations.iter().enumerate() {
			if *i == j {
				continue
			};
			if attestations.contains(i) {
				num_incoming_attestations += 1;
			}
		}
		if num_incoming_attestations < threshold {
			continue
		}
		included_participants.push(*i);
	}
	get_updated_participants(participants, &included_participants)
}

pub fn find_majority_vote(
	participants: &Vec<usize>,
	participant_votes: &Vec<u32>,
) -> Result<(u32, u32), MajorityVoteError> {
	let mut n_vote_candidates: Vec<(u32, u32)> = vec![];
	for i in participants {
		let this_vote = participant_votes[*i];
		match n_vote_candidates.iter().position(|&(n, _c)| n == this_vote) {
			Some(idx) => n_vote_candidates[idx].1 += 1, //safe; <= number of candidates
			_ => n_vote_candidates.insert(0, (this_vote, 1)),
		};
	}

	if n_vote_candidates.is_empty() {
		return Err(MajorityVoteError::BallotEmpty)
	}
	// sort by descending vote count
	n_vote_candidates.sort_by(|a, b| b.1.cmp(&a.1));
	if n_vote_candidates[0].1 < 3 {
		//safe; n_vote_candidate not empty checked aboveÏ
		return Err(MajorityVoteError::NoDependableVote)
	}
	let (n_confirmed, vote_count) = n_vote_candidates[0];
	Ok((n_confirmed, vote_count))
}

pub enum MajorityVoteError {
	BallotEmpty,
	NoDependableVote,
}

pub struct UpdatedParticipants {
	pub included: Vec<usize>,
	pub excluded: Vec<usize>,
}

fn get_updated_participants(
	participants: &Vec<usize>,
	included_participants: &Vec<usize>,
) -> UpdatedParticipants {
	let excluded_participants: Vec<usize> = participants
		.clone()
		.into_iter()
		.filter(|i| !included_participants.contains(i))
		.collect();
	UpdatedParticipants {
		included: included_participants.clone(),
		excluded: excluded_participants.clone(),
	}
}
