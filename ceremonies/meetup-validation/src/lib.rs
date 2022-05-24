mod tests;

pub fn get_updated_participants(
	participants: &Vec<usize>,
	participant_votes: &Vec<u32>,
	participant_attestations: &Vec<Vec<usize>>,
	outgoing_attestation_threshold_fn: fn(usize) -> usize,
	incoming_attestation_threshold_fn: fn(usize) -> usize,
) -> Result<UpdatedParticipants, MeetupValidationError> {
	let mut updated_participants =
		UpdatedParticipants { included: participants.clone(), excluded: vec![] };
	updated_participants.exclude_participants(
		get_excluded_participants_no_vote(&updated_participants.included, participant_votes),
		ExclusionReason::NoVote,
	);

	let (n_confirmed, num_votes) =
		find_majority_vote(&updated_participants.included, participant_votes)?;

	updated_participants.exclude_participants(
		get_excluded_participants_wrong_vote(
			&updated_participants.included,
			participant_votes,
			n_confirmed,
		),
		ExclusionReason::WrongVote,
	);

	updated_participants.exclude_participants(
		get_excluded_participants_outgoing_attestations(
			&updated_participants.included,
			participant_attestations,
			outgoing_attestation_threshold_fn,
		),
		ExclusionReason::TooFewOutgoingAttestations,
	);

	let incoming_attestation_threshold =
		incoming_attestation_threshold_fn(updated_participants.included.len());

	updated_participants.exclude_participants(
		get_excluded_participants_incoming_attestations(
			&updated_participants.included,
			participant_attestations,
			incoming_attestation_threshold,
		),
		ExclusionReason::TooFewIncomingAttestations,
	);

	Ok(updated_participants)
}

fn get_excluded_participants_no_vote(
	participants: &Vec<usize>,
	participant_votes: &Vec<u32>,
) -> Vec<usize> {
	let mut excluded_participants: Vec<usize> = vec![];
	for i in participants {
		match participant_votes[*i] {
			v if v > 0 => continue,
			_ => excluded_participants.push(*i),
		}
	}
	excluded_participants
}

fn get_excluded_participants_wrong_vote(
	participants: &Vec<usize>,
	participant_votes: &Vec<u32>,
	n_confirmed: u32,
) -> Vec<usize> {
	let mut excluded_participants: Vec<usize> = vec![];
	for i in participants {
		if participant_votes[*i] != n_confirmed {
			excluded_participants.push(*i)
		}
	}
	excluded_participants
}

fn get_excluded_participants_outgoing_attestations(
	participants: &Vec<usize>,
	participant_attestations: &Vec<Vec<usize>>,
	threshold_fn: fn(usize) -> usize,
) -> Vec<usize> {
	let mut relevant_attestations = filter_attestations(participants, participant_attestations);

	let mut excluded_participants: Vec<usize> = vec![];
	let mut included_participants: Vec<usize> = participants.clone();

	let grouped_participants =
		group_participants_by_num_outgoing_attestations(participants, participant_attestations);

	for (num_attestations, ps) in grouped_participants {
		if num_attestations < threshold_fn(included_participants.len()) {
			ps.clone().into_iter().for_each(|p| excluded_participants.push(p));

			// remove the participants from the included participants and the attestation vectors
			included_participants.retain(|k| !ps.contains(k));
			filter_attestations(&included_participants, &relevant_attestations);
		}
	}
	excluded_participants
}

fn get_excluded_participants_incoming_attestations(
	participants: &Vec<usize>,
	participant_attestations: &Vec<Vec<usize>>,
	threshold: usize,
) -> Vec<usize> {
	let mut excluded_participants: Vec<usize> = vec![];
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
			excluded_participants.push(*i);
		}
	}
	excluded_participants
}

fn find_majority_vote(
	participants: &Vec<usize>,
	participant_votes: &Vec<u32>,
) -> Result<(u32, u32), MeetupValidationError> {
	let mut n_vote_candidates: Vec<(u32, u32)> = vec![];
	for i in participants {
		let this_vote = participant_votes[*i];
		match n_vote_candidates.iter().position(|&(n, _c)| n == this_vote) {
			Some(idx) => n_vote_candidates[idx].1 += 1, //safe; <= number of candidates
			_ => n_vote_candidates.insert(0, (this_vote, 1)),
		};
	}

	if n_vote_candidates.is_empty() {
		return Err(MeetupValidationError::BallotEmpty)
	}
	// sort by descending vote count
	n_vote_candidates.sort_by(|a, b| b.1.cmp(&a.1));
	if n_vote_candidates[0].1 < 3 {
		//safe; n_vote_candidate not empty checked aboveÏ
		return Err(MeetupValidationError::NoDependableVote)
	}
	let (n_confirmed, vote_count) = n_vote_candidates[0];
	Ok((n_confirmed, vote_count))
}

fn filter_attestations(
	participants: &Vec<usize>,
	participant_attestations: &Vec<Vec<usize>>,
) -> Vec<Vec<usize>> {
	// filter out participants from the attestation vectors that are not in the participants vector anymore.
	participant_attestations
		.clone()
		.iter()
		.map(|a| a.clone().into_iter().filter(|j| participants.contains(j)).collect())
		.collect()
}

fn group_participants_by_num_outgoing_attestations(
	participants: &Vec<usize>,
	participant_attestations: &Vec<Vec<usize>>,
) -> Vec<(usize, Vec<usize>)> {
	let mut sorted_participants: Vec<usize> = participants.clone();

	// sort ascending by number of attestations
	sorted_participants.sort_by(|a, b| {
		(participant_attestations[*a].len() as i32)
			.cmp(&(participant_attestations[*b].len() as i32))
	});

	let mut grouped_participants: Vec<(usize, Vec<usize>)> = vec![];
	for p in sorted_participants {
		let num_attestations = participant_attestations[p].len();
		let last = grouped_participants.last_mut();
		if let Some((_, group)) = last.filter(|(k, _)| k == &num_attestations) {
			group.push(p);
		} else {
			grouped_participants.push((num_attestations, vec![p]));
		}
	}
	grouped_participants
}

#[derive(PartialEq, Debug)]
pub enum MeetupValidationError {
	BallotEmpty,
	NoDependableVote,
}
#[derive(Clone, PartialEq, Debug)]
pub enum ExclusionReason {
	NoVote,
	WrongVote,
	TooFewIncomingAttestations,
	TooFewOutgoingAttestations,
}

#[derive(PartialEq, Debug)]
pub struct ExcludedParticipant {
	pub index: usize,
	pub reason: ExclusionReason,
}

#[derive(PartialEq, Debug)]
pub struct UpdatedParticipants {
	pub included: Vec<usize>,
	pub excluded: Vec<ExcludedParticipant>,
}

impl UpdatedParticipants {
	pub fn exclude_participants(&mut self, excluded: Vec<usize>, reason: ExclusionReason) {
		self.included =
			self.included.clone().into_iter().filter(|i| !excluded.contains(i)).collect();
		for i in excluded {
			self.excluded.push(ExcludedParticipant { index: i, reason: reason.clone() })
		}
	}
}
