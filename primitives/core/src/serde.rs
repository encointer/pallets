//! Custom serde serialization helpers

/// Serialization shim for arbitrary arrays that is consistent with `polkadot-js`'s implementation.
///
/// `polkadot-js` sends us a `0x01020304`, but the default rust implementation for arrays expects a
/// `[0x01, 0x02, 0x03, 0x04]`. Here, we use a similar serialization as substrate uses for `vec`,
/// but we transform it to an array before returning.
pub mod serialize_array {
	use impl_serde::serialize::{deserialize_check_len, ExpectedLen};
	use serde::Deserializer;

	// default serialize is fine
	pub use impl_serde::serialize::serialize;

	pub use deserialize_array as deserialize;

	pub fn deserialize_array<'de, D, const T: usize>(deserializer: D) -> Result<[u8; T], D::Error>
	where
		D: Deserializer<'de>,
	{
		// All hail the stable const generics!
		let mut arr = [0u8; T];
		deserialize_check_len(deserializer, ExpectedLen::Exact(&mut arr[..]))?;

		Ok(arr)
	}
}

/// Serialization shim for fixed point numbers that is consistent with `polkadot-js`'s implementation.
///
/// This is needed in particular for fixed point types that map to a i-/u128 as serde has problems
/// with it: https://github.com/paritytech/substrate/issues/4641
pub mod serialize_fixed {
	use fixed::traits::Fixed;
	use serde::{de::Error, Deserializer};

	use impl_serde::serde::Deserialize;

	#[cfg(not(feature = "std"))]
	extern crate alloc;

	#[cfg(not(feature = "std"))]
	use alloc::ToString;

	pub use deserialize_fixed as deserialize;
	pub use serialize_fixed as serialize;

	// i-/u128 and hence all the 128bit substrate-fixed types do not serialize well into JSON for
	// custom RPCs, so we serialize it as a String.
	pub fn serialize_fixed<S, F: Fixed>(f: &F, s: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		s.serialize_str(&f.to_string())
	}

	pub fn deserialize_fixed<'de, D, F: Fixed>(d: D) -> Result<F, D::Error>
	where
		D: Deserializer<'de>,
	{
		let s = String::deserialize(d)?;
		F::from_str(&s).map_err(D::Error::custom)
	}
}

#[cfg(test)]
mod tests {
	use super::{serialize_array, serialize_fixed};
	use fixed::{traits::Fixed, types::I64F64};

	fn deserialize_array<const T: usize>(arr: &str) -> [u8; T] {
		let mut der = serde_json::Deserializer::new(serde_json::de::StrRead::new(arr));
		serialize_array::deserialize(&mut der).unwrap()
	}

	fn serialize_array<const T: usize>(arr: [u8; T]) -> String {
		let mut v = vec![];

		let mut ser = serde_json::Serializer::new(std::io::Cursor::new(&mut v));
		serialize_array::serialize(&arr, &mut ser).unwrap();

		String::from_utf8(v).unwrap()
	}

	fn serialize_fixed_point<F: Fixed>(f: F) -> String {
		let mut v = vec![];

		let mut ser = serde_json::Serializer::new(std::io::Cursor::new(&mut v));
		serialize_fixed::serialize(&f, &mut ser).unwrap();

		String::from_utf8(v).unwrap()
	}

	fn deserialize_fixed_point<F: Fixed>(f: &str) -> F {
		let mut der = serde_json::Deserializer::new(serde_json::de::StrRead::new(f));
		serialize_fixed::deserialize(&mut der).unwrap()
	}

	#[test]
	fn deserialize_array_works() {
		assert_eq!(deserialize_array("\"0x0000\""), [0x00, 0x00]);
		assert_eq!(deserialize_array("\"0x0100\""), [0x01, 0x00]);
		assert_eq!(deserialize_array("\"0x0010\""), [0x00, 0x10]);
	}

	#[test]
	fn serialize_array_works() {
		assert_eq!(serialize_array([0x00, 0x00]), "\"0x0000\"".to_owned());
		assert_eq!(serialize_array([0x01, 0x00]), "\"0x0100\"".to_owned());
		assert_eq!(serialize_array([0x00, 0x10]), "\"0x0010\"".to_owned());
	}

	#[test]
	fn serialize_fixed_works() {
		assert_eq!(
			serialize_fixed_point(I64F64::from_num(18.6808776855468714473)),
			"\"18.6808776855468714473\"".to_owned()
		);

		assert_eq!(
			serialize_fixed_point(I64F64::from_num(0.6808776855468714473)),
			"\"0.6808776855468714473\"".to_owned()
		);

		assert_eq!(serialize_fixed_point(I64F64::from_num(1)), "\"1\"".to_owned());
	}

	#[test]
	fn deserialize_fixed_works() {
		assert_eq!(
			deserialize_fixed_point::<I64F64>("\"18.6808776855468714473\""),
			I64F64::from_num(18.6808776855468714473)
		);

		assert_eq!(
			deserialize_fixed_point::<I64F64>("\"0.6808776855468714473\""),
			I64F64::from_num(0.6808776855468714473)
		);

		assert_eq!(deserialize_fixed_point::<I64F64>("\"1\""), I64F64::from_num(1));
	}
}
