// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// conformance: simultaneous serial forms stay grouped without a fabricated chord order.
include!("../recipes/02-techniques/simultaneous-forms/setup.rs");

#[test]
fn public_simultaneous_forms_recipe_runs() {
    simultaneous_forms_recipe().expect("simultaneous forms scenario");
}
