use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::{
	crypto::{AccountId32, ByteArray},
	ed25519, sr25519,
};

/// An opaque 32-byte cryptographic identifier.
///
/// Very similar to the `AccountId32`, but it is purposely different
/// as it does not implement the SS58 codec.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct Bip340([u8; 32]);

impl Bip340 {
	/// Create a new instance from its raw inner byte value.
	///
	/// Equivalent to this types `From<[u8; 32]>` implementation. For the lack of const
	/// support in traits we have this constructor.
	pub const fn new(inner: [u8; 32]) -> Self {
		Self(inner)
	}
}

impl ByteArray for Bip340 {
	const LEN: usize = 32;
}

impl AsRef<[u8]> for Bip340 {
	fn as_ref(&self) -> &[u8] {
		&self.0[..]
	}
}

impl AsMut<[u8]> for Bip340 {
	fn as_mut(&mut self) -> &mut [u8] {
		&mut self.0[..]
	}
}

impl AsRef<[u8; 32]> for Bip340 {
	fn as_ref(&self) -> &[u8; 32] {
		&self.0
	}
}

impl AsMut<[u8; 32]> for Bip340 {
	fn as_mut(&mut self) -> &mut [u8; 32] {
		&mut self.0
	}
}

impl From<[u8; 32]> for Bip340 {
	fn from(x: [u8; 32]) -> Self {
		Self::new(x)
	}
}

impl<'a> TryFrom<&'a [u8]> for Bip340 {
	type Error = ();
	fn try_from(x: &'a [u8]) -> Result<Bip340, ()> {
		if x.len() == 32 {
			let mut data = [0; 32];
			data.copy_from_slice(x);
			Ok(Bip340(data))
		} else {
			Err(())
		}
	}
}

impl From<Bip340> for [u8; 32] {
	fn from(x: Bip340) -> [u8; 32] {
		x.0
	}
}

impl From<Bip340> for AccountId32 {
	fn from(x: Bip340) -> AccountId32 {
		x.0.into()
	}
}

impl From<sr25519::Public> for Bip340 {
	fn from(k: sr25519::Public) -> Self {
		k.0.into()
	}
}

impl From<ed25519::Public> for Bip340 {
	fn from(k: ed25519::Public) -> Self {
		k.0.into()
	}
}

impl core::fmt::Display for Bip340 {
	fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
		write!(f, "0x{}", sp_core::hexdisplay::HexDisplay::from(&self.0))
	}
}

impl core::fmt::Debug for Bip340 {
	fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
		write!(f, "0x{}", sp_core::hexdisplay::HexDisplay::from(&self.0))
	}
}

#[cfg(feature = "serde_derive")]
impl serde::Serialize for Bip340 {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		serializer.serialize_str(&self.to_string())
	}
}

#[cfg(feature = "serde_derive")]
impl<'de> serde::Deserialize<'de> for Bip340 {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		sp_std::str::FromStr::from_str(&core::alloc::string::String::deserialize(deserializer)?)
			.map_err(|e| serde::de::Error::custom(e.to_string()))
	}
}

impl sp_std::str::FromStr for Bip340 {
	type Err = &'static str;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		array_bytes::hex_n_into(s.trim_start_matches("0x")).map_err(|_| "invalid hex address.")
	}
}

#[cfg(test)]
mod tests {
	use crate::bip340::Bip340;
	use sp_core::{sr25519, Pair};
	use std::str::FromStr;

	#[test]
	pub fn bip340_to_string_works() {
		let zero_key = Bip340::new([0u8; 32]);
		let zero_key_str = zero_key.to_string();

		assert_eq!(zero_key_str.len(), 66);
		assert_eq!(
			zero_key_str,
			"0x0000000000000000000000000000000000000000000000000000000000000000"
		);
	}

	#[test]
	pub fn bip340_from_str_works() {
		let zero_key_str = "0x0000000000000000000000000000000000000000000000000000000000000000";
		let zero_key: Bip340 = Bip340::from_str(&zero_key_str).unwrap();
		assert_eq!(zero_key, Bip340::new([0u8; 32]));
	}

	#[test]
	pub fn bip340_serialize_works() {
		let zero_key = Bip340::new([0u8; 32]);
		let zero_key_str = serde_json::to_string(&zero_key).unwrap();

		// contains leading and trailing ".
		assert_eq!(zero_key_str.len(), 68);
		assert_eq!(
			zero_key_str,
			"\"0x0000000000000000000000000000000000000000000000000000000000000000\""
		);
	}

	#[test]
	pub fn bip340_deserialize_works() {
		let zero_key_str = "\"0x0000000000000000000000000000000000000000000000000000000000000000\"";
		let zero_key: Bip340 = serde_json::from_str(&zero_key_str).unwrap();
		assert_eq!(zero_key, Bip340::new([0u8; 32]));
	}

	#[test]
	pub fn bip340_deserialize_without_prefix_works() {
		let zero_key_str = "\"0000000000000000000000000000000000000000000000000000000000000000\"";
		let zero_key: Bip340 = serde_json::from_str(&zero_key_str).unwrap();
		assert_eq!(zero_key, Bip340::new([0u8; 32]));
	}

	#[test]
	pub fn bip340_deserialize_alice_works() {
		let alice_str = "\"0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d\"";
		let alice: Bip340 = serde_json::from_str(alice_str).unwrap();
		assert_eq!(
			alice,
			Bip340::from(sr25519::Pair::from_string_with_seed("//Alice", None).unwrap().0.public())
		);
	}
}
