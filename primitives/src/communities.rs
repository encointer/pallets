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

use bs58;
use codec::{Decode, Encode, MaxEncodedLen};
use concat_arrays::concat_arrays;
use crc::{Crc, CRC_32_CKSUM};
use ep_core::fixed::types::I64F64;
use geohash::GeoHash as GeohashGeneric;
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;
use sp_std::{fmt, fmt::Formatter, prelude::Vec, str::FromStr};

#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "serde_derive")]
use ep_core::serde::{serialize_array, serialize_fixed};

use crate::{
	balances::Demurrage,
	common::{
		validate_ascii, validate_ipfs_cid, AsByteOrNoop, IpfsCid, IpfsValidationError, PalletString,
	},
};

use crate::error::CommunityIdentifierError;
pub use ep_core::fixed::traits::{LossyFrom, LossyInto};

use consts::GEO_HASH_BUCKET_RESOLUTION;

pub type GeoHash = GeohashGeneric<GEO_HASH_BUCKET_RESOLUTION>;
pub type CommunityIndexType = u32;
pub type LocationIndexType = u32;
pub type Degree = I64F64;
pub type NominalIncome = I64F64;
pub type MinSolarTripTimeType = u32;
pub type MaxSpeedMpsType = u32;


#[derive(
	Encode,
	Decode,
	Copy,
	Clone,
	PartialEq,
	Eq,
	RuntimeDebug,
	PartialOrd,
	Ord,
	TypeInfo,
	MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub enum RangeError {
	LessThanZero,
	LessThanOrEqualZero
}

/// Ensure that the demurrage is in a sane range.
/// Must be positive for demuragge to decrease balances
/// zero is legit as it effectively disables demurrage
pub fn validate_demurrage(demurrage: &Demurrage) -> Result<(), RangeError> {
	if demurrage < &Demurrage::from_num(0) {
		return Err(RangeError::LessThanZero)
	}
	Ok(())
}

/// Ensure that the nominal is in a sane range.
pub fn validate_nominal_income(nominal_income: &NominalIncome) -> Result<(), RangeError> {
	if nominal_income <= &NominalIncome::from_num(0) {
		return Err(RangeError::LessThanOrEqualZero)
	}
	Ok(())
}

#[derive(
	Encode, Decode, Copy, Clone, PartialEq, Eq, Default, PartialOrd, Ord, TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct CommunityIdentifier {
	#[cfg_attr(feature = "serde_derive", serde(with = "serialize_array"))]
	geohash: [u8; 5],
	#[cfg_attr(feature = "serde_derive", serde(with = "serialize_array"))]
	digest: [u8; 4],
}

fn fmt(cid: &CommunityIdentifier, f: &mut Formatter<'_>) -> fmt::Result {
	match sp_std::str::from_utf8(&cid.geohash) {
		Ok(geohash_str) => write!(f, "{}{}", geohash_str, bs58::encode(cid.digest).into_string()),
		Err(e) => {
			log::error!("[Cid.fmt] {:?}", e);
			Err(fmt::Error)
		},
	}
}

impl fmt::Display for CommunityIdentifier {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		fmt(self, f)
	}
}

impl fmt::Debug for CommunityIdentifier {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		fmt(self, f)
	}
}

impl CommunityIdentifier {
	pub fn new<AccountId: Encode>(
		location: Location,
		bootstrappers: Vec<AccountId>,
	) -> Result<CommunityIdentifier, CommunityIdentifierError> {
		let geohash = GeoHash::try_from_params(location.lat, location.lon);
		match geohash {
			Ok(v) => {
				let mut geohash_cropped = [0u8; 5];
				geohash_cropped.clone_from_slice(&v[0..5]);

				let crc_engine = Crc::<u32>::new(&CRC_32_CKSUM);
				let mut digest = crc_engine.digest();
				digest.update(&(bootstrappers).encode());

				Ok(CommunityIdentifier {
					geohash: geohash_cropped,
					digest: digest.finalize().to_be_bytes(),
				})
			},
			Err(_) => Err(CommunityIdentifierError::InvalidCoordinateRange()),
		}
	}

	pub fn as_array(self) -> [u8; 9] {
		concat_arrays!(self.geohash, self.digest)
	}
}

impl FromStr for CommunityIdentifier {
	type Err = bs58::decode::Error;

	fn from_str(cid: &str) -> Result<Self, Self::Err> {
		let mut geohash: [u8; 5] = [0u8; 5];
		let mut digest: [u8; 4] = [0u8; 4];

		geohash.clone_from_slice(cid[..5].as_bytes());
		digest.clone_from_slice(&bs58::decode(&cid[5..]).into_vec().map_err(decorate_bs58_err)?);

		Ok(Self { geohash, digest })
	}
}

/// If we just returned the error as is, it would be confusing, as the index size is not the actual
/// index of the &str passed to the `CommunityIdentifier::from_str` method. Hence, we increase the
/// index by the geohash size.
///
///
fn decorate_bs58_err(err: bs58::decode::Error) -> bs58::decode::Error {
	use bs58::decode::Error as Bs58Err;
	match err {
		Bs58Err::InvalidCharacter { character, index } =>
			Bs58Err::InvalidCharacter { character, index: index + 5 },
		err => err,
	}
}

// Location in lat/lon. Fixpoint value in degree with 8 decimal bits and 24 fractional bits
#[derive(
	Encode,
	Decode,
	Copy,
	Clone,
	PartialEq,
	Eq,
	Default,
	RuntimeDebug,
	PartialOrd,
	Ord,
	TypeInfo,
	MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct Location {
	#[cfg_attr(feature = "serde_derive", serde(with = "serialize_fixed"))]
	pub lat: Degree,
	#[cfg_attr(feature = "serde_derive", serde(with = "serialize_fixed"))]
	pub lon: Degree,
}

impl Location {
	pub fn new(lat: Degree, lon: Degree) -> Location {
		Location { lat, lon }
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct CommunityMetadata {
	/// utf8 encoded name
	pub name: PalletString,
	/// utf8 encoded abbreviation of the name
	pub symbol: PalletString,
	/// IPFS cid to assets necessary for community branding
	pub assets: IpfsCid,
	/// ipfs cid for style resources
	pub theme: Option<IpfsCid>,
	/// optional link to a community site
	pub url: Option<PalletString>,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct CidName {
	pub cid: CommunityIdentifier,
	pub name: PalletString,
}

impl CidName {
	pub fn new(cid: CommunityIdentifier, name: PalletString) -> Self {
		Self { cid, name }
	}
}

impl CommunityMetadata {
	pub fn new(
		name: PalletString,
		symbol: PalletString,
		assets: IpfsCid,
		theme: Option<IpfsCid>,
		url: Option<PalletString>,
	) -> Result<CommunityMetadata, CommunityMetadataError> {
		let meta = CommunityMetadata { name, symbol, assets, theme, url };
		match meta.validate() {
			Ok(()) => Ok(meta),
			Err(e) => Err(e),
		}
	}

	/// Ensures valid ascii sequences for the the string fields and that the amount of characters is
	/// limited (to prevent runtime storage bloating), or correct for the respective usage depending
	/// on the field.
	///
	/// Only ascii characters are allowed because the character set is sufficient. Furthermore,
	/// they strictly encode to one byte, which allows length checks.
	pub fn validate(&self) -> Result<(), CommunityMetadataError> {
		validate_ascii(self.name.as_bytes_or_noop())
			.map_err(CommunityMetadataError::InvalidAscii)?;
		validate_ascii(self.symbol.as_bytes_or_noop())
			.map_err(CommunityMetadataError::InvalidAscii)?;
		validate_ipfs_cid(&self.assets).map_err(CommunityMetadataError::InvalidIpfsCid)?;

		if self.name.len() > 20 {
			return Err(CommunityMetadataError::TooManyCharactersInName(self.name.len() as u8))
		}

		if self.symbol.len() != 3 {
			return Err(CommunityMetadataError::InvalidAmountCharactersInSymbol(
				self.symbol.len() as u8
			))
		}

		if let Some(u) = &self.url {
			validate_ascii(u.as_bytes_or_noop()).map_err(CommunityMetadataError::InvalidAscii)?;
			if u.len() >= 20 {
				return Err(CommunityMetadataError::TooManyCharactersInUrl(u.len() as u8))
			}
		}
		Ok(())
	}
}

impl Default for CommunityMetadata {
	/// Default implementation, which passes `self::validate()` for easy pallet testing
	fn default() -> Self {
		CommunityMetadata {
			name: "Default".into(),
			symbol: "DEF".into(),
			assets: "Defau1tCidThat1s46Characters1nLength1111111111".into(),
			theme: None,
			url: Some("DefaultUrl".into()),
		}
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub enum CommunityMetadataError {
	/// Invalid ascii character at \[index\]
	InvalidAscii(u8),
	InvalidIpfsCid(IpfsValidationError),
	/// Too many characters in name. Allowed: 20. Used: \[amount\]
	TooManyCharactersInName(u8),
	/// Invalid amount of characters symbol. Must be 3. Used: \[amount\]
	InvalidAmountCharactersInSymbol(u8),
	/// Too many characters in url. Allowed: 20. Used: \[amount\]
	TooManyCharactersInUrl(u8),
}

pub mod consts {
	use super::{Degree, Location};
	use ep_core::fixed::types::{I32F0, U0F64};

	// sun travels at a speed of 24h * 3600s / 360° = 240s/°
	// 1 degree of longitude in km at latitude x = cos(x) * 111.3194
	// seconds per degree with speed v in m/s =
	// (cos(x) * 111.3194) / (v/1000)
	// (cos(x) * 111.3194) / (83/1000) = 240, solve for x ==> x == 79.6917
	// above a latitude with absolute value > 79.6917, a human can travel faster than the sun
	// when he moves along the same latitude, so it simplifies things to exclude those locations.
	// as the northern most population is at 78 degrees, we use 78
	pub const MAX_ABS_LATITUDE: Degree = Degree::from_bits(78i128 << 64);

	pub const DATELINE_DISTANCE_M: u32 = 1_000_000; // meetups may not be closer to dateline than this

	pub const NORTH_POLE: Location =
		Location { lon: Degree::from_bits(0i128), lat: Degree::from_bits(90i128 << 64) };
	pub const SOUTH_POLE: Location =
		Location { lon: Degree::from_bits(0i128), lat: Degree::from_bits(-90i128 << 64) };
	pub const DATELINE_LON: Degree = Degree::from_bits(180i128 << 64);

	// dec2hex(round(pi/180 * 2^64),16)
	pub const RADIANS_PER_DEGREE: U0F64 = U0F64::from_bits(0x0477D1A894A74E40);

	// dec2hex(6371000,8)
	// in meters
	pub const MEAN_EARTH_RADIUS: I32F0 = I32F0::from_bits(0x006136B8);

	// dec2hex(111319,8)
	// in meters
	pub const METERS_PER_DEGREE_AT_EQUATOR: I32F0 = I32F0::from_bits(0x0001B2D7);

	/// the number of base32 digits to use (as opposed to number of bits or bytes of information)
	pub const GEO_HASH_BUCKET_RESOLUTION: usize = 5;

	/// Dirty bit key for offfchain storage
	pub const CACHE_DIRTY_KEY: &[u8] = b"dirty";
}

#[cfg(test)]
mod tests {
	use crate::{
		bs58_verify::Bs58Error,
		common::IpfsValidationError,
		communities::{
			validate_demurrage, validate_nominal_income, CommunityIdentifier, CommunityMetadata,
			CommunityMetadataError, Degree, Demurrage, Location, NominalIncome, RangeError
		},
	};
	use sp_std::str::FromStr;
	use std::assert_matches::assert_matches;

	#[test]
	fn demurrage_smaller_0_fails() {
		assert_eq!(validate_demurrage(&Demurrage::from_num(-1)), Err(RangeError::LessThanZero));
	}

	#[test]
	fn nominal_income_smaller_0_fails() {
		assert_eq!(validate_nominal_income(&NominalIncome::from_num(-1)), Err(RangeError::LessThanOrEqualZero));
	}

	#[test]
	fn validate_metadata_works() {
		assert_eq!(CommunityMetadata::default().validate(), Ok(()));
	}

	#[test]
	fn validate_metadata_fails_for_invalid_ascii() {
		let meta = CommunityMetadata { name: "€".into(), ..Default::default() };
		assert_eq!(meta.validate(), Err(CommunityMetadataError::InvalidAscii(0)));
	}

	#[test]
	fn validate_metadata_fails_for_invalid_assets_cid() {
		let meta = CommunityMetadata {
			assets: "IhaveCorrectLengthButWrongSymbols1111111111111".into(),
			..Default::default()
		};
		assert_eq!(
			meta.validate(),
			Err(CommunityMetadataError::InvalidIpfsCid(IpfsValidationError::InvalidBase58(
				Bs58Error::NonBs58Character(0)
			)))
		);
	}

	#[test]
	fn format_communityidentifier_works() {
		let empty = Vec::<i64>::new();
		assert_eq!(
			CommunityIdentifier::new(
				Location::new(Degree::from_num(48.669), Degree::from_num(-4.329)),
				empty.clone()
			)
			.unwrap()
			.to_string(),
			"gbsuv7YXq9G"
		);
		assert_eq!(
			CommunityIdentifier::new(
				Location::new(Degree::from_num(50.0), Degree::from_num(15.0)),
				empty.clone()
			)
			.unwrap()
			.to_string(),
			"u2fsm7YXq9G"
		);
		assert_eq!(
			CommunityIdentifier::new(
				Location::new(Degree::from_num(-60.0), Degree::from_num(10.0)),
				empty.clone()
			)
			.unwrap()
			.to_string(),
			"hjr4e7YXq9G"
		);
		assert_eq!(
			CommunityIdentifier::new(
				Location::new(Degree::from_num(-89.5), Degree::from_num(-87.0)),
				empty.clone()
			)
			.unwrap()
			.to_string(),
			"4044u7YXq9G"
		);
	}

	#[test]
	fn cid_from_str_works() {
		assert_eq!(CommunityIdentifier::from_str("gbsuv7YXq9G").unwrap().to_string(), "gbsuv7YXq9G")
	}

	#[test]
	fn invalid_cid_from_str_errs() {
		assert_matches!(
			CommunityIdentifier::from_str("gbsuv7YXq9l").unwrap_err(),
			bs58::decode::Error::InvalidCharacter { character: 'l', index: 10 }
		)
	}

	#[test]
	fn cid_serializes_correctly() {
		assert_eq!(
			serde_json::to_string(&CommunityIdentifier::from_str("gbsuv7YXq9G").unwrap()).unwrap(),
			"{\"geohash\":\"0x6762737576\",\"digest\":\"0xffffffff\"}"
		)
	}

	#[test]
	fn cid_deserializes_correctly() {
		let cid_json = "{\"geohash\":\"0x6762737576\",\"digest\":\"0xffffffff\"}";

		assert_eq!(
			serde_json::from_str::<CommunityIdentifier>(cid_json).unwrap(),
			CommunityIdentifier::from_str("gbsuv7YXq9G").unwrap()
		);
	}
}
