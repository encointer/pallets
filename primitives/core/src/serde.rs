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

#[cfg(test)]
mod tests {
	use super::serialize_array;

	fn deserialize<const T: usize>(arr: &str) -> [u8; T] {
		let mut der = serde_json::Deserializer::new(serde_json::de::StrRead::new(arr));
		serialize_array::deserialize(&mut der).unwrap()
	}

	fn serialize<const T: usize>(arr: [u8; T]) -> String {
		let mut v = vec![];

		let mut ser = serde_json::Serializer::new(std::io::Cursor::new(&mut v));
		serialize_array::serialize(&arr, &mut ser).unwrap();

		String::from_utf8(v).unwrap()
	}

	#[test]
	fn deserialize_works() {
		assert_eq!(deserialize("\"0x0000\""), [0x00, 0x00]);
		assert_eq!(deserialize("\"0x0100\""), [0x01, 0x00]);
		assert_eq!(deserialize("\"0x0010\""), [0x00, 0x10]);
	}

	#[test]
	fn serialize_works() {
		assert_eq!(serialize([0x00, 0x00]), "\"0x0000\"".to_owned());
		assert_eq!(serialize([0x01, 0x00]), "\"0x0100\"".to_owned());
		assert_eq!(serialize([0x00, 0x10]), "\"0x0010\"".to_owned());
	}
}
