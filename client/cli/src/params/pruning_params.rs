// This file is part of Substrate.

// Copyright (C) 2020-2022 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::error;
use clap::Args;
use sc_service::{KeepBlocks, PruningMode, Role};

/// Parameters to define the pruning mode
#[derive(Debug, Clone, PartialEq, Args)]
pub struct PruningParams {
	/// Specify the state pruning mode, a number of blocks to keep or 'archive'.
	///
	/// Default is to keep all block states if the node is running as a
	/// validator (i.e. 'archive'), otherwise state is only kept for the last
	/// 256 blocks.
	#[clap(long, value_name = "PRUNING_MODE")]
	pub pruning: Option<String>,
	/// Specify the number of finalized blocks to keep in the database.
	///
	/// Default is to keep all blocks.
	#[clap(long, value_name = "COUNT")]
	pub keep_blocks: Option<u32>,
}

impl PruningParams {
	/// Get the pruning value from the parameters
	pub fn state_pruning(&self, role: &Role) -> error::Result<PruningMode> {
		// by default we disable pruning if the node is an authority (i.e.
		// `ArchiveAll`), otherwise we keep state for the last 256 blocks.
		Ok(match &self.pruning {
			Some(ref s) if s == "archive" => PruningMode::ArchiveAll,
			None if role.is_authority() => PruningMode::ArchiveAll,
			None => PruningMode::default(),
			Some(s) =>
				PruningMode::keep_blocks(s.parse().map_err(|_| {
					error::Error::Input("Invalid pruning mode specified".to_string())
				})?),
		})
	}

	/// Get the block pruning value from the parameters
	pub fn keep_blocks(&self) -> error::Result<KeepBlocks> {
		Ok(match self.keep_blocks {
			Some(n) => KeepBlocks::Some(n),
			None => KeepBlocks::All,
		})
	}
}
