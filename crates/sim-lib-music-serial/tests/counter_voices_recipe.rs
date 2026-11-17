// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// conformance: counter voices remain distinct serial readings of one plan.
include!("../recipes/02-techniques/counter-voices/setup.rs");

#[test]
fn public_counter_voices_recipe_runs() {
    counter_voices().expect("counter voices scenario");
}
