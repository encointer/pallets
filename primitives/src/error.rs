use codec::{Encode, Decode};
use scale_info::TypeInfo;

#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Encode, Decode, TypeInfo, Eq, PartialEq)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub enum CommunityIdentifierError {
    InvalidCoordinateRange()
}
