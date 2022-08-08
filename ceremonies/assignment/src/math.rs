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
use sp_runtime::traits::{Hash, Zero};
use sp_std::{ops::Rem, vec::Vec};

pub fn checked_modulo<T: Rem<Output = T> + Zero + PartialEq>(dividend: T, divisor: T) -> Option<T> {
	let zero = T::zero();
	match divisor {
		_ if divisor == zero => Option::None,
		_ => Some(dividend % divisor),
	}
}

pub fn checked_ceil_division(dividend: u64, divisor: u64) -> Option<u64> {
	let dd = dividend.checked_add(divisor)?.checked_sub(1)?;

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
	let mut j: u64 = 25;
	while j <= n {
		let i_plus_two = i.checked_add(2u64).expect("i^2 does not overflow, so i + 2 is safe; qed");
		if n % i == 0u64 || n % (i_plus_two) == 0u64 {
			return false
		}
		i = i.checked_add(6u64).expect("i^2 does not overflow, so i + 6 is safe; qed");

		if let Some(i_squared) = i.checked_pow(2) {
			j = i_squared;
		} else {
			// if i overflows we can be sure that j <= n does not hold
			break
		}
	}
	true
}

pub fn get_greatest_common_denominator(a: u64, b: u64) -> u64 {
	if a == 0 || b == 0 {
		return 0
	}

	if a == b {
		return a
	}

	if a > b {
		return get_greatest_common_denominator(a.checked_sub(b).expect("a > b; qed"), b)
	}

	get_greatest_common_denominator(a, b.checked_sub(a).expect("b <= a; qed"))
}

pub fn find_prime_below(mut n: u64) -> u64 {
	if n <= 2 {
		return 2u64
	}
	if n % 2 == 0 {
		n = n.checked_sub(1).expect("n > 2; qed");
	}
	while n > 0 {
		if is_prime(n) {
			return n
		}
		if let Some(n_minus_two) = n.checked_sub(2) {
			n = n_minus_two;
		} else {
			break
		}
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

pub fn checked_mod_inv(a: i64, module: i64) -> Option<i64> {
	let mut mn = (module, a);
	let mut xy = (0i64, 1i64);

	while mn.1 != 0 {
		xy = (xy.1, xy.0.checked_sub(mn.0.checked_div(mn.1)?.checked_mul(xy.1)?)?);
		mn = (mn.1, checked_modulo::<i64>(mn.0, mn.1)?);
	}

	while xy.0 < 0 {
		xy.0 = xy.0.checked_add(module)?;
	}
	Some(xy.0)
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
		assert_eq!(checked_mod_inv(2, 7).unwrap(), 4);
		assert_eq!(checked_mod_inv(69, 113).unwrap(), 95);
		assert_eq!(checked_mod_inv(111, 113).unwrap(), 56);
	}

	#[test]
	fn greatest_common_denominator_works() {
		assert_eq!(get_greatest_common_denominator(2, 7), 1);
		assert_eq!(get_greatest_common_denominator(0, 1), 0);
		assert_eq!(get_greatest_common_denominator(3, 9), 3);
		assert_eq!(get_greatest_common_denominator(8, 36), 4);
		assert_eq!(get_greatest_common_denominator(8, 1), 1);
	}

	#[test]
	fn checked_ceil_division_works() {
		assert_eq!(checked_ceil_division(6, 4).unwrap(), 2);
		assert_eq!(checked_ceil_division(12, 3).unwrap(), 4);
		assert_eq!(checked_ceil_division(0, 0), None);
	}
}
