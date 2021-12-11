// Copyright (c) 2019 Alain Brenzikofer
// This file is part of Encointer
//
// Encointer is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Encointer is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Encointer.  If not, see <http://www.gnu.org/licenses/>.

//! Math functions used in the ceremonies pallet.

use encointer_primitives::{RandomNumberGenerator, RandomPermutation};
use frame_support::sp_runtime::traits::Hash;
use sp_std::vec::Vec;

pub fn checked_ceil_division(dividend: u64, divisor: u64) -> Option<u64> {
	let dd = dividend.checked_add(divisor)? - 1;

	dd.checked_div(divisor)
}

pub fn is_coprime(a: u64, b: u64) -> bool {
	get_greatest_common_denominator(a, b) == 1
}

pub fn is_prime(n: u64) -> bool {
	if n <= 3 {
		return n > 1
	}
	if n % 2 == 0 || n % 3 == 0 {
		return false
	}
	if n < 25 {
		return true
	}
	let mut i: u64 = 5;
	while i.pow(2) <= n {
		if n % i == 0u64 || n % (i + 2u64) == 0u64 {
			return false
		}
		i += 6u64;
	}
	return true
}

pub fn get_greatest_common_denominator(a: u64, b: u64) -> u64 {
	if a == 0 || b == 0 {
		return 0
	}

	if a == b {
		return a
	};

	if a > b {
		return get_greatest_common_denominator(a - b, b)
	}

	return get_greatest_common_denominator(a, b - a)
}

pub fn find_prime_below(mut n: u64) -> u64 {
	if n <= 2 {
		return 2u64
	}
	if n % 2 == 0 {
		n -= 1;
	}
	while n > 0 {
		if is_prime(n) {
			return n
		}
		n -= 2;
	}
	2u64
}

pub fn find_random_coprime_below<H: Hash>(
	upper_bound: u64,
	random_source: &mut RandomNumberGenerator<H>,
) -> u64 {
	if upper_bound <= 1 {
		return 0
	}

	if upper_bound == 2 {
		return 1
	}

	(1..upper_bound)
		.collect::<Vec<_>>()
		.random_permutation(random_source)
		.expect("Upper bound is checked to be > 2; qed")
		.into_iter()
		.find(|i| is_coprime(upper_bound, *i))
		.unwrap_or(1)
}

pub fn mod_inv(a: i64, module: i64) -> i64 {
	let mut mn = (module, a);
	let mut xy = (0, 1);

	while mn.1 != 0 {
		xy = (xy.1, xy.0 - (mn.0 / mn.1) * xy.1);
		mn = (mn.1, mn.0 % mn.1);
	}

	while xy.0 < 0 {
		xy.0 += module;
	}
	xy.0
}

#[cfg(test)]
mod tests {
	use super::*;
	use sp_core::H256;
	use sp_runtime::traits::BlakeTwo256;

	#[test]
	fn is_prime_works() {
		assert_eq!(is_prime(0), false);
		assert_eq!(is_prime(1), false);
		assert_eq!(is_prime(2), true);
		assert_eq!(is_prime(3), true);
		assert_eq!(is_prime(113), true);
		assert_eq!(is_prime(114), false);
		assert_eq!(is_prime(115), false);
	}

	#[test]
	fn find_coprime_below_works() {
		let mut random_source = RandomNumberGenerator::<BlakeTwo256>::new(H256::random());

		for i in 2..100 {
			let coprime = find_random_coprime_below(i, &mut random_source);
			assert_eq!(get_greatest_common_denominator(coprime, i), 1)
		}
	}

	#[test]
	fn find_prime_below_works() {
		assert_eq!(find_prime_below(0), 2);
		assert_eq!(find_prime_below(1), 2);
		assert_eq!(find_prime_below(1), 2);
		assert_eq!(find_prime_below(5), 5);
		assert_eq!(find_prime_below(10), 7);
		assert_eq!(find_prime_below(118), 113);
		assert_eq!(find_prime_below(113), 113);
	}

	#[test]
	fn mod_inv_works() {
		assert_eq!(mod_inv(2, 7), 4);
		assert_eq!(mod_inv(69, 113), 95);
		assert_eq!(mod_inv(111, 113), 56);
	}

	#[test]
	fn greatest_common_denominator_works() {
		assert_eq!(get_greatest_common_denominator(2, 7), 1);
		assert_eq!(get_greatest_common_denominator(0, 1), 0);
		assert_eq!(get_greatest_common_denominator(3, 9), 3);
		assert_eq!(get_greatest_common_denominator(8, 36), 4);
		assert_eq!(get_greatest_common_denominator(8, 1), 1);
	}
}
