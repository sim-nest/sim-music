// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// conformance: a caller-registered realizer adapts an immutable plan and unknown components fail closed.
include!("../../sim-lib-music-serial/recipes/02-adaptation/third-party-adaptation/setup.rs");

#[test]
fn public_third_party_adaptation_recipe_runs() {
    third_party_adaptation().expect("third-party adaptation scenario");
}
