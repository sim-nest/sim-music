// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// conformance: an immutable serial plan spans voices and groups equal onsets without a chord order.
include!("../recipes/01-basics/immutable-plan/setup.rs");

#[test]
fn public_immutable_plan_recipe_runs() {
    immutable_plan().expect("immutable plan scenario");
}
