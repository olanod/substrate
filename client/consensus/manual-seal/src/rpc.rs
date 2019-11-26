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
use jsonrpc_core::{Result, Error, ErrorCode};
use jsonrpc_derive::rpc;
use futures::channel::mpsc;

/// The "engine" receives these messages over a channel
pub enum EngineCommand<Hash> {
	/// Tells the engine to propose a new block
	///
	/// if create_empty == true, it will create empty blocks if there are no transactions
	/// in the transaction pool
	SealNewBlock {
		create_empty: bool,
		parent_hash: Option<Hash>
	}
}

#[rpc]
pub trait ManualSealApi<Hash> {
	#[rpc(name = "engine_createBlock")]
	fn create_block(
		&self,
		create_empty: bool,
		parent_hash: Option<Hash>
	) -> Result<()>;
}

/// A struct that implements the [`ManualSealApi`].
pub struct ManualSeal<Hash> {
	import_block_channel: mpsc::UnboundedSender<EngineCommand<Hash>>,
}

impl<Hash> ManualSeal<Hash> {
	/// Create new `ManualSeal` with the given reference to the client.
	pub fn new(import_block_channel: mpsc::UnboundedSender<EngineCommand<Hash>>) -> Self {
		Self { import_block_channel }
	}
}

impl<Hash: Send + 'static> ManualSealApi<Hash> for ManualSeal<Hash> {
	fn create_block(
		&self,
		create_empty: bool,
		parent_hash: Option<Hash>
	) -> Result<()> {
		self.import_block_channel.unbounded_send(
			EngineCommand::SealNewBlock {
				create_empty,
				parent_hash
			}
		).map_err(|err| {
			if err.is_disconnected() {
				log::warn!("Received sealing request but Manual Sealing task has been dropped");
			}

			Error {
				code: ErrorCode::ServerError(500),
				message: "Server is shutting down".into(),
				data: None
			}
		})?;
		Ok(())
	}
}