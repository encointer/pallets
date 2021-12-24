//! Everything regarding meetup assignments

use crate::math::{checked_ceil_division, checked_modulo, find_prime_below};
use encointer_primitives::{
	ceremonies::{AssignmentParams, MeetupIndexType, ParticipantIndexType},
	RandomNumberGenerator,
};
use sp_runtime::traits::Hash;

pub mod math;

/// Assigns a participant to a meetup.
///
/// Returns an error if the checked math operations fail.
pub fn assignment_fn(
	participant_index: ParticipantIndexType,
	assignment_params: AssignmentParams,
	n: u64,
) -> Option<MeetupIndexType> {
	participant_index
		.checked_mul(assignment_params.s1)?
		.checked_add(assignment_params.s2)
		.and_then(|div| checked_modulo(div, assignment_params.m))
		.and_then(|div| checked_modulo(div, n))
}

/// Generates randomized `[AssignmentParams]` for `num_participants` to be distributed across
/// `num_meetups`.
///
pub fn generate_assignment_function_params<Hashing: Hash>(
	num_participants: u64,
	num_meetups: u64,
	random_source: &mut RandomNumberGenerator<Hashing>,
) -> AssignmentParams {
	let max_skips = 200;
	let m = find_prime_below(num_participants) as u32;
	let mut skip_count = 0;
	let mut s1 = random_source.pick_non_zero_u32(m - 1); //safe; m > 1, since prime
	let mut s2 = random_source.pick_non_zero_u32(m - 1);

	while skip_count <= max_skips {
		s1 = random_source.pick_non_zero_u32(m - 1);
		s2 = random_source.pick_non_zero_u32(m - 1);
		if validate_equal_mapping(
			num_participants,
			AssignmentParams { m: m as u64, s1: s1 as u64, s2: s2 as u64 },
			num_meetups,
		) {
			break
		} else {
			skip_count += 1;
		}
	}
	return AssignmentParams { m: m as u64, s1: s1 as u64, s2: s2 as u64 }
}

fn validate_equal_mapping(
	num_participants: u64,
	assignment_params: AssignmentParams,
	n: u64,
) -> bool {
	if num_participants < 2 {
		return true
	}

	let mut meetup_index_count: Vec<u64> = vec![0; n as usize];
	let meetup_index_count_max =
		checked_ceil_division(num_participants - assignment_params.m, n).unwrap_or(0);

	for i in assignment_params.m..num_participants {
		let meetup_index = match assignment_fn(i, assignment_params, n) {
			Some(i) => i as usize,
			None => return false,
		};

		meetup_index_count[meetup_index] += 1; // safe; <= num_participants
		if meetup_index_count[meetup_index] > meetup_index_count_max {
			return false
		}
	}
	true
}
