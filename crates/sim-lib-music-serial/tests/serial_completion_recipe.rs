// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// conformance: additive completion preserves the structural ledger and records all-sounding material separately.
include!("../recipes/02-adaptation/serial-consonance-completion/setup.rs");

#[test]
fn public_serial_completion_recipe_runs() {
    serial_consonance_completion().expect("serial completion recipe");
}
