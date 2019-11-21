// Copyright 2019 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! RPC interface for the ManualSeal Engine.
use jsonrpc_core::{Result};
use jsonrpc_derive::rpc;
use futures::channel::mpsc;

/// The "engine" receives these messages over a channel
pub enum EngineCommand {
	/// Tells the engine to propose a new block
	///
	/// if force == true, it will create empty blocks.
	SealNewBlock {
		force: bool
	},
	/// Tells the engine to create a fork
	///
	/// TODO: implement CreateFork message handling
	CreateFork
}

#[rpc]
pub trait ManualSealApi {
	#[rpc(name = "engine_createBlock")]
	fn create_block(
		&self,
		force: bool,
	) -> Result<()>;
}

/// A struct that implements the [`ManualSealApi`].
pub struct ManualSeal {
	import_block_channel: mpsc::UnboundedSender<EngineCommand>,
}

impl ManualSeal {
	/// Create new `ManualSeal` with the given reference to the client.
	pub fn new(import_block_channel: mpsc::UnboundedSender<EngineCommand>) -> Self {
		Self { import_block_channel }
	}
}

impl ManualSealApi for ManualSeal {
	fn create_block(
		&self,
		force: bool,
	) -> Result<()> {
		let _ = self.import_block_channel.unbounded_send(EngineCommand::SealNewBlock { force });
		Ok(())
	}
}