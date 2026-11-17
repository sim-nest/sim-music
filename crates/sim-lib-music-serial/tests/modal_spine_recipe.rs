// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// conformance: modal and caller-defined spines land one immutable plan without claiming a strict aggregate.
include!("../recipes/02-adaptation/modal-spine/setup.rs");

#[test]
fn public_modal_spine_recipe_runs() {
    modal_spine().expect("modal spine scenario");
}
