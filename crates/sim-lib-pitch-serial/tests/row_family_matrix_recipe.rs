// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// conformance: twelve-tone row families and matrices retain every alias and coordinate.
include!("../recipes/01-basics/row-family-matrix/setup.rs");

#[test]
fn public_row_family_matrix_recipe_runs() {
    row_family_matrix().expect("row family matrix scenario");
}
