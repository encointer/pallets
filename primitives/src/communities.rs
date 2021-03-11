use codec::{Decode, Encode};
use fixed::types::I64F64;
use rstd::vec::Vec;
use sp_core::{RuntimeDebug, H256};

pub use fixed::traits::{LossyFrom, LossyInto};

pub type CommunityIndexType = u32;
pub type LocationIndexType = u32;
pub type Degree = I64F64;
pub type Demurrage = I64F64;
pub type CommunityIdentifier = H256;

// Location in lat/lon. Fixpoint value in degree with 8 decimal bits and 24 fractional bits
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct Location {
    pub lat: Degree,
    pub lon: Degree,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct CommunityMetadata {
    /// utf8 encoded name
    pub name: Vec<u8>,
    /// utf8 encoded abbreviation of the name
    pub symbol: Vec<u8>,
    /// multi-resolution resource for the community icon
    pub icons: Vec<Favicon>,
    /// optional color scheme or other customizable styles to shape app appearance
    pub theme: Option<Theme>,
    /// optional link to a community site
    pub url: Option<Vec<u8>>,
}

pub type IpfsCid = Vec<u8>;

#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct Favicon {
    src: IpfsCid,
    sizes: Vec<u8>,
    density: u8,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct Theme {
    // Todo: extend
    /// primary theme color from which the accent colors are derived by the material app design guide line
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
