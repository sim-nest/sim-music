// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// conformance: certified series transforms compose and recover the source by inverse.
include!("../recipes/01-basics/certified-transforms/setup.rs");

#[test]
fn public_certified_transform_recipe_runs() {
    certified_transform_scenario().expect("certified transform scenario");
}
