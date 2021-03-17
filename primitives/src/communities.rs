use codec::{Decode, Encode};
use encoding_rs::Encoding;
use fixed::types::I64F64;
use sp_core::{RuntimeDebug, H256};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use crate::balances::Demurrage;
use crate::common::{IpfsCid, PalletString};

pub use fixed::traits::{LossyFrom, LossyInto};

pub type CommunityIndexType = u32;
pub type LocationIndexType = u32;
pub type Degree = I64F64;
pub type NominalIncome = I64F64;
pub type CommunityIdentifier = H256;

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

// Location in lat/lon. Fixpoint value in degree with 8 decimal bits and 24 fractional bits
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct Location {
    pub lat: Degree,
    pub lon: Degree,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
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
        let mut res = Self::validate_ascii(&self.name)?;
        if res > 20 {
            return Err(CommunityMetadataError::TooManyCharactersInName(res));
        }

        res = Self::validate_ascii(&self.symbol)?;
        if res != 3 {
            return Err(CommunityMetadataError::InvalidAmountCharactersInSymbol(res));
        }

        // Todo: Be more strict, check only base58 characters.
        // Might be supported by in no_std environment https://github.com/mycorrhiza/bs58-rs/
        res = Self::validate_ascii(&self.icons)?;
        if res != 46 {
            return Err(CommunityMetadataError::InvalidAmountCharactersInIpfsCid(
                res,
            ));
        }

        if let Some(u) = &self.url {
            res = Self::validate_ascii(u)?;
            if res >= 20 {
                return Err(CommunityMetadataError::TooManyCharactersInNameUrl(res));
            }
        }
        Ok(())
    }

    /// If valid, returns the length of the ascii sequence.
    fn validate_ascii(bytes: &[u8]) -> Result<u8, CommunityMetadataError> {
        let res = Encoding::ascii_valid_up_to(bytes);
        if res == bytes.len() {
            Ok(res as u8)
        } else {
            Err(CommunityMetadataError::InvalidAscii)
        }
    }
}

impl Default for CommunityMetadata {
    /// Default implementation, which passes `self::validate()` for easy pallet testing
    fn default() -> Self {
        CommunityMetadata {
            name: "Default".into(),
            symbol: "DEF".into(),
            icons: "DefaultCidThatIs46CharactersInLength1111111111".into(),
            theme: None,
            url: Some("DefaultUrl".into()),
        }
    }
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub enum CommunityMetadataError {
    InvalidAscii,
    /// Too many characters in name. Allowed: 20. Used: \[amount\]
    TooManyCharactersInName(u8),
    /// Invalid amount of characters symbol. Must be 3. Used: \[amount\]
    InvalidAmountCharactersInSymbol(u8),
    /// Invalid amount of characters IpfsCid. Must be 46. Used: \[amount\]
    InvalidAmountCharactersInIpfsCid(u8),
    /// Too many characters in url. Allowed: 20. Used: \[amount\]
    TooManyCharactersInNameUrl(u8),
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
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

    pub const DATELINE_DISTANCE_M: u32 = 1_000_000; // meetups may not be closer to dateline (or poles) than this

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
}

#[cfg(test)]
mod tests {
    use crate::communities::{
        validate_demurrage, validate_nominal_income, CommunityMetadata, CommunityMetadataError,
        Demurrage, NominalIncome,
    };
    use codec::Encode;

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
        let meta = CommunityMetadata {
            name: "Default".into(),
            symbol: "DEF".into(),
            url: Some("Default".into()),
            ..Default::default()
        };
        assert_eq!(meta.validate(), Ok(()));
    }

    #[test]
    fn validate_metadata_fails_for_invalid_ascii() {
        let meta = CommunityMetadata {
            name: 11111111111_i128.encode(),
            ..Default::default()
        };
        assert_eq!(meta.validate(), Err(CommunityMetadataError::InvalidAscii));
    }
}
