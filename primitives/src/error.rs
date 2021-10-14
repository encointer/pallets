#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};

#[derive(Debug)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub enum CommunityIdentifierError {
    InvalidCoordinateRange()
}
