#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_std::{vec, vec::Vec};

/// This module is about finding which participants get their rewards based on their votes and attestations.
/// The participant vecs are always vecs of participant ids
/// The partitipant_vote and partcipant_attestations vecs (and their derived vecs) are indexed by the participant index
/// ie. participant_votes[i] holds the vote of participant i

type ParticipantIndex = usize;
type Participants = Vec<ParticipantIndex>;
type Attestations = Vec<Vec<ParticipantIndex>>;
// (a, b) : a is the number of attestations
//			b are the participants that have those number of attestations
type ParticipantGroup = (usize, Participants);

pub fn get_participant_judgements(
	participants: &Participants,
	participant_votes: &Vec<u32>,
	participant_attestations: &Attestations,
	attestation_threshold_fn: fn(usize) -> usize,
) -> Result<ParticipantJudgements, MeetupValidationError> {
	let mut participant_judgements =
		ParticipantJudgements { legit: participants.clone(), excluded: vec![] };
	participant_judgements.exclude_participants(get_excluded_participants_no_vote(
		&participant_judgements.legit,
		participant_votes,
	)?);

	let (n_confirmed, _num_votes) =
		find_majority_vote(&participant_judgements.legit, participant_votes)?;

	participant_judgements.exclude_participants(get_excluded_participants_wrong_vote(
		&participant_judgements.legit,
		participant_votes,
		n_confirmed,
	)?);

	participant_judgements.exclude_participants(get_excluded_participants_num_attestations(
		&participant_judgements.legit,
		participant_attestations.clone(),
		attestation_threshold_fn,
	)?);

	Ok(participant_judgements)
}

fn get_excluded_participants_no_vote(
	participants: &Vec<ParticipantIndex>,
	participant_votes: &Vec<u32>,
) -> Result<Vec<(ParticipantIndex, ExclusionReason)>, MeetupValidationError> {
	// We want to get rid of all participants that did not vote (ie. have a vote of 0 (default storage value) because they did not receive any attestations).
	// This needs to happen before we compute the majority vote, because otherwise it would be possible to receive a majority vote of 0
	// in the case where more than half of the participants did not show up.

	let mut excluded_participants: Vec<(ParticipantIndex, ExclusionReason)> = vec![];
	for i in participants {
		match participant_votes.get_or_err(*i)? {
			v if v > &0 => continue,
			_ => excluded_participants.push((*i, ExclusionReason::NoVote)),
		}
	}
	Ok(excluded_participants)
}

fn get_excluded_participants_wrong_vote(
	participants: &Participants,
	participant_votes: &Vec<u32>,
	n_confirmed: u32,
) -> Result<Vec<(ParticipantIndex, ExclusionReason)>, MeetupValidationError> {
	let mut excluded_participants: Vec<(ParticipantIndex, ExclusionReason)> = vec![];
	for i in participants {
		if participant_votes.get_or_err(*i)? != &n_confirmed {
			excluded_participants.push((*i, ExclusionReason::WrongVote))
		}
	}
	Ok(excluded_participants)
}

/// The algorithm works as follows:
/// We find the participant with the fewest incoming/outgoing attestations
/// If it if below the threhsold, we exclude the participant
fn get_excluded_participants_num_attestations(
	participants: &Participants,
	participant_attestations: Attestations,
	threshold_fn: fn(usize) -> usize,
) -> Result<Vec<(usize, ExclusionReason)>, MeetupValidationError> {
	let mut relevant_attestations = filter_attestations(participants, participant_attestations);

	let mut excluded_participants: Vec<(ParticipantIndex, ExclusionReason)> = vec![];
	let mut participants_to_process: Vec<ParticipantIndex> = participants.clone();

	// we will never loop more than the number of participants,
	// because either participants are excluded or we break out of the loop
	let max_iterations = participants_to_process.len();

	for _ in 0..max_iterations {
		// if all participants were excluded, exit the loop
		if participants_to_process.len() == 0 {
			return Ok(excluded_participants)
		};

		let participants_grouped_by_outgoing_attestations =
			group_participants_by_num_outgoing_attestations(
				participants_to_process.clone(),
				&relevant_attestations,
			)?;
		let participants_grouped_by_incoming_attestations =
			group_participants_by_num_incoming_attestations(
				participants_to_process.clone(),
				&relevant_attestations,
			)?;

		let min_num_outgoing_attestations =
			participants_grouped_by_outgoing_attestations.get_or_err(0)?.0;
		let min_num_incoming_attestations =
			participants_grouped_by_incoming_attestations.get_or_err(0)?.0;

		let mut maybe_participants_to_exclude_with_reason: Option<(
			&Participants,
			ExclusionReason,
		)> = None;
		if min_num_incoming_attestations < min_num_outgoing_attestations {
			if min_num_incoming_attestations < threshold_fn(participants_to_process.len()) {
				maybe_participants_to_exclude_with_reason = Some((
					&participants_grouped_by_incoming_attestations.get_or_err(0)?.1,
					ExclusionReason::TooFewIncomingAttestations,
				));
			}
		} else {
			if min_num_outgoing_attestations < threshold_fn(participants_to_process.len()) {
				maybe_participants_to_exclude_with_reason = Some((
					&participants_grouped_by_outgoing_attestations.get_or_err(0)?.1,
					ExclusionReason::TooFewOutgoingAttestations,
				));
			}
		}
		if let Some((participants_to_exclude, exclusion_reason)) =
			maybe_participants_to_exclude_with_reason
		{
			participants_to_exclude
				.iter()
				.for_each(|p| excluded_participants.push((*p, exclusion_reason)));

			// remove the participants from the included participants and the attestation vectors
			participants_to_process.retain(|k| !participants_to_exclude.contains(k));
			relevant_attestations =
				filter_attestations(&participants_to_process, relevant_attestations.clone());
			continue
		} else {
			// if all participants are above the threshold and therefore no participants were removed, we exit the loop
			break
		}
	}
	Ok(excluded_participants)
}

fn find_majority_vote(
	participants: &Participants,
	participant_votes: &Vec<u32>,
) -> Result<(u32, u32), MeetupValidationError> {
	let mut n_vote_candidates: Vec<(u32, u32)> = vec![];
	for i in participants {
		let this_vote = participant_votes.get_or_err(*i)?;
		match n_vote_candidates.iter().position(|&(n, _c)| n == *this_vote) {
			Some(idx) => n_vote_candidates[idx].1 = n_vote_candidates.get_or_err(idx)?.1 + 1,
			_ => n_vote_candidates.insert(0, (*this_vote, 1)),
		};
	}

	if n_vote_candidates.is_empty() {
		return Err(MeetupValidationError::BallotEmpty)
	}
	// sort by descending vote count
	n_vote_candidates.sort_by(|a, b| b.1.cmp(&a.1));
	if n_vote_candidates.get_or_err(0)?.1 < 3 {
		return Err(MeetupValidationError::NoDependableVote)
	}
	let (n_confirmed, vote_count) = n_vote_candidates.get_or_err(0)?;
	Ok((*n_confirmed, *vote_count))
}

fn filter_attestations(
	participants: &Participants,
	participant_attestations: Attestations,
) -> Attestations {
	// filter out participants from the attestation vectors that are not in the participants vector anymore.
	participant_attestations
		.into_iter()
		.map(|mut a| {
			a.retain(|j| participants.contains(j));
			a
		})
		.collect()
}

fn group_participants_by_num_incoming_attestations(
	participants: Participants,
	participant_attestations: &Attestations,
) -> Result<Vec<ParticipantGroup>, MeetupValidationError> {
	let num_incoming_attestations: Participants = (0..participant_attestations.len())
		.into_iter()
		.map(|p| {
			participant_attestations
				.into_iter()
				.enumerate()
				.filter(|(idx, a)| &p != idx && a.contains(&p))
				.map(|item| item.1.clone())
				.collect::<Attestations>()
				.len()
		})
		.collect();

	group_indices_by_value(participants, &num_incoming_attestations)
}

fn group_participants_by_num_outgoing_attestations(
	participants: Participants,
	participant_attestations: &Attestations,
) -> Result<Vec<ParticipantGroup>, MeetupValidationError> {
	let num_outgoing_attestations: Participants =
		participant_attestations.iter().map(|a| a.len()).collect();

	group_indices_by_value(participants, &num_outgoing_attestations)
}

fn group_indices_by_value(
	indices: Participants,
	values: &Vec<usize>,
) -> Result<Vec<ParticipantGroup>, MeetupValidationError> {
	match indices.iter().max() {
		Some(max) =>
			if max >= &values.len() {
				return Err(MeetupValidationError::IndexOutOfBounds)
			},
		None => (),
	}

	let mut sorted_indices: Participants = indices;
	// sort ascending by value
	sorted_indices.sort_by(|a, b| (values[*a] as i32).cmp(&(values[*b] as i32)));

	let mut grouped_indices: Vec<ParticipantGroup> = vec![];
	for p in sorted_indices {
		let value = values.get_or_err(p)?;
		let last = grouped_indices.last_mut();
		if let Some((_, group)) = last.filter(|(k, _)| k == value) {
			group.push(p);
		} else {
			grouped_indices.push((*value, vec![p]));
		}
	}
	Ok(grouped_indices)
}

trait GetOrErr {
	type Item;
	type Error;
	fn get_or_err(&self, i: usize) -> Result<&Self::Item, Self::Error>;
}

impl<T> GetOrErr for Vec<T> {
	type Item = T;
	type Error = MeetupValidationError;
	fn get_or_err(&self, i: usize) -> Result<&Self::Item, Self::Error> {
		self.get(i).ok_or(Self::Error::IndexOutOfBounds)
	}
}

#[derive(
	Encode,
	Decode,
	Clone,
	Copy,
	PartialEq,
	Eq,
	Debug,
	TypeInfo,
	MaxEncodedLen,
	Serialize,
	Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum MeetupValidationError {
	BallotEmpty,
	NoDependableVote,
	IndexOutOfBounds,
}
#[derive(
	Encode,
	Decode,
	Clone,
	Copy,
	PartialEq,
	Eq,
	Debug,
	TypeInfo,
	MaxEncodedLen,
	Serialize,
	Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum ExclusionReason {
	NoVote,
	WrongVote,
	TooFewIncomingAttestations,
	TooFewOutgoingAttestations,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcludedParticipant {
	pub index: usize,
	pub reason: ExclusionReason,
}
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantJudgements {
	pub legit: Vec<usize>,
	pub excluded: Vec<ExcludedParticipant>,
}

impl ParticipantJudgements {
	pub fn exclude_participants(&mut self, excluded: Vec<(usize, ExclusionReason)>) {
		self.legit.retain(|&i| !excluded.iter().any(|p| p.0 == i));
		for p in excluded {
			self.excluded.push(ExcludedParticipant { index: p.0, reason: p.1 })
		}
	}
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod meetup_scenario_tests;
