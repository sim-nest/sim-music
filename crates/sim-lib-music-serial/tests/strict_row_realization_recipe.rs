// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// conformance: strict row realization preserves plan, event, voice, and ordinal identity.
include!("../recipes/01-basics/strict-row-realization/setup.rs");

#[test]
fn public_strict_row_realization_recipe_runs() {
    strict_row_realization().expect("strict row realization scenario");
}
