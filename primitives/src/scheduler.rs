use codec::{Decode, Encode};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

pub type CeremonyIndexType = u32;

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CeremonyPhaseType {
    REGISTERING,
    ASSIGNING,
    ATTESTING,
}

impl Default for CeremonyPhaseType {
    fn default() -> Self {
        CeremonyPhaseType::REGISTERING
    }
}
