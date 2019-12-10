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
use consensus_common::ImportedAux;
use jsonrpc_core::{Error, ErrorCode};
use jsonrpc_derive::rpc;
use futures::channel::{
	mpsc::{self, TrySendError},
	oneshot,
};
use futures::TryFutureExt;

type FutureResult<T> = Box<dyn jsonrpc_core::futures::Future<Item = T, Error = Error> + Send>;
type Sender<T> = Option<oneshot::Sender<std::result::Result<T, crate::Error>>>;

/// message sent by rpc to the background authorship task
pub enum EngineCommand<Hash> {
	/// Tells the engine to propose a new block
	///
	/// if create_empty == true, it will create empty blocks if there are no transactions
	/// in the transaction pool
	SealNewBlock {
		create_empty: bool,
		parent_hash: Option<Hash>,
		sender: Sender<ImportedAux>,
	},
	/// Tells the engine to finalize the block with the supplied hash
	FinalizeBlock {
		hash: Hash,
		sender: Sender<()>,
	}
}

#[rpc]
pub trait ManualSealApi<Hash> {
	/// Instructs the manual-seal background task to create a new block
	#[rpc(name = "engine_createBlock")]
	fn create_block(
		&self,
		create_empty: bool,
		parent_hash: Option<Hash>
	) -> FutureResult<ImportedAux>;

	/// Instructs the manual-seal background task to finalize a block
	#[rpc(name = "engine_finalizeBlock")]
	fn finalize_block(
		&self,
		hash: Hash,
	) -> FutureResult<()>;
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
	) -> FutureResult<ImportedAux> {
		let (sender, receiver) = oneshot::channel();
		let result = self.import_block_channel.unbounded_send(
			EngineCommand::SealNewBlock {
				create_empty,
				parent_hash,
				sender: Some(sender),
			}
		);
		let future = async {
			if let Err(e) = result {
				return Err(map_error(e))
			};

			match receiver.await {
				// all good
				Ok(Ok(aux)) => Ok(aux),
				// error from the authorship task
				Ok(Err(e)) => {
					Err(Error {
						code: ErrorCode::ServerError(500),
						message: format!("{}", e),
						data: None
					})
				}
				// channel has been dropped
				Err(_) => {
					Err(Error {
						code: ErrorCode::ServerError(500),
						message: "Server is shutting down".into(),
						data: None
					})
				}
			}
		};

		Box::new(Box::pin(future).compat())
	}

	fn finalize_block(&self, hash: Hash) -> FutureResult<()> {
		let (sender, receiver) = oneshot::channel();
		let result = self.import_block_channel.unbounded_send(
			EngineCommand::FinalizeBlock { hash, sender: Some(sender) }
		);

		let future = async {
			if let Err(e) = result {
				return Err(map_error(e))
			};

			match receiver.await {
				// all good
				Ok(Ok(())) => Ok(()),
				// error from the authorship task
				Ok(Err(e)) => {
					Err(Error {
						code: ErrorCode::ServerError(500),
						message: format!("{}", e),
						data: None
					})
				}
				// channel has been dropped
				Err(_) => {
					Err(Error {
						code: ErrorCode::ServerError(500),
						message: "Server is shutting down".into(),
						data: None
					})
				}
			}
		};

		Box::new(Box::pin(future).compat())
	}
}

fn map_error<H>(error: TrySendError<EngineCommand<H>>) -> Error {
	if error.is_disconnected() {
		log::warn!("Received sealing request but Manual Sealing task has been dropped");
	}

	Error {
		code: ErrorCode::ServerError(500),
		message: "Server is shutting down".into(),
		data: None
	}
}

pub fn send_result<T: std::fmt::Debug>(
	sender: Sender<T>,
	result: std::result::Result<T, crate::Error>
) {
	sender.map(|sender| {
		sender.send(result)
			.expect("receiving end isn't dropped until it receives a message; qed")
	});
}
