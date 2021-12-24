//! Everything regarding meetup assignments

use crate::math::{checked_ceil_division, checked_modulo, find_prime_below, mod_inv};
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

// Todo add documentation to this function.
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

fn assignment_fn_inverse(
	meetup_index: u64,
	assignment_params: AssignmentParams,
	n: u64,
	num_participants: u64,
) -> Vec<ParticipantIndexType> {
	if n <= 0 {
		return vec![]
	}

	let mut result: Vec<ParticipantIndexType> = vec![];
	let mut max_index = assignment_params.m.checked_sub(meetup_index).unwrap_or(0) / n;
	// ceil
	if (assignment_params.m as i64 - meetup_index as i64).rem_euclid(n as i64) != 0 {
		max_index += 1; //safe; m prime below num_participants
	}

	for i in 0..max_index {
		let t2 = mod_inv(assignment_params.s1 as i64, assignment_params.m as i64);
		let t3 = (n as i64)
			.checked_mul(i as i64)
			.and_then(|x| x.checked_add(meetup_index as i64))
			.and_then(|x| x.checked_sub(assignment_params.s2 as i64))
			.map(|x| x.rem_euclid(assignment_params.m as i64))
			.and_then(|x| x.checked_mul(t2))
			.map(|x| x.rem_euclid(assignment_params.m as i64));

		if t3.is_none() || t3.unwrap() >= num_participants as i64 {
			continue
		}
		result.push(t3.unwrap() as u64);
		if t3.unwrap() < num_participants as i64 - assignment_params.m as i64 {
			result.push(t3.unwrap() as u64 + assignment_params.m)
		}
	}
	result
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn assignment_fn_works() {
		new_test_ext().execute_with(|| {
			assert_eq!(assignment_fn(6, AssignmentParams { m: 4, s1: 5, s2: 3 }, 5).unwrap(), 1)
		});
	}

	#[test]
	fn validate_equal_mapping_works() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				validate_equal_mapping(2761, AssignmentParams { m: 2753, s1: 2326, s2: 1099 }, 427),
				false
			);
			assert_eq!(
				validate_equal_mapping(2761, AssignmentParams { m: 2753, s1: 2325, s2: 1099 }, 427),
				true
			);
		});
	}
}
