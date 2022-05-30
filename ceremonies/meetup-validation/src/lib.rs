/// This module is about finding which participants get their rewards based on their votes and attestations.
/// The participant vecs are always vecs of participant ids
/// The partitipant_vote and partcipant_attestations vecs (and their derived vecs) are indexed by the participant index
/// ie. participant_votes[i] holds the vote of participant i

pub fn get_updated_participants(
	participants: &Vec<usize>,
	participant_votes: &Vec<u32>,
	participant_attestations: &Vec<Vec<usize>>,
	attestation_threshold_fn: fn(usize) -> usize,
) -> Result<UpdatedParticipants, MeetupValidationError> {
	let mut updated_participants =
		UpdatedParticipants { included: participants.clone(), excluded: vec![] };
	updated_participants.exclude_participants(get_excluded_participants_no_vote(
		&updated_participants.included,
		participant_votes,
	));

	let (n_confirmed, _num_votes) =
		find_majority_vote(&updated_participants.included, participant_votes)?;

	updated_participants.exclude_participants(get_excluded_participants_wrong_vote(
		&updated_participants.included,
		participant_votes,
		n_confirmed,
	));

	updated_participants.exclude_participants(get_excluded_participants_num_attestations(
		&updated_participants.included,
		participant_attestations,
		attestation_threshold_fn,
	));

	Ok(updated_participants)
}

fn get_excluded_participants_no_vote(
	participants: &Vec<usize>,
	participant_votes: &Vec<u32>,
) -> Vec<(usize, ExclusionReason)> {
	let mut excluded_participants: Vec<usize> = vec![];
	for i in participants {
		match participant_votes[*i] {
			v if v > 0 => continue,
			_ => excluded_participants.push(*i),
		}
	}
	excluded_participants
		.into_iter()
		.map(|p| (p, ExclusionReason::NoVote))
		.collect()
}

fn get_excluded_participants_wrong_vote(
	participants: &Vec<usize>,
	participant_votes: &Vec<u32>,
	n_confirmed: u32,
) -> Vec<(usize, ExclusionReason)> {
	let mut excluded_participants: Vec<usize> = vec![];
	for i in participants {
		if participant_votes[*i] != n_confirmed {
			excluded_participants.push(*i)
		}
	}
	excluded_participants
		.into_iter()
		.map(|p| (p, ExclusionReason::WrongVote))
		.collect()
}

/// The algorithm works as follows:
/// We find the participant with the fewest incoming/outgoing attestations
/// If it if below the threhsold, we exclude the participant
fn get_excluded_participants_num_attestations(
	participants: &Vec<usize>,
	participant_attestations: &Vec<Vec<usize>>,
	threshold_fn: fn(usize) -> usize,
) -> Vec<(usize, ExclusionReason)> {
	let mut relevant_attestations = filter_attestations(participants, participant_attestations);

	let mut excluded_participants: Vec<(usize, ExclusionReason)> = vec![];
	let mut participants_to_process: Vec<usize> = participants.clone();

	while participants_to_process.len() > 0 {
		let (
			participants_grouped_by_outgoing_attestations,
			participants_grouped_by_incoming_attestations,
		) = group_participants_by_num_incoming_and_outgoing_attestations(
			&participants_to_process,
			&relevant_attestations,
		);

		let min_num_outgoing_attestations = participants_grouped_by_outgoing_attestations[0].0;
		let min_num_incoming_attestations = participants_grouped_by_incoming_attestations[0].0;

		let mut maybe_participants_to_exclude_with_reason: Option<(Vec<usize>, ExclusionReason)> =
			None;
		if min_num_incoming_attestations < min_num_outgoing_attestations {
			if min_num_incoming_attestations < threshold_fn(participants_to_process.len()) {
				maybe_participants_to_exclude_with_reason = Some((
					participants_grouped_by_incoming_attestations[0].1.clone(),
					ExclusionReason::TooFewIncomingAttestations,
				));
			}
		} else {
			if min_num_outgoing_attestations < threshold_fn(participants_to_process.len()) {
				maybe_participants_to_exclude_with_reason = Some((
					participants_grouped_by_outgoing_attestations[0].1.clone(),
					ExclusionReason::TooFewOutgoingAttestations,
				));
			}
		}
		if let Some((participants_to_exclude, exclusion_reason)) =
			maybe_participants_to_exclude_with_reason
		{
			participants_to_exclude
				.clone()
				.into_iter()
				.for_each(|p| excluded_participants.push((p, exclusion_reason)));

			// remove the participants from the included participants and the attestation vectors
			participants_to_process.retain(|k| !participants_to_exclude.contains(k));
			relevant_attestations =
				filter_attestations(&participants_to_process, &relevant_attestations);
			continue
		}

		// if all participants are above the threshold and therefore no participants were removed, we exit the loop
		break
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

fn group_participants_by_num_incoming_and_outgoing_attestations(
	participants: &Vec<usize>,
	participant_attestations: &Vec<Vec<usize>>,
) -> (Vec<(usize, Vec<usize>)>, Vec<(usize, Vec<usize>)>) {
	let num_outgoing_attestations: Vec<usize> =
		participant_attestations.iter().map(|a| a.len()).collect();
	let num_incoming_attestations: Vec<usize> = (0..participant_attestations.len())
		.into_iter()
		.map(|p| {
			participant_attestations
				.into_iter()
				.enumerate()
				.filter(|(idx, a)| &p != idx && a.contains(&p))
				.map(|item| item.1.clone())
				.collect::<Vec<Vec<usize>>>()
				.len()
		})
		.collect();
	return (
		group_participants_by_num_attestations(participants, &num_outgoing_attestations),
		group_participants_by_num_attestations(participants, &num_incoming_attestations),
	)
}

fn group_participants_by_num_attestations(
	participants: &Vec<usize>,
	num_attestations: &Vec<usize>,
) -> Vec<(usize, Vec<usize>)> {
	let mut sorted_participants: Vec<usize> = participants.clone();
	// sort ascending by number of attestations
	sorted_participants
		.sort_by(|a, b| (num_attestations[*a] as i32).cmp(&(num_attestations[*b] as i32)));

	let mut grouped_participants: Vec<(usize, Vec<usize>)> = vec![];
	for p in sorted_participants {
		let num_attestations = num_attestations[p];
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
#[derive(Clone, PartialEq, Debug, Copy)]
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
	pub fn exclude_participants(&mut self, excluded: Vec<(usize, ExclusionReason)>) {
		self.included = self
			.included
			.clone()
			.into_iter()
			.filter(|&i| !excluded.iter().any(|p| p.0 == i))
			.collect();
		for p in excluded {
			self.excluded.push(ExcludedParticipant { index: p.0, reason: p.1.clone() })
		}
	}
}

#[cfg(test)]
mod tests;
