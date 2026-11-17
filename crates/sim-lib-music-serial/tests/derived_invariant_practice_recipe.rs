// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// conformance: derived serial practice retains explicit invariant ledgers.
include!("../recipes/02-techniques/derived-invariant-practice/setup.rs");

#[test]
fn public_derived_invariant_practice_recipe_runs() {
    derived_invariant_practice().expect("derived and invariant scenario");
}
