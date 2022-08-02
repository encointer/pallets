use sp_arithmetic::traits::BaseArithmetic;
pub trait AbsDiff<T> {
	fn abs_diff(&self, other: T) -> T;
}

impl<T: BaseArithmetic + Copy> AbsDiff<T> for T {
	fn abs_diff(&self, other: T) -> T {
		if *self > other {
			*self - other
		} else {
			other - *self
		}
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn abs_diff_works() {
		assert_eq!(12u64.abs_diff(10u64), 2u64);
		assert_eq!(10u64.abs_diff(13u64), 3u64);
		assert_eq!(10u64.abs_diff(10u64), 0u64);
	}
}
