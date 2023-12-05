// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! This calls another contract as passed as its account id.
#![no_std]
#![no_main]

extern crate common;
use uapi::{HostFn, HostFnImpl as api};

#[no_mangle]
#[polkavm_derive::polkavm_export]
pub extern "C" fn deploy() {
	call();
}

#[no_mangle]
#[polkavm_derive::polkavm_export]
/// Reads the first byte as the exit status and copy all but the first 4 bytes of the input as
/// output data.
pub extern "C" fn call() {
	let mut buffer = [0u8; 128];
	let input = &mut &mut buffer[..];

	// Read the input data.
	api::input(input);
	let exit_status = uapi::ReturnFlags::from_bits(input[0] as u32).unwrap();
	let output = &input[4..];

	api::return_value(exit_status, output);
}
