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

            for i in 0..=size {
                // swap remove is O(1)
                r.push(input.swap_remove(random.pick_usize(size - i)));
            }
            return Some(r);
        };
    }
}
