//! Everything regarding meetup assignments

#![cfg_attr(not(feature = "std"), no_std)]

use crate::math::{checked_ceil_division, checked_mod_inv, checked_modulo, find_prime_below};
use encointer_primitives::{
	ceremonies::{AssignmentParams, MeetupIndexType, MeetupTimeOffsetType, ParticipantIndexType},
	communities::{Location, LossyFrom},
	RandomNumberGenerator,
};
use sp_runtime::{
	traits::{AtLeast32Bit, Hash},
	SaturatedConversion,
};
use sp_std::{prelude::Vec, vec};

pub mod math;

/// Assigns a participant to a meetup.
pub fn assignment_fn(
	participant_index: ParticipantIndexType,
	assignment_params: AssignmentParams,
	assignment_count: u64,
) -> Option<MeetupIndexType> {
	participant_index
		.checked_mul(assignment_params.s1)?
		.checked_add(assignment_params.s2)
		.and_then(|div| checked_modulo::<u64>(div, assignment_params.m))
		.and_then(|div| checked_modulo::<u64>(div, assignment_count))
}

/// Generates randomized `[AssignmentParams]` for `num_participants` to be distributed across
/// `num_meetups`.
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
			break;
		} else {
			skip_count += 1; // safe; skip_count <= 200;
		}
	}
	AssignmentParams { m: m as u64, s1: s1 as u64, s2: s2 as u64 }
}

// Todo add documentation to this function.
fn validate_equal_mapping(
	num_participants: u64,
	assignment_params: AssignmentParams,
	meetup_count: u64,
) -> bool {
	if num_participants < 2 {
		return true;
	}

	let mut meetup_index_count: Vec<u64> = vec![0; meetup_count as usize];
	let meetup_index_count_max =
		checked_ceil_division(num_participants - assignment_params.m, meetup_count).unwrap_or(0);

	for i in assignment_params.m..num_participants {
		let meetup_index = match assignment_fn(i, assignment_params, meetup_count) {
			Some(i) => i as usize,
			None => return false,
		};

		meetup_index_count[meetup_index] += 1; // safe; <= num_participants
		if meetup_index_count[meetup_index] > meetup_index_count_max {
			return false;
		}
	}
	true
}

/// Performs the inverse function of `assignment_fn` for all participants in a meetup.
///
/// Returns all participants with `assignment_params` belonging to the meetup with `meetup_index`
/// given the `meetup_count` and `participant_count`.
pub fn assignment_fn_inverse(
	meetup_index: u64,
	assignment_params: AssignmentParams,
	assignment_count: u64,
	participant_count: u64,
) -> Option<Vec<ParticipantIndexType>> {
	if assignment_count == 0 {
		return Some(vec![]);
	}

	let mut max_index = assignment_params.m.saturating_sub(meetup_index) / assignment_count;
	let mut result: Vec<ParticipantIndexType> = Vec::with_capacity(max_index as usize);
	// ceil
	if (assignment_params.m.saturating_sub(meetup_index) as i64).rem_euclid(assignment_count as i64) !=
		0
	{
		max_index += 1; //safe; m prime below num_participants
	}

	for i in 0..max_index {
		let t2 = checked_mod_inv(assignment_params.s1 as i64, assignment_params.m as i64)?;

		let t3 = match t3(assignment_count, i, meetup_index, assignment_params, t2) {
			Some(t3) => t3,
			None => continue,
		};

		if t3 >= participant_count {
			continue;
		}

		result.push(t3);

		if let Some(t3_plus_m) = t3.checked_add(assignment_params.m) {
			if t3_plus_m < participant_count {
				result.push(t3_plus_m)
			}
		}
	}
	Some(result)
}

fn t3(
	n: u64,
	current_index: u64,
	meetup_index: MeetupIndexType,
	params: AssignmentParams,
	t2: i64,
) -> Option<u64> {
	let t3 = (n as i64)
		.checked_mul(current_index as i64)?
		.checked_add(meetup_index as i64)?
		.checked_sub(params.s2 as i64)?
		.checked_rem_euclid(params.m as i64)?
		.checked_mul(t2)?
		.checked_rem_euclid(params.m as i64)?;

	Some(t3 as u64)
}

pub fn meetup_index(
	participant_index: ParticipantIndexType,
	params: AssignmentParams,
	meetup_count: MeetupIndexType,
) -> Option<MeetupIndexType> {
	Some(assignment_fn(participant_index, params, meetup_count)? + 1)
}

pub fn get_meetup_location_index(
	meetup_index: MeetupIndexType,
	locations: &[Location],
	location_assignment_params: AssignmentParams,
) -> Option<MeetupIndexType> {
	assignment_fn(meetup_index, location_assignment_params, locations.len() as u64)
}

pub fn meetup_location(
	meetup_index: MeetupIndexType,
	locations: Vec<Location>,
	location_assignment_params: AssignmentParams,
) -> Option<Location> {
	let location_idx =
		get_meetup_location_index(meetup_index, &locations, location_assignment_params)?;

	if location_idx < locations.len() as u64 {
		Some(locations[(location_idx) as usize])
	} else {
		None
	}
}

pub fn meetup_time<Moment: Copy + AtLeast32Bit>(
	location: Location,
	attesting_start: Moment,
	one_day: Moment,
	offset: MeetupTimeOffsetType,
) -> Moment {
	let one_day_u64: u64 = one_day.saturated_into();
	let per_degree: i64 = one_day_u64 as i64 / 360i64;

	// The meetups start at high sun at 180 degrees and during one day the meetup locations travel
	// along the globe until the very last meetup happens at high sun at -180 degrees.
	// So we scale the range 180...-180 to 0...360
	// rounding to the lower integer degree. Max error: 240s = 4min
	let lon: i64 = (i64::lossy_from(location.lon.round_to_zero()) - 180i64).abs();

	let lon_time = lon * per_degree;
	let attesting_start_u64: u64 = attesting_start.saturated_into();
	let meetup_time = attesting_start_u64 as i64 + lon_time + offset as i64;
	(meetup_time as u64).saturated_into()
}

#[cfg(test)]
mod tests {
	use super::*;
	use encointer_primitives::communities::Degree;
	#[test]
	fn meetup_time_works() {
		let attesting_start = 1671408000000u64; // Mon Dec 19 2022 00:00:00 UTC
		let moments_per_day = 86400000u64; // ms per day
		let meetup_time_offset = -2100000; // 35 minutes
		let greenbay_location =
			Location { lat: Degree::from_num(0.0), lon: Degree::from_num(-88.15) };
		assert_eq!(
			{
				meetup_time(greenbay_location, attesting_start, moments_per_day, meetup_time_offset)
			},
			1671470220000u64
		) // Mon Dec 19 2022 17:17:00 UTC
	}

	#[test]
	fn assignment_fn_works() {
		assert_eq!(assignment_fn(6, AssignmentParams { m: 4, s1: 5, s2: 3 }, 5).unwrap(), 1)
	}

	#[test]
	fn validate_equal_mapping_works() {
		assert!(!validate_equal_mapping(
			2761,
			AssignmentParams { m: 2753, s1: 2326, s2: 1099 },
			427
		));
		assert!(validate_equal_mapping(
			2761,
			AssignmentParams { m: 2753, s1: 2325, s2: 1099 },
			427
		));
	}

	#[test]
	fn assignment_fn_inverse_works() {
		let mut s1 = 78u64;
		let mut s2 = 23u64;
		let mut n = 12u64;
		let mut num_participants = 118u64;
		let mut m = 113u64;

		let mut assignment_params = AssignmentParams { m, s1, s2 };
		check_assignment(num_participants, assignment_params, n);

		s1 = 1u64;
		s2 = 1u64;
		n = 2u64;
		num_participants = 20u64;
		m = 19u64;
		assignment_params = AssignmentParams { m, s1, s2 };
		check_assignment(num_participants, assignment_params, n);
		s1 = 1u64;
		s2 = 1u64;
		n = 1u64;
		num_participants = 10u64;
		m = 7u64;
		assignment_params = AssignmentParams { m, s1, s2 };
		check_assignment(num_participants, assignment_params, n);

		// in the case where there is only one participant, m will be 2 because it is the smallest
		// prime number
		s1 = 1u64;
		s2 = 1u64;
		n = 1u64;
		num_participants = 1u64;
		m = 2u64;
		assignment_params = AssignmentParams { m, s1, s2 };
		check_assignment(num_participants, assignment_params, n);
	}

	fn check_assignment(num_participants: u64, assignment_params: AssignmentParams, n: u64) {
		let mut locations: Vec<u64> = vec![0; num_participants as usize];

		for i in 0..num_participants {
			locations[i as usize] = assignment_fn(i, assignment_params, n).unwrap();
		}

		let mut assigned_participants: Vec<bool> = vec![false; num_participants as usize];

		// inverse function yields the same result
		for i in 0..n {
			let participants =
				assignment_fn_inverse(i, assignment_params, n, num_participants).unwrap();
			for p in participants {
				assigned_participants[p as usize] = true;
				assert_eq!(locations[p as usize], i)
			}
		}

		// all participants were assigned
		for val in assigned_participants {
			assert!(val);
		}
	}
}
