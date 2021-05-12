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

use sp_runtime::traits::Hash;
use sp_runtime::RandomNumberGenerator;

#[cfg(not(feature = "std"))]
use rstd::vec::Vec;

/// Pseudo-random permutation. It's as secure as the combination of the seed with which the
/// RandomNumberGenerator is constructed and the hash function it uses to cycle the elements.
pub trait RandomPermutation {
    type Item;

    /// Random permutation from an array of elements. This is guaranteed to return `Some` except
    /// in the case that `self` is empty.
    fn random_permutation<Hashing: Hash>(
        self,
        random: &mut RandomNumberGenerator<Hashing>,
    ) -> Option<Vec<Self::Item>>;
}

impl<T> RandomPermutation for Vec<T> {
    type Item = T;

    fn random_permutation<Hashing: Hash>(
        self,
        random: &mut RandomNumberGenerator<Hashing>,
    ) -> Option<Vec<T>> {
        // Make it `mut`. Rust does not allow `mut self` as argument because the semantics for
        // the caller is the same: the method consumes `self`.
        let mut input = self;

        if input.is_empty() {
            return None;
        } else {
            let mut r = vec![];
            let size = input.len();

            for i in 1..=size {
                // swap remove is O(1)
                r.push(input.swap_remove(random.pick_usize(size - i)));
            }
            return Some(r);
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_runtime::traits::BlakeTwo256;

    #[test]
    fn random_permutation_works() {
        let mut random_source =
            RandomNumberGenerator::<BlakeTwo256>::new(BlakeTwo256::hash(b"my_seed"));
        let mut random_source_2 =
            RandomNumberGenerator::<BlakeTwo256>::new(BlakeTwo256::hash(b"my_seed2"));
        let input = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        assert_eq!(
            input.clone().random_permutation(&mut random_source),
            Some(vec![5, 9, 7, 4, 6, 8, 2, 3, 1, 10])
        );

        // second time should yield other output
        assert_eq!(
            input.clone().random_permutation(&mut random_source),
            Some(vec![9, 8, 3, 5, 6, 2, 10, 4, 7, 1])
        );

        // different seed, different output
        assert_eq!(
            input.random_permutation(&mut random_source_2),
            Some(vec![1, 7, 8, 9, 2, 3, 10, 5, 4, 6])
        );

        assert_eq!(
            Vec::<u8>::new().random_permutation(&mut random_source_2),
            None
        )
    }
}
