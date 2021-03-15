use codec::{Decode, Encode};
use fixed::types::I64F64;
use sp_core::RuntimeDebug;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

// We're working with fixpoint here.
pub type BalanceType = I64F64;

#[derive(Encode, Decode, Default, RuntimeDebug, Clone, Copy)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct BalanceEntry<BlockNumber> {
    /// The balance of the account after last manual adjustment
    pub principal: BalanceType,
    /// The time (block height) at which the balance was last adjusted
    pub last_update: BlockNumber,
}

pub mod consts {
    pub const DEFAULT_DEMURRAGE: i128 = 0x0000000000000000000001E3F0A8A973_i128;
}
