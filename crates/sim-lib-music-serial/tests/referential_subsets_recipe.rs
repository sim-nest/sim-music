// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// conformance: referential subsets stay licensed by the immutable serial plan.
include!("../recipes/02-techniques/referential-subsets/setup.rs");

#[test]
fn public_referential_subsets_recipe_runs() {
    referential_subsets().expect("referential subsets scenario");
}
