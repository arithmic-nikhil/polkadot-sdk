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

//! This contract tests the behavior of adding / removing delegate_dependencies when delegate
//! calling into a contract.
#![no_std]
#![no_main]

use common::input;
use uapi::{HostFn, HostFnImpl as api};

#[repr(u32)]
#[allow(dead_code)]
enum Action {
	Noop = 0,
	AddDelegateDependency,
	RemoveDelegateDependency,
	Terminate,
}

const ALICE: [u8; 32] = [1u8; 32];

/// Load input data and perform the action specified by the input.
/// Return the code hash of the contract to delegate call to.
fn load_input() -> [u8; 32] {
	input!(action: u32, code_hash: [u8; 32],);
	let action = unsafe { core::mem::transmute::<u32, Action>(action) };

	match action {
		Action::Noop => {},
		Action::AddDelegateDependency => {
			#[allow(deprecated)]
			api::add_delegate_dependency(code_hash);
		},
		Action::RemoveDelegateDependency => {
			#[allow(deprecated)]
			api::remove_delegate_dependency(code_hash);
		},
		Action::Terminate => {
			api::terminate_v1(&ALICE);
		},
	}

	let mut buffer = [0u8; 32];
	buffer.copy_from_slice(code_hash);
	buffer
}

#[no_mangle]
#[polkavm_derive::polkavm_export]
pub extern "C" fn deploy() {
	load_input();
}

#[no_mangle]
#[polkavm_derive::polkavm_export]
pub extern "C" fn call() {
	let code_hash = load_input();
	api::delegate_call(uapi::CallFlags::empty(), &code_hash, &[], None).unwrap();
}
