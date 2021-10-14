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
use codec::{Decode, Encode};
use concat_arrays::concat_arrays;
use crc::{Crc, CRC_32_CKSUM};
use fixed::types::I64F64;
use geohash::GeoHash;
use sp_core::RuntimeDebug;
use rstd::{prelude::Vec, fmt, fmt::Formatter};

#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};

use crate::balances::Demurrage;
use crate::common::{validate_ascii, validate_ipfs_cid, IpfsCid, IpfsValidationError, PalletString, AsByteOrNoop};

pub use fixed::traits::{LossyFrom, LossyInto};
use crate::error::CommunityIdentifierError;

pub type CommunityIndexType = u32;
pub type LocationIndexType = u32;
pub type Degree = I64F64;
pub type NominalIncome = I64F64;

/// Ensure that the demurrage is in a sane range.
///
/// Todo: Other sanity checks, e.g., 0 < e^(demurrage_per_block*sum(phase_durations)) < 1?
pub fn validate_demurrage(demurrage: &Demurrage) -> Result<(), ()> {
    if demurrage < &Demurrage::from_num(0) {
        return Err(());
    }
    Ok(())
}

/// Ensure that the nominal is in a sane range.
pub fn validate_nominal_income(nominal_income: &NominalIncome) -> Result<(), ()> {
    if nominal_income <= &NominalIncome::from_num(0) {
        return Err(());
    }
    Ok(())
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Default, PartialOrd, Ord)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct CommunityIdentifier {
    geohash: [u8; 5],
    digest: [u8; 4],
}


fn fmt(cid: &CommunityIdentifier, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{}{}", rstd::str::from_utf8(&cid.geohash).unwrap(), bs58::encode(cid.digest).into_string())
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
    pub fn new<AccountId: Encode>(location: Location, bootstrappers: Vec<AccountId>) -> Result<CommunityIdentifier, CommunityIdentifierError> {
        let geohash = GeoHash::try_from_params(location.lat, location.lon, 5);
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
                })},
            Err(_) => Err(CommunityIdentifierError::InvalidCoordinateRange())
        }
    }

    pub fn as_array(self) -> [u8; 9] {
        concat_arrays!(self.geohash, self.digest)
    }
}

// Location in lat/lon. Fixpoint value in degree with 8 decimal bits and 24 fractional bits
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Default, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct Location {
    pub lat: Degree,
    pub lon: Degree,
}

impl Location {
    pub fn new(lat: Degree, lon: Degree) -> Location {
        Location { lat, lon}
    }
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct CommunityMetadata {
    /// utf8 encoded name
    pub name: PalletString,
    /// utf8 encoded abbreviation of the name
    pub symbol: PalletString,
    /// ipfs link to multi-resolution resource for the community icon
    pub icons: IpfsCid,
    /// optional color scheme or other customizable styles to shape app appearance
    pub theme: Option<Theme>,
    /// optional link to a community site
    pub url: Option<PalletString>,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
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
        icons: IpfsCid,
        theme: Option<Theme>,
        url: Option<PalletString>,
    ) -> Result<CommunityMetadata, CommunityMetadataError> {
        let meta = CommunityMetadata {
            name,
            symbol,
            icons,
            theme,
            url,
        };
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
        validate_ascii(&self.name.as_bytes_or_noop()).map_err(|e| CommunityMetadataError::InvalidAscii(e))?;
        validate_ascii(&self.symbol.as_bytes_or_noop()).map_err(|e| CommunityMetadataError::InvalidAscii(e))?;
        validate_ipfs_cid(&self.icons).map_err(|e| CommunityMetadataError::InvalidIpfsCid(e))?;

        if self.name.len() > 20 {
            return Err(CommunityMetadataError::TooManyCharactersInName(
                self.name.len() as u8,
            ));
        }

        if self.symbol.len() != 3 {
            return Err(CommunityMetadataError::InvalidAmountCharactersInSymbol(
                self.symbol.len() as u8,
            ));
        }

        if let Some(u) = &self.url {
            validate_ascii(u.as_bytes_or_noop()).map_err(|e| CommunityMetadataError::InvalidAscii(e))?;
            if u.len() >= 20 {
                return Err(CommunityMetadataError::TooManyCharactersInUrl(u.len() as u8));
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
            icons: "Defau1tCidThat1s46Characters1nLength1111111111".into(),
            theme: None,
            url: Some("DefaultUrl".into()),
        }
    }
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
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

#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct Theme {
    // Todo: extend
    /// primary theme color from which the accent colors are derived by the material app design guide line
    ///
    /// e.g. black = 0xFF000000
    primary_swatch: u32,
}

pub mod consts {
    use super::{Degree, Location};
    use fixed::types::{I32F0, U0F64};

    pub const MAX_SPEED_MPS: i32 = 83; // [m/s] max speed over ground of adversary
    pub const MIN_SOLAR_TRIP_TIME_S: i32 = 1; // [s] minimum adversary trip time between two locations measured in local (solar) time.

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

    pub const NORTH_POLE: Location = Location {
        lon: Degree::from_bits(0i128),
        lat: Degree::from_bits(90i128 << 64),
    };
    pub const SOUTH_POLE: Location = Location {
        lon: Degree::from_bits(0i128),
        lat: Degree::from_bits(-90i128 << 64),
    };
    pub const DATELINE_LON: Degree = Degree::from_bits(180i128 << 64);

    // dec2hex(round(pi/180 * 2^64),16)
    pub const RADIANS_PER_DEGREE: U0F64 = U0F64::from_bits(0x0477D1A894A74E40);

    // dec2hex(6371000,8)
    // in meters
    pub const MEAN_EARTH_RADIUS: I32F0 = I32F0::from_bits(0x006136B8);

    // dec2hex(111319,8)
    // in meters
    pub const METERS_PER_DEGREE_AT_EQUATOR : I32F0 = I32F0::from_bits(0x0001B2D7);

    /// the number of base32 digits to use (as opposed to number of bits or bytes of information)
    pub const BUCKET_RESOLUTION : usize = 5usize;

    /// Dirty bit key for offfchain storage
    pub const CACHE_DIRTY_KEY: &[u8] = b"dirty";
}

#[cfg(test)]
mod tests {
    use crate::bs58_verify::{Bs58Error};
    use crate::common::{IpfsValidationError};
    use crate::communities::{
        validate_demurrage, validate_nominal_income, CommunityMetadata, CommunityMetadataError,
        Demurrage, NominalIncome, Location, CommunityIdentifier, Degree
    };

    #[test]
    fn demurrage_smaller_0_fails() {
        assert_eq!(validate_demurrage(&Demurrage::from_num(-1)), Err(()));
    }

    #[test]
    fn nominal_income_smaller_0_fails() {
        assert_eq!(
            validate_nominal_income(&NominalIncome::from_num(-1)),
            Err(())
        );
    }

    #[test]
    fn validate_metadata_works() {
        assert_eq!(CommunityMetadata::default().validate(), Ok(()));
    }

    #[test]
    fn validate_metadata_fails_for_invalid_ascii() {
        let meta = CommunityMetadata {
            name: "€".into(),
            ..Default::default()
        };
        assert_eq!(
            meta.validate(),
            Err(CommunityMetadataError::InvalidAscii(0))
        );
    }

    #[test]
    fn validate_metadata_fails_for_invalid_icons_cid() {
        let meta = CommunityMetadata {
            icons: "IhaveCorrectLengthButWrongSymbols1111111111111".into(),
            ..Default::default()
        };
        assert_eq!(
            meta.validate(),
            Err(CommunityMetadataError::InvalidIpfsCid(
                IpfsValidationError::InvalidBase58(Bs58Error::NonBs58Character(0))
            ))
        );
    }

    #[test]
    fn format_communityidentifier_works() {
        let empty = Vec::<i64>::new();
        assert_eq!(CommunityIdentifier::new(Location::new(Degree::from_num(48.669), Degree::from_num(-4.329)), empty.clone())
                       .unwrap().to_string(), "gbsuv7YXq9G");
        assert_eq!(CommunityIdentifier::new(Location::new(Degree::from_num(50.0), Degree::from_num(15.0)), empty.clone())
                       .unwrap().to_string(), "u2fsm7YXq9G");
        assert_eq!(CommunityIdentifier::new(Location::new(Degree::from_num(-60.0), Degree::from_num(10.0)), empty.clone())
                       .unwrap().to_string(), "hjr4e7YXq9G");
        assert_eq!(CommunityIdentifier::new(Location::new(Degree::from_num(-89.5), Degree::from_num(-87.0)), empty.clone())
                       .unwrap().to_string(), "4044u7YXq9G");
    }
}
