
#[cfg(feature = "std")]
use std::fmt;

use codec::{Encode, Decode};
use sp_core::RuntimeDebug;

use crate::traits::Block as BlockT;
use crate::generic::block::BlockId;

/// Something to specify the context for a call
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(serde::Serialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "std", serde(deny_unknown_fields))]
pub enum CallAt<Block: BlockT> {
	At(BlockId<Block>),
}

impl<Block: BlockT> CallAt<Block> {
	pub fn block_id(&self) -> BlockId<Block> {
		let Self::At(b) = self;
		*b
	}
}

#[cfg(feature = "std")]
impl<Block: BlockT> fmt::Display for CallAt<Block> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let Self::At(b) = self;
		write!(f, "At[{}]", b)
	}
}

impl<Block: BlockT> Copy for CallAt<Block> {}

// FIXME transient thing
impl<Block: BlockT> From<&BlockId<Block>> for CallAt<Block> {
	fn from(block_id: &BlockId<Block>) -> Self {
		Self::At(*block_id)
	}
}
// FIXME transient thing
impl<Block: BlockT> From<BlockId<Block>> for CallAt<Block> {
	fn from(block_id: BlockId<Block>) -> Self {
		Self::At(block_id)
	}
}
