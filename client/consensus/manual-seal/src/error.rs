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

//! A manual sealing engine: the engine listens for rpc calls to seal blocks and create forks
//! This is suitable for a testing environment.
use derive_more::{Display, From};
use consensus_common::{Error as ConsensusError, ImportResult};
use sp_blockchain::Error as BlockchainError;
use inherents::Error as InherentsError;

/// errors encountered by background block authorship task
#[derive(Display, Debug, From)]
pub enum Error {
	/// An error occurred while importing the block
	#[display(fmt = "Block import failed: {:?}", _0)]
	BlockImportError(ImportResult),
	/// Transaction pool is empty, cannot create a block
	#[display(fmt = "Transaction pool is empty, set create_empty to true,\
	if you want to create empty blocks")]
	EmptyTransactionPool,
	/// encountered during creation of Proposer.
	#[display(fmt = "Consensus Error: {}", _0)]
	ConsensusError(ConsensusError),
	/// Failed to create Inherents data
	#[display(fmt = "Inherents Error: {}", _0)]
	InherentError(InherentsError),
	/// Proposer failed to propose new block
	#[display(fmt = "Proposer Error: {}", _0)]
	ProposerError(String),
	/// error encountered during finalization
	#[display(fmt = "Finalization Error: {}", _0)]
	BlockchainError(BlockchainError),
	/// Supplied parent_hash doesn't exist in chain
	#[display(fmt = "Supplied parent_hash doesn't exist in chain")]
	BlockNotFound,
	/// Some other error.
	#[display(fmt="Other error: {}", _0)]
	Other(Box<dyn std::error::Error + Send>),
}
