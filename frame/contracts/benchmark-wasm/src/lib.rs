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

//! Tests for the runtime interface traits and proc macros.

#![cfg_attr(not(feature = "std"), no_std)]

pub use contracts::{Schedule, prepare_contract, EnvCheck};
#[cfg(not(feature = "std"))]
use primitives::to_substrate_wasm_fn_return_value;
use rstd::vec::Vec;

// Inlucde the WASM binary
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

/// This function is not used, but we require it for the compiler to include `runtime-io`.
/// `runtime-io` is required for its panic and oom handler.
#[no_mangle]
pub fn import_runtime_io() {
	runtime_io::misc::print_utf8(&[]);
}

primitives::wasm_export_functions! {
	fn test_code(code: Vec<u8>) {
		let schedule = Schedule::default();

		for _ in 0 .. 1000 {
			assert!(prepare_contract::<EnvCheck>(&code, &schedule).is_ok());
		}
	}
}
