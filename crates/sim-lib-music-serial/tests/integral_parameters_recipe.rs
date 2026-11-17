// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// conformance: integral serial parameters exhaust their declared domain with a receipt.
include!("../recipes/03-parameters/integral-parameters/setup.rs");

#[test]
fn public_integral_parameters_recipe_runs() {
    integral_parameters().expect("integral parameters scenario");
}
